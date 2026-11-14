//! Procedural placeholder geometry for one room. Final art comes later; the point here is that every
//! tile type has a distinct, recognisable shape at the correct place.
use crate::Game;
use bevy::prelude::*;
use pop_core::level::*;

/// Height of one tile row (tile width and depth are 1).
pub const TILE_H: f32 = 2.0;
const SLAB: f32 = 0.3;

#[derive(Component)]
pub struct RoomEntity;

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
}

impl Palette {
    pub fn new(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Self {
        let mut m = |r, g, b| mats.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.9, ..default() });
        Palette {
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            stone: m(0.62, 0.52, 0.36),
            dark: m(0.16, 0.14, 0.12),
            wood: m(0.45, 0.28, 0.14),
            metal: m(0.55, 0.57, 0.62),
            red: m(0.75, 0.12, 0.1),
            blue: m(0.15, 0.35, 0.85),
            glass: m(0.5, 0.75, 0.9),
            flame: mats.add(StandardMaterial { base_color: Color::srgb(1.0, 0.6, 0.1), emissive: LinearRgba::rgb(8.0, 3.0, 0.5), ..default() }),
        }
    }
}

struct B<'a, 'w, 's> {
    cmds: &'a mut Commands<'w, 's>,
    p: &'a Palette,
    origin: Vec3,
}

impl B<'_, '_, '_> {
    /// Box from `lo` to `hi` (corner coordinates).
    fn put(&mut self, mat: &Handle<StandardMaterial>, lo: Vec3, hi: Vec3) {
        self.cmds.spawn((
            Mesh3d(self.p.cube.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(self.origin + (lo + hi) / 2.0).with_scale(hi - lo),
            RoomEntity,
        ));
    }
}

pub fn build_room(mut cmds: Commands, g: Res<Game>, p: Res<Palette>) {
    spawn_room(&mut cmds, &g.levels[g.level], g.room, Vec3::ZERO, &p);
}

/// Spawn `room` (1-based) with its left/bottom corner at `origin`; neighbours can be drawn with other origins.
pub fn spawn_room(cmds: &mut Commands, level: &Level, room: usize, origin: Vec3, p: &Palette) {
    let mut b = B { cmds, p, origin };
    // back wall
    b.put(&p.dark, Vec3::new(-0.5, -SLAB, -1.0), Vec3::new(10.5, 3.0 * TILE_H, -0.9));
    for row in 0..ROWS {
        for col in 0..COLS {
            let t = level.tile(room, col, row);
            tile(&mut b, col as f32, (ROWS - 1 - row) as f32 * TILE_H, t);
        }
    }
}

