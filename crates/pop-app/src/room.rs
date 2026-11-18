//! Geometry for one room: a box per tile piece. With `tile_textures` = original the pieces carry the original tile images
//! (`tiles`, docs/texture_inventory.md 5); otherwise, and for pieces without an image, plain coloured boxes.
use crate::tiles::{Role, Tex, TileSet};
use crate::Game;
use bevy::prelude::*;
use pop_core::level::*;

/// Height of one tile row (tile width and depth are 1).
pub const TILE_H: f32 = 2.0;
const SLAB: f32 = 0.3;
/// The back wall box (z, depth) behind every room, front face at -0.9.
const BACK_WALL: (f32, f32) = (-1.0, -0.9);
/// Back face of the floor slabs and wall blocks: inside the back wall box, so slabs, walls and the back wall close without
/// a gap (no torch or sun light through the seams, no slit at an angle). The front stays at 0.5.
const SLAB_BACK: f32 = -0.95;

#[derive(Component)]
pub struct RoomEntity;

/// Geometry of the cutscene set (the princess chamber), kept apart from the room rebuilds.
#[derive(Component)]
pub struct Chamber;

#[derive(Resource)]
pub struct Palette {
    pub cube: Handle<Mesh>,
    stone: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    blue: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    flame: Handle<StandardMaterial>,
    /// original tile images [dungeon, palace] (None: procedural)
    tiles: [Option<TileSet>; 2],
    /// voxel skeleton and potions (None: procedural)
    items: Option<crate::items::Items>,
}

impl Palette {
    /// `tiles_dir`: game data folder to load the tile images from (None = procedural tiles); `columns` / `objects`: voxel
    /// columns / voxel objects.
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>, tiles_dir: Option<&str>, columns: bool, objects: bool) -> Self {
        let tiles = tiles_dir.map_or([None, None], |d| crate::tiles::load_sets(d, imgs, mats));
        let items = tiles_dir.map(|d| crate::items::Items::load(d, columns, objects, meshes, mats));
        let mut m = |r, g, b| mats.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.9, ..default() });
        Palette {
            cube: meshes.add(textured_cube()),
            stone: m(0.62, 0.52, 0.36),
            dark: m(0.16, 0.14, 0.12),
            wood: m(0.45, 0.28, 0.14),
            metal: m(0.55, 0.57, 0.62),
            red: m(0.75, 0.12, 0.1),
            blue: m(0.15, 0.35, 0.85),
            glass: m(0.5, 0.75, 0.9),
            flame: mats.add(StandardMaterial { base_color: Color::srgb(1.0, 0.6, 0.1), emissive: LinearRgba::rgb(8.0, 3.0, 0.5), ..default() }),
            tiles,
            items,
        }
    }
}

/// Unit cube whose image stands upright on every side (Bevy's cuboid flips it on the front face): u runs left to right as
/// seen from outside, v from the top down; on the top face v runs from the back row (image row 0) to the front.
fn textured_cube() -> Mesh {
    let mut mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
    let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()).unwrap_or_default().to_vec();
    let nrm = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).and_then(|a| a.as_float3()).unwrap_or_default().to_vec();
    let uv: Vec<[f32; 2]> = pos
        .iter()
        .zip(&nrm)
        .map(|(p, n)| if n[1] != 0.0 { [p[0] + 0.5, p[2] + 0.5] } else if n[0] != 0.0 { [0.5 - p[2] * n[0], 0.5 - p[1]] } else { [0.5 + p[0] * n[2], 0.5 - p[1]] })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh
}

/// World size of `n` screen pixels across / up (32 px per tile column, TILE_H_PX per row).
fn px_w(n: f32) -> f32 {
    n / 32.0
}
fn px_h(n: f32) -> f32 {
    n * TILE_H / TILE_H_PX as f32
}

struct B<'a, 'w, 's> {
    cmds: &'a mut Commands<'w, 's>,
    p: &'a Palette,
    origin: Vec3,
    chamber: bool,
    /// room number (palace wall variants) and whether it is the level's start room (no stairs behind its exit door)
    room: usize,
    start_room: bool,
    tiles: Option<&'a TileSet>,
    /// voxel items and the set index (0 dungeon, 1 palace); None in the chamber and with procedural tiles
    items: Option<(&'a crate::items::Items, usize)>,
    /// the tile being built (tagged on its entities as `TileOf`, for incremental rebuilds); None: back wall, chamber
    cur: Option<TileOf>,
}

/// The room tile (room, index) an entity was built for: `rebuild_on_change` respawns only the tiles that changed.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileOf(u8, u8);

impl<'a> B<'a, '_, '_> {
    /// A voxel item model: bottom-left corner of its image at `lo` (room coordinates), sprite plane at `lo.z`.
    fn item(&mut self, m: &crate::items::Model, mat: &Handle<StandardMaterial>, lo: Vec3) -> Entity {
        let px = Vec3::new(1.0 / 32.0, TILE_H / TILE_H_PX as f32, 1.0 / 32.0);
        let e = self.cmds.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(self.origin + lo).with_scale(px))).id();
        self.tag(e);
        e
    }

    /// The voxel column of image `id`, if built (else false: draw the flat piece): image left edge at x `left`, its kept rows
    /// stretched from `y0` up to `y1`, axis at depth `z`.
    fn voxel_column(&mut self, id: Option<u16>, left: f32, y0: f32, y1: f32, z: f32) -> bool {
        let Some(((items, set), id)) = self.items.zip(id) else { return false };
        let Some(c) = items.columns[set].get(&id) else { return false };
        self.column_mesh(c, &items.mat, left, y0, y1, z);
        true
    }

    /// A voxel column mesh: image left edge at `left`, rows stretched over y0..y1, axis at depth `z`.
    fn column_mesh(&mut self, c: &crate::items::Column, mat: &Handle<StandardMaterial>, left: f32, y0: f32, y1: f32, z: f32) {
        let s = Vec3::new(1.0 / 32.0, (y1 - y0) / c.h as f32, 1.0 / 32.0);
        let e = self.cmds.spawn((Mesh3d(c.mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(self.origin + Vec3::new(left, y0, z - c.az * s.z)).with_scale(s))).id();
        self.tag(e);
    }

    /// The voxel objects of this room's set (None: flat objects, procedural tiles or the chamber).
    fn objects(&self) -> Option<(&'a crate::items::Items, &'a crate::items::Objects)> {
        let (items, set) = self.items?;
        Some((items, &items.objects[set])).filter(|(_, o)| !o.ids.is_empty())
    }

    /// A voxel object: the bottom-left corner of its image at `lo` (room coordinates), `sy` times the pixel height, plane at
    /// `lo.z`; one voxel = one pixel (x, z 1/32 unit). Shadow casting as the room pieces.
    fn object(&mut self, m: &crate::items::Model, mat: &Handle<StandardMaterial>, lo: Vec3, sy: f32) -> Entity {
        let px = Vec3::new(1.0 / 32.0, px_h(sy), 1.0 / 32.0);
        let e = self.cmds.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(self.origin + lo).with_scale(px))).id();
        self.tag(e);
        e
    }

    /// The debris heap (if built) lying on the floor of the tile at (x, y): the model's image is the floor seen from above,
    /// its rows spread over the tile depth (row 0 at the back), its layers standing up from the floor surface.
    fn debris(&mut self, x: f32, y: f32) -> bool {
        let Some((items, Some(m))) = self.objects().map(|(i, o)| (i, o.debris.clone())) else { return false };
        let t = Transform::from_translation(self.origin + Vec3::new(x, y, 0.5)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(1.0 / 32.0, 1.0 / m.h as f32, DEBRIS_LAYER));
        let e = self.cmds.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(items.mat.clone()), t)).id();
        self.tag(e);
        true
    }

    /// Box from `lo` to `hi` (corner coordinates).
    fn put(&mut self, mat: &Handle<StandardMaterial>, lo: Vec3, hi: Vec3) {
        self.put_id(mat, lo, hi);
    }

    /// A cut-out image standing upright: bottom-left corner at `lo`, as thin as possible without the faces touching.
    fn sprite(&mut self, t: &Tex, lo: Vec3) {
        self.sprite_sy(t, lo, 1.0);
    }

    /// The same, stretched vertically by `sy`.
    fn sprite_sy(&mut self, t: &Tex, lo: Vec3, sy: f32) {
        self.put(&t.mat, lo, lo + Vec3::new(px_w(t.w), px_h(t.h) * sy, 0.004));
    }

    fn put_id(&mut self, mat: &Handle<StandardMaterial>, lo: Vec3, hi: Vec3) -> Entity {
        let e = self.cmds.spawn((Mesh3d(self.p.cube.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(self.origin + (lo + hi) / 2.0).with_scale(hi - lo))).id();
        self.tag(e);
        e
    }

    /// Back face z of slabs and wall blocks (SLAB_BACK; the cutscene set keeps its tuned -0.5).
    fn back(&self) -> f32 {
        if self.chamber { -0.5 } else { SLAB_BACK }
    }

    /// A floor slab (or wall block) from `y0` to `y1` in the column at `x`, reaching back into the back wall.
    fn slab(&mut self, mat: &Handle<StandardMaterial>, x: f32, y0: f32, y1: f32) -> Entity {
        let z = self.back();
        self.put_id(mat, Vec3::new(x, y0, z), Vec3::new(x + 1.0, y1, 0.5))
    }

    fn tag(&mut self, e: Entity) {
        if self.chamber {
            self.cmds.entity(e).insert(Chamber);
        } else {
            self.cmds.entity(e).insert(RoomEntity);
            if let Some(t) = self.cur {
                self.cmds.entity(e).insert(t);
            }
        }
    }
}

