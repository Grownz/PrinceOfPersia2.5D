//! Performance tools (docs/enhanced_perf.md): the F9 / `POP_FPS=1` frame-time overlay, the `POP_BENCH=<frames>` benchmark
//! run and the `POP_FX` cost toggles for enhanced mode. Nothing here runs or spawns while unused: the overlay is spawned on
//! first use, GPU pass timing (`RenderDiagnosticsPlugin`, wgpu timestamp queries) is only added with POP_FPS / POP_BENCH.
use crate::settings::RenderMode;
use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::VecDeque;

/// Frame-time statistics over a window of frames (ms).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    pub avg: f32,
    pub min: f32,
    pub max: f32,
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
    /// mean of the slowest 1% of frames (at least one frame), the "1% low" in ms
    pub low1: f32,
}

/// Nearest-rank percentile `p` (0..=100) of an ascending slice.
fn percentile(sorted: &[f32], p: f32) -> f32 {
    let rank = ((p / 100.0) * sorted.len() as f32).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn stats(ms: &[f32]) -> Stats {
    if ms.is_empty() {
        return Stats::default();
    }
    let mut s = ms.to_vec();
    s.sort_by(f32::total_cmp);
    let n = s.len();
    let worst = n.div_ceil(100);
    Stats {
        avg: s.iter().sum::<f32>() / n as f32,
        min: s[0],
        max: s[n - 1],
        p50: percentile(&s, 50.0),
        p95: percentile(&s, 95.0),
        p99: percentile(&s, 99.0),
        low1: s[n - worst..].iter().sum::<f32>() / worst as f32,
    }
}

/// `POP_RES=1920x1080` -> (1920, 1080).
pub fn parse_res(v: &str) -> Option<(u32, u32)> {
    let (w, h) = v.trim().split_once(['x', 'X'])?;
    Some((w.trim().parse().ok().filter(|&w| w >= 320)?, h.trim().parse().ok().filter(|&h| h >= 200)?))
}

/// Enhanced-mode cost toggles from `POP_FX` (comma separated): `no_<feature>` or `<feature>=off` for torch_shadows,
/// sun_shadow, ssao, bloom, smaa, hdr (implies no bloom), ambient (the classic fill light), indirect (GPU indirect draws); `lights=0..4` shadow-casting
/// torches, `point_map=` / `dir_map=` shadow map sizes, `ssao=low|medium|high|ultra`, `filter=hw|gauss|temporal`, `rooms=1`
/// (no neighbouring rooms), `casters=boxes|none` (voxel room objects / all room geometry stop casting shadows),
/// `core3d_shadows` (light shadow views on the full `Core3d` schedule, as before `enhanced::LightShadowViews`).
/// For measurements only: defaults change nothing, classic ignores them except `rooms`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fx {
    pub torch_shadows: bool,
    pub sun_shadow: bool,
    pub ssao: bool,
    pub bloom: bool,
    pub smaa: bool,
    pub hdr: bool,
    pub ambient: bool,
    /// GPU indirect drawing (off: `NoIndirectDrawing` on the camera and so on its shadow views)
    pub indirect: bool,
    pub lights: Option<usize>,
    pub point_map: Option<usize>,
    pub dir_map: Option<usize>,
    pub ssao_quality: Option<String>,
    pub filter: Option<String>,
    pub rooms: Option<usize>,
    pub casters: Option<String>,
    /// light shadow views run Bevy's full `Core3d` schedule again (before `enhanced::LightShadowViews`), for A/B runs
    pub core3d_shadows: bool,
}

impl Default for Fx {
    fn default() -> Self {
        Fx { torch_shadows: true, sun_shadow: true, ssao: true, bloom: true, smaa: true, hdr: true, ambient: true, indirect: true, lights: None, point_map: None, dir_map: None, ssao_quality: None, filter: None, rooms: None, casters: None, core3d_shadows: false }
    }
}

