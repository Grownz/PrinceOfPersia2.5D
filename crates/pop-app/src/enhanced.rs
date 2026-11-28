//! R2 "enhanced" render mode (docs/render_roadmap.md 9): torch and sun shadow maps, SSAO, bloom on the flames and a
//! softer, less blue ambient. Every GPU, no ray tracing. Classic adds none of it: at startup in classic nothing here touches
//! the scene, so the classic image stays identical.
use crate::settings::{RenderMode, Settings};
use bevy::anti_alias::smaa::Smaa;
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, PointLightShadowMap, ShadowFilteringMethod};
use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::post_process::bloom::Bloom;
use bevy::render::view::Msaa;
use bevy::window::{Monitor, PrimaryWindow};

/// The classic ambient light (on the camera).
pub fn classic_ambient() -> AmbientLight {
    AmbientLight { color: Color::srgb(0.6, 0.65, 0.9), brightness: 150.0, ..default() }
}

/// Enhanced ambient: brighter and less blue, so voxel faces turned away from the torches do not sink into blue-black
/// (SSAO darkens the ambient term again in creases).
fn enhanced_ambient() -> AmbientLight {
    AmbientLight { color: Color::srgb(0.78, 0.76, 0.86), brightness: 220.0, ..default() }
}

/// (shadow-casting torches, point shadow map size, sun shadow map size) per `shadow_quality`.
pub fn shadow_budget(quality: &str) -> (usize, usize, usize) {
    match quality {
        "off" => (0, 0, 0),
        "low" => (3, 256, 1024),
        _ => (6, 512, 2048),
    }
}

/// Shadow-casting torches: the `shadow_quality` budget, unless POP_FX overrides it.
fn torch_count(set: &Settings) -> usize {
    let fx = crate::perf::fx();
    if fx.torch_shadows { fx.lights.unwrap_or(shadow_budget(&set.shadow_quality).0) } else { 0 }
}

/// Indices of the `n` lights nearest to `focus` (xy distance: the camera looks straight down -z).
pub fn nearest(lights: &[Vec2], focus: Vec2, n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..lights.len()).collect();
    idx.sort_by(|&a, &b| lights[a].distance_squared(focus).total_cmp(&lights[b].distance_squared(focus)));
    idx.truncate(n);
    idx
}

/// p95 limit for the automatic fallback: 1.25 refresh periods of the slowest monitor, counted at no more than 60 Hz.
/// With vsync a frame is shown for whole periods (16.7 or 33.3 ms at 60 Hz), so the 25% separates "on time" (plus
/// jitter) from "missed vblank". On a high-refresh monitor a game running at 60-144 fps is still smooth enough to keep
/// enhanced (the only alternative is classic), so the limit never gets stricter than at 60 Hz (20.8 ms).
pub fn fallback_limit_ms(slowest_hz: f32) -> f32 {
    1250.0 / slowest_hz.clamp(1.0, 60.0)
}

const WARMUP_FRAMES: u32 = 600;
const WINDOW_FRAMES: usize = 300;
/// Consecutive slow windows before falling back: one hitch (shader compile, room change) spoils at most one window.
const SLOW_WINDOWS: u32 = 3;

/// Why the watchdog fell back (for the log line).
#[derive(Debug, PartialEq)]
pub struct SlowVerdict {
    pub avg_ms: f32,
    pub p95_ms: f32,
    pub limit_ms: f32,
}

/// Pure fallback logic: fed one frame time at a time, answers once `SLOW_WINDOWS` consecutive windows had a p95 over
/// `fallback_limit_ms`. An unfocused frame (throttled background window) restarts everything, warm-up included.
#[derive(Default)]
pub struct FallbackJudge {
    warm: u32,
    frames: Vec<f32>,
    slow: u32,
}

impl FallbackJudge {
    pub fn frame(&mut self, dt_ms: f32, focused: bool, slowest_hz: f32) -> Option<SlowVerdict> {
        if !focused {
            *self = Self::default();
            return None;
        }
        if self.warm < WARMUP_FRAMES {
            self.warm += 1;
            return None;
        }
        self.frames.push(dt_ms);
        if self.frames.len() < WINDOW_FRAMES {
            return None;
        }
        let avg_ms = self.frames.iter().sum::<f32>() / WINDOW_FRAMES as f32;
        self.frames.sort_by(f32::total_cmp);
        let p95_ms = self.frames[(WINDOW_FRAMES * 95).div_ceil(100) - 1]; // nearest rank: up to 15 slow frames pass
        self.frames.clear();
        let limit_ms = fallback_limit_ms(slowest_hz);
        self.slow = if p95_ms > limit_ms { self.slow + 1 } else { 0 };
        (self.slow >= SLOW_WINDOWS).then_some(SlowVerdict { avg_ms, p95_ms, limit_ms })
    }
}

/// Sun (fill) illuminance: classic, and enhanced (lower, so the torches stay the main light next to the sun's shadows).
pub const CLASSIC_SUN: f32 = 4000.0;
/// Enhanced sun illuminance per `sun_brightness` step ("high" is the former fixed value).
fn sun_lux(step: &str) -> f32 {
    match step { "dim" => 300.0, "low" => 800.0, "medium" => 1600.0, _ => 3300.0 }
}