/// The current room plus its 8 neighbours (left/right/up/down and the diagonals via the neighbours' links): (room, dx, dy,
/// origin); room 0 = no room there (outside the level).
fn rooms_around(g: &Game) -> Vec<(usize, i32, i32, Vec3)> {
    let level = &g.world.level;
    let link = |r: u8, d: usize| if (1..=ROOMS as u8).contains(&r) { level.links[r as usize - 1][d] } else { 0 };
    let [l, r, u, d] = level.links[g.room - 1];
    // (room, dx, dy) in room units; a diagonal room is reached through whichever neighbour exists
    let around = [
        (g.room as u8, 0, 0),
        (l, -1, 0),
        (r, 1, 0),
        (u, 0, 1),
        (d, 0, -1),
        (if link(u, 0) != 0 { link(u, 0) } else { link(l, 2) }, -1, 1),
        (if link(u, 1) != 0 { link(u, 1) } else { link(r, 2) }, 1, 1),
        (if link(d, 0) != 0 { link(d, 0) } else { link(l, 3) }, -1, -1),
        (if link(d, 1) != 0 { link(d, 1) } else { link(r, 3) }, 1, -1),
    ];
    // POP_FX=rooms=1 (perf measurements): only the current room
    around
        .into_iter()
        .take(crate::perf::fx().rooms.unwrap_or(9))
        .map(|(room, dx, dy)| {
            let room = if (1..=ROOMS as u8).contains(&room) { room as usize } else { 0 };
            (room, dx, dy, Vec3::new(dx as f32 * COLS as f32, dy as f32 * ROWS as f32 * TILE_H, 0.0))
        })
        .collect()
}

/// The room builder for `room` (1-based) with its left/bottom corner at `origin`; neighbours are drawn with other origins.
fn room_builder<'a, 'w, 's>(cmds: &'a mut Commands<'w, 's>, level: &Level, room: usize, origin: Vec3, p: &'a Palette, palace: bool) -> B<'a, 'w, 's> {
    let items = p.items.as_ref().map(|i| (i, palace as usize));
    B { cmds, p, origin, chamber: false, room, start_room: room == level.start_room as usize, tiles: p.tiles[palace as usize].as_ref(), items, cur: None }
}

/// The builder for the cutscene set (a palace chamber): entities carry `Chamber` instead of `RoomEntity`; palace tile images,
/// voxel columns and objects as in a palace level room (procedural tiles: the plain boxes, as before).
fn chamber_builder<'a, 'w, 's>(cmds: &'a mut Commands<'w, 's>, room: usize, origin: Vec3, p: &'a Palette) -> B<'a, 'w, 's> {
    B { cmds, p, origin, chamber: true, room, start_room: false, tiles: p.tiles[1].as_ref(), items: p.items.as_ref().map(|i| (i, 1)), cur: None }
}

pub fn spawn_chamber(cmds: &mut Commands, level: &Level, room: usize, origin: Vec3, p: &Palette) {
    spawn_tiles(&mut chamber_builder(cmds, room, origin, p), level, &[], room);
}

/// A plain box of the room palette in the chamber set (`mat`: "stone", "dark", "wood", "flame"; anything else = metal).
pub fn chamber_box(cmds: &mut Commands, p: &Palette, mat: &str, lo: Vec3, hi: Vec3) {
    let m = match mat { "stone" => p.stone.clone(), "dark" => p.dark.clone(), "wood" => p.wood.clone(), "flame" => p.flame.clone(), _ => p.metal.clone() };
    chamber_builder(cmds, 0, Vec3::ZERO, p).put(&m, lo, hi);
}

/// A front column of the chamber set from the cutscene's own column image (PV 952; the palace set has no 10 px column),
/// `at` (world) = image left edge, bottom, axis depth; up to `top` (world y). `vox` with voxel columns (as the levels'
/// voxel columns), else the flat image `tex`; false with procedural tiles (the caller keeps its box).
pub fn chamber_column(cmds: &mut Commands, p: &Palette, vox: &crate::items::Column, tex: &Tex, at: Vec3, top: f32) -> bool {
    let mut b = chamber_builder(cmds, 0, Vec3::ZERO, p);
    let Some((items, set)) = b.items.filter(|_| b.tiles.is_some()) else { return false };
    if !items.columns[set].is_empty() {
        b.column_mesh(vox, &items.mat, at.x, at.y, top, at.z);
    } else {
        let w = px_w(tex.w);
        b.put(&tex.mat, at - Vec3::new(0.0, 0.0, w / 2.0), Vec3::new(at.x + w, top, at.z + w / 2.0));
    }
    true
}

/// A torch holder (346) in the chamber set as in the levels, `top` = the level's flame anchor (world: cup centre x, y the
/// flame stands on): the voxel holder with voxel objects, else the flat image; false with procedural tiles (keep the box).
pub fn chamber_holder(cmds: &mut Commands, p: &Palette, top: Vec3) -> bool {
    let mut b = chamber_builder(cmds, 0, Vec3::ZERO, p);
    if let Some((items, h, axis)) = b.objects().filter(|(i, _)| !i.flames.is_empty()).and_then(|(i, o)| Some((i, o.ids.get(&346)?.clone(), o.holder_axis))) {
        let e = b.object(&h, &items.mat, Vec3::new(top.x - px_w(axis), top.y - px_h(h.h as f32 - 2.0), TORCH_Z), 1.0);
        b.cmds.entity(e).insert(bevy::light::NotShadowCaster);
        return true;
    }
    let Some(s) = b.tiles.map_or(vec![], |s| s.pieces(Tile::Torch, 0)).into_iter().find(|(r, _)| *r == Role::TorchStub) else { return false };
    b.sprite(&s.1, Vec3::new(top.x - 0.5, top.y - px_h(s.1.h - 2.0), -0.86));
    true
}

