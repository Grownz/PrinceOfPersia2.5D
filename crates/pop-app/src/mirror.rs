//! The polished palace back wall as a true mirror (rides on the setting `atmosphere`, enhanced mode, palace levels). A planar mirror
//! shows the world reflected through its plane, so that is what is drawn: every mesh in front of the wall (the room pieces, the loose
//! floors and falling slabs, the debris, the rubble piles and blood stains, the flames, the figures and their swords) gets a twin entity
//! with the original's mesh and the reflected transform (`reflect`: z -> 2 * wall - z, scale z = -1), on render layer 1 only. A second
//! 3D camera (`MirrorCam`, a child of the level camera, so the very same view for any pan, fight zoom or shake) sees only that layer
//! and draws the twins with their own depth buffer into a half-size HDR image, lit by twins of the lights (torches, potions, sparks,
//! the sun: reflected the same way, without shadows) and the level camera's ambient light, so the mirror world is lit as the room
//! is, seen from the other side. A thin quad over each room's back wall (`Glass`,
//! `MirrorWall`, shaders/mirror.wesl) adds that image at the same screen position, faint and a little warm: it is drawn after the
//! opaque pass with the depth test, so it shows only where the back wall itself is visible and everything standing in front of the
//! wall hides it exactly. No per-frame CPU work for the still pieces: the twins are made for meshes without one (a room build, a
//! rebuilt gate, a new falling slab), they follow their original's transform, mesh, material and visibility on change only, and
//! despawn with it (`TwinOf` is a linked relationship). Twins carry no `RoomEntity`.
use crate::room::{RoomEntity, BACK_WALL, SLAB, TILE_H};
use crate::settings::{RenderMode, Settings};
use crate::traces::{StainEnt, TraceMaterial};
use bevy::asset::embedded_asset;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Hdr, RenderTarget};
use bevy::light::NotShadowCaster;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, Face, ShaderType, TextureFormat};
use bevy::render::view::{DebandDither, Msaa, Tonemapping};
use bevy::shader::ShaderRef;
use std::collections::HashMap;

/// z of the wall's front face: the mirror plane.
pub const WALL: f32 = BACK_WALL.1;
/// Strength of the mirror image and its warm marble tint (rgb factors of the added image).
const STRENGTH: f32 = 0.2;
const TINT: Vec3 = Vec3::new(1.0, 0.92, 0.8);
/// The render layer of the twins and the mirror camera; the image is this many times smaller than the view (a slight blur).
const LAYER: usize = 1;
const DIV: u32 = 2;
/// The glass quad lies this far in front of the wall face (behind the decor pieces at -0.88).
const GLASS_LIFT: f32 = 0.001;

/// The transform of `t`'s mirror image through the wall plane z = WALL: translation z -> 2 * WALL - z, the rotation conjugated by
/// the reflection (quaternion x, y negated) and scale z negated, so that its matrix is the reflection times `t`'s matrix.
pub fn reflect(t: &Transform) -> Transform {
    let (p, r, s) = (t.translation, t.rotation, t.scale);
    Transform { translation: Vec3::new(p.x, p.y, 2.0 * WALL - p.z), rotation: Quat::from_xyzw(-r.x, -r.y, r.z, r.w), scale: Vec3::new(s.x, s.y, -s.z) }
}

/// The glass quad (a unit rectangle) over the back wall box `wall` of a room: the room's 10 columns and 3 rows exactly (the wall box
/// reaches half a column into each neighbour and down under the floor: the neighbours' glass would overlap and add twice).
pub fn glass(wall: &Transform) -> Transform {
    let lo = wall.translation - wall.scale / 2.0;
    let origin = Vec2::new(lo.x + 0.5, lo.y + SLAB);
    let size = Vec2::new(pop_core::level::COLS as f32, pop_core::level::ROWS as f32 * TILE_H);
    Transform::from_translation((origin + size / 2.0).extend(WALL + GLASS_LIFT)).with_scale(size.extend(1.0))
}

