//! R3 trial (docs/r3_solari.md): `render_mode = "rt_experimental"` with Bevy Solari ray-traced lighting. Only built with the
//! `solari` cargo feature, and the plugins are only added when the game starts in that mode (they switch every
//! StandardMaterial to the deferred path and build acceleration structures every frame).
//!
//! What traces: opaque lit StandardMaterial meshes (room boxes, textured pieces, voxel columns/objects, voxel characters) as
//! a copy with exactly the vertex layout Solari wants (position, normal, uv, tangent; U32 indices). What does not: alpha-masked
//! materials (sprite characters, cut-out tile images; Solari has no alpha test, they would shadow as rectangles) and
//! unlit / emissive ones (flames, potion bubbles): they still show and are lit through the G-buffer, but rays pass through
//! them. Solari has no point lights, so every torch `PointLight` gets an invisible emissive box (ray-traced only) at its flame.
use crate::settings::RenderMode;
use bevy::asset::AssetId;
use bevy::camera::{CameraMainTextureUsages, Exposure, Hdr};
use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureUsages;
use bevy::render::view::Msaa;
use bevy::solari::prelude::{RaytracingMesh3d, SolariLighting, SolariPlugins};
use std::collections::HashMap;

/// Present when the adapter has Solari's wgpu features: F5 can then cycle into `Rt`.
#[derive(Resource)]
pub struct RtAvailable;

pub fn plugin(app: &mut App) {
    app.add_plugins(SolariPlugins)
        .init_resource::<RtState>()
        .add_systems(Startup, check)
        .add_systems(Update, (apply, materials, meshes, torches).chain().before(crate::enhanced::apply));
}

/// Startup: without the features (Solari's plugins then skip themselves with a warning) run enhanced instead.
fn check(mut cmds: Commands, device: Option<Res<bevy::render::renderer::RenderDevice>>, mut mode: ResMut<RenderMode>) {
    let need = SolariPlugins::required_wgpu_features();
    match device.map(|d| d.features()) {
        Some(f) if f.contains(need) => {
            cmds.insert_resource(RtAvailable);
            info!("render mode rt_experimental: Solari ray-traced lighting (no denoiser in this build)");
        }
        f => {
            warn!("rt_experimental: adapter lacks {:?}; using enhanced", f.map_or(need, |f| need.difference(f)));
            *mode = RenderMode::Enhanced;
        }
    }
}

#[derive(Resource, Default)]
struct RtState {
    /// original values of the materials changed for Rt (restored when leaving it)
    mats: HashMap<AssetId<StandardMaterial>, StandardMaterial>,
    /// ray-tracing copy per raster mesh
    copies: HashMap<AssetId<Mesh>, Handle<Mesh>>,
    proxy: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    on: bool,
}

/// An entity whose material does not trace (alpha mask, unlit, emissive): checked once.
#[derive(Component)]
struct NotTraced;

/// The ray-traced-only emissive box standing in for a torch light.
#[derive(Component)]
struct TorchProxy;

#[derive(Component)]
struct HasProxy;

/// Flame proxy: box size, offset from the torch light (the light sits 0.3 above and 0.6 in front of the flame stem) and
/// radiance. 250 000 lm (the classic point light) from a Lambertian box of area A: L = flux / (pi A).
const PROXY_SIZE: Vec3 = Vec3::new(0.2, 0.3, 0.2);
const PROXY_OFFSET: Vec3 = Vec3::new(0.0, -0.18, -0.55);
const PROXY_FLUX: f32 = 250_000.0;

/// Camera and sun on entering / leaving Rt. Runs before `enhanced::apply`, which ignores Rt and redoes classic/enhanced.
fn apply(mode: Res<RenderMode>, mut st: ResMut<RtState>, mut cmds: Commands, cam: Query<Entity, With<Camera3d>>, mut sun: Query<&mut DirectionalLight>) {
    let want = *mode == RenderMode::Rt;
    if want == st.on {
        return;
    }
    let Ok(cam) = cam.single() else { return };
    st.on = want;
    let mut c = cmds.entity(cam);
    if want {
        c.remove::<(ScreenSpaceAmbientOcclusion, bevy::anti_alias::smaa::Smaa, NormalPrepass, bevy::render::view::NoIndirectDrawing)>().insert((
            // POP_RESTIR=1: ReSTIR resampling (less noise, more cost); the only noise reduction without DLSS-RR
            SolariLighting { restir: std::env::var("POP_RESTIR").is_ok_and(|v| v != "0"), ..default() },
            CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
            Msaa::Off,
            Bloom { intensity: 0.12, ..Bloom::NATURAL },
        ));
        // Solari replaces shadow maps; the ambient light and the point lights are ignored by it
        for mut l in &mut sun {
            (l.shadow_maps_enabled, l.illuminance) = (false, crate::enhanced::CLASSIC_SUN);
        }
        info!("render mode rt_experimental");
    } else {
        c.remove::<(SolariLighting, DeferredPrepass, MotionVectorPrepass, DepthPrepass, CameraMainTextureUsages, Hdr, Bloom)>().insert(Msaa::default());
    }
}