/// Voxel torch flames for the chamber (voxel objects with the holder built): frames, material and the bottom-left corner of
/// a flame relative to its `top` (as `chamber_holder`), one voxel = one pixel (scale `voxel_px`).
pub fn chamber_flames(p: &Palette) -> Option<(&[crate::items::Model], &Handle<StandardMaterial>, Vec3)> {
    let i = p.items.as_ref().filter(|i| !i.flames.is_empty() && i.objects[1].ids.contains_key(&346))?;
    Some((&i.flames, &i.flame_mat, Vec3::new(-px_w(FLAME_AXIS), -px_h(FLAME_SINK), TORCH_Z)))
}

/// Scale of a voxel model whose one voxel is one screen pixel (1/32 unit across and deep, TILE_H/63 up).
pub fn voxel_px() -> Vec3 {
    Vec3::new(1.0 / 32.0, px_h(1.0), 1.0 / 32.0)
}

/// What a tile's pieces are built from: kind, modifier (not for a loose floor: its entity animates itself from it), the
/// pressure plate lift and the background piece from the left neighbour. Equal keys build identical entities, so
/// `rebuild_on_change` keeps those tiles.
type TileKey = (Tile, u8, u32, Option<(u16, i32)>);

fn tile_key(level: &Level, room: usize, idx: usize, palace: bool) -> TileKey {
    let t = level.tile_at(room, idx);
    let lift = match t.kind { Tile::DropButton | Tile::RaiseButton => plate_lift(level.link_timer(t.modifier)), _ => 0.0 };
    let (lk, lm) = left_tile(level, room, idx);
    // loose floors, flames, bubbles and the sword glint animate themselves from the modifier: no rebuild for those bits
    let m = match t.kind {
        Tile::LooseFloor | Tile::Torch | Tile::TorchDebris | Tile::Sword => 0,
        Tile::Potion => t.modifier & 0xE0,
        _ => t.modifier,
    };
    (t.kind, m, lift.to_bits(), crate::tiles::back_decor(lk, lm, t.kind, palace))
}

/// Tile `idx` of the builder's room.
fn spawn_tile(b: &mut B, level: &Level, room: usize, idx: usize) {
    let (col, row) = (idx % COLS, idx / COLS);
    let t = level.tile(room, col, row);
    let (x, y) = (col as f32, (ROWS - 1 - row) as f32 * TILE_H);
    b.cur = (!b.chamber).then_some(TileOf(room as u8, idx as u8));
    if t.kind == Tile::LooseFloor {
        // its own entity, moved every frame by `animate` (wobble from the trob state)
        let e = b.slab(&plain_floor(b), x, y - SLAB, y);
        let base = b.origin + Vec3::new(x + 0.5, y - SLAB / 2.0, (b.back() + 0.5) / 2.0);
        b.cmds.entity(e).insert(Loose { room: room as u8, idx: idx as u8, base });
    } else {
        let lift = match t.kind { Tile::DropButton | Tile::RaiseButton => plate_lift(level.link_timer(t.modifier)), _ => 0.0 };
        tile(b, x, y, t, lift, idx as u8);
    }
    // the original's background piece from the left neighbour, on the back wall (not in the cutscene set)
    let (lk, lm) = left_tile(level, room, idx);
    if let Some((d, up)) = b.tiles.filter(|_| !b.chamber).and_then(|s| s.decor(lk, lm, t.kind)) {
        // flat on the wall: its shadow would be invisible, and each cut-out material is its own shadow draw
        let lo = Vec3::new(x, y + px_h(up), BACK_Z);
        let e = b.put_id(&d.mat, lo, lo + Vec3::new(px_w(d.w), px_h(d.h), 0.004));
        b.cmds.entity(e).insert(bevy::light::NotShadowCaster);
    }
    b.cur = None;
}

/// Kind and modifier of the tile left of tile `idx` of `room` (through the room links; outside the level: a wall).
fn left_tile(level: &Level, room: usize, idx: usize) -> (Tile, u8) {
    let (c, r) = ((idx % COLS) as i32 - 1, (idx / COLS) as i32);
    (level.tile_wrapped(room as i32, c, r), level.modifier_wrapped(room as i32, c, r))
}

/// The back wall of a room: one dark box over the whole room (and half a column into each neighbour), at BACK_WALL.
/// Every light is in front of it, so in the levels it casts no shadow (it only filled the shadow maps).
fn back_wall(b: &mut B) {
    let p = b.p;
    let e = b.put_id(&p.dark, Vec3::new(-0.5, -SLAB, BACK_WALL.0), Vec3::new(10.5, 3.0 * TILE_H, BACK_WALL.1));
    if !b.chamber {
        b.cmds.entity(e).insert(bevy::light::NotShadowCaster);
    }
}

fn spawn_tiles(b: &mut B, level: &Level, mobs: &[pop_core::sim::Mob], room: usize) {
    back_wall(b);
    for idx in 0..ROWS * COLS {
        spawn_tile(b, level, room, idx);
    }
    spawn_falling(b, mobs, room);
}

/// Columns of a missing left / right neighbour that are built (the game camera shows up to about 2.4 columns past the
/// room at 16:9 and 4.8 at 21:9).
const EDGE_COLS: usize = 5;

/// A place around the shown room with no room (outside the level), `dx` / `dy` in rooms: its back wall, and the level edge
/// as the original draws it (SDLPoP get_tile_to_draw, from memory: wall at the left edge, floor in the row above) and as
/// the game treats it (`Level::tile_wrapped`: wall): a row of floors at the bottom of a missing room above, wall blocks
/// elsewhere; only the cells next to the shown room.
fn spawn_edge(b: &mut B, dx: i32, dy: i32) {
    back_wall(b);
    for idx in 0..ROWS * COLS {
        let (col, row) = (idx % COLS, idx / COLS);
        let near = (dx >= 0 || col >= COLS - EDGE_COLS) && (dx <= 0 || col < EDGE_COLS) && (dy >= 0 || row == 0) && (dy <= 0 || row == ROWS - 1);
        if near {
            let kind = if dy > 0 { Tile::Floor } else { Tile::Wall };
            tile(b, col as f32, (ROWS - 1 - row) as f32 * TILE_H, TileRef { kind, flags: 0, modifier: 0 }, 0.0, idx as u8);
        }
    }
}

/// Falling loose floors: mob y is a screen line, 65 + 63 * row = floor line of that row; drawn one tick behind the sim
/// and interpolated by `animate` (y of the last tick = y - speed).
fn spawn_falling(b: &mut B, mobs: &[pop_core::sim::Mob], room: usize) {
    for m in mobs.iter().filter(|m| m.room as usize == room) {
        let (x, y) = ((m.xh >> 2) as f32, (191.0 - m.y as f32) * TILE_H / 63.0);
        let e = b.slab(&plain_floor(b), x, y - SLAB, y);
        let base = b.origin + Vec3::new(x + 0.5, y - SLAB / 2.0, (b.back() + 0.5) / 2.0);
        b.cmds.entity(e).insert(Falling { base, up: m.speed.max(0) as f32 * TILE_H / 63.0 });
    }
}

/// The normal floor's material (loose floors and falling slabs look like it, as in the original).
fn plain_floor(b: &B) -> Handle<StandardMaterial> {
    b.tiles.and_then(|s| s.floor(Tile::Floor)).unwrap_or_else(|| b.p.stone.clone())
}

/// A loose floor slab: tile `idx` of `room`, slab centre `base` (world).
#[derive(Component)]
pub struct Loose {
    room: u8,
    idx: u8,
    base: Vec3,
}

/// A falling slab: centre at the current tick, `up` = how far above that it was one tick earlier (world units).
#[derive(Component)]
pub struct Falling {
    base: Vec3,
    up: f32,
}

/// A potion's bubbles (room, tile index): the frame shown is the sim's `Level::anim_frame`.
#[derive(Component)]
pub struct Bubbles(u8, u8);