/// The twin material of a room or figure material: the same colour, texture, cut-out and surface, the culled side swapped (the
/// reflection reverses the winding of every face; Bevy reflects the normals with the transform), not metallic (the mirror view has
/// no environment map, steel would go black); `unlit` for POP_MIRROR=truth, whose twins are seen by the level camera with the
/// real lights.
pub fn twin_material(m: &StandardMaterial, unlit: bool) -> StandardMaterial {
    let cull_mode = m.cull_mode.map(|f| if f == Face::Back { Face::Front } else { Face::Back });
    StandardMaterial { unlit: unlit || m.unlit, cull_mode, metallic: 0.0, ..m.clone() }
}

/// The light of the mirror world for a light at `t`: a point light at the reflected place, a directional light shining the
/// reflected way (a light's transform keeps scale one: its forward axis is what counts).
pub fn reflect_light(t: &Transform) -> Transform {
    let p = reflect(t).translation;
    let d = t.forward().as_vec3();
    Transform::from_translation(p).looking_to(Vec3::new(d.x, d.y, -d.z), Vec3::Y)
}

#[derive(Clone, Copy, Default, ShaderType)]
struct WallU {
    tint: Vec4,
}

/// The glass over the back wall: adds the mirror camera's image at the fragment's screen position.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct MirrorWall {
    #[uniform(0)]
    u: WallU,
    #[texture(1)]
    #[sampler(2)]
    image: Handle<Image>,
}

impl Material for MirrorWall {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(bevy::asset::AssetPath::parse("embedded://pop_app/shaders/mirror.wesl"))
    }
    // premultiplied blending with alpha 0: out = image * tint + what is there (the wall keeps its own lit look)
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

/// The camera that draws the twins.
#[derive(Component)]
pub struct MirrorCam;

/// The glass quad of a back wall (a twin of the wall entity that does not follow its transform).
#[derive(Component)]
pub struct Glass;

/// A mirror twin of `0`, despawned with it.
#[derive(Component)]
#[relationship(relationship_target = Twins)]
pub struct TwinOf(Entity);

/// The twins of an entity (one).
#[derive(Component)]
#[relationship_target(relationship = TwinOf, linked_spawn)]
pub struct Twins(Vec<Entity>);

/// `POP_MIRROR` (debugging, docs/DEVELOPING.md): `truth` = the twins also on the level camera's layer and the back walls hidden, so the
/// mirror world is seen directly through the open wall (the ground truth the glass must match); `off` = no mirror; a number = the
/// strength instead of STRENGTH.
fn env() -> &'static str {
    static S: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    S.get_or_init(|| std::env::var("POP_MIRROR").unwrap_or_default())
}

/// Enhanced + atmosphere in a palace level: the mirror is on (not on the title: `level` is the last game's there).
pub fn wanted(mode: &RenderMode, set: &Settings, level: usize, screen: crate::flow::Screen) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere && level != 0 && screen != crate::flow::Screen::Title && crate::kid::palace(level) && env() != "off"
}

#[derive(Default)]
pub struct State {
    on: bool,
    image: Handle<Image>,
    glass: Option<(Handle<Mesh>, Handle<MirrorWall>)>,
    mats: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}

type Figure = Or<(With<RoomEntity>, With<crate::kid::KidSprite>, With<crate::kid::GuardSprite>, With<crate::kid::SwordSprite>)>;
type New<'a> = (Entity, &'a Transform, &'a Mesh3d, &'a MeshMaterial3d<StandardMaterial>, &'a Visibility);
type NewTrace<'a> = (Entity, &'a Transform, &'a Mesh3d, &'a MeshMaterial3d<TraceMaterial>, &'a Visibility);
type Moved<'a> = (&'a Twins, &'a Transform, &'a Mesh3d, Option<&'a MeshMaterial3d<StandardMaterial>>, &'a Visibility);
type MovedFilter = Or<(Changed<Transform>, Changed<Mesh3d>, Changed<MeshMaterial3d<StandardMaterial>>, Changed<Visibility>)>;
type TwinItem<'a> = (&'a mut Transform, &'a mut Mesh3d, Option<&'a mut MeshMaterial3d<StandardMaterial>>, &'a mut Visibility);