/// Every frame: follows the `sun_brightness` setting live (enhanced mode only; apply() sets it on a mode change).
pub fn sun_brightness(mode: Res<RenderMode>, set: Res<Settings>, mut sun: Query<&mut DirectionalLight, Without<crate::mirror::TwinOf>>) {
    if *mode != RenderMode::Enhanced || !set.is_changed() {
        return;
    }
    for mut l in &mut sun {
        l.illuminance = sun_lux(&set.sun_brightness);
    }
}
/// Enhanced torches are brighter: their shadows and the position nearer the wall take light away.
pub const ENHANCED_TORCH_GAIN: f32 = 1.4;

/// Torch light: shadows on (only the nearest few), biases large enough for 1/32-unit voxel steps.
const DEPTH_BIAS: f32 = 0.02;
const NORMAL_BIAS: f32 = 0.6;

/// Applies the mode to camera, ambient, sun and shadow-map resources whenever it changes (not at a classic startup).
#[allow(clippy::too_many_arguments)]
pub fn apply(
    mode: Res<RenderMode>,
    set: Res<Settings>,
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
    mut cmds: Commands,
    mut cam: Query<(Entity, &mut AmbientLight), (With<Camera3d>, Without<crate::mirror::MirrorCam>)>,
    mut sun: Query<(Entity, &mut DirectionalLight), Without<crate::mirror::TwinOf>>,
    mut point_map: ResMut<PointLightShadowMap>,
    mut dir_map: ResMut<DirectionalLightShadowMap>,
    mut last: Local<Option<(RenderMode, String, bool)>>,
) {
    // runs again when the mode, the shadow quality or SSAO changes (the menu applies them live)
    let now = (*mode, set.shadow_quality.clone(), set.ssao);
    if last.as_ref() == Some(&now) || (last.is_none() && *mode == RenderMode::Classic) {
        *last = Some(now);
        return;
    }
    let Ok((cam_e, mut amb)) = cam.single_mut() else { return };
    *last = Some(now);
    // the RT trial sets up its own camera (solari.rs); leaving it comes back through here
    #[cfg(feature = "solari")]
    if *mode == RenderMode::Rt {
        return;
    }
    let mut c = cmds.entity(cam_e);
    if *mode == RenderMode::Classic {
        c.remove::<(Hdr, Bloom, ScreenSpaceAmbientOcclusion, DepthPrepass, NormalPrepass, Smaa, bevy::render::view::NoIndirectDrawing)>().insert((Msaa::default(), ShadowFilteringMethod::default()));
        *amb = classic_ambient();
        for (_, mut l) in &mut sun {
            (l.shadow_maps_enabled, l.illuminance, l.shadow_normal_bias) = (false, CLASSIC_SUN, DirectionalLight::default().shadow_normal_bias);
        }
        (*point_map, *dir_map) = (default(), default());
        info!("render mode classic");
        return;
    }
    let (shadows, point_size, dir_size) = shadow_budget(&set.shadow_quality);
    // POP_FX (perf.rs) overrides, for cost measurements only; all default to the values above
    let fx = crate::perf::fx();
    let torches = torch_count(&set);
    let (point_size, dir_size) = (fx.point_map.unwrap_or(point_size), fx.dir_map.unwrap_or(dir_size));
    // SSAO needs 5 storage textures per stage (bevy_pbr skips its plugin below that) and MSAA off
    let ssao_ok = device.is_some_and(|d| d.limits().max_storage_textures_per_shader_stage >= 5);
    let ssao = set.ssao && ssao_ok && fx.ssao;
    // a re-run with SSAO turned off must undo what the last run inserted
    c.remove::<(ScreenSpaceAmbientOcclusion, Smaa, DepthPrepass, NormalPrepass)>().insert(Msaa::default());
    // flames (emissive 8, 3, 0.5) are the only values far above 1, so the threshold-free natural bloom only glows there
    match (fx.hdr, fx.bloom) {
        (true, true) => {
            c.insert((Hdr, Bloom { intensity: 0.12, ..Bloom::NATURAL }));
        }
        (true, false) => {
            c.insert(Hdr);
        }
        _ => {}
    }
    if ssao {
        use ScreenSpaceAmbientOcclusionQualityLevel as Q;
        let quality_level = match fx.ssao_quality.as_deref() { Some("low") => Q::Low, Some("high") => Q::High, Some("ultra") => Q::Ultra, _ => Q::Medium };
        c.insert((Msaa::Off, ScreenSpaceAmbientOcclusion { quality_level, ..default() }));
        if fx.smaa {
            c.insert(Smaa::default());
        }
    }
    c.insert(match fx.filter.as_deref() {
        Some("hw") => ShadowFilteringMethod::Hardware2x2,
        Some("gauss") => ShadowFilteringMethod::Gaussian,
        Some(_) => ShadowFilteringMethod::Temporal,
        None if set.shadow_quality == "low" => ShadowFilteringMethod::Hardware2x2,
        None => ShadowFilteringMethod::Gaussian,
    });
    if !fx.indirect {
        c.insert(bevy::render::view::NoIndirectDrawing);
    }
    *amb = if fx.ambient { enhanced_ambient() } else { classic_ambient() };
    for (e, mut l) in &mut sun {
        // a little more normal bias than the default 1.8: the stacked wall boxes showed blotches on faces facing the sun
        (l.shadow_maps_enabled, l.illuminance, l.shadow_normal_bias) = (shadows > 0 && fx.sun_shadow, sun_lux(&set.sun_brightness), 2.5);
        // one cascade over the visible rooms (camera ~9 units away), so the sun's shadow texels stay small
        cmds.entity(e).insert(CascadeShadowConfigBuilder { num_cascades: 1, minimum_distance: 1.0, maximum_distance: 30.0, ..default() }.build());
    }
    if shadows > 0 {
        (point_map.size, dir_map.size) = (point_size, dir_size);
    }
    info!(
        "render mode enhanced: bloom, HDR, ambient fill, shadows {} ({} torch shadows, maps {}/{}), SSAO {}",
        set.shadow_quality, torches, point_size, dir_size,
        if ssao { "on (MSAA off, SMAA)" } else if !ssao_ok { "unavailable on this adapter" } else { "off" }
    );
}