/// Parses `POP_FX`; unknown tokens are returned for a warning.
pub fn parse_fx(v: &str) -> (Fx, Vec<String>) {
    let mut fx = Fx::default();
    let mut bad = Vec::new();
    for tok in v.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let (k, val) = tok.split_once('=').unwrap_or((tok, ""));
        let (k, off) = match k.strip_prefix("no_") {
            Some(k) => (k, true),
            None => (k, val == "off"),
        };
        let num = || val.parse::<usize>().ok();
        let ok = match k {
            "torch_shadows" if off => { fx.torch_shadows = false; true }
            "sun_shadow" if off => { fx.sun_shadow = false; true }
            "ssao" if off => { fx.ssao = false; true }
            "bloom" if off => { fx.bloom = false; true }
            "smaa" if off => { fx.smaa = false; true }
            "hdr" if off => { fx.hdr = false; true }
            "ambient" if off => { fx.ambient = false; true }
            "indirect" if off => { fx.indirect = false; true }
            "lights" => { fx.lights = num().filter(|&n| n <= 8); fx.lights.is_some() }
            "point_map" => { fx.point_map = num().filter(|n| n.is_power_of_two()); fx.point_map.is_some() }
            "dir_map" => { fx.dir_map = num().filter(|n| n.is_power_of_two()); fx.dir_map.is_some() }
            "ssao" if ["low", "medium", "high", "ultra"].contains(&val) => { fx.ssao_quality = Some(val.into()); true }
            "filter" if ["hw", "gauss", "temporal"].contains(&val) => { fx.filter = Some(val.into()); true }
            "rooms" => { fx.rooms = num().filter(|&n| n >= 1); fx.rooms.is_some() }
            "casters" if ["boxes", "none"].contains(&val) => { fx.casters = Some(val.into()); true }
            "core3d_shadows" => { fx.core3d_shadows = true; true }
            _ => false,
        };
        if !ok {
            bad.push(tok.to_string());
        }
    }
    (fx, bad)
}

/// The process-wide `POP_FX` toggles (read once).
pub fn fx() -> &'static Fx {
    static FX: std::sync::OnceLock<Fx> = std::sync::OnceLock::new();
    FX.get_or_init(|| {
        let (fx, bad) = parse_fx(&std::env::var("POP_FX").unwrap_or_default());
        if !bad.is_empty() {
            warn!("POP_FX: ignored {bad:?}");
        }
        fx
    })
}

/// `POP_BENCH=<frames>` (measured frames after the warm-up).
pub fn bench_frames() -> Option<usize> {
    std::env::var("POP_BENCH").ok()?.parse().ok().filter(|&n| n > 0)
}

/// GPU pass timing is only set up when the numbers are wanted at startup (it cannot be added later); `POP_GPU_TIMES=0`
/// turns it off for a benchmark.
pub fn gpu_timing() -> bool {
    std::env::var("POP_GPU_TIMES").map_or(std::env::var("POP_FPS").is_ok() || bench_frames().is_some(), |v| v != "0")
}

/// Top-level GPU pass times (ms, smoothed) recorded by `RenderDiagnosticsPlugin`, largest first.
fn gpu_passes(store: &DiagnosticsStore, avg: bool) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = store
        .iter()
        .filter_map(|d| {
            let p = d.path().as_str();
            let pass = p.strip_prefix("render/")?.strip_suffix("/elapsed_gpu")?;
            let ms = if avg { d.average() } else { d.smoothed() }?;
            (!pass.contains('/') || std::env::var("POP_GPU_ALL").is_ok()).then(|| (pass.to_string(), ms))
        })
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

/// `POP_FX=casters=boxes|none` (enhanced): new room meshes stop casting shadows, the voxel ones (anything but the shared
/// box mesh) or all of them; characters keep casting.
pub fn casters(mut cmds: Commands, mode: Res<RenderMode>, p: Option<Res<crate::room::Palette>>, q: Query<(Entity, &Mesh3d), (With<crate::room::RoomEntity>, Added<Mesh3d>)>) {
    let (Some(c), Some(p), RenderMode::Enhanced) = (fx().casters.as_deref(), p, *mode) else { return };
    for (e, m) in &q {
        if c == "none" || m.0 != p.cube {
            cmds.entity(e).insert(bevy::light::NotShadowCaster);
        }
    }
}

