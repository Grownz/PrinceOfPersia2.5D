//! Enhanced-mode air (setting `atmosphere`): slanted light shafts under the palace windows, sparse dust motes near the torches and
//! rising embers over every torch flame. All of it is drawn by one additive, unlit material whose fragment shader (shaders/fx.wesl,
//! compiled into the exe) animates from `globals.time`: a few quads per room, no particles and no per-frame CPU work. The room
//! builder (room.rs) spawns bare `FxSpawn` markers with the room's other entities (so they follow its lifecycle); `sync` gives them
//! mesh and material only while enhanced + atmosphere is on and takes them off again when it is switched off (live, no rebuild).
use crate::settings::{RenderMode, Settings};
use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use pop_core::level::Tile;

/// The sun runs from upper left to lower right in the camera's view (enhanced sun at (-4, 9, 10) looking at the room): the shaft
/// moves this far right per unit it falls (4 / 9, the light's x / y).
pub const SHAFT_SLOPE: f32 = 4.0 / 9.0;
/// Width of a shaft (a window is 1 tile wide) and its offset from the tile's left edge; its depth: just in front of the window
/// pieces (back wall front face -0.9, pieces at -0.88) and behind the torch holders (-0.77) and the figures (0.25).
pub const SHAFT_W: f32 = 0.55;
pub const SHAFT_X0: f32 = 0.12;
pub const SHAFT_Z: f32 = -0.8;
/// Share of a window piece's height where its shaft starts (inside the opening, not on the arch's top edge).
const SHAFT_TOP: f32 = 0.9;
/// Embers rise in a quad this wide above the flame; dust is cut where the nearest torch's glow ends (radius `DUST_PAD`, a bit more
/// than where the shader's gaussian falls under its discard threshold).
pub const EMBER_W: f32 = 0.5;
pub const EMBER_H: f32 = 0.44;
pub const DUST_PAD: f32 = 1.7;
/// Dust and embers stand in front of the back wall and the holders, behind the figures.
pub const DUST_Z: f32 = -0.6;
pub const EMBER_Z: f32 = -0.55;

/// The tile kinds with a window opening on the back wall (balconies, lattice screens): a light shaft falls from each.
pub fn shaft_tile(k: Tile) -> bool {
    use Tile::*;
    matches!(k, BalconyLeft | BalconyRight | LatticePillar | LatticeSupport | SmallLattice | LatticeLeft | LatticeRight)
}

/// A shaft from the window piece standing on `floor` (tile x, floor y) with its opening `open` high: (translation of the shaft
/// mesh's bottom left corner, vertical scale of the unit-height mesh). The shaft lands as far right of its start as the sun slants.
pub fn shaft_place(x: f32, floor: f32, open: f32) -> (Vec3, Vec3) {
    let h = (open * SHAFT_TOP).max(0.3);
    (Vec3::new(x + SHAFT_X0, floor, SHAFT_Z), Vec3::new(1.0, h, 1.0))
}

/// The dust quad of a room: centre and size around its torches' glows; None without torches (nothing to light the motes).
pub fn dust_rect(spots: &[Vec2]) -> Option<(Vec2, Vec2)> {
    let (lo, hi) = spots.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), s| (l.min(*s), h.max(*s)));
    (!spots.is_empty()).then(|| ((lo + hi) / 2.0, hi - lo + Vec2::splat(2.0 * DUST_PAD)))
}

/// The ember quad above a flame at `top` (x centre, y of the cup): centre and size; it ends just under the slab above (the cup is
/// 0.65 of a tile high, the slab's underside 0.095 below the tile's top).
pub fn ember_rect(x: f32, top: f32) -> (Vec2, Vec2) {
    (Vec2::new(x, top + 0.15 + EMBER_H / 2.0), Vec2::new(EMBER_W, EMBER_H))
}

/// Bit `row * 10 + col` set for each full opaque block of the room (`Wall`: the only tile drawn as a whole-tile block; floors are
/// thin slabs, pillars, gates and torches stand free), the dust shader's occluders.
pub fn solid_mask(kinds: impl Iterator<Item = Tile>) -> u32 {
    kinds.enumerate().filter(|(_, k)| *k == Tile::Wall).fold(0, |m, (i, _)| m | 1 << i)
}

/// Pure mirror of the shader's line of sight, in room tile coordinates (x = column, y = row from the top, rows 3 high):
/// 8 samples from `a` to `b`, a solid cell other than the one holding `b` blocks.
pub fn los_clear(mask: u32, a: Vec2, b: Vec2) -> bool {
    let cell = |p: Vec2| (p.x.floor() as i32, p.y.floor() as i32);
    let solid = |c: (i32, i32)| (0..10).contains(&c.0) && (0..3).contains(&c.1) && mask >> (c.1 * 10 + c.0) & 1 != 0;
    (0..8).all(|s| {
        let c = cell(a.lerp(b, (s as f32 + 0.5) / 8.0));
        c == cell(b) || !solid(c)
    })
}