/// Does a light at `p` with `range` reach the picture (the drawn room plus the margin the camera shows, 1.5 units)?
fn touches_picture(p: Vec2, range: f32) -> bool {
    let (w, h) = (pop_core::level::COLS as f32, pop_core::level::ROWS as f32 * crate::room::TILE_H);
    let (dx, dy) = ((-1.5 - p.x).max(p.x - w - 1.5).max(0.0), (-1.5 - p.y).max(p.y - h - 1.5).max(0.0));
    dx * dx + dy * dy <= range * range
}

/// A torch's light range clipped to the open space it sees (`light_range`): `cur` eases to `target` (world units).
#[derive(Component)]
pub struct Reach { cur: f32, target: f32 }

/// Grid the line of sight runs on: the drawn room plus one room each way (tile coordinates -10..20 x -3..6).
const GRID_W: i32 = 3 * pop_core::level::COLS as i32;
const GRID_H: i32 = 3 * pop_core::level::ROWS as i32;

/// Range of the light at world `at` (drawn-room frame, y up) so it does not reach past the geometry it sees: the farthest open tile
/// centre within `max` that a grid line of sight (sampled every quarter unit, the light's and the target's own tile never block)
/// reaches, plus a tile of margin, clamped to 2..`max`. `solid(col, row)` takes drawn-room tile coordinates (outside the grid = solid).
/// Runs on room changes only; per torch about 100 tiles x 20 samples.
pub fn light_range(solid: impl Fn(i32, i32) -> bool, at: Vec2, max: f32) -> f32 {
    use crate::room::TILE_H;
    let rows = pop_core::level::ROWS as i32;
    let cell = |p: Vec2| (p.x.floor() as i32, rows - 1 - (p.y / TILE_H).floor() as i32);
    let own = cell(at);
    let mut far = 0.0f32;
    for (col, row) in (-rows..GRID_H - rows).flat_map(|r| (-(GRID_W / 3)..GRID_W - GRID_W / 3).map(move |c| (c, r))) {
        let to = Vec2::new(col as f32 + 0.5, (rows - 1 - row) as f32 * TILE_H + TILE_H * 0.5);
        let dist = to.distance(at);
        if dist <= far || dist > max || solid(col, row) {
            continue;
        }
        let steps = (dist / 0.25).ceil() as i32;
        if (1..steps).all(|s| { let c = cell(at.lerp(to, s as f32 / steps as f32)); c == own || c == (col, row) || !solid(c.0, c.1) }) {
            far = dist;
        }
    }
    (far + 1.0).clamp(2.0, max)
}

/// A torch light's classic z, kept while enhanced mode moves it.
#[derive(Component)]
pub struct ClassicZ(f32);

/// Enhanced torch lights sit this far in front of the back wall's face (z -0.9), just in front of the flame: at the classic
/// z (-0.2, 0.7 out) the exit/door boxes next to the torches stood beside the light and threw large shadows on the wall.
const ENHANCED_TORCH_Z: f32 = -0.6;