/// Every frame, after everything that moves, swaps or builds meshes: the mirror camera and its image (sized to the view), twins for
/// the meshes that have none, the twins of changed originals updated; all of it removed while the mirror is off.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync(
    mut cmds: Commands,
    (mode, screen): (Res<RenderMode>, Res<bevy::prelude::State<crate::flow::Screen>>), // one param: a system takes at most 16
    set: Res<Settings>,
    g: Res<crate::Game>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut wall_mats: ResMut<Assets<MirrorWall>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    main: Query<(Entity, &Camera), (With<Camera3d>, Without<MirrorCam>)>,
    cams: Query<Entity, With<MirrorCam>>,
    new: Query<New, (Figure, Without<Twins>, Without<TwinOf>)>,
    new_traces: Query<NewTrace, (Or<(With<RoomEntity>, With<StainEnt>)>, Without<Twins>, Without<TwinOf>)>,
    moved: Query<Moved, MovedFilter>,
    mut twins: Query<TwinItem, (With<TwinOf>, Without<Twins>, Without<Glass>)>,
    all: Query<Entity, With<TwinOf>>,
    mut st: Local<State>,
) {
    if !wanted(&mode, &set, g.level, *screen.get()) {
        if std::mem::take(&mut st.on) {
            all.iter().chain(cams.iter()).for_each(|e| cmds.entity(e).despawn());
            images.remove(&st.image);
            *st = State::default();
        }
        return;
    }
    let Ok((main_e, main_cam)) = main.single() else { return };
    let size = (main_cam.physical_viewport_size().unwrap_or(UVec2::ONE) / DIV).max(UVec2::ONE);
    if !std::mem::replace(&mut st.on, true) {
        st.image = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None));
        cmds.spawn((
            MirrorCam,
            Camera3d::default(),
            Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            RenderTarget::Image(st.image.clone().into()),
            (Hdr, Msaa::Off, Tonemapping::None, DebandDither::Disabled),
            RenderLayers::layer(LAYER),
            Transform::IDENTITY,
            ChildOf(main_e),
        ));
        let k = env().parse::<f32>().unwrap_or(STRENGTH);
        let m = wall_mats.add(MirrorWall { u: WallU { tint: (TINT * k).extend(0.0) }, image: st.image.clone() });
        st.glass = Some((meshes.add(Rectangle::new(1.0, 1.0)), m));
    } else if images.get(&st.image).is_some_and(|i| i.size() != size) {
        // the window (or the upside-down picture's image) was resized: the image follows (get first, so it is not marked changed)
        if let Some(mut i) = images.get_mut(&st.image) {
            i.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
        }
    }
    let truth = env() == "truth";
    let layers = if truth { RenderLayers::from_layers(&[0, LAYER]) } else { RenderLayers::layer(LAYER) };
    let Some((quad, glass_mat)) = st.glass.clone() else { return };
    for (e, t, mesh, mat, vis) in &new {
        // the back wall (wider than a room) is the mirror itself: it gets the glass
        if t.scale.x > 5.0 {
            cmds.spawn((TwinOf(e), Glass, glass(t), Mesh3d(quad.clone()), MeshMaterial3d(glass_mat.clone()), if truth { Visibility::Hidden } else { Visibility::Inherited }, NotShadowCaster));
            if truth {
                cmds.entity(e).insert(Visibility::Hidden);
            }
            continue;
        }
        let Some(m) = std_mats.get(&mat.0) else { continue };
        let m = twin_material(m, truth);
        let h = st.mats.entry(mat.0.id()).or_insert_with(|| std_mats.add(m)).clone();
        cmds.spawn((TwinOf(e), reflect(t), Mesh3d(mesh.0.clone()), MeshMaterial3d(h), *vis, layers.clone(), NotShadowCaster));
    }
    // the rubble piles and blood stains: multiply decals, the same material darkens the mirror image of the floor (it draws both sides)
    for (e, t, mesh, mat, vis) in &new_traces {
        cmds.spawn((TwinOf(e), reflect(t), Mesh3d(mesh.0.clone()), MeshMaterial3d(mat.0.clone()), *vis, layers.clone(), NotShadowCaster));
    }
    for (tw, t, mesh, mat, vis) in &moved {
        for &e in &tw.0 {
            let Ok((mut tt, mut tm, tmat, mut tv)) = twins.get_mut(e) else { continue };
            tt.set_if_neq(reflect(t));
            if tm.0 != mesh.0 {
                tm.0 = mesh.0.clone();
            }
            tv.set_if_neq(*vis);
            if let (Some(mut tmat), Some(mat)) = (tmat, mat) {
                let Some(m) = std_mats.get(&mat.0).map(|m| twin_material(m, truth)) else { continue };
                let h = st.mats.entry(mat.0.id()).or_insert_with(|| std_mats.add(m)).clone();
                if tmat.0 != h {
                    tmat.0 = h;
                }
            }
        }
    }
}

