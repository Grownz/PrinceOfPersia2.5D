//! Geometry for one room: a box per tile piece. With `tile_textures` = original the pieces carry the original tile images
//! (`tiles`, docs/texture_inventory.md 5); otherwise, and for pieces without an image, plain coloured boxes.
use crate::tiles::{Tex, TileSet};
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
}

impl Palette {
    /// `tiles_dir`: game data folder to load the tile images from (None = procedural tiles).
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>, tiles_dir: Option<&str>) -> Self {
        let tiles = tiles_dir.map_or([None, None], |d| crate::tiles::load_sets(d, imgs, mats));
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
    tiles: Option<&'a TileSet>,
}

impl B<'_, '_, '_> {
    /// Box from `lo` to `hi` (corner coordinates).
    fn put(&mut self, mat: &Handle<StandardMaterial>, lo: Vec3, hi: Vec3) {
        self.put_id(mat, lo, hi);
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
    spawn_tiles(&mut B { cmds, p, origin, chamber: false, tiles: p.tiles[palace as usize].as_ref() }, level, mobs, room);
}

/// The same builder for the cutscene set: entities carry `Chamber` instead of `RoomEntity`. The set keeps the procedural look.
pub fn spawn_chamber(cmds: &mut Commands, level: &Level, room: usize, origin: Vec3, p: &Palette) {
    spawn_tiles(&mut B { cmds, p, origin, chamber: true, tiles: None }, level, &[], room);
}

/// A plain box of the room palette in the chamber set (`mat`: "stone", "dark", "wood", "flame"; anything else = metal).
pub fn chamber_box(cmds: &mut Commands, p: &Palette, mat: &str, lo: Vec3, hi: Vec3) {
    let m = match mat { "stone" => p.stone.clone(), "dark" => p.dark.clone(), "wood" => p.wood.clone(), "flame" => p.flame.clone(), _ => p.metal.clone() };
    B { cmds, p, origin: Vec3::ZERO, chamber: true, tiles: None }.put(&m, lo, hi);
}