/// A voxel torch flame (room, tile index): the frame shown is the sim's `Level::anim_frame`.
#[derive(Component)]
pub struct TorchFlame(u8, u8);

/// Loose floor wobble amplitudes: vertical bounce (world units, about 1.6 screen px) and roll (radians).
const WOBBLE_DY: f32 = 0.05;
const WOBBLE_ROLL: f32 = 0.08;

/// Wobble of a loose floor from its modifier (docs/tiles.md 5.1/5.3) and the tick progress `alpha`: (lift, roll).
/// 0x80..0x83 = knocked (shake for 4 ticks, decaying to 0 at 0x84); 1..10 = about to fall (growing); else still.
/// One bounce per tick, the roll changes side every tick; `alpha` makes it continuous between ticks.
pub fn wobble(m: u8, alpha: f32) -> (f32, f32) {
    let (k, amp) = match m {
        0x80..=0x83 => {
            let k = (m & 0x7F) as f32 + alpha;
            (k, (1.0 - k / 4.0).max(0.0))
        }
        1..=10 => {
            let k = m as f32 + alpha;
            (k, 0.6 + 0.04 * k)
        }
        _ => return (0.0, 0.0),
    };
    let s = (k * std::f32::consts::PI).sin();
    (WOBBLE_DY * amp * s.abs(), WOBBLE_ROLL * amp * s)
}

/// Pressure plates stand out by 10% of the slab thickness (SLAB = 0.3 world units, so 0.03, about one screen pixel of the
/// 63-px row) while up; pressed (link timer > 1, docs/tiles.md 4.2) they are at floor level. Visual only.
pub const PLATE_LIFT: f32 = 0.1 * SLAB;
pub fn plate_lift(timer: u8) -> f32 {
    if timer > 1 { 0.0 } else { PLATE_LIFT }
}

/// Every frame: loose floor wobble, falling slab interpolation, potion bubble frames.
#[allow(clippy::type_complexity)]
pub fn animate(
    g: Res<Game>,
    p: Res<Palette>,
    mut loose: Query<(&Loose, &mut Transform), Without<Falling>>,
    mut falling: Query<(&Falling, &mut Transform), Without<Loose>>,
    mut bubbles: Query<(&Bubbles, &mut Mesh3d), Without<TorchFlame>>,
    mut flames: Query<(&TorchFlame, &mut Mesh3d), Without<Bubbles>>,
) {
    let a = g.world.alpha();
    for (l, mut t) in &mut loose {
        let (dy, roll) = wobble(g.world.level.tile_at(l.room as usize, l.idx as usize).modifier, a);
        t.set_if_neq(Transform { translation: l.base + Vec3::Y * dy, rotation: Quat::from_rotation_z(roll), scale: t.scale });
    }
    for (f, mut t) in &mut falling {
        t.translation = f.base + Vec3::Y * f.up * (1.0 - a);
    }
    let anim = |room: u8, idx: u8| g.world.level.anim_frame(room as usize, idx as usize);
    if let Some(frames) = p.items.as_ref().map(|i| &i.bubbles).filter(|f| !f.is_empty()) {
        for (b, mut m) in &mut bubbles {
            // frames 1..7; 0 = a room never entered (not animated yet): shown as frame 1
            let f = anim(b.0, b.1).unwrap_or(1).clamp(1, 7) as usize;
            let want = &frames[f - 1].mesh;
            if m.0 != *want {
                m.0 = want.clone();
            }
        }
    }
    if let Some(frames) = p.items.as_ref().map(|i| &i.flames).filter(|f| !f.is_empty()) {
        for (f, mut m) in &mut flames {
            let want = &frames[anim(f.0, f.1).unwrap_or(0) as usize % frames.len()].mesh;
            if m.0 != *want {
                m.0 = want.clone();
            }
        }
    }
}