/// What a room's builder asks for.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum FxSpawn {
    Shaft,
    Embers,
    /// up to four torches in world xy (the strength of each is 1; the rest of the array stays 0)
    Dust([Vec2; 4], usize, u32, Vec2),
}

#[derive(Clone, Copy, Default, ShaderType)]
pub struct FxU {
    color: Vec4,
    kind: Vec4,
    torch: [Vec4; 4],
    mask: UVec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct FxMaterial {
    #[uniform(0)]
    u: FxU,
}

impl Material for FxMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(bevy::asset::AssetPath::parse("embedded://pop_app/shaders/fx.wesl"))
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    // lights only the colour: no depth or normal prepass (SSAO) and no shadow views
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

/// The shaft and ember meshes and materials, shared by every room (dust has its own material per room: its torch positions).
#[derive(Resource)]
pub struct FxShared {
    shaft: Handle<Mesh>,
    quad: Handle<Mesh>,
    shaft_mat: Handle<FxMaterial>,
    ember_mat: Handle<FxMaterial>,
}

/// The shaft mesh: a parallelogram of unit height whose bottom is `SHAFT_SLOPE` right of its top; uv x across (0..1), y down (0 at the window).
fn shaft_mesh() -> Mesh {
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 1.0, 0.0], [SHAFT_W, 1.0, 0.0], [SHAFT_SLOPE, 0.0, 0.0], [SHAFT_SLOPE + SHAFT_W, 0.0, 0.0]]);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
    m.insert_indices(Indices::U16(vec![0, 2, 1, 1, 2, 3]));
    m
}

fn uniform(kind: f32, size: Vec2, color: Vec3) -> FxMaterial {
    FxMaterial { u: FxU { color: color.extend(1.0), kind: Vec4::new(kind, size.x, size.y, 0.0), torch: [Vec4::ZERO; 4], mask: UVec4::ZERO } }
}

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<FxMaterial>>) {
    cmds.insert_resource(FxShared {
        shaft: meshes.add(shaft_mesh()),
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        shaft_mat: mats.add(uniform(0.0, Vec2::ZERO, Vec3::ONE)),
        ember_mat: mats.add(uniform(2.0, Vec2::new(EMBER_W, EMBER_H), Vec3::ONE)),
    });
}

/// Enhanced + atmosphere: the effects are on.
fn wanted(mode: &RenderMode, set: &Settings) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere
}

/// Sun colour for the shafts: the sun's own colour (`mood::atmosphere` sets it from the time of day: warm at dusk, blue at night),
/// scaled down with the sun brightness setting.
fn shaft_colour(sun: &DirectionalLight) -> Vec3 {
    let c = sun.color.to_linear();
    Vec3::new(c.red, c.green, c.blue) * (sun.illuminance / 3300.0).clamp(0.0, 1.0).sqrt().max(0.25)
}