fn spawn_tiles(b: &mut B, level: &Level, mobs: &[pop_core::sim::Mob], room: usize) {
    let p = b.p;
    // back wall
    b.put(&p.dark, Vec3::new(-0.5, -SLAB, -1.0), Vec3::new(10.5, 3.0 * TILE_H, -0.9));
    for row in 0..ROWS {
        for col in 0..COLS {
            let t = level.tile(room, col, row);
            tile(b, col as f32, (ROWS - 1 - row) as f32 * TILE_H, t);
        }
    }
    // falling loose floors: mob y is a screen line, 65 + 63 * row = floor line of that row
    for m in mobs.iter().filter(|m| m.room as usize == room) {
        let (x, y) = ((m.xh >> 2) as f32, (191.0 - m.y as f32) * TILE_H / 63.0);
        b.put(&p.wood, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
    }
}

/// `x`: left edge, `y`: floor surface height of this tile's row.
fn tile(b: &mut B, x: f32, y: f32, t: TileRef) {
    use Tile::*;
    let p = b.p;
    let (stone, dark, wood, metal) = (p.stone.clone(), p.dark.clone(), p.wood.clone(), p.metal.clone());
    // original images: the floor of this tile type and its extra piece (pillar, door post, exit frame, wall face)
    let fl = b.tiles.and_then(|s| s.floor(t.kind)).unwrap_or_else(|| stone.clone());
    let ex: Option<Tex> = b.tiles.and_then(|s| s.extra(t.kind, t.modifier)).cloned();
    let floor = |b: &mut B, m: &Handle<StandardMaterial>| b.put(m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
    let column = |b: &mut B, m: &Handle<StandardMaterial>, w: f32, h: f32| {
        b.put(m, Vec3::new(x + 0.5 - w / 2.0, y, -w / 2.0), Vec3::new(x + 0.5 + w / 2.0, y + h, w / 2.0))
    };
    match t.kind {
        Empty => {}
        // textured: one row tall, so the faces of stacked walls and of the floor above do not overlap (z-fighting)
        Wall => match &ex {
            Some(w) => b.put(&w.mat, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y + TILE_H - SLAB, 0.5)),
            None => b.put(&stone, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y + TILE_H, 0.5)),
        },
        Floor | Skeleton | Sword | Debris | BalconyLeft | BalconyRight => {
            floor(b, &fl);
            match t.kind {
                Skeleton => b.put(&p.glass, Vec3::new(x + 0.3, y, -0.2), Vec3::new(x + 0.7, y + 0.15, 0.2)),
                Sword => b.put(&metal, Vec3::new(x + 0.2, y, -0.05), Vec3::new(x + 0.8, y + 0.05, 0.05)),
                Debris => b.put(&dark, Vec3::new(x + 0.2, y, -0.2), Vec3::new(x + 0.8, y + 0.15, 0.2)),
                _ => {}
            }
        }
        LooseFloor => floor(b, &wood),
        Spikes => {
            floor(b, &fl);
            // modifier: 0 retracted, 1..=4 rising, >= 0x80 or 255 out, 5..=9 retracting
            let h = match t.modifier { 0 => 0.04, 255 => 0.45, m if m & 0x80 != 0 => 0.45, m => (m as f32 * 0.11).min(0.45) };
            for i in 0..4 {
                let sx = x + 0.15 + i as f32 * 0.23;
                b.put(&metal, Vec3::new(sx, y, -0.05), Vec3::new(sx + 0.06, y + h, 0.05));
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
                column(b, &stone, 0.5, TILE_H);
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
            b.put(&metal, Vec3::new(x, y, -0.5), Vec3::new(x + 1.0, y + TILE_H * 0.9, -0.45));
        }
        Gate => {
            floor(b, &fl);
            if let Some(d) = &ex {
                door_post(b, d, x, y);
            }
            // runtime gate modifier: 0 = closed, 188 = open (docs/tiles.md)
            let open = (t.modifier as f32 / 188.0).clamp(0.0, 1.0);
            for i in 0..5 {
                let sx = x + 0.1 + i as f32 * 0.2;
                b.put(&metal, Vec3::new(sx, y + open * TILE_H * 0.8, -0.05), Vec3::new(sx + 0.05, y + TILE_H * 0.95, 0.05));
            }
        }
        StuckButton | DropButton | RaiseButton => {
            floor(b, &fl);
            // the original shows a button only by its own floor images (raised plate, lip); keep the marker box without them
            if b.tiles.and_then(|s| s.floor(t.kind)).is_none() {
                let m = if t.kind == RaiseButton { &p.blue } else { &p.red };
                b.put(m, Vec3::new(x + 0.3, y, -0.2), Vec3::new(x + 0.7, y + 0.1, 0.2));
            }
        }
        Potion => {
            floor(b, &fl);
            b.put(&p.red, Vec3::new(x + 0.4, y, -0.1), Vec3::new(x + 0.6, y + 0.3, 0.1));
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
                    b.put(&dark, Vec3::new(x, y, -0.5), Vec3::new(x + w, y + h, -0.3));
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
            b.put(&metal, Vec3::new(x + 0.1, y, -0.05), Vec3::new(x + 0.9, y + 0.5, 0.05));
        }
        Torch | TorchDebris => {
            floor(b, &fl);
            let top = Vec3::new(x + 0.5, y + TILE_H * 0.65, -0.8);
            let flame = b.put_id(&p.flame, top - Vec3::new(0.06, 0.0, 0.06), top + Vec3::new(0.06, 0.25, 0.06));
            // the flame sits next to its own light: it must not shadow it (only matters for enhanced-mode shadow maps)
            b.cmds.entity(flame).insert(bevy::light::NotShadowCaster);
            let e = b.cmds.spawn((torch_light(), Transform::from_translation(b.origin + top + Vec3::new(0.0, 0.3, 0.6)))).id();
            b.tag(e);
        }
        Unknown31 => floor(b, &p.blue),
    }
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