type LightSrc<'a> = (Entity, &'a Transform, Option<&'a PointLight>, Option<&'a DirectionalLight>);
type LightMoved<'a> = (&'a Twins, &'a Transform, Option<&'a PointLight>, Option<&'a DirectionalLight>);
type LightTwin<'a> = (&'a mut Transform, Option<&'a mut PointLight>, Option<&'a mut DirectionalLight>);
type AnyLight = Or<(With<PointLight>, With<DirectionalLight>)>;

/// Every frame after `sync`, while the mirror camera exists: twins for the lights that have none, the twins of changed lights
/// updated (the torches flicker and move every frame), the level camera's ambient light copied. No shadows in the mirror world:
/// a shadowed light costs shadow views on the CPU.
#[allow(clippy::type_complexity)]
pub fn lights(
    mut cmds: Commands,
    main: Query<Ref<AmbientLight>, (With<Camera3d>, Without<MirrorCam>)>,
    cam: Query<(Entity, Has<AmbientLight>), With<MirrorCam>>,
    new: Query<LightSrc, (AnyLight, Without<Twins>, Without<TwinOf>)>,
    moved: Query<LightMoved, (AnyLight, Or<(Changed<Transform>, Changed<PointLight>, Changed<DirectionalLight>)>)>,
    mut twins: Query<LightTwin, (AnyLight, With<TwinOf>, Without<Twins>)>,
) {
    let Ok((cam_e, has_amb)) = cam.single() else { return };
    if let Ok(a) = main.single() {
        if a.is_changed() || !has_amb {
            cmds.entity(cam_e).insert(a.into_inner().clone());
        }
    }
    let point = |l: &PointLight| PointLight { shadow_maps_enabled: false, ..l.clone() };
    let sun = |l: &DirectionalLight| DirectionalLight { shadow_maps_enabled: false, ..l.clone() };
    for (e, t, p, d) in &new {
        let mut c = cmds.spawn((TwinOf(e), reflect_light(t), RenderLayers::layer(LAYER)));
        if let Some(p) = p {
            c.insert(point(p));
        } else if let Some(d) = d {
            c.insert(sun(d));
        }
    }
    for (tw, t, p, d) in &moved {
        for &e in &tw.0 {
            let Ok((mut tt, tp, td)) = twins.get_mut(e) else { continue };
            tt.set_if_neq(reflect_light(t));
            if let (Some(mut tp), Some(p)) = (tp, p) {
                *tp = point(p);
            }
            if let (Some(mut td), Some(d)) = (td, d) {
                *td = sun(d);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/mirror.wesl");
    app.add_plugins(MaterialPlugin::<MirrorWall>::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_mirror_on_the_title() {
        use crate::flow::Screen;
        let set = Settings { atmosphere: true, ..default() };
        let (palace, enh) = ((1..=14).find(|&l| crate::kid::palace(l)).unwrap(), RenderMode::Enhanced);
        assert!(wanted(&enh, &set, palace, Screen::Playing) && !wanted(&enh, &set, palace, Screen::Title));
    }

    /// The reflection through the wall plane as a matrix: z -> 2 * WALL - z.
    fn mirror_matrix() -> Mat4 {
        Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0 * WALL)) * Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0))
    }

    #[test]
    fn the_twin_transform_is_the_reflection_of_the_original() {
        let ts = [
            Transform::from_xyz(3.0, 2.0, 0.25).with_scale(Vec3::new(1.0, 0.1, 1.45)),
            Transform::from_xyz(4.5, 1.9, 0.0).with_rotation(Quat::from_rotation_z(0.08)).with_scale(Vec3::new(1.0, 0.095, 1.45)),
            Transform::from_xyz(2.0, 4.0, 0.5).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(1.0 / 32.0, 0.1, 0.02)),
            Transform::from_xyz(-1.0, 0.3, -0.4).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, -1.1, 0.7)).with_scale(Vec3::new(0.5, 2.0, 0.25)),
        ];
        for t in ts {
            let want = mirror_matrix() * t.to_matrix();
            let got = reflect(&t).to_matrix();
            assert!(want.abs_diff_eq(got, 1e-5), "{t:?}\n{want}\n{got}");
            // a point of the mesh lands where a mirror puts it: same x and y, as far behind the wall as it was in front
            let p = t.transform_point(Vec3::new(0.3, -0.2, 0.4));
            let q = reflect(&t).transform_point(Vec3::new(0.3, -0.2, 0.4));
            assert!((q - Vec3::new(p.x, p.y, 2.0 * WALL - p.z)).length() < 1e-5);
            assert!(reflect(&t).to_matrix().determinant() < 0.0, "a reflection reverses the winding");
        }
    }

    /// Where a point's mirror image appears to the eye: the ray from the eye to the reflected point crosses the wall plane where the
    /// law of reflection (equal angles, in the plane of incidence) puts the bounce. Checks the setup against optics, not against itself.
    #[test]
    fn the_reflected_point_is_seen_where_the_light_bounces_off_the_wall() {
        let eye = Vec3::new(5.0, 4.2, 14.0);
        for p in [Vec3::new(2.0, 0.0, 0.5), Vec3::new(8.5, 4.0, -0.2), Vec3::new(5.0, 1.0, 0.1)] {
            let q = reflect(&Transform::from_translation(p)).translation;
            let k = (WALL - eye.z) / (q.z - eye.z);
            let hit = eye + (q - eye) * k;
            assert!((hit.z - WALL).abs() < 1e-5);
            // incoming (p -> hit) and outgoing (hit -> eye): the part along the wall normal (z) flips, the parts along the wall stay
            let (a, b) = ((hit - p).normalize(), (eye - hit).normalize());
            assert!((a.z + b.z).abs() < 1e-4 && (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4, "{a} {b}");
        }
    }

    #[test]
    fn the_glass_covers_exactly_the_room_on_the_wall_face() {
        // the back wall box of the room at origin (10, 6): x -0.5..10.5, y -SLAB..6, z -1..-0.9 (room.rs `back_wall`)
        let lo = Vec3::new(10.0 - 0.5, 6.0 - SLAB, BACK_WALL.0);
        let hi = Vec3::new(10.0 + 10.5, 6.0 + 3.0 * TILE_H, BACK_WALL.1);
        let g = glass(&Transform::from_translation((lo + hi) / 2.0).with_scale(hi - lo));
        let (a, b) = (g.translation - g.scale / 2.0, g.translation + g.scale / 2.0);
        assert!((a.truncate() - Vec2::new(10.0, 6.0)).length() < 1e-5 && (b.truncate() - Vec2::new(20.0, 12.0)).length() < 1e-5, "{a} {b}");
        assert!(g.translation.z > WALL && g.translation.z < -0.88, "in front of the wall face, behind the decor pieces");
    }

    #[test]
    fn twins_keep_colour_texture_and_cut_out_unlit_with_the_other_side_culled() {
        let m = StandardMaterial { base_color: Color::srgb(1.0, 0.5, 0.25), alpha_mode: AlphaMode::Mask(0.4), ..default() };
        let t = twin_material(&m, false);
        assert!(!t.unlit && t.alpha_mode == AlphaMode::Mask(0.4) && t.base_color == m.base_color && t.metallic == 0.0);
        assert_eq!(t.cull_mode, Some(Face::Front));
        assert_eq!(twin_material(&StandardMaterial { cull_mode: None, ..default() }, false).cull_mode, None);
        assert!(twin_material(&m, true).unlit && twin_material(&StandardMaterial { unlit: true, ..default() }, false).unlit, "flames stay unlit");
    }

    #[test]
    fn a_light_twin_sits_at_the_reflected_place_and_shines_the_reflected_way() {
        let sun = Transform::from_xyz(-4.0, 9.0, 10.0).looking_at(Vec3::new(5.0, 3.0, 0.0), Vec3::Y);
        let t = reflect_light(&sun);
        let (d, d2) = (sun.forward().as_vec3(), t.forward().as_vec3());
        assert!((d2 - Vec3::new(d.x, d.y, -d.z)).length() < 1e-5 && (t.scale - Vec3::ONE).length() < 1e-6);
        assert!((t.translation - Vec3::new(-4.0, 9.0, 2.0 * WALL - 10.0)).length() < 1e-5);
    }
}