/// `x`: left edge, `y`: floor surface height of this tile's row; `lift`: pressure plate height above the floor (0 pressed);
/// `idx`: tile index in the room (potion bubble phase).
fn tile(b: &mut B, x: f32, y: f32, t: TileRef, lift: f32, idx: u8) {
    use Tile::*;
    let p = b.p;
    let (stone, dark, metal) = (p.stone.clone(), p.dark.clone(), p.metal.clone());
    // original images: the floor of this tile type and its extra piece (pillar, door post, exit frame, wall face)
    let fl = b.tiles.and_then(|s| s.floor(t.kind)).unwrap_or_else(|| stone.clone());
    let ex: Option<Tex> = b.tiles.and_then(|s| s.extra(t.kind, t.modifier)).cloned();
    // the other pieces (chomper and spike frames, door slices, torch stub, balcony, lattice, wall top), tiles::pieces
    let pc: Vec<(Role, Tex)> = b.tiles.map_or(vec![], |s| s.pieces(t.kind, t.modifier));
    let piece = |r: Role| pc.iter().find(|(pr, _)| *pr == r).map(|(_, t)| t.clone());
    let floor = |b: &mut B, m: &Handle<StandardMaterial>| {
        b.slab(m, x, y - SLAB, y);
    };
    let column = |b: &mut B, m: &Handle<StandardMaterial>, w: f32, h: f32| {
        b.put(m, Vec3::new(x + 0.5 - w / 2.0, y, -w / 2.0), Vec3::new(x + 0.5 + w / 2.0, y + h, w / 2.0))
    };
    match t.kind {
        Empty => {}
        // textured: one row tall, so the faces of stacked walls and of the floor above do not overlap (z-fighting)
        // dungeon: the face image on the block and the top strip as a thin cap on its top face; palace: built brick faces
        Wall => match ex.map(|w| w.mat).or_else(|| b.tiles.and_then(|s| s.palace_wall(b.room, idx))) {
            Some(m) => {
                let top = y + TILE_H - SLAB;
                match piece(Role::WallTop) {
                    Some(cap) => {
                        b.slab(&m, x, y - SLAB, top - CAP);
                        b.slab(&cap.mat, x, top - CAP, top);
                    }
                    None => {
                        b.slab(&m, x, y - SLAB, top);
                    }
                }
            }
            None => {
                b.slab(&stone, x, y - SLAB, y + TILE_H);
            }
        },
        Skeleton => match b.items.and_then(|(i, set)| Some((i.skeleton[set].clone()?, i.mat.clone()))) {
            // the bones as a voxel relief standing on the plain floor, centred on the tile, behind the figures (z 0.25)
            Some((m, mat)) => {
                floor(b, &plain_floor(b));
                b.item(&m, &mat, Vec3::new(x + 0.5 - px_w(m.w as f32) / 2.0, y, SKELETON_Z));
            }
            None => {
                floor(b, &fl);
                b.put(&p.glass, Vec3::new(x + 0.3, y, -0.2), Vec3::new(x + 0.7, y + 0.15, 0.2));
            }
        },
        // voxel objects: the rubble as a heap on the plain floor
        Debris if b.objects().is_some_and(|(_, o)| o.debris.is_some()) => {
            floor(b, &plain_floor(b));
            b.debris(x, y);
        }
        Floor | Sword | Debris | BalconyLeft | BalconyRight => {
            floor(b, &fl);
            match t.kind {
                Sword => b.put(&metal, Vec3::new(x + 0.2, y, -0.05), Vec3::new(x + 0.8, y + 0.05, 0.05)),
                Debris => b.put(&dark, Vec3::new(x + 0.2, y, -0.2), Vec3::new(x + 0.8, y + 0.15, 0.2)),
                // palace balcony: the window (floor rows cut) on the back wall, its arch above it
                _ => back_panel(b, &piece, x, y),
            }
        }
        LooseFloor => {} // its own entity, spawn_tiles
        // voxel objects: each frame piece as round pointed rods standing on the floor, in three rows through the depth
        // (the 2D layering): the fore piece in front of the figures, the back piece without the spikes the fore piece
        // repeats just behind them, the right piece (drawn in the next column in 2D, the back of the sheared floor) at the
        // back of the tile, moved left by the floor's shear
        Spikes if b.objects().is_some_and(|(_, o)| o.ids.contains_key(&1332)) => {
            floor(b, &fl);
            let (items, o) = b.objects().unwrap();
            for (r, id) in b.tiles.map_or(vec![], |s| s.piece_ids(Spikes, t.modifier)) {
                let (dx, z) = match r { Role::SpikeFore => (0.0, SPIKE_ROWS[0]), Role::SpikeBack => (0.0, SPIKE_ROWS[1]), _ => (px_w(32.0 - FLOOR_SHEAR), SPIKE_ROWS[2]) };
                if let Some(m) = o.ids.get(&id) {
                    b.object(m, &items.mat, Vec3::new(x + dx, y - px_h(1.0), z), 1.0);
                }
            }
        }
        Spikes => {
            floor(b, &fl);
            // modifier: 0 retracted, 1..=4 rising, >= 0x80 or 255 out, 5..=9 retracting
            if b.tiles.is_some_and(|s| !s.pieces(Spikes, 0x80).is_empty()) {
                // the frame images: back piece in the column, its right part in the next column, the front piece before
                // the figures; standing on the floor (frames 0 and 9 draw nothing)
                for (r, z) in [(Role::SpikeBack, SPIKE_Z.0), (Role::SpikeRight, SPIKE_Z.0), (Role::SpikeFore, SPIKE_Z.1)] {
                    if let Some(s) = piece(r) {
                        b.sprite(&s, Vec3::new(if r == Role::SpikeRight { x + 1.0 } else { x }, y, z));
                    }
                }
            } else {
                let h = match t.modifier { 0 => 0.04, 255 => 0.45, m if m & 0x80 != 0 => 0.45, m => (m as f32 * 0.11).min(0.45) };
                for i in 0..4 {
                    let sx = x + 0.15 + i as f32 * 0.23;
                    b.put(&metal, Vec3::new(sx, y, -0.05), Vec3::new(sx + 0.06, y + h, 0.05));
                }
            }
        }
        Pillar | BigPillarBottom | BigPillarTop | LatticePillar | LatticeSupport => match &ex {
            // the column image (record +9, the piece drawn in front of the kid): a voxel column built from it, the image 8 px
            // into the tile as the 2D game draws it, base on the floor, top under the slab above (a big pillar bottom goes
            // on into its top tile, so the two meet without a seam); flat style: the image on a square column
            Some(c) => {
                if t.kind != BigPillarTop {
                    floor(b, &fl); // the big pillar top has no floor piece: the column goes on through
                }
                let top = if t.kind == BigPillarBottom { TILE_H } else { TILE_H - SLAB };
                if !b.voxel_column(b.tiles.and_then(|s| s.extra_id(t.kind, t.modifier)), x + px_w(8.0), y, y + top, COLUMN_Z) {
                    column(b, &c.mat, px_w(c.w), TILE_H);
                }
            }
            None => {
                floor(b, &fl);
                // palace lattice support: its arch panel on the back wall instead of the stone column
                match piece(Role::BackPanel) {
                    Some(_) => back_panel(b, &piece, x, y),
                    None => column(b, &stone, 0.5, TILE_H),
                }
            }
        },
        DoortopWithFloor | Doortop => {
            floor(b, &fl);
            match &ex {
                Some(d) => door_post(b, d, x, y, t.kind, t.modifier),
                None => b.put(&p.red, Vec3::new(x + 0.1, y, -0.9), Vec3::new(x + 0.9, y + TILE_H * 0.9, -0.85)),
            }
        }
        SmallLattice | LatticeLeft | LatticeRight => {
            floor(b, &fl);
            match piece(Role::BackPanel) {
                Some(_) => back_panel(b, &piece, x, y),
                None => b.put(&metal, Vec3::new(x, y, -0.5), Vec3::new(x + 1.0, y + TILE_H * 0.9, -0.45)),
            }
        }
        Gate => {
            floor(b, &fl);
            if let Some(d) = &ex {
                door_post(b, d, x, y, t.kind, t.modifier);
            }
            for (lo, hi) in gate_bars(t.modifier) {
                b.put(&metal, Vec3::new(x, y, 0.0) + lo, Vec3::new(x, y, 0.0) + hi);
            }
        }
        StuckButton | DropButton | RaiseButton => {
            // pressed (lift 0): the original draws a held drop button as the stuck button and a raise button as plain floor
            let look = match (t.kind, lift == 0.0) { (DropButton, true) => StuckButton, (RaiseButton, true) => Floor, (k, _) => k };
            let m = b.tiles.and_then(|s| s.floor(look)).unwrap_or_else(|| stone.clone());
            b.slab(&m, x, y - SLAB, y + lift);
            // the original shows a button only by its own floor images (raised plate, lip); keep the marker box without them
            if b.tiles.and_then(|s| s.floor(t.kind)).is_none() {
                let m = if t.kind == RaiseButton { &p.blue } else { &p.red };
                b.put(m, Vec3::new(x + 0.3, y + lift, -0.2), Vec3::new(x + 0.7, y + lift + 0.1, 0.2));
            }
        }
        Potion => {
            floor(b, &fl);
            let (big, colour) = crate::items::potion_look(t.modifier >> 5);
            match b.items.and_then(|(i, _)| Some((i, i.bottles.clone()?))) {
                // bottle in front of the figures (the original draws it as a fore piece), bubbles on its neck in the
                // potion's colour (emissive), placed as the original draws them (items::POTION_X / BUBBLE_DX)
                Some((i, bottles)) => {
                    let set = b.items.map_or(0, |(_, s)| s);
                    let bt = &bottles[set][big as usize];
                    let lo = Vec3::new(x + px_w(crate::items::POTION_X), y, POTION_Z);
                    b.item(bt, &i.mat, lo);
                    if let (Some(c), Some(f)) = (colour, i.bubbles.first()) {
                        let at = lo + Vec3::new(px_w(crate::items::BUBBLE_DX), px_h(bt.h as f32), 0.0);
                        let e = b.item(f, &i.glow[c], at);
                        b.cmds.entity(e).insert((Bubbles(b.room as u8, idx), bevy::light::NotShadowCaster));
                    }
                }
                None => b.put(if colour == Some(2) { &p.blue } else { &p.red }, Vec3::new(x + 0.4, y, -0.1), Vec3::new(x + 0.6, y + if big { 0.4 } else { 0.3 }, 0.1)),
            }
        }
        Mirror => {
            floor(b, &fl);
            b.put(&p.glass, Vec3::new(x + 0.1, y, -0.4), Vec3::new(x + 0.9, y + TILE_H * 0.9, -0.35));
        }
        ExitLeft | ExitRight if ex.is_some() && b.objects().is_some_and(|(_, o)| o.exit[(t.kind == ExitRight) as usize].is_some()) => {
            floor(b, &fl);
            exit_voxel(b, x, y, t, ex.as_ref().unwrap());
        }
        ExitLeft | ExitRight => {
            floor(b, &fl);
            match &ex {
                // the frame image (without its floor rows) in front of the dark doorway, the lintel image above it
                Some(f) => {
                    let (w, h) = (px_w(f.w), px_h(f.h));
                    // layers inside the doorway, back to front: dark box, stairs, door panel, frame
                    b.put(&dark, Vec3::new(x, y, -0.5), Vec3::new(x + w, y + h, -0.33));
                    if let (Some(gap), true) = (b.tiles.and_then(|s| s.exit_gap), t.kind == ExitLeft) {
                        let mid = x + px_w((gap.0 + gap.1) / 2.0);
                        if let Some(s) = piece(Role::Stairs).filter(|_| !b.start_room) {
                            b.sprite(&s, Vec3::new(mid - px_w(s.w) / 2.0, y, -0.325));
                        }
                        if let (Some(sl), Some(bot)) = (piece(Role::DoorSlice), piece(Role::DoorBottom)) {
                            // modifier 0 closed .. 43 open: the panel's bottom edge is that many px above the floor
                            let (x0, mut by) = (mid - px_w(sl.w) / 2.0, t.modifier.min(43) as f32);
                            b.sprite(&bot, Vec3::new(x0, y + px_h(by), -0.315));
                            by += bot.h;
                            while by < crate::tiles::EXIT_FRAME_H as f32 {
                                b.sprite(&sl, Vec3::new(x0, y + px_h(by), -0.315));
                                by += sl.h;
                            }
                        }
                    }
                    b.put(&f.mat, Vec3::new(x, y, -0.3), Vec3::new(x + w, y + h, -0.28));
                    let lintel = b.tiles.and_then(|s| s.img(if t.kind == ExitLeft { 238 } else { 240 })).cloned();
                    if let Some(l) = lintel {
                        b.put(&l.mat, Vec3::new(x, y + h, -0.3), Vec3::new(x + px_w(l.w), y + h + px_h(l.h), -0.28));
                    }
                }
                None => b.put(&dark, Vec3::new(x + 0.1, y, -0.5), Vec3::new(x + 0.9, y + TILE_H * 0.85, -0.3)),
            }
        }
        // voxel objects: the blade and the upper teeth as thin plates standing diagonally through the depth (the 2D blade
        // is drawn sheared: its left part in front of the figures, the rest behind), stretched to the slab above as before
        Chomper if b.objects().is_some_and(|(_, o)| o.ids.contains_key(&1304)) => {
            floor(b, &fl);
            let (items, o) = b.objects().unwrap();
            let sy = (TILE_H - SLAB) / px_h(crate::tiles::EXIT_FRAME_H as f32);
            let z = JAW_DIAG_Z - (crate::items::JAW_XREF - 13.5) * crate::items::JAW_SHEAR / 32.0;
            for (r, id) in b.tiles.map_or(vec![], |s| s.piece_ids(Chomper, t.modifier)) {
                let lift = match r { Role::JawBack => 0.0, Role::JawTop(off) => px_h(off as f32) * sy, _ => continue };
                if let Some(m) = o.ids.get(&id) {
                    b.object(m, &items.mat, Vec3::new(x, y + lift, z), sy);
                }
            }
        }
        Chomper => {
            floor(b, &fl);
            match piece(Role::JawBack) {
                // the blade frame behind the figures, its front slice before them, upper teeth level with the blade top
                // the 47 px from the floor surface to the blade top are stretched to the slab above, so the closed blade
                // and the teeth reach the ceiling (the 3D slab is thinner than the 13 floor rows of the 2D tile)
                Some(j) => {
                    let sy = (TILE_H - SLAB) / px_h(crate::tiles::EXIT_FRAME_H as f32);
                    b.sprite_sy(&j, Vec3::new(x, y, JAW_Z.0), sy);
                    if let Some(f) = piece(Role::JawFore) {
                        b.sprite_sy(&f, Vec3::new(x, y, JAW_Z.1), sy);
                    }
                    if let Some((Role::JawTop(off), teeth)) = pc.iter().find(|(r, _)| matches!(r, Role::JawTop(_))).cloned() {
                        b.sprite_sy(&teeth, Vec3::new(x, y + px_h(off as f32) * sy, JAW_Z.0), sy);
                    }
                }
                None => b.put(&metal, Vec3::new(x + 0.1, y, -0.05), Vec3::new(x + 0.9, y + 0.5, 0.05)),
            }
        }
        Torch | TorchDebris => {
            // voxel objects: the torch-with-debris floor is the plain floor with the rubble heap on it
            if t.kind == TorchDebris && b.objects().is_some_and(|(_, o)| o.debris.is_some()) {
                floor(b, &plain_floor(b));
                b.debris(x, y);
            } else {
                floor(b, &fl);
            }
            let top = Vec3::new(x + 0.5, y + TILE_H * 0.65, -0.8);
            // voxel objects: the holder as a revolved rod near the back wall, the flame (9 frames) standing on its cup
            if let Some((items, h, axis)) = b.objects().filter(|(i, _)| !i.flames.is_empty()).and_then(|(i, o)| Some((i, o.ids.get(&346)?.clone(), o.holder_axis))) {
                // the holder stands between its light and the wall: no shadow (enhanced), like the flame
                let e = b.object(&h, &items.mat, Vec3::new(x, top.y - px_h(h.h as f32 - 2.0), TORCH_Z), 1.0);
                b.cmds.entity(e).insert(bevy::light::NotShadowCaster);
                let e = b.object(&items.flames[0], &items.flame_mat, Vec3::new(x + px_w(axis - FLAME_AXIS), top.y - px_h(FLAME_SINK), TORCH_Z), 1.0);
                b.cmds.entity(e).insert((TorchFlame(b.room as u8, idx), bevy::light::NotShadowCaster));
                let e = b.cmds.spawn((torch_light(), Transform::from_translation(b.origin + top + Vec3::new(0.0, 0.3, 0.6)))).id();
                b.tag(e);
                return;
            }
            // the holder (346, 32 px wide, the cup at its top centre) on the back wall below the flame
            if let Some(s) = piece(Role::TorchStub) {
                b.sprite(&s, Vec3::new(x, top.y - px_h(s.h - 2.0), -0.86));
            }
            let flame = b.put_id(&p.flame, top - Vec3::new(0.06, 0.0, 0.06), top + Vec3::new(0.06, 0.25, 0.06));
            // the flame sits next to its own light: it must not shadow it (only matters for enhanced-mode shadow maps)
            b.cmds.entity(flame).insert(bevy::light::NotShadowCaster);
            let e = b.cmds.spawn((torch_light(), Transform::from_translation(b.origin + top + Vec3::new(0.0, 0.3, 0.6)))).id();
            b.tag(e);
        }
        Unknown31 => floor(b, &p.blue),
    }
}