/// Every frame: in enhanced mode the `n` torches nearest to the view cast shadows, the rest stay shadowless (a point
/// shadow is 6 cube faces); in classic every light is back to `room::torch_light`'s values and its classic position.
pub fn torch_shadows(mut cmds: Commands, mode: Res<RenderMode>, set: Res<Settings>, g: Res<crate::Game>, time: Res<Time>, mut drawn: Local<u8>, cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::mirror::MirrorCam>)>, overlays: Query<Entity, (Added<Mesh3d>, Or<(With<crate::wear::Decal>, With<crate::fx::FxSpawn>)>, Without<bevy::light::NotShadowCaster>)>, mut lights: Query<(Entity, &mut Transform, &mut PointLight, Option<&ClassicZ>, Option<&crate::room::TorchLight>, Option<&mut Reach>, Option<&crate::room::ChamberTorch>), (Without<crate::room::BubbleLight>, Without<crate::particles::Spark>, Without<Drinking>, Without<crate::mirror::TwinOf>)>) {
    let enhanced = *mode == RenderMode::Enhanced;
    // decals and fx quads are never occluders: they would put holes into the torch shadow maps
    for e in &overlays {
        cmds.entity(e).insert(bevy::light::NotShadowCaster);
    }
    let n = if enhanced { torch_count(&set) } else { 0 };
    let focus = cam.single().map_or(Vec2::ZERO, |t| t.translation().truncate());
    let d = PointLight::default();
    let (level, room) = (&g.world.level, g.world.shared.drawn_room);
    let room_changed = std::mem::replace(&mut *drawn, room) != room;
    let k = 1.0 - (-time.delta_secs() * 8.0).exp();
    let grid: Vec<bool> = if enhanced && room != 0 { (-3..6).flat_map(|r| (-10..20).map(move |c| (c, r))).map(|(c, r)| level.tile_wrapped(room as i32, c, r) == pop_core::level::Tile::Wall).collect() } else { vec![] };
    let max = crate::room::torch_light().range;
    // pass 1: every enhanced torch only reaches as far as the open space it sees (the tile grid line of sight, `light_range`; classic keeps
    // the full range); the shadow candidates are the lights whose clipped reach touches the picture
    let mut cands: Vec<(usize, f32)> = Vec::new();
    for (i, (e, t, mut l, _, flick, reach, chamber)) in lights.iter_mut().enumerate() {
        let mut range = max;
        if enhanced && flick.is_some() && chamber.is_none() && room != 0 {
            let at = t.translation.truncate();
            let target = || light_range(|c, r| !(-10..20).contains(&c) || !(-3..6).contains(&r) || grid[((r + 3) * GRID_W + c + 10) as usize], at, max);
            match reach {
                Some(mut r) => {
                    if room_changed { r.target = target(); }
                    r.cur += (r.target - r.cur) * k;
                    range = r.cur;
                }
                None => {
                    let v = target();
                    cmds.entity(e).insert(Reach { cur: v, target: v });
                    range = v;
                }
            }
        }
        if l.range != range { l.range = range; }
        // the set's lights count only while the camera is on the set (the picture is then round `ORIGIN`, the levels' lights are far away)
        let on_set = focus.distance(crate::cutscene::ORIGIN.truncate()) < 20.0;
        let at = t.translation.truncate() - chamber.map_or(Vec2::ZERO, |_| crate::cutscene::ORIGIN.truncate());
        if enhanced && (chamber.is_none() || on_set) && touches_picture(at, range) {
            // hysteresis: a light that already casts keeps its place against one that is almost as near
            cands.push((i, t.translation.truncate().distance_squared(focus) * if l.shadow_maps_enabled { 0.5 } else { 1.0 }));
        }
    }
    cands.sort_by(|a, b| a.1.total_cmp(&b.1));
    let on: Vec<usize> = cands.iter().take(n).map(|c| c.0).collect();
    for (i, (e, mut t, mut l, cz, flick, _, chamber)) in lights.iter_mut().enumerate() {
        let want = if on.contains(&i) { (true, DEPTH_BIAS, NORMAL_BIAS) } else { (false, d.shadow_depth_bias, d.shadow_normal_bias) };
        let intensity = crate::room::torch_light().intensity * if enhanced { ENHANCED_TORCH_GAIN } else { 1.0 } * flick.filter(|_| enhanced || chamber.is_none()).map_or(1.0, |f| f.2);
        if (l.shadow_maps_enabled, l.shadow_depth_bias, l.shadow_normal_bias, l.intensity) != (want.0, want.1, want.2, intensity) {
            (l.shadow_maps_enabled, l.shadow_depth_bias, l.shadow_normal_bias, l.intensity) = (want.0, want.1, want.2, intensity);
        }
        match (enhanced, cz) {
            _ if chamber.is_some() => {} // `room::flicker` places the set's lights
            (true, None) => {
                cmds.entity(e).insert(ClassicZ(t.translation.z));
                t.translation.z = ENHANCED_TORCH_Z;
            }
            (false, Some(ClassicZ(z))) => {
                t.translation.z = *z;
                cmds.entity(e).remove::<ClassicZ>();
            }
            _ => {}
        }
    }
}

/// Intensity of a potion's bubble light (enhanced only; torches are 250 000).
const BUBBLE_LIGHT: f32 = 24_000.0;
/// Bubble lights that shine at once (the nearest to the view): each casts shadows so it cannot shine through walls and
/// slabs; the farther ones are off. With shadows off (`shadow_quality`) none shines, as it would leak.
const BUBBLE_LIGHTS: usize = 2;

/// A potion's light after the potion was drunk: it flies from the bottle's place to the kid's face during the first `DRINK_FLY`
/// ticks of the animation and fades exponentially until its end (`DRINK_TICKS`), then goes out.
#[derive(Component)]
pub struct Drinking {
    start: u64,
    from: Vec3,
    intensity: f32,
}

