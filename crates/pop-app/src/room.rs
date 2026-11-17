//! Geometry for one room: a box per tile piece. With `tile_textures` = original the pieces carry the original tile images
//! (`tiles`, docs/texture_inventory.md 5); otherwise, and for pieces without an image, plain coloured boxes.
use crate::tiles::{Role, Tex, TileSet};
use crate::Game;
use bevy::prelude::*;
use pop_core::level::*;

/// Height of one tile row (tile width and depth are 1).
pub const TILE_H: f32 = 2.0;
const SLAB: f32 = 0.3;

#[derive(Component)]
pub struct RoomEntity;

/// Geometry of the cutscene set (the princess chamber), kept apart from the room rebuilds.
#[derive(Component)]
pub struct Chamber;

#[derive(Resource)]
pub struct Palette {
    cube: Handle<Mesh>,
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
    /// `tiles_dir`: game data folder to load the tile images from (None = procedural tiles).
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>, tiles_dir: Option<&str>) -> Self {
        let tiles = tiles_dir.map_or([None, None], |d| crate::tiles::load_sets(d, imgs, mats));
        let items = tiles_dir.map(|d| crate::items::Items::load(d, meshes, mats));
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
}

impl B<'_, '_, '_> {
    /// A voxel item model: bottom-left corner of its image at `lo` (room coordinates), sprite plane at `lo.z`.
    fn item(&mut self, m: &crate::items::Model, mat: &Handle<StandardMaterial>, lo: Vec3) -> Entity {
        let px = Vec3::new(1.0 / 32.0, TILE_H / TILE_H_PX as f32, 1.0 / 32.0);
        let e = self.cmds.spawn((Mesh3d(m.mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(self.origin + lo).with_scale(px))).id();
        self.tag(e);
        e
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

    fn tag(&mut self, e: Entity) {
        if self.chamber {
            self.cmds.entity(e).insert(Chamber);
        } else {
            self.cmds.entity(e).insert(RoomEntity);
        }
    }
}

/// The current room plus its 8 neighbours (left/right/up/down and the diagonals via the neighbours' links).
pub fn build_room(mut cmds: Commands, g: Res<Game>, p: Res<Palette>) {
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
    for (room, dx, dy) in around {
        if (1..=ROOMS as u8).contains(&room) {
            let origin = Vec3::new(dx as f32 * COLS as f32, dy as f32 * ROWS as f32 * TILE_H, 0.0);
            spawn_room(&mut cmds, level, &g.world.mobs, room as usize, origin, &p, crate::kid::palace(g.level));
        }
    }
}

/// Spawn `room` (1-based) with its left/bottom corner at `origin`; neighbours can be drawn with other origins.
pub fn spawn_room(cmds: &mut Commands, level: &Level, mobs: &[pop_core::sim::Mob], room: usize, origin: Vec3, p: &Palette, palace: bool) {
    let items = p.items.as_ref().map(|i| (i, palace as usize));
    spawn_tiles(&mut B { cmds, p, origin, chamber: false, room, start_room: room == level.start_room as usize, tiles: p.tiles[palace as usize].as_ref(), items }, level, mobs, room);
}

/// The same builder for the cutscene set: entities carry `Chamber` instead of `RoomEntity`. The set keeps the procedural look.
pub fn spawn_chamber(cmds: &mut Commands, level: &Level, room: usize, origin: Vec3, p: &Palette) {
    spawn_tiles(&mut B { cmds, p, origin, chamber: true, room, start_room: false, tiles: None, items: None }, level, &[], room);
}

/// A plain box of the room palette in the chamber set (`mat`: "stone", "dark", "wood", "flame"; anything else = metal).
pub fn chamber_box(cmds: &mut Commands, p: &Palette, mat: &str, lo: Vec3, hi: Vec3) {
    let m = match mat { "stone" => p.stone.clone(), "dark" => p.dark.clone(), "wood" => p.wood.clone(), "flame" => p.flame.clone(), _ => p.metal.clone() };
    B { cmds, p, origin: Vec3::ZERO, chamber: true, room: 0, start_room: false, tiles: None, items: None }.put(&m, lo, hi);
}

fn spawn_tiles(b: &mut B, level: &Level, mobs: &[pop_core::sim::Mob], room: usize) {
    let p = b.p;
    // back wall
    b.put(&p.dark, Vec3::new(-0.5, -SLAB, -1.0), Vec3::new(10.5, 3.0 * TILE_H, -0.9));
    for row in 0..ROWS {
        for col in 0..COLS {
            let t = level.tile(room, col, row);
            let (x, y) = (col as f32, (ROWS - 1 - row) as f32 * TILE_H);
            if t.kind == Tile::LooseFloor {
                // its own entity, moved every frame by `animate` (wobble from the trob state)
                let e = b.put_id(&plain_floor(b), Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
                let base = b.origin + Vec3::new(x + 0.5, y - SLAB / 2.0, 0.0);
                b.cmds.entity(e).insert(Loose { room: room as u8, idx: (row * COLS + col) as u8, base });
                continue;
            }
            let lift = match t.kind { Tile::DropButton | Tile::RaiseButton => plate_lift(level.link_timer(t.modifier)), _ => 0.0 };
            tile(b, x, y, t, lift, (row * COLS + col) as u8);
        }
    }
    // falling loose floors: mob y is a screen line, 65 + 63 * row = floor line of that row; drawn one tick behind the sim
    // and interpolated by `animate` (y of the last tick = y - speed)
    for m in mobs.iter().filter(|m| m.room as usize == room) {
        let (x, y) = ((m.xh >> 2) as f32, (191.0 - m.y as f32) * TILE_H / 63.0);
        let e = b.put_id(&plain_floor(b), Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
        let base = b.origin + Vec3::new(x + 0.5, y - SLAB / 2.0, 0.0);
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

/// A potion's bubbles: the frame shown is (tick + phase) mod 7.
#[derive(Component)]
pub struct Bubbles(u8);

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
pub fn animate(g: Res<Game>, p: Res<Palette>, mut loose: Query<(&Loose, &mut Transform), Without<Falling>>, mut falling: Query<(&Falling, &mut Transform), Without<Loose>>, mut bubbles: Query<(&Bubbles, &mut Mesh3d)>) {
    let a = g.world.alpha();
    for (l, mut t) in &mut loose {
        let (dy, roll) = wobble(g.world.level.tile_at(l.room as usize, l.idx as usize).modifier, a);
        t.set_if_neq(Transform { translation: l.base + Vec3::Y * dy, rotation: Quat::from_rotation_z(roll), scale: t.scale });
    }
    for (f, mut t) in &mut falling {
        t.translation = f.base + Vec3::Y * f.up * (1.0 - a);
    }
    if let Some(frames) = p.items.as_ref().map(|i| &i.bubbles).filter(|f| !f.is_empty()) {
        for (b, mut m) in &mut bubbles {
            let want = &frames[(g.world.tick as usize + b.0 as usize) % frames.len()].mesh;
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
    let floor = |b: &mut B, m: &Handle<StandardMaterial>| b.put(m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
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
                        b.put(&m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, top - CAP, 0.5));
                        b.put(&cap.mat, Vec3::new(x, top - CAP, -0.5), Vec3::new(x + 1.0, top, 0.5));
                    }
                    None => b.put(&m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, top, 0.5)),
                }
            }
            None => b.put(&stone, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y + TILE_H, 0.5)),
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
            // the column image (record +9, the piece drawn in front of the kid) on a square column of the image's width
            Some(c) => {
                if t.kind != BigPillarTop {
                    floor(b, &fl); // the big pillar top has no floor piece: the column goes on through
                }
                column(b, &c.mat, px_w(c.w), TILE_H);
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
                Some(d) => door_post(b, d, x, y),
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
                door_post(b, d, x, y);
            }
            for (lo, hi) in gate_bars(t.modifier) {
                b.put(&metal, Vec3::new(x, y, 0.0) + lo, Vec3::new(x, y, 0.0) + hi);
            }
        }
        StuckButton | DropButton | RaiseButton => {
            // pressed (lift 0): the original draws a held drop button as the stuck button and a raise button as plain floor
            let look = match (t.kind, lift == 0.0) { (DropButton, true) => StuckButton, (RaiseButton, true) => Floor, (k, _) => k };
            let m = b.tiles.and_then(|s| s.floor(look)).unwrap_or_else(|| stone.clone());
            b.put(&m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y + lift, 0.5));
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
                // potion's colour (emissive), x from the original's draw_xh + 3 (24 px into the tile)
                Some((i, bottles)) => {
                    let bt = &bottles[big as usize];
                    let lo = Vec3::new(x + 0.25, y, POTION_Z);
                    b.item(bt, &i.mat, lo);
                    if let (Some(c), Some(f)) = (colour, i.bubbles.first()) {
                        let at = lo + Vec3::new(px_w((bt.w as f32 - f.w as f32) / 2.0), px_h(bt.h as f32 - 1.0), 0.0);
                        let e = b.item(f, &i.glow[c], at);
                        b.cmds.entity(e).insert((Bubbles(idx), bevy::light::NotShadowCaster));
                    }
                }
                None => b.put(if colour == Some(2) { &p.blue } else { &p.red }, Vec3::new(x + 0.4, y, -0.1), Vec3::new(x + 0.6, y + if big { 0.4 } else { 0.3 }, 0.1)),
            }
        }
        Mirror => {
            floor(b, &fl);
            b.put(&p.glass, Vec3::new(x + 0.1, y, -0.4), Vec3::new(x + 0.9, y + TILE_H * 0.9, -0.35));
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
            floor(b, &fl);
            let top = Vec3::new(x + 0.5, y + TILE_H * 0.65, -0.8);
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
/// Depth (behind, in front of the figures) of the chomper blade and its front slice, and of the spike pieces.
const JAW_Z: (f32, f32) = (-0.1, 0.42);
const SPIKE_Z: (f32, f32) = (-0.05, 0.42);
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

/// The door post image (record +9, 8 px wide) at the right edge of the cell, standing on the back half of the floor.
fn door_post(b: &mut B, d: &Tex, x: f32, y: f32) {
    let w = px_w(d.w);
    b.put(&d.mat, Vec3::new(x + 1.0 - w, y, -0.4), Vec3::new(x + 1.0, y + TILE_H, -0.4 + w));
}

/// The light of a torch (levels and the cutscene set).
pub fn torch_light() -> PointLight {
    PointLight { color: Color::srgb(1.0, 0.65, 0.3), intensity: 250_000.0, range: 9.0, shadow_maps_enabled: false, ..default() }
}

/// Room rebuilds so far (POP_PERF).
pub static REBUILDS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Despawn and rebuild whenever level or room changed.
pub fn rebuild_on_change(mut cmds: Commands, g: Res<Game>, old: Query<Entity, With<RoomEntity>>, p: Res<Palette>, mut last: Local<(usize, usize, u32)>) {
    if *last == (g.level, g.room, g.world.version) {
        return;
    }
    *last = (g.level, g.room, g.world.version);
    REBUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    for e in &old {
        cmds.entity(e).despawn();
    }
    build_room(cmds, g, p);
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