/// Depth of the voxel items' sprite plane: the skeleton behind the figures (VOXEL_Z 0.25), the bottle in front of them.
const SKELETON_Z: f32 = -0.1;
const POTION_Z: f32 = 0.4;
/// Depth of the voxel columns' axis: behind the figures (VOXEL_Z 0.25, about 0.06 deep behind it), so the widest base
/// (big pillar, radius 11.5 px = 0.36) ends at z 0.16 and the kid never cuts through a column.
const COLUMN_Z: f32 = -0.2;
/// Depth (behind, in front of the figures) of the chomper blade and its front slice, and of the spike pieces.
const JAW_Z: (f32, f32) = (-0.1, 0.42);
const SPIKE_Z: (f32, f32) = (-0.05, 0.42);
/// Voxel objects: depth of the spike rows [fore, back, right piece] (figures at 0.25), the 2D floor's shear (px the back
/// edge of the floor top is drawn right of its front edge, the right piece's offset), depth where the chomper blade
/// crosses the figures' plane, the torch holder's axis (back wall face at -0.9) and the flame image's anchor (x of its
/// stem, rows it sinks into the cup), the debris layer height and the exit layers (frame plane, door panel, stairs, dark
/// doorway).
const SPIKE_ROWS: [f32; 3] = [0.4, 0.05, -0.3];
const FLOOR_SHEAR: f32 = 26.0;
const JAW_DIAG_Z: f32 = 0.25;
const TORCH_Z: f32 = -0.77;
const FLAME_AXIS: f32 = 7.5;
const FLAME_SINK: f32 = 1.0;
const DEBRIS_LAYER: f32 = 1.0 / 32.0;
const EXIT_Z: [f32; 4] = [-0.33, -0.4, -0.5, -0.58];