/// Every frame, after `mood::atmosphere`: gives the markers their mesh and material while the effects are on (and removes them
/// when they are switched off); keeps the shaft colour at the sun's.
#[allow(clippy::type_complexity)]
pub fn sync(
    mut cmds: Commands,
    mode: Res<RenderMode>,
    set: Res<Settings>,
    shared: Option<Res<FxShared>>,
    mut mats: ResMut<Assets<FxMaterial>>,
    bare: Query<(Entity, &FxSpawn), Without<Mesh3d>>,
    clothed: Query<Entity, (With<FxSpawn>, With<Mesh3d>)>,
    sun: Query<Ref<DirectionalLight>, Without<crate::mirror::TwinOf>>,
    mut on: Local<bool>,
) {
    let Some(shared) = shared else { return };
    let want = wanted(&mode, &set);
    if !want {
        if std::mem::take(&mut *on) {
            for e in &clothed {
                cmds.entity(e).remove::<(Mesh3d, MeshMaterial3d<FxMaterial>)>();
            }
        }
        return;
    }
    let first = !std::mem::replace(&mut *on, true);
    for (e, k) in &bare {
        let (mesh, mat) = match *k {
            FxSpawn::Shaft => (shared.shaft.clone(), shared.shaft_mat.clone()),
            FxSpawn::Embers => (shared.quad.clone(), shared.ember_mat.clone()),
            FxSpawn::Dust(spots, n, mask, origin) => {
                let mut m = uniform(1.0, origin, Vec3::ONE);
                m.u.mask.x = mask;
                for (t, s) in m.u.torch.iter_mut().zip(&spots[..n]) {
                    *t = Vec4::new(s.x, s.y, 1.0, 0.0);
                }
                (shared.quad.clone(), mats.add(m))
            }
        };
        cmds.entity(e).insert((Mesh3d(mesh), MeshMaterial3d(mat)));
    }
    // the sun changes colour only with the time of day (and when its brightness is set)
    if let Some(l) = sun.iter().next().filter(|l| first || l.is_changed()) {
        if let Some(mut m) = mats.get_mut(&shared.shaft_mat) {
            m.u.color = shaft_colour(&l).extend(1.0);
        }
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/fx.wesl");
    app.add_plugins(MaterialPlugin::<FxMaterial>::default()).add_systems(Startup, setup);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_get_shafts_and_floors_do_not() {
        for k in [Tile::BalconyLeft, Tile::BalconyRight, Tile::LatticePillar, Tile::LatticeSupport, Tile::SmallLattice, Tile::LatticeLeft, Tile::LatticeRight] {
            assert!(shaft_tile(k), "{k:?}");
        }
        for k in [Tile::Floor, Tile::Torch, Tile::Wall, Tile::Pillar, Tile::Gate, Tile::Empty] {
            assert!(!shaft_tile(k), "{k:?}");
        }
    }

    #[test]
    fn shaft_slants_with_the_sun_and_stays_behind_the_figures() {
        let (at, scale) = shaft_place(3.0, 2.0, 1.8);
        assert!(at.x > 3.0 && at.y == 2.0 && at.z > -0.9 && at.z < 0.25);
        assert!((scale.y - 1.62).abs() < 1e-5);
        // the mesh's bottom is SHAFT_SLOPE per unit of height right of its top
        let m = shaft_mesh();
        let p = m.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()).unwrap().to_vec();
        assert_eq!((p[0][1], p[2][1], p[2][0] - p[0][0]), (1.0, 0.0, SHAFT_SLOPE));
        assert!(shaft_place(0.0, 0.0, 0.0).1.y >= 0.3);
    }

    #[test]
    fn dust_covers_the_torch_glows_and_needs_a_torch() {
        assert_eq!(dust_rect(&[]), None);
        let (c, s) = dust_rect(&[Vec2::new(2.0, 1.0), Vec2::new(6.0, 3.0)]).unwrap();
        assert_eq!((c, s), (Vec2::new(4.0, 2.0), Vec2::new(4.0 + 2.0 * DUST_PAD, 2.0 + 2.0 * DUST_PAD)));
    }

    #[test]
    fn walls_are_the_dust_occluders() {
        let kinds = (0..30).map(|i| if i == 3 || i == 12 { Tile::Wall } else { Tile::Floor });
        assert_eq!(solid_mask(kinds), 1 << 3 | 1 << 12);
        assert_eq!(solid_mask([Tile::Torch, Tile::Pillar, Tile::Gate, Tile::Empty].into_iter()), 0);
    }

    #[test]
    fn a_wall_between_blocks_the_line_but_not_the_torch_cell() {
        let m = 1 << 5; // column 5, top row
        assert!(!los_clear(m, Vec2::new(3.5, 0.5), Vec2::new(7.5, 0.5)));
        assert!(los_clear(m, Vec2::new(3.5, 1.5), Vec2::new(7.5, 1.5)));
        assert!(los_clear(m, Vec2::new(3.5, 0.5), Vec2::new(5.5, 0.5)), "the target's own cell never blocks");
        assert!(los_clear(0, Vec2::new(0.5, 0.5), Vec2::new(9.5, 2.5)));
    }

    #[test]
    fn embers_rise_from_the_flame_to_the_ceiling() {
        let (c, s) = ember_rect(5.5, 1.3);
        assert!((c.y - s.y / 2.0 - 1.45).abs() < 1e-5 && c.x == 5.5);
        // under the slab above the torch's tile (its underside 2 - 0.095 high)
        assert!(c.y + s.y / 2.0 < 2.0 - 0.095);
    }

    #[test]
    fn shaft_colour_follows_the_sun() {
        let night = DirectionalLight { color: Color::srgb(0.55, 0.68, 1.0), illuminance: 3300.0, ..default() };
        let dusk = DirectionalLight { color: Color::srgb(1.0, 0.88, 0.74), illuminance: 3300.0, ..default() };
        let (n, d) = (shaft_colour(&night), shaft_colour(&dusk));
        assert!(n.z / n.x > d.z / d.x);
        let dim = DirectionalLight { illuminance: 300.0, ..dusk };
        assert!(shaft_colour(&dim).x < d.x && shaft_colour(&dim).x > 0.2);
    }
}