/// Solari multiplies emissive (and the G-buffer's unlit colour) by the camera exposure, raster does not
/// (`emissive_exposure_weight` 0): scale them so flames, bubbles and unlit sprites keep their brightness.
fn rt_gain() -> f32 {
    1.0 / Exposure::default().exposure()
}

/// While in Rt every unlit / emissive material (new ones too: the character cache adds them lazily) is scaled once; leaving
/// Rt restores the originals.
fn materials(st: ResMut<RtState>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let st = st.into_inner();
    if !st.on {
        for (id, m) in st.mats.drain() {
            if let Some(mut cur) = mats.get_mut(id) {
                *cur = m;
            }
        }
        return;
    }
    // the torch proxies' material is already in Solari's units
    let proxy = st.proxy.as_ref().map(|p| p.1.id());
    let new: Vec<AssetId<StandardMaterial>> = mats.iter().filter(|(id, m)| (m.unlit || m.emissive != LinearRgba::BLACK) && !st.mats.contains_key(id) && Some(*id) != proxy).map(|(id, _)| id).collect();
    let g = rt_gain();
    for id in new {
        let Some(mut m) = mats.get_mut(id) else { continue };
        st.mats.insert(id, m.clone());
        if m.unlit {
            m.base_color = (m.base_color.to_linear() * g).with_alpha(m.base_color.alpha()).into();
        }
        m.emissive *= g;
    }
}

/// The mesh with exactly Solari's layout: position, normal, uv (zeros if absent), tangent (any vector perpendicular to the
/// normal: no normal maps here), U32 indices. None for non-triangle meshes or missing data.
pub fn rt_copy(m: &Mesh) -> Option<Mesh> {
    if m.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let pos = m.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?.to_vec();
    let nrm = m.attribute(Mesh::ATTRIBUTE_NORMAL)?.as_float3()?.to_vec();
    let uv = match m.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(uv)) => uv.clone(),
        _ => vec![[0.0, 0.0]; pos.len()],
    };
    let tan: Vec<[f32; 4]> = nrm
        .iter()
        .map(|n| {
            let n = Vec3::from(*n);
            let t = n.cross(if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or(Vec3::X);
            [t.x, t.y, t.z, 1.0]
        })
        .collect();
    let idx: Vec<u32> = match m.indices() {
        Some(i) => i.iter().map(|i| i as u32).collect(),
        None => (0..pos.len() as u32).collect(),
    };
    Some(
        Mesh::new(PrimitiveTopology::TriangleList, default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tan)
            .with_inserted_indices(Indices::U32(idx)),
    )
}

/// Does this material trace? Opaque, lit, not emissive (checked on the original values, before `materials` scales them).
fn traces(m: &StandardMaterial) -> bool {
    m.alpha_mode == AlphaMode::Opaque && !m.unlit && m.emissive == LinearRgba::BLACK
}

/// Gives every new opaque lit mesh entity its ray-tracing copy, follows mesh swaps (character frames), and takes it all
/// away when leaving Rt.
#[allow(clippy::type_complexity)]
fn meshes(
    st: ResMut<RtState>,
    mut cmds: Commands,
    mut assets: ResMut<Assets<Mesh>>,
    mats: Res<Assets<StandardMaterial>>,
    new: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>), (Without<RaytracingMesh3d>, Without<NotTraced>)>,
    mut swapped: Query<(&Mesh3d, &mut RaytracingMesh3d), Changed<Mesh3d>>,
    traced: Query<Entity, Or<(With<RaytracingMesh3d>, With<NotTraced>)>>,
) {
    let st = st.into_inner();
    if !st.on {
        for e in &traced {
            cmds.entity(e).remove::<(RaytracingMesh3d, NotTraced)>();
        }
        return;
    }
    let mut copy = |h: &Handle<Mesh>| -> Option<Handle<Mesh>> {
        if let Some(c) = st.copies.get(&h.id()) {
            return Some(c.clone());
        }
        let m = rt_copy(assets.get(h)?)?;
        let c = assets.add(m);
        st.copies.insert(h.id(), c.clone());
        Some(c)
    };
    let (mut ok, mut masked, mut unlit) = (0, 0, 0);
    for (e, mesh, mat) in &new {
        let m = st.mats.get(&mat.id()).or_else(|| mats.get(mat));
        let Some(m) = m else { continue };
        if !traces(m) {
            if m.alpha_mode == AlphaMode::Opaque { unlit += 1 } else { masked += 1 }
            cmds.entity(e).insert(NotTraced);
            continue;
        }
        // mesh not loaded yet: try again next frame
        if let Some(c) = copy(&mesh.0) {
            cmds.entity(e).insert(RaytracingMesh3d(c));
            ok += 1;
        }
    }
    for (mesh, mut rt) in &mut swapped {
        if let Some(c) = copy(&mesh.0) {
            if rt.0 != c {
                rt.0 = c;
            }
        }
    }
    if ok + masked + unlit > 0 {
        info!("solari: {ok} meshes traced, not traced: {masked} alpha-masked/blended, {unlit} unlit/emissive (G-buffer only)");
    }
}