/// `x`: left edge, `y`: floor surface height of this tile's row.
fn tile(b: &mut B, x: f32, y: f32, t: TileRef) {
    use Tile::*;
    let p = b.p;
    let (stone, dark, wood, metal) = (p.stone.clone(), p.dark.clone(), p.wood.clone(), p.metal.clone());
    let floor = |b: &mut B, m: &Handle<StandardMaterial>| b.put(m, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y, 0.5));
    let column = |b: &mut B, m: &Handle<StandardMaterial>, w: f32, h: f32| {
        b.put(m, Vec3::new(x + 0.5 - w / 2.0, y, -w / 2.0), Vec3::new(x + 0.5 + w / 2.0, y + h, w / 2.0))
    };
    match t.kind {
        Empty => {}
        Wall => b.put(&stone, Vec3::new(x, y - SLAB, -0.5), Vec3::new(x + 1.0, y + TILE_H, 0.5)),
        Floor | Skeleton | Sword | Debris | BalconyLeft | BalconyRight => {
            floor(b, &stone);
            match t.kind {
                Skeleton => b.put(&p.glass, Vec3::new(x + 0.3, y, -0.2), Vec3::new(x + 0.7, y + 0.15, 0.2)),
                Sword => b.put(&metal, Vec3::new(x + 0.2, y, -0.05), Vec3::new(x + 0.8, y + 0.05, 0.05)),
                Debris => b.put(&dark, Vec3::new(x + 0.2, y, -0.2), Vec3::new(x + 0.8, y + 0.15, 0.2)),
                _ => {}
            }
        }
        LooseFloor => floor(b, &wood),
        Spikes => {
            floor(b, &stone);
            for i in 0..4 {
                let sx = x + 0.15 + i as f32 * 0.23;
                b.put(&metal, Vec3::new(sx, y, -0.05), Vec3::new(sx + 0.06, y + 0.45, 0.05));
            }
        }
        Pillar | BigPillarBottom | BigPillarTop | LatticePillar | LatticeSupport => {
            floor(b, &stone);
            column(b, &stone, 0.5, TILE_H);
        }
        DoortopWithFloor | Doortop => {
            floor(b, &stone);
            b.put(&p.red, Vec3::new(x + 0.1, y, -0.9), Vec3::new(x + 0.9, y + TILE_H * 0.9, -0.85));
        }
        SmallLattice | LatticeLeft | LatticeRight => {
            floor(b, &stone);
            b.put(&metal, Vec3::new(x, y, -0.5), Vec3::new(x + 1.0, y + TILE_H * 0.9, -0.45));
        }
        Gate => {
            floor(b, &stone);
            // Data only contains modifiers 0, 1, 2 (mostly 2) and 188; 188 = fully open at runtime.
            // Treating 0 and 188 as open, 1 and 2 as closed is a guess.
            let open = if matches!(t.modifier, 0 | 188) { 1.0 } else { 0.0 };
            for i in 0..5 {
                let sx = x + 0.1 + i as f32 * 0.2;
                b.put(&metal, Vec3::new(sx, y + open * TILE_H * 0.8, -0.05), Vec3::new(sx + 0.05, y + TILE_H * 0.95, 0.05));
            }
        }
        StuckButton | DropButton | RaiseButton => {
            floor(b, &stone);
            let m = if t.kind == RaiseButton { &p.blue } else { &p.red };
            b.put(m, Vec3::new(x + 0.3, y, -0.2), Vec3::new(x + 0.7, y + 0.1, 0.2));
        }
        Potion => {
            floor(b, &stone);
            b.put(&p.red, Vec3::new(x + 0.4, y, -0.1), Vec3::new(x + 0.6, y + 0.3, 0.1));
        }
        Mirror => {
            floor(b, &stone);
            b.put(&p.glass, Vec3::new(x + 0.1, y, -0.4), Vec3::new(x + 0.9, y + TILE_H * 0.9, -0.35));
        }
        ExitLeft | ExitRight => {
            floor(b, &stone);
            b.put(&dark, Vec3::new(x + 0.1, y, -0.5), Vec3::new(x + 0.9, y + TILE_H * 0.85, -0.3));
        }
        Chomper => {
            floor(b, &stone);
            b.put(&metal, Vec3::new(x + 0.1, y, -0.05), Vec3::new(x + 0.9, y + 0.5, 0.05));
        }
        Torch | TorchDebris => {
            floor(b, &stone);
            let top = Vec3::new(x + 0.5, y + TILE_H * 0.65, -0.8);
            b.put(&p.flame, top - Vec3::new(0.06, 0.0, 0.06), top + Vec3::new(0.06, 0.25, 0.06));
            b.cmds.spawn((
                PointLight { color: Color::srgb(1.0, 0.65, 0.3), intensity: 250_000.0, range: 9.0, shadow_maps_enabled: false, ..default() },
                Transform::from_translation(b.origin + top + Vec3::new(0.0, 0.3, 0.6)),
                RoomEntity,
            ));
        }
        Unknown31 => floor(b, &p.blue),
    }
}

/// Despawn and rebuild whenever level or room changed.
pub fn rebuild_on_change(mut cmds: Commands, g: Res<Game>, old: Query<Entity, With<RoomEntity>>, p: Res<Palette>, mut last: Local<(usize, usize)>) {
    if *last == (g.level, g.room) {
        return;
    }
    *last = (g.level, g.room);
    for e in &old {
        cmds.entity(e).despawn();
    }
    build_room(cmds, g, p);
}
