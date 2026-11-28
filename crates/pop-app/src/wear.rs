//! Enhanced-mode weathering (rides on the setting `atmosphere`): the same few tile images stop repeating. A room gets a handful of
//! quads laid a hair in front of its wall-block faces, over its floor tops and in front of its back wall, with one unlit multiply
//! material whose fragment shader (shaders/wear.wesl, compiled into the exe) jitters the tone of every tile and ages the stone: moss
//! and damp from the floor line, cobwebs in the corners (dungeon), grime, streaks, hairline cracks, soot over the torches. Plus a few
//! shallow puddles on the dungeon floors. No per-frame CPU work, no shadows, no prepass. The room builder (room.rs) spawns bare
//! `Decal` / `Puddle` markers with the room's other entities (so they follow its lifecycle); `sync` gives them mesh and material only
//! while enhanced + atmosphere is on and takes them off again when it is switched off (live, no rebuild).
use crate::room::{SLAB, SLAB_FRONT};
use crate::settings::{RenderMode, Settings};
use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use pop_core::level::{Tile, COLS, ROWS};

/// How far in front of the surface a decal lies (the wall marks are 0.004 thick, the back wall's front face is at -0.9).
const WALL_LIFT: f32 = 0.007;
const FLOOR_LIFT: f32 = 0.004;
const BACK_LIFT: f32 = 0.003;
const BACK_FRONT: f32 = -0.9;
/// A puddle lies this far above the floor decal.
const PUDDLE_LIFT: f32 = 0.007;

/// Everything of a room the decal shader needs, so equal rooms look equal: `surface` 0 wall faces, 1 floor tops, 2 back wall.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Decal {
    surface: u8,
    palace: bool,
    seed: u32,
    /// bit `row * 10 + col`: wall blocks / floors that get a decal
    walls: u32,
    floors: u32,
    torch: [Vec2; 4],
    torches: usize,
    origin: Vec2,
}

/// A shallow puddle on a dungeon floor (a bare marker; its transform places and sizes it).
#[derive(Component)]
pub struct Puddle;

/// Seed of a room: the level and the room number, so a room looks the same on every visit.
pub fn seed(level: usize, room: usize) -> u32 {
    level as u32 * 25 + room as u32
}