/// Torch lights: an emissive box (ray-traced only, no raster mesh) as child of each `PointLight`; removed on leaving Rt.
fn torches(st: ResMut<RtState>, mut cmds: Commands, mut assets: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, lights: Query<Entity, (With<PointLight>, Without<HasProxy>)>, proxies: Query<Entity, With<TorchProxy>>, marked: Query<Entity, With<HasProxy>>) {
    let st = st.into_inner();
    if !st.on {
        for e in &proxies {
            cmds.entity(e).despawn();
        }
        for e in &marked {
            cmds.entity(e).remove::<HasProxy>();
        }
        return;
    }
    let (mesh, mat) = st
        .proxy
        .get_or_insert_with(|| {
            let s = PROXY_SIZE;
            let area = 2.0 * (s.x * s.y + s.y * s.z + s.x * s.z);
            let radiance = PROXY_FLUX / (std::f32::consts::PI * area);
            let colour = crate::room::torch_light().color.to_linear();
            let mesh = assets.add(rt_copy(&Mesh::from(Cuboid::from_size(s))).expect("cuboid is a triangle list"));
            (mesh, mats.add(StandardMaterial { base_color: Color::BLACK, emissive: colour * radiance, ..default() }))
        })
        .clone();
    for e in &lights {
        cmds.entity(e).insert(HasProxy).with_child((RaytracingMesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_translation(PROXY_OFFSET), TorchProxy));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The copy has exactly Solari's layout (bevy_solari scene/blas.rs `is_mesh_raytracing_compatible`), from a mesh with
    /// U16 indices and no UVs (a voxel mesh has colours instead), and keeps the triangles.
    #[test]
    fn rt_copy_has_solari_layout() {
        let src = Mesh::new(PrimitiveTopology::TriangleList, default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 3])
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 0.0, 0.0, 1.0]; 3])
            .with_inserted_indices(Indices::U16(vec![0, 1, 2]));
        let c = rt_copy(&src).unwrap();
        let ids: Vec<_> = c.attributes().map(|(a, _)| (a.id, a.format)).collect();
        let want = [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL, Mesh::ATTRIBUTE_UV_0, Mesh::ATTRIBUTE_TANGENT].map(|a| (a.id, a.format));
        assert_eq!(ids, want);
        assert!(matches!(c.indices(), Some(Indices::U32(i)) if i == &vec![0, 1, 2]));
        assert!(c.enable_raytracing);
        // tangents are unit length and perpendicular to the normal
        let Some(VertexAttributeValues::Float32x4(t)) = c.attribute(Mesh::ATTRIBUTE_TANGENT) else { panic!() };
        assert!(t.iter().all(|t| (Vec3::new(t[0], t[1], t[2]).length() - 1.0).abs() < 1e-5 && t[2].abs() < 1e-5));
        // the cube (room boxes) converts too; a line list does not
        assert!(rt_copy(&Mesh::from(Cuboid::default())).is_some());
        assert!(rt_copy(&Mesh::new(PrimitiveTopology::LineList, default())).is_none());
    }

    #[test]
    fn only_opaque_lit_non_emissive_materials_trace() {
        assert!(traces(&StandardMaterial::default()));
        assert!(!traces(&StandardMaterial { alpha_mode: AlphaMode::Mask(0.5), ..default() }));
        assert!(!traces(&StandardMaterial { unlit: true, ..default() }));
        assert!(!traces(&StandardMaterial { emissive: LinearRgba::rgb(8.0, 3.0, 0.5), ..default() }));
    }
}