impl Drinking {
    pub fn new(start: u64, from: Vec3, intensity: f32) -> Self {
        Drinking { start, from, intensity }
    }
}

const DRINK_FLY: f32 = 12.0;
/// e^-DRINK_FADE of the light is left at the end of the animation.
const DRINK_FADE: f32 = 5.0;
/// Where the flask is at the mouth, relative to the kid sprite's centre (world units, the kid is about 1.5 tall) and in front of him.
const MOUTH: Vec3 = Vec3::new(0.12, 0.3, 0.3);

/// (position share along the flight, intensity share) of a drinking light `t` ticks after the pick-up; None once the animation is over.
fn drinking_at(t: f32) -> Option<(f32, f32)> {
    let end = pop_core::sim::DRINK_TICKS as f32;
    if !(0.0..end).contains(&t) {
        return None;
    }
    let u = (t / DRINK_FLY).min(1.0);
    Some((u * u * (3.0 - 2.0 * u), (-DRINK_FADE * t / end).exp()))
}

pub fn drinking_lights(
    mut cmds: Commands,
    g: Res<crate::Game>,
    kid: Query<&Transform, With<crate::kid::KidSprite>>,
    mut q: Query<(Entity, &Drinking, &mut Transform, &mut PointLight), Without<crate::kid::KidSprite>>,
) {
    let Ok(k) = kid.single() else { return };
    let now = g.world.tick as f32 + g.world.alpha();
    let dir = if g.world.kid.facing_left { -1.0 } else { 1.0 };
    for (e, d, mut t, mut l) in &mut q {
        let Some((fly, fade)) = drinking_at(now - d.start as f32) else {
            cmds.entity(e).despawn();
            continue;
        };
        t.translation = d.from.lerp(k.translation + MOUTH * Vec3::new(dir, 1.0, 1.0), fly);
        l.intensity = d.intensity * fade;
    }
}

/// The bubbles' faint coloured light shines in enhanced mode only.
pub fn bubble_lights(mode: Res<RenderMode>, set: Res<Settings>, cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::mirror::MirrorCam>)>, mut q: Query<(&Transform, &mut PointLight), With<crate::room::BubbleLight>>) {
    let n = if *mode == RenderMode::Enhanced && torch_count(&set) > 0 { BUBBLE_LIGHTS } else { 0 };
    let focus = cam.single().map_or(Vec2::ZERO, |t| t.translation().truncate());
    let pos: Vec<Vec2> = q.iter().map(|(t, _)| t.translation.truncate()).collect();
    let on = nearest(&pos, focus, n);
    for (i, (_, mut l)) in q.iter_mut().enumerate() {
        let lit = on.contains(&i);
        let (i, s) = (if lit { BUBBLE_LIGHT } else { 0.0 }, lit);
        if (l.intensity, l.shadow_maps_enabled) != (i, s) {
            (l.intensity, l.shadow_maps_enabled) = (i, s);
            (l.shadow_depth_bias, l.shadow_normal_bias, l.shadow_map_near_z) = (DEPTH_BIAS, NORMAL_BIAS, 0.02);
        }
    }
}

/// Point (and spot) light shadow views, all in one go. Bevy 0.20 renders each cube face of a shadowed point light as a root
/// view that runs the whole `Core3d` camera schedule (~45 systems on the multithreaded executor; all but five skip, the view
/// has no camera components), and each of the five records its own command encoder. 4 torches = 24 schedule runs and 120
/// command buffers per frame, serial on the render thread: the largest CPU cost of enhanced mode (docs/enhanced_perf.md 5).
/// This system runs in `RenderGraphSystems::Render` (after the sparse buffer uploads in `Begin`: ordered only before
/// `camera_driver`, it ran ahead of them and drew changed meshes one frame stale) and calls exactly those five Bevy systems,
/// per view in `Core3d`'s order and with its run condition, sharing one command encoder per light; then it takes the views out of `camera_driver`'s list for this frame
/// (`prepare_lights` adds them again next frame). Same commands in the same order on the GPU, same image.
pub fn light_shadow_views_plugin(app: &mut App) {
    use bevy::render::{renderer::{RenderGraph, RenderGraphSystems}, RenderApp};
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    render_app.add_systems(RenderGraph, light_shadow_views.in_set(RenderGraphSystems::Render).before(bevy::core_pipeline::schedule::camera_driver));
}