#[derive(Component)]
pub struct FpsText;

/// Rolling frame times for the overlay, filled only while it is on.
#[derive(Default)]
pub struct Rolling {
    ms: VecDeque<f32>,
    ema: f32,
}

const WINDOW: usize = 120;

/// F9 / `POP_FPS=1`: frame-time overlay in the top-right corner, above the HUD; refreshed every 10 frames.
#[allow(clippy::too_many_arguments)]
pub fn overlay(
    mut cmds: Commands,
    k: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mode: Res<RenderMode>,
    store: Option<Res<DiagnosticsStore>>,
    win: Query<&Window, With<PrimaryWindow>>,
    meshes: Query<&ViewVisibility, With<Mesh3d>>,
    points: Query<&PointLight, Without<crate::mirror::TwinOf>>,
    suns: Query<&DirectionalLight, Without<crate::mirror::TwinOf>>,
    mut text: Query<(&mut Text, &mut Visibility), With<FpsText>>,
    mut st: Local<(Option<bool>, Rolling, u32)>,
) {
    let on = st.0.get_or_insert_with(|| std::env::var("POP_FPS").is_ok_and(|v| v != "0"));
    if k.just_pressed(KeyCode::F9) {
        *on = !*on;
        for (_, mut v) in &mut text {
            *v = if *on { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
    if !*on {
        return;
    }
    if text.is_empty() {
        cmds.spawn((
            Text::new(""),
            TextFont::from_font_size(14.0),
            TextColor(Color::srgb(0.9, 1.0, 0.6)),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Node { position_type: PositionType::Absolute, right: Val::Px(8.0), top: Val::Px(8.0), padding: UiRect::all(Val::Px(4.0)), ..default() },
            GlobalZIndex(30),
            FpsText,
        ));
    }
    let (_, r, frame) = &mut *st;
    let dt = time.delta_secs() * 1000.0;
    r.ema = if r.ema == 0.0 { dt } else { r.ema * 0.95 + dt * 0.05 };
    r.ms.push_back(dt);
    if r.ms.len() > WINDOW {
        r.ms.pop_front();
    }
    *frame += 1;
    if *frame % 10 != 0 {
        return;
    }
    let s = stats(r.ms.make_contiguous());
    let res = win.single().map_or("?".into(), |w| format!("{}x{}", w.physical_width(), w.physical_height()));
    let visible = meshes.iter().filter(|v| v.get()).count();
    let shadow_points = points.iter().filter(|l| l.shadow_maps_enabled).count();
    let shadow_suns = suns.iter().filter(|l| l.shadow_maps_enabled).count();
    let mut out = format!(
        "{:.0} fps  {:.2} ms\n{WINDOW} frames: avg {:.2} min {:.2} max {:.2} 1% low {:.2} ms\n{:?} {res}\nmeshes {visible} visible / {} | lights {} | shadow casters: {shadow_points} point + {shadow_suns} sun",
        1000.0 / r.ema.max(0.001), r.ema, s.avg, s.min, s.max, s.low1, *mode, meshes.iter().len(), points.iter().len()
    );
    match store.as_deref().map(|s| gpu_passes(s, false)).filter(|p| !p.is_empty()) {
        Some(p) => {
            out += &format!("\nGPU {:.2} ms:", p.iter().map(|x| x.1).sum::<f64>());
            for (name, ms) in p.iter().take(8) {
                out += &format!("\n  {name} {ms:.2}");
            }
        }
        None => out += "\nGPU pass times: off (start with POP_FPS=1; needs TIMESTAMP_QUERY)",
    }
    for (mut t, _) in &mut text {
        t.0.clone_from(&out);
    }
}

/// `POP_BENCH=<frames>`: after `POP_BENCH_WARMUP` frames (default 500: shaders compile asynchronously) record that many
/// frame times, log one `BENCH ...` line (plus the averaged GPU pass times when available) and quit.
pub fn bench(time: Res<Time<Real>>, mode: Res<RenderMode>, style: Res<crate::settings::CharacterStyle>, store: Option<Res<DiagnosticsStore>>, win: Query<&Window, With<PrimaryWindow>>, mut exit: MessageWriter<AppExit>, mut st: Local<(usize, Vec<f32>, bool, usize)>) {
    let Some(n) = bench_frames() else { return };
    let warm = std::env::var("POP_BENCH_WARMUP").ok().and_then(|v| v.parse().ok()).unwrap_or(500);
    st.0 += 1;
    if st.0 <= warm || st.2 {
        return;
    }
    st.1.push(time.delta_secs() * 1000.0);
    // a background window is throttled by Windows (16.67 ms frames): the line counts the frames measured unfocused
    st.3 += !win.single().is_ok_and(|w| w.focused) as usize;
    if st.1.len() < n {
        return;
    }
    let s = stats(&st.1);
    let res = win.single().map_or("?".into(), |w| format!("{}x{}", w.physical_width(), w.physical_height()));
    let gpu = store.as_deref().map(|s| gpu_passes(s, true)).unwrap_or_default();
    let gpu_sum: f64 = gpu.iter().map(|x| x.1).sum();
    let passes: Vec<String> = gpu.iter().filter(|x| x.1 >= 0.005).map(|(p, ms)| format!("{p}={ms:.3}")).collect();
    info!(
        "BENCH render={:?} chars={:?} res={res} fx={} frames={n} avg_ms={:.3} p50={:.3} p95={:.3} p99={:.3} max={:.3} fps={:.1} unfocused_frames={} gpu_ms={gpu_sum:.3} gpu=[{}]",
        *mode, *style, std::env::var("POP_FX").unwrap_or_default().replace(' ', ""), s.avg, s.p50, s.p95, s.p99, s.max, 1000.0 / s.avg.max(0.001), st.3, passes.join(" ")
    );
    st.2 = true;
    exit.write(AppExit::Success);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_over_a_window() {
        let ms: Vec<f32> = (1..=200).map(|i| i as f32).collect();
        let s = stats(&ms);
        assert_eq!((s.min, s.max, s.p50, s.p95, s.p99), (1.0, 200.0, 100.0, 190.0, 198.0));
        assert!((s.avg - 100.5).abs() < 1e-4);
        // 1% of 200 = the two slowest frames
        assert_eq!(s.low1, 199.5);
        let one = stats(&[4.0]);
        assert_eq!((one.min, one.max, one.p99, one.low1, one.avg), (4.0, 4.0, 4.0, 4.0, 4.0));
        assert_eq!(stats(&[]), Stats::default());
    }

    #[test]
    fn res_and_fx_parse() {
        assert_eq!(parse_res("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_res(" 3840X2160 "), Some((3840, 2160)));
        assert_eq!(parse_res("1920"), None);
        assert_eq!(parse_res("10x10"), None);
        assert_eq!(parse_fx(""), (Fx::default(), vec![]));
        let (fx, bad) = parse_fx("no_ssao, bloom=off,lights=2,point_map=512,dir_map=1000,ssao=low,filter=hw,rooms=1,casters=boxes,warp");
        assert!(!fx.ssao && !fx.bloom && fx.torch_shadows && fx.hdr);
        assert_eq!((fx.lights, fx.point_map, fx.dir_map, fx.rooms), (Some(2), Some(512), None, Some(1)));
        assert_eq!((fx.ssao_quality.as_deref(), fx.filter.as_deref(), fx.casters.as_deref()), (Some("low"), Some("hw"), Some("boxes")));
        assert_eq!(bad, vec!["dir_map=1000".to_string(), "warp".to_string()]);
        let (fx, bad) = parse_fx("torch_shadows=off,no_sun_shadow,no_smaa,no_hdr,ambient=off,lights=0,no_indirect,core3d_shadows");
        assert!(bad.is_empty() && fx.core3d_shadows && !fx.indirect && !fx.torch_shadows && !fx.sun_shadow && !fx.smaa && !fx.hdr && !fx.ambient && fx.lights == Some(0));
    }
}