/// A well mixed value in 0..1 from three numbers (integer hash, no state).
pub fn unit(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Bit `row * 10 + col` set for each tile of `kinds` (30 tiles, row-major from the top) that satisfies `f`.
pub fn mask(kinds: &[Tile], f: impl Fn(Tile) -> bool) -> u32 {
    kinds.iter().enumerate().filter(|(_, k)| f(**k)).fold(0, |m, (i, _)| m | 1 << i)
}

/// Floors whose top is a plain slab in view: they get the floor decal (and are where the palace's reflections may show).
pub fn plain_floor(k: Tile) -> bool {
    matches!(k, Tile::Floor | Tile::Torch | Tile::TorchDebris)
}

/// The decal quads of a room: (marker, centre in room coordinates, size, lying flat). The wall faces and the floors only when the room
/// has some; every room has a back wall.
pub fn quads(seed: u32, palace: bool, kinds: &[Tile], spots: &[Vec2], origin: Vec2) -> Vec<(Decal, Vec3, Vec2, bool)> {
    let mut t = [Vec2::ZERO; 4];
    let n = spots.len().min(4);
    t[..n].copy_from_slice(&spots[..n]);
    let d = |surface, walls, floors| Decal { surface, palace, seed, walls, floors, torch: t, torches: n, origin };
    let (walls, floors) = (mask(kinds, |k| k == Tile::Wall), mask(kinds, plain_floor));
    let mid = COLS as f32 / 2.0;
    let mut v = vec![];
    if walls != 0 {
        v.push((d(0, walls, 0), Vec3::new(mid, 3.0 * crate::room::TILE_H / 2.0 - SLAB, SLAB_FRONT + WALL_LIFT), Vec2::new(COLS as f32, ROWS as f32 * crate::room::TILE_H), false));
    }
    for row in 0..ROWS {
        if floors >> (row * COLS) & ((1 << COLS) - 1) != 0 {
            let y = (ROWS - 1 - row) as f32 * crate::room::TILE_H;
            v.push((d(1, 0, floors), Vec3::new(mid, y + FLOOR_LIFT, (BACK_FRONT + SLAB_FRONT) / 2.0), Vec2::new(COLS as f32, SLAB_FRONT - BACK_FRONT), true));
        }
    }
    v.push((d(2, walls, 0), Vec3::new(mid, 3.0 * crate::room::TILE_H / 2.0 - SLAB, BACK_FRONT + BACK_LIFT), Vec2::new(COLS as f32, ROWS as f32 * crate::room::TILE_H), false));
    v
}

/// Tiles a puddle keeps away from (on the tile and in the columns next to it): doors, exits, traps, plates and loose floors.
fn keep_clear(k: Tile) -> bool {
    use Tile::*;
    matches!(k, Gate | ExitLeft | ExitRight | Spikes | LooseFloor | Chomper | DropButton | RaiseButton | StuckButton | Doortop | DoortopWithFloor)
}

/// The puddles of a dungeon room: 0 to 3 shallow ellipses on plain floor tiles that have no door, exit, trap or loose floor on them or
/// next to them: (centre in room coordinates, radii x and z). Deterministic from the seed.
pub fn puddles(seed: u32, kinds: &[Tile]) -> Vec<(Vec3, Vec2)> {
    let ok: Vec<usize> = (0..kinds.len()).filter(|&i| kinds[i] == Tile::Floor && [-1, 1].iter().all(|d| (i % COLS) as i32 + d < 0 || (i % COLS) as i32 + d >= COLS as i32 || !keep_clear(kinds[(i as i32 + d) as usize]))).collect();
    let u = unit(seed, 0, 101);
    let n = if u < 0.3 { 0 } else if u < 0.65 { 1 } else if u < 0.9 { 2 } else { 3 };
    let mut free = ok;
    let mut out = vec![];
    for k in 0..n {
        if free.is_empty() {
            break;
        }
        let i = free.remove((unit(seed, k, 102) * free.len() as f32) as usize % free.len());
        let (col, row) = (i % COLS, i / COLS);
        let r = Vec2::new(0.28 + 0.18 * unit(seed, k, 103), 0.2 + 0.14 * unit(seed, k, 104));
        let x = col as f32 + r.x + 0.04 + (1.0 - 2.0 * r.x - 0.08) * unit(seed, k, 105);
        let z = -0.5 + 0.6 * unit(seed, k, 106);
        out.push((Vec3::new(x, (ROWS - 1 - row) as f32 * crate::room::TILE_H + FLOOR_LIFT + PUDDLE_LIFT, z), r));
    }
    out
}

#[derive(Clone, Copy, Default, ShaderType)]
pub struct WearU {
    kind: Vec4,
    org: Vec4,
    mask: UVec4,
    torch: [Vec4; 4],
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WearMaterial {
    #[uniform(0)]
    u: WearU,
}

impl Material for WearMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(bevy::asset::AssetPath::parse("embedded://pop_app/shaders/wear.wesl"))
    }
    // out = rgb * what is there: the shader's output is the factor the lit scene gets multiplied with
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

fn material(d: &Decal) -> WearMaterial {
    let mut torch = [Vec4::ZERO; 4];
    for (t, s) in torch.iter_mut().zip(&d.torch[..d.torches]) {
        *t = Vec4::new(s.x, s.y, 1.0, 0.0);
    }
    WearMaterial { u: WearU { kind: Vec4::new(d.surface as f32, d.palace as u32 as f32, d.seed as f32, 0.0), org: d.origin.extend(0.0).extend(0.0), mask: UVec4::new(d.walls, d.floors, 0, 0), torch } }
}

/// The unit quad and the puddle's mesh, material and soft-edged texture, shared by every room.
#[derive(Resource)]
pub struct WearShared {
    quad: Handle<Mesh>,
    puddle: Handle<StandardMaterial>,
}

/// Alpha of a puddle: a soft, slightly lobed ellipse (the quad's uv, centre 0.5).
fn puddle_image() -> Image {
    const N: u32 = 32;
    let mut px = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = ((x as f32 + 0.5) / N as f32 * 2.0 - 1.0, (y as f32 + 0.5) / N as f32 * 2.0 - 1.0);
            let ang = dy.atan2(dx);
            let r = (dx * dx + dy * dy).sqrt() / (1.0 + 0.1 * (3.0 * ang + 1.0).sin() + 0.07 * (5.0 * ang + 2.0).sin());
            let a = 1.0 - ((r - 0.45) / 0.5).clamp(0.0, 1.0);
            px.extend([255, 255, 255, (a * a * (3.0 - 2.0 * a) * 255.0) as u8]);
        }
    }
    Image::new(Extent3d { width: N, height: N, depth_or_array_layers: 1 }, TextureDimension::D2, px, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)
}

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut imgs: ResMut<Assets<Image>>) {
    cmds.insert_resource(WearShared {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        // thin dark wet film that mirrors the torches and the environment map: smooth, reflective, see-through
        puddle: mats.add(StandardMaterial { base_color: Color::srgba(0.015, 0.02, 0.03, 0.55), base_color_texture: Some(imgs.add(puddle_image())), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 0.6, ..default() }),
    });
}