/// Height of the dungeon wall's top strip cap (on top of the face block, no coplanar faces).
const CAP: f32 = 0.01;
/// Back-wall depth of the balcony windows and lattice arches (the back wall's front face is at -0.9).
const BACK_Z: f32 = -0.88;

/// The palace balcony window / lattice arch panel on the back wall, standing on the floor, its top piece above it.
fn back_panel(b: &mut B, piece: &dyn Fn(Role) -> Option<Tex>, x: f32, y: f32) {
    if let Some(p) = piece(Role::BackPanel) {
        b.sprite(&p, Vec3::new(x, y, BACK_Z));
        if let Some(t) = piece(Role::BackTop) {
            b.sprite(&t, Vec3::new(x, y + px_h(p.h), BACK_Z));
        }
    }
}

/// Gate plane: logic x tl+10 .. tl+13 of the 14-unit column (gate wall_type 1, docs/collisions.md 3) = 0.71 .. 0.93 of the
/// tile; the bars stand across the corridor at its middle, 11.5 / 14.
pub const GATE_X: f32 = 11.5 / 14.0;

/// Gate bars as boxes (lo, hi) relative to (tile left x, floor y, z 0): turned 90 degrees about the vertical axis, so the
/// gate is a grille in the y-z plane across the corridor at GATE_X, from the door post's front face (z -0.15) to the
/// floor's front edge. Four vertical bars along z plus a bottom rail and a middle rail, slightly thick along x so the gate
/// reads edge-on from the game camera. Runtime modifier 0 = closed .. 188 = open (docs/tiles.md 3): the grille slides up.
pub fn gate_bars(modifier: u8) -> Vec<(Vec3, Vec3)> {
    let open = (modifier as f32 / 188.0).clamp(0.0, 1.0);
    let (bottom, top) = (open * TILE_H * 0.8, TILE_H * 0.95);
    let (x0, x1) = (GATE_X - 0.03, GATE_X + 0.03);
    let mut v: Vec<(Vec3, Vec3)> = (0..4)
        .map(|i| {
            let z = -0.1 + i as f32 * 0.5 / 3.0;
            (Vec3::new(x0, bottom, z - 0.025), Vec3::new(x1, top, z + 0.025))
        })
        .collect();
    for y in [bottom + 0.02, (bottom + top) / 2.0] {
        v.push((Vec3::new(x0 + 0.005, y, -0.15), Vec3::new(x1 - 0.005, y + 0.05, 0.45)));
    }
    v
}

/// A voxel exit door: dark doorway, stairs (not in the start room), the door panel slid up by the modifier (ExitLeft) and
/// the frame with its lintel as a thick relief in front; `f` = the flat frame (size of the doorway).
fn exit_voxel(b: &mut B, x: f32, y: f32, t: TileRef, f: &Tex) {
    let (items, o) = b.objects().unwrap();
    let (w, h) = (px_w(f.w), px_h(f.h));
    b.put(&b.p.dark, Vec3::new(x, y, EXIT_Z[3] - 0.04), Vec3::new(x + w, y + h, EXIT_Z[3]));
    if let (Some(gap), Tile::ExitLeft) = (b.tiles.and_then(|s| s.exit_gap), t.kind) {
        let mid = x + px_w((gap.0 + gap.1) / 2.0);
        if let Some(s) = o.stairs.as_ref().filter(|_| !b.start_room) {
            b.object(s, &items.mat, Vec3::new(mid - px_w(s.w as f32) / 2.0, y, EXIT_Z[2]), 1.0);
        }
        if let Some(p) = o.panels.get(crate::items::panel_slices(t.modifier)) {
            b.object(p, &items.mat, Vec3::new(mid - px_w(p.w as f32) / 2.0, y + px_h(t.modifier.min(43) as f32), EXIT_Z[1]), 1.0);
        }
    }
    if let Some(m) = &o.exit[(t.kind == Tile::ExitRight) as usize] {
        b.object(m, &items.mat, Vec3::new(x, y, EXIT_Z[0]), 1.0);
    }
}

/// The door post image (record +9, 8 px wide) at the right edge of the cell, standing on the back half of the floor
/// (z -0.4 .. -0.15, the gate grille starts at its front): a voxel column, or the image on a box (flat style).
fn door_post(b: &mut B, d: &Tex, x: f32, y: f32, kind: Tile, modifier: u8) {
    let w = px_w(d.w);
    if !b.voxel_column(b.tiles.and_then(|s| s.extra_id(kind, modifier)), x + 1.0 - w, y, y + TILE_H, -0.4 + w / 2.0) {
        b.put(&d.mat, Vec3::new(x + 1.0 - w, y, -0.4), Vec3::new(x + 1.0, y + TILE_H, -0.4 + w));
    }
}

/// The light of a torch (levels and the cutscene set). It sits 0.1 below the slab above (TILE_H * 0.65 + 0.3 = 1.6 over
/// the floor, the slab's underside at 1.7): with Bevy's default shadow near plane (0.1) that slab was clipped out of the
/// shadow map's upward face and the light leaked through it in a 90-degree wedge onto the back wall above (enhanced).
pub fn torch_light() -> PointLight {
    PointLight { color: Color::srgb(1.0, 0.65, 0.3), intensity: 250_000.0, range: 9.0, shadow_maps_enabled: false, shadow_map_near_z: TORCH_NEAR_Z, ..default() }
}

/// Near plane of the torch shadow maps, well inside the 0.1 gap between a torch light and the slab above it.
const TORCH_NEAR_Z: f32 = 0.02;

/// Room rebuilds so far (POP_PERF).
pub static REBUILDS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The rooms on screen as last built: (room, origin, key of each tile).
#[derive(Default)]
pub struct Built(Vec<(usize, Vec3, Vec<TileKey>)>);