mod shadow_views {
    pub use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass};
    pub use bevy::core_pipeline::schedule::RootNonCameraView;
    pub use bevy::pbr::*;
    pub use bevy::prelude::*;
    pub use bevy::render::batching::gpu_preprocessing::{BatchedInstanceBuffers, BuildIndirectParametersMetadata, IndirectParametersBuffers};
    pub use bevy::render::diagnostic::{DiagnosticsRecorder, RecordDiagnostics};
    pub use bevy::render::render_phase::ViewBinnedRenderPhases;
    pub use bevy::render::render_resource::PipelineCache;
    pub use bevy::render::renderer::{CurrentView, RenderContext, ViewQuery};
    pub use bevy::render::occlusion_culling::OcclusionCulling;
    pub use bevy::render::view::{ExtractedView, NoIndirectDrawing, ViewUniformOffset};
    pub type Prepass = Or<(With<DepthPrepass>, With<NormalPrepass>, With<MotionVectorPrepass>, With<DeferredPrepass>, With<ShadowView>)>;
    /// `early_prepass_build_indirect_parameters`' run condition in `Core3d`
    pub type Indirect = (With<PreprocessBindGroups>, Without<SkipGpuPreprocess>, Without<NoIndirectDrawing>, Prepass);
    pub type Mesh = BatchedInstanceBuffers<MeshUniform, MeshInputUniform>;
    /// the parameters of `allocate_uniforms`, `unpack_bins`, `early_gpu_preprocess`, `early_prepass_build_indirect_parameters`
    /// and `shared_shadow_pass` without their `RenderContext`
    pub type Alloc = (
        ViewQuery<'static, 'static, Option<&'static ViewLightEntities>, Without<SkipGpuPreprocess>>,
        Query<'static, 'static, &'static ExtractedView, Without<SkipGpuPreprocess>>,
        Query<'static, 'static, &'static LightEntity>,
        Res<'static, Mesh>,
        Res<'static, PipelineCache>,
        Res<'static, PreprocessPipelines>,
        Res<'static, UniformAllocationBindGroups>,
    );
    pub type Unpack = (
        ViewQuery<'static, 'static, Option<&'static ViewLightEntities>, Without<SkipGpuPreprocess>>,
        Query<'static, 'static, &'static ExtractedView, Without<SkipGpuPreprocess>>,
        Query<'static, 'static, &'static LightEntity>,
        Res<'static, Mesh>,
        Res<'static, PipelineCache>,
        Res<'static, PreprocessPipelines>,
        Res<'static, BinUnpackingBindGroups>,
    );
    pub type Preprocess = (
        ViewQuery<'static, 'static, Option<&'static ViewLightEntities>, Without<SkipGpuPreprocess>>,
        Query<'static, 'static, (&'static ExtractedView, Option<&'static PreprocessBindGroups>, Option<&'static ViewUniformOffset>, Has<NoIndirectDrawing>, Has<OcclusionCulling>), Without<SkipGpuPreprocess>>,
        Query<'static, 'static, &'static LightEntity>,
        Res<'static, Mesh>,
        Res<'static, PipelineCache>,
        Res<'static, PreprocessPipelines>,
    );
    pub type Build = (
        ViewQuery<'static, 'static, &'static ExtractedView>,
        Res<'static, PreprocessPipelines>,
        Option<Res<'static, BuildIndirectParametersBindGroups>>,
        Res<'static, PipelineCache>,
        Option<Res<'static, IndirectParametersBuffers>>,
        Res<'static, BuildIndirectParametersMetadata>,
    );
    pub type Pass = (
        &'static World,
        ViewQuery<'static, 'static, (Entity, &'static ShadowView, &'static ExtractedView, Has<OcclusionCulling>)>,
        Res<'static, ViewBinnedRenderPhases<Shadow>>,
    );
    pub type Ctx = RenderContext<'static, 'static>;
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn light_shadow_views(
    world: &mut World,
    views: &mut QueryState<Entity, With<shadow_views::RootNonCameraView>>,
    indirect: &mut QueryState<(), shadow_views::Indirect>,
    mut ctx_s: Local<Vec<bevy::ecs::system::SystemState<shadow_views::Ctx>>>,
    alloc_s: &mut bevy::ecs::system::SystemState<shadow_views::Alloc>,
    unpack_s: &mut bevy::ecs::system::SystemState<shadow_views::Unpack>,
    pre_s: &mut bevy::ecs::system::SystemState<shadow_views::Preprocess>,
    build_s: &mut bevy::ecs::system::SystemState<shadow_views::Build>,
    pass_s: &mut bevy::ecs::system::SystemState<shadow_views::Pass>,
) {
    use shadow_views::*;
    if crate::perf::fx().core3d_shadows {
        return;
    }
    let list: Vec<Entity> = views.iter(world).collect();
    // the late (occlusion culling) passes are not called here: views with OcclusionCulling stay on Core3d
    if list.is_empty() || list.iter().any(|v| world.entity(*v).contains::<OcclusionCulling>()) {
        return;
    }
    // one encoder per light (6 cube faces): Bevy finishes encoders in parallel just before the submit, one big one alone took
    // ~1 ms there
    let groups = list.len().div_ceil(6);
    while ctx_s.len() < groups {
        let s = bevy::ecs::system::SystemState::new(world);
        ctx_s.push(s);
    }
    // `any_match_filter` is evaluated per schedule run in Core3d; nothing changes between the views of one frame
    let build_indirect = indirect.iter(world).next().is_some();
    if let (Some(d), Ok(mut ctx)) = (world.get_resource::<DiagnosticsRecorder>(), ctx_s[0].get(world)) {
        d.begin_time_span(ctx.command_encoder(), "light_shadow_views".into());
    }
    for (i, v) in list.iter().enumerate() {
        world.insert_resource(CurrentView(*v));
        let w: &World = world;
        let ctx_s = &mut ctx_s[i / 6];
        // each call skips when its parameters do not validate, as a system in a schedule would
        if let (Ok((a, b, c, d, e, f, g)), Ok(ctx)) = (alloc_s.get(w), ctx_s.get(w)) {
            allocate_uniforms(a, b, c, d, e, f, g, ctx);
        }
        if let (Ok((a, b, c, d, e, f, g)), Ok(ctx)) = (unpack_s.get(w), ctx_s.get(w)) {
            unpack_bins(a, b, c, d, e, f, g, ctx);
        }
        if let (Ok((a, b, c, d, e, f)), Ok(ctx)) = (pre_s.get(w), ctx_s.get(w)) {
            early_gpu_preprocess(a, b, c, d, e, f, ctx);
        }
        if build_indirect {
            if let (Ok((a, b, c, d, e, f)), Ok(ctx)) = (build_s.get(w), ctx_s.get(w)) {
                early_prepass_build_indirect_parameters(a, b, c, d, e, f, ctx);
            }
        }
        if let (Ok((a, b, c)), Ok(ctx)) = (pass_s.get(w), ctx_s.get(w)) {
            shared_shadow_pass::<EARLY_SHADOW_PASS>(a, b, c, ctx);
        }
    }
    world.remove_resource::<CurrentView>();
    if let (Some(d), Ok(mut ctx)) = (world.get_resource::<DiagnosticsRecorder>(), ctx_s[groups - 1].get(world)) {
        d.end_time_span(ctx.command_encoder());
    }
    // the encoders go to PendingCommandBuffers here, in view order, before camera_driver adds the cameras' work
    for s in &mut ctx_s[..groups] {
        s.apply(world);
    }
    for v in list {
        world.entity_mut(v).remove::<RootNonCameraView>();
    }
}

/// F5: classic -> enhanced (-> rt_experimental when Solari is running) -> classic, saved in settings.json.
pub fn mode_key(k: Res<ButtonInput<KeyCode>>, mut mode: ResMut<RenderMode>, mut set: ResMut<Settings>, #[cfg(feature = "solari")] rt: Option<Res<crate::solari::RtAvailable>>) {
    if !k.just_pressed(KeyCode::F5) {
        return;
    }
    *mode = match *mode {
        RenderMode::Classic => RenderMode::Enhanced,
        #[cfg(feature = "solari")]
        RenderMode::Enhanced if rt.is_some() => RenderMode::Rt,
        _ => RenderMode::Classic,
    };
    set.render_mode = mode.key().into();
    set.save();
}

/// Automatic fallback: in enhanced mode, `FallbackJudge` (600-frame warm-up, 300-frame windows, p95 against the refresh
/// period, 3 slow windows in a row, focused only) switches to classic for this session (not saved). Fires at most once,
/// so it cannot oscillate.
pub fn watchdog(
    mut mode: ResMut<RenderMode>,
    time: Res<Time<Real>>,
    win: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
    mut st: Local<(FallbackJudge, bool)>,
) {
    if st.1 || *mode != RenderMode::Enhanced {
        st.0 = FallbackJudge::default();
        return;
    }
    let focused = win.single().is_ok_and(|w| w.focused);
    let hz = monitors.iter().filter_map(|m| m.refresh_rate_millihertz).filter(|&m| m > 0).min().map_or(60.0, |mhz| mhz as f32 / 1000.0);
    if let Some(v) = st.0.frame(time.delta_secs() * 1000.0, focused, hz) {
        warn!(
            "enhanced renderer too slow: avg {:.2} ms, p95 {:.2} ms > {:.2} ms (1.25 periods at {hz:.0} Hz, capped at 60) in {SLOW_WINDOWS} windows of {WINDOW_FRAMES} frames in a row; falling back to classic for this session",
            v.avg_ms, v.p95_ms, v.limit_ms
        );
        *mode = RenderMode::Classic;
        st.1 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A torch behind a full wall column only reaches the open space it sees; a gap lets it reach further; in the open it keeps the full range.
    #[test]
    fn light_range_stops_at_walls() {
        let at = Vec2::new(-3.5, 3.0); // row 1 of the room to the left
        let wall = |c: i32, r: i32| c <= -5 || c == -2 || !(0..3).contains(&r);
        assert!(light_range(wall, at, 9.0) <= 3.5, "nothing past the wall");
        let gap = |c: i32, r: i32| c <= -5 || (c == -2 && r != 1) || !(0..3).contains(&r);
        assert!(light_range(gap, at, 9.0) > 7.0, "through the gap");
        assert_eq!(light_range(|_, _| false, at, 9.0), 9.0);
        assert_eq!(light_range(|_, _| true, at, 9.0), 2.0, "boxed in: the minimum");
    }

    /// A drunk potion's light: starts at the bottle, arrives at the kid after DRINK_FLY ticks, only ever gets darker and is out at the end.
    #[test]
    fn drinking_light_flies_then_fades_out() {
        assert_eq!(drinking_at(0.0), Some((0.0, 1.0)));
        assert_eq!(drinking_at(DRINK_FLY).unwrap().0, 1.0);
        assert_eq!(drinking_at(DRINK_FLY + 3.0).unwrap().0, 1.0, "then it follows the kid");
        let ts: Vec<f32> = (0..210).map(|i| i as f32 / 10.0).collect();
        assert!(ts.windows(2).all(|w| drinking_at(w[1]).unwrap().1 < drinking_at(w[0]).unwrap().1 && drinking_at(w[1]).unwrap().0 >= drinking_at(w[0]).unwrap().0));
        assert!(drinking_at(20.9).unwrap().1 < 0.01 && drinking_at(21.0).is_none() && drinking_at(-0.1).is_none());
    }

    #[test]
    fn nearest_picks_closest_lights_in_order() {
        let l = [Vec2::new(10.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(-3.0, 0.0), Vec2::new(0.5, 0.0)];
        assert_eq!(nearest(&l, Vec2::ZERO, 2), vec![3, 1]);
        assert_eq!(nearest(&l, Vec2::ZERO, 9).len(), 4);
        assert!(nearest(&l, Vec2::ZERO, 0).is_empty());
    }

    #[test]
    fn budgets_and_fallback_threshold() {
        assert_eq!(shadow_budget("off").0, 0);
        assert!(shadow_budget("low").0 < shadow_budget("high").0);
        assert_eq!(shadow_budget("unknown"), shadow_budget("high"));
        assert!((fallback_limit_ms(60.0) - 20.833).abs() < 0.01);
        assert_eq!(fallback_limit_ms(144.0), fallback_limit_ms(60.0));
        assert!((fallback_limit_ms(30.0) - 41.667).abs() < 0.01);
    }

    /// Feeds `windows` windows of frame times from `frame(i)` after the warm-up; the first verdict and the window it came in.
    fn run(windows: usize, focused: bool, hz: f32, frame: impl Fn(usize) -> f32) -> Option<(usize, SlowVerdict)> {
        let mut j = FallbackJudge::default();
        for _ in 0..WARMUP_FRAMES {
            assert!(j.frame(100.0, focused, hz).is_none(), "warm-up never counts");
        }
        (0..windows * WINDOW_FRAMES).find_map(|i| j.frame(frame(i), focused, hz).map(|v| (i / WINDOW_FRAMES, v)))
    }

    #[test]
    fn fallback_steady_60fps_with_vsync_jitter_stays() {
        assert!(run(20, true, 60.0, |i| if i % 7 == 0 { 17.4 } else { 16.6 }).is_none());
        // vsync off, fast GPU
        assert!(run(20, true, 60.0, |_| 3.0).is_none());
    }

    #[test]
    fn fallback_slow_p95_triggers_after_three_windows() {
        // vsync at 60 Hz, every 10th frame misses vblank (33.3 ms): avg 18.3 ms, but p95 over 16.7 means 30 fps stutter
        let (w, v) = run(20, true, 60.0, |i| if i % 10 == 0 { 33.3 } else { 16.7 }).unwrap();
        assert_eq!(w, 2);
        assert_eq!(v.p95_ms, 33.3);
        assert!(v.avg_ms < v.limit_ms, "the old average rule would not have fired");
    }

    #[test]
    fn fallback_single_spike_or_one_bad_window_does_not_trigger() {
        // one 2 s hitch, then a few slow frames per window (room changes)
        assert!(run(20, true, 60.0, |i| match i {
            0 => 2000.0,
            _ if i % 300 < 10 => 40.0,
            _ => 16.7,
        })
        .is_none());
        // a whole slow window (shader compiles after a level change), twice, but never three in a row
        assert!(run(20, true, 60.0, |i| if (i / WINDOW_FRAMES) % 3 == 0 { 40.0 } else { 16.7 }).is_none());
    }

    #[test]
    fn fallback_ignores_unfocused_frames() {
        assert!(run(20, false, 60.0, |_| 50.0).is_none());
        // focus lost mid-way restarts the warm-up and the streak
        let mut j = FallbackJudge::default();
        let mut fired = false;
        for i in 0..20 * WINDOW_FRAMES {
            fired |= j.frame(50.0, i % 1000 != 999, 60.0).is_some();
        }
        assert!(!fired);
    }

    #[test]
    fn fallback_high_refresh_monitors() {
        // 144 Hz with vsync, enhanced only reaching 72 fps (every frame misses one vblank): smooth enough, stays
        assert!(run(20, true, 144.0, |_| 13.9).is_none());
        // 240 Hz, vsync off, 100-150 fps
        assert!(run(20, true, 240.0, |i| if i % 2 == 0 { 6.7 } else { 10.0 }).is_none());
        // 144 Hz but under 48 fps for more than 5% of the frames: falls back
        assert!(run(20, true, 144.0, |i| if i % 10 == 0 { 25.0 } else { 6.9 }).is_some());
    }
}