/// Enhanced + atmosphere: the weathering is on.
fn wanted(mode: &RenderMode, set: &Settings) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere
}

/// Every frame, after `mood::atmosphere`: gives the markers their mesh and material while the weathering is on, takes them off when
/// it is switched off.
#[allow(clippy::type_complexity)]
pub fn sync(
    mut cmds: Commands,
    mode: Res<RenderMode>,
    set: Res<Settings>,
    shared: Option<Res<WearShared>>,
    mut mats: ResMut<Assets<WearMaterial>>,
    decals: Query<(Entity, &Decal), Without<Mesh3d>>,
    puddles: Query<Entity, (With<Puddle>, Without<Mesh3d>)>,
    clothed: Query<(Entity, Has<Decal>), (Or<(With<Decal>, With<Puddle>)>, With<Mesh3d>)>,
    mut on: Local<bool>,
) {
    let Some(shared) = shared else { return };
    if !wanted(&mode, &set) {
        if std::mem::take(&mut *on) {
            for (e, decal) in &clothed {
                let mut c = cmds.entity(e);
                c.remove::<Mesh3d>();
                if decal {
                    c.remove::<MeshMaterial3d<WearMaterial>>();
                } else {
                    c.remove::<MeshMaterial3d<StandardMaterial>>();
                }
            }
        }
        return;
    }
    *on = true;
    for (e, d) in &decals {
        cmds.entity(e).insert((Mesh3d(shared.quad.clone()), MeshMaterial3d(mats.add(material(d)))));
    }
    for e in &puddles {
        cmds.entity(e).insert((Mesh3d(shared.quad.clone()), MeshMaterial3d(shared.puddle.clone())));
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/wear.wesl");
    app.add_plugins(MaterialPlugin::<WearMaterial>::default()).add_systems(Startup, setup);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(f: impl Fn(usize) -> Tile) -> Vec<Tile> {
        (0..COLS * ROWS).map(f).collect()
    }

    #[test]
    fn the_same_room_hashes_the_same_and_rooms_differ() {
        assert_eq!(unit(7, 3, 9), unit(7, 3, 9));
        assert_ne!(unit(7, 3, 9), unit(7, 4, 9));
        assert_eq!(seed(3, 5), seed(3, 5));
        assert_ne!(seed(3, 5), seed(5, 3));
        let u: Vec<f32> = (0..1000).map(|i| unit(i, 1, 2)).collect();
        assert!(u.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = u.iter().sum::<f32>() / 1000.0;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
    }

    #[test]
    fn masks_mark_the_right_cells() {
        let k = room(|i| match i { 3 => Tile::Wall, 12 => Tile::Floor, 25 => Tile::Torch, 26 => Tile::Spikes, _ => Tile::Empty });
        assert_eq!(mask(&k, |t| t == Tile::Wall), 1 << 3);
        assert_eq!(mask(&k, plain_floor), 1 << 12 | 1 << 25);
    }

    #[test]
    fn a_room_gets_its_decals_just_in_front_of_its_surfaces() {
        let k = room(|i| match i { 3 => Tile::Wall, 12 => Tile::Floor, 25 => Tile::Floor, _ => Tile::Empty });
        let q = quads(1, false, &k, &[], Vec2::ZERO);
        // wall faces, two floor rows (rows 1 and 2) and the back wall
        assert_eq!(q.iter().map(|(d, ..)| d.surface).collect::<Vec<_>>(), vec![0, 1, 1, 2]);
        let wall = q[0].1.z;
        assert!(wall > SLAB_FRONT + 0.004 && wall < SLAB_FRONT + 0.05, "past the wall marks, not in the kid's way");
        let back = q[3].1.z;
        assert!(back > BACK_FRONT && back < BACK_FRONT + 0.05);
        // floor decals lie at the slab top of their row, over its whole depth
        let ys: Vec<f32> = q.iter().filter(|(d, ..)| d.surface == 1).map(|(_, c, ..)| c.y).collect();
        assert_eq!(ys, vec![2.0 + FLOOR_LIFT, FLOOR_LIFT]);
        assert!(q[1].3 && !q[0].3 && q[1].2.y == SLAB_FRONT - BACK_FRONT);
        // a room without walls or floors keeps only the back wall
        assert_eq!(quads(1, true, &room(|_| Tile::Empty), &[], Vec2::ZERO).len(), 1);
        // torches are passed on (at most four)
        let d = quads(1, true, &k, &[Vec2::ONE; 6], Vec2::ZERO)[0].0;
        assert_eq!((d.torches, d.palace), (4, true));
    }

    #[test]
    fn puddles_are_deterministic_few_and_keep_clear_of_traps_and_doors() {
        let k = room(|i| match i % COLS { 4 => Tile::Spikes, 7 => Tile::Gate, 0 => Tile::LooseFloor, _ => Tile::Floor });
        let mut counts = [0; 4];
        for s in 0..200 {
            let p = puddles(s, &k);
            assert_eq!(p, puddles(s, &k));
            counts[p.len()] += 1;
            for (c, r) in &p {
                let col = c.x.floor() as usize;
                // on a floor tile whose neighbours are none of the forbidden kinds: only columns 2 and 9 qualify
                assert!(matches!(col, 2 | 9), "column {col}");
                assert!(c.x - r.x >= col as f32 && c.x + r.x <= col as f32 + 1.0, "inside its tile");
                assert!(c.z - r.y > -0.95 && c.z + r.y < 0.5);
            }
        }
        // 0..=3 per room, every count occurs
        assert!(counts.iter().all(|&n| n > 0), "{counts:?}");
        // nothing to put them on
        assert!(puddles(1, &room(|_| Tile::Empty)).is_empty());
        assert!(puddles(1, &room(|_| Tile::Spikes)).is_empty());
    }

    #[test]
    fn puddle_texture_is_soft_edged() {
        let im = puddle_image();
        let d = im.data.unwrap();
        let a = |x: usize, y: usize| d[(y * 32 + x) * 4 + 3];
        assert_eq!(a(16, 16), 255);
        assert_eq!(a(0, 0), 0);
        assert!(a(16, 25) > 0 && a(16, 25) < 255);
    }
}