/// Level or room changed: despawn and rebuild the 9 rooms. Only the world changed (a gate, plate, loose floor, door, a new
/// world on restart): respawn just the tiles whose key changed and the falling slabs; everything else, the torch lights
/// with their shadow maps included, stays as it is (a respawned shadowed light gets new shadow views every time).
#[allow(clippy::type_complexity)]
pub fn rebuild_on_change(
    mut cmds: Commands,
    g: Res<Game>,
    old: Query<(Entity, Option<&TileOf>, Has<Falling>), With<RoomEntity>>,
    p: Res<Palette>,
    mut last: Local<(usize, usize, u32, u32)>,
    mut built: Local<Built>,
) {
    let now = (g.level, g.room, g.world.version, g.gen);
    if *last == now {
        return;
    }
    let full = (last.0, last.1) != (g.level, g.room) || built.0.is_empty();
    *last = now;
    REBUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (level, palace) = (&g.world.level, crate::kid::palace(g.level));
    if full {
        for (e, ..) in &old {
            cmds.entity(e).despawn();
        }
        built.0.clear();
        for (room, dx, dy, origin) in rooms_around(&g) {
            let b = &mut room_builder(&mut cmds, level, room, origin, &p, palace);
            if room == 0 {
                spawn_edge(b, dx, dy);
                continue;
            }
            spawn_tiles(b, level, &g.world.mobs, room);
            built.0.push((room, origin, (0..ROWS * COLS).map(|i| tile_key(level, room, i, palace)).collect()));
        }
        return;
    }
    let mut changed = Vec::new();
    for (room, _, keys) in &mut built.0 {
        for (idx, k) in keys.iter_mut().enumerate() {
            let now = tile_key(level, *room, idx, palace);
            // a room can show twice (links that loop): one entry in `changed`, respawned at each origin below
            if *k != now {
                *k = now;
                if !changed.contains(&TileOf(*room as u8, idx as u8)) {
                    changed.push(TileOf(*room as u8, idx as u8));
                }
            }
        }
    }
    for (e, t, falling) in &old {
        if falling || t.is_some_and(|t| changed.contains(t)) {
            cmds.entity(e).despawn();
        }
    }
    for (room, origin, _) in &built.0 {
        let b = &mut room_builder(&mut cmds, level, *room, *origin, &p, palace);
        for t in changed.iter().filter(|t| t.0 as usize == *room) {
            spawn_tile(b, level, *room, t.1 as usize);
        }
        spawn_falling(b, &g.world.mobs, *room);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_lift_is_ten_percent_of_the_slab_and_zero_when_pressed() {
        assert!((PLATE_LIFT - 0.03).abs() < 1e-6);
        assert_eq!(plate_lift(0), PLATE_LIFT);
        assert_eq!(plate_lift(1), PLATE_LIFT); // pops up when the timer reaches 1
        assert_eq!(plate_lift(5), 0.0);
        assert_eq!(plate_lift(2), 0.0);
    }

    #[test]
    fn wobble_follows_the_trob_state() {
        assert_eq!(wobble(0, 0.5), (0.0, 0.0));
        assert_eq!(wobble(0xF5, 0.5), (0.0, 0.0)); // level 13 countdown: still
        // knock: decays to nothing at 0x84 (0x83 + alpha 1)
        let (d1, _) = wobble(0x80, 0.5);
        let (d3, _) = wobble(0x83, 0.5);
        assert!(d1 > d3 && d3 > 0.0 && d1 <= WOBBLE_DY);
        assert!(wobble(0x83, 1.0).0.abs() < 1e-6);
        // about to fall: the roll changes side every tick, bounded
        let (r1, r2) = (wobble(3, 0.5).1, wobble(4, 0.5).1);
        assert!(r1 * r2 < 0.0 && r1.abs() <= WOBBLE_ROLL);
        // continuous across a tick: end of tick k = start of tick k + 1
        let (a, b) = (wobble(5, 0.999), wobble(6, 0.0));
        assert!((a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3);
    }

    #[test]
    fn tile_key_changes_only_with_the_look() {
        let mut l = Level::parse(&[0u8; 4096]).expect("zero level");
        l.set_tile(1, 3, Tile::Gate);
        let closed = tile_key(&l, 1, 3, false);
        l.set_modifier(1, 3, 40);
        assert_ne!(tile_key(&l, 1, 3, false), closed, "a moving gate is rebuilt");
        l.set_tile(1, 4, Tile::LooseFloor);
        let still = tile_key(&l, 1, 4, false);
        l.set_modifier(1, 4, 0x81);
        assert_eq!(tile_key(&l, 1, 4, false), still, "a wobbling loose floor animates itself");
        l.set_tile(1, 4, Tile::Empty);
        assert_ne!(tile_key(&l, 1, 4, false), still, "a fallen loose floor is rebuilt");
        assert_eq!(tile_key(&l, 1, 5, false), tile_key(&l, 1, 5, false));
        // flames, bubbles and the sword glint animate themselves; a potion's kind is part of the look
        for (t, a, b) in [(Tile::Torch, 0, 8), (Tile::Sword, 1, 40), (Tile::Potion, 0x21, 0x27)] {
            l.set_tile(1, 8, t);
            l.set_modifier(1, 8, a);
            let k = tile_key(&l, 1, 8, false);
            l.set_modifier(1, 8, b);
            assert_eq!(tile_key(&l, 1, 8, false), k, "{t:?}");
        }
        l.set_modifier(1, 8, 0x41);
        assert_ne!(tile_key(&l, 1, 8, false), tile_key(&{ let mut c = l.clone(); c.set_modifier(1, 8, 0x21); c }, 1, 8, false));
        // the background piece of tile 7 comes from tile 6: rebuilt when that changes
        l.set_tile(1, 6, Tile::Empty);
        let plain = tile_key(&l, 1, 7, false);
        l.set_modifier(1, 6, 1);
        assert_ne!(tile_key(&l, 1, 7, false), plain, "a background piece appears");
    }

    /// Floor slabs and wall blocks reach into the back wall box (no gap for light or the eye), the back wall covers the
    /// whole room, and a place outside the level gets a back wall and its edge geometry.
    #[test]
    fn slabs_and_walls_close_against_the_back_wall() {
        let mut world = World::new();
        let (mut meshes, mut mats, mut imgs) = (Assets::<Mesh>::default(), Assets::<StandardMaterial>::default(), Assets::<Image>::default());
        let p = Palette::new(&mut meshes, &mut mats, &mut imgs, None, false, false);
        let mut l = Level::parse(&[0u8; 4096]).expect("zero level");
        for (i, t) in [(0, Tile::Floor), (1, Tile::Wall), (2, Tile::LooseFloor), (3, Tile::DropButton), (4, Tile::Pillar), (11, Tile::Torch)] {
            l.set_tile(1, i, t);
        }
        let boxes = |world: &mut World| -> Vec<(Vec3, Vec3)> {
            world.query_filtered::<&Transform, With<RoomEntity>>().iter(world).filter(|t| t.scale != Vec3::ONE).map(|t| (t.translation - t.scale / 2.0, t.translation + t.scale / 2.0)).collect()
        };
        let mut q = bevy::ecs::world::CommandQueue::default();
        spawn_tiles(&mut room_builder(&mut Commands::new(&mut q, &world), &l, 1, Vec3::ZERO, &p, false), &l, &[], 1);
        q.apply(&mut world);
        let all = boxes(&mut world);
        let wall = *all.iter().find(|(lo, hi)| lo.z == BACK_WALL.0 && hi.z == BACK_WALL.1).expect("back wall");
        assert!(wall.0.x <= 0.0 && wall.1.x >= COLS as f32 && wall.0.y <= -SLAB && wall.1.y >= ROWS as f32 * TILE_H);
        // every box that spans the tile depth (slabs, walls, plates, loose floors) ends inside the back wall box
        let slabs: Vec<_> = all.iter().filter(|(lo, hi)| hi.z == 0.5 && hi.x - lo.x == 1.0).collect();
        assert!(slabs.len() >= 6, "{slabs:?}");
        for (lo, _) in &slabs {
            assert!(lo.z < BACK_WALL.1 && lo.z > BACK_WALL.0, "slab back {lo}");
        }
        // outside the level: back wall plus the wall cells next to the room (left neighbour missing: its right columns)
        world.clear_entities();
        spawn_edge(&mut room_builder(&mut Commands::new(&mut q, &world), &l, 0, Vec3::ZERO, &p, false), -1, 0);
        q.apply(&mut world);
        let edge = boxes(&mut world);
        assert!(edge.iter().any(|(lo, hi)| lo.z == BACK_WALL.0 && hi.z == BACK_WALL.1));
        let cells: Vec<_> = edge.iter().filter(|(_, hi)| hi.z == 0.5).collect();
        assert_eq!(cells.len(), EDGE_COLS * ROWS);
        assert!(cells.iter().all(|(lo, _)| lo.x >= (COLS - EDGE_COLS) as f32 && lo.z < BACK_WALL.1));
    }

    #[test]
    fn gate_stands_across_the_corridor_and_slides_up() {
        let closed = gate_bars(0);
        for (lo, hi) in &closed {
            // thin along x at the gate's collision position, long along z or y
            assert!(lo.x >= 10.0 / 14.0 && hi.x <= 13.0 / 14.0 && hi.x - lo.x < 0.1, "{lo} {hi}");
            assert!(lo.z >= -0.15 && hi.z <= 0.45);
        }
        assert!(closed.iter().any(|(lo, hi)| hi.z - lo.z > 0.5), "a rail spans the corridor");
        assert_eq!(closed.iter().filter(|(lo, hi)| hi.y - lo.y > 1.0).count(), 4, "four vertical bars");
        let open = gate_bars(188);
        assert!(open[0].0.y > 1.5 && closed[0].0.y == 0.0);
        assert_eq!(gate_bars(255)[0].0.y, open[0].0.y); // 238 / 255 = fully open
    }
}
