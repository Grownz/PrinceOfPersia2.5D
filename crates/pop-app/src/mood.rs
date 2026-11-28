//! Enhanced-mode atmosphere (setting `atmosphere`): the time of day from the remaining level time (dusk -> deep night), a mood per
//! level (damp dungeon, golden palace), a red pulse in the last 5 minutes, and steel that gleams (metallic material + a small
//! environment cube map filtered on the GPU). The grade runs in the tonemapping shader (`ColorGrading`); the CPU only writes the
//! colour of the sun and of the ambient light, from constants and the time left (never from the current value, so nothing
//! drifts). The sun illuminance (enhanced.rs `sun_brightness`) and the ambient brightness (`apply`) are not touched here.
use crate::flow::Screen;
use crate::settings::{RenderMode, Settings};
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal};

/// Fraction of the 60 minutes left: 1 = fresh, 0 = out of time (720 logic ticks per minute, 12 per second).
pub fn time_frac(rem_min: i32, rem_tick: i32) -> f32 {
    let secs = rem_min as f32 * 60.0 + rem_tick as f32 / 12.0;
    (secs / 3600.0).clamp(0.0, 1.0)
}

/// What a grade sets: the ColorGrading values and the colour multipliers of sun and ambient light.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Grade {
    pub exposure: f32,
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub sun: [f32; 3],
    pub ambient: [f32; 3],
}

impl Grade {
    /// No change at all (title, cutscenes, the demo).
    pub const NEUTRAL: Grade = Grade { exposure: 0.0, temperature: 0.0, tint: 0.0, saturation: 1.0, sun: [1.0; 3], ambient: [1.0; 3] };

    fn then(self, o: Grade) -> Grade {
        let m = |a: [f32; 3], b: [f32; 3]| [a[0] * b[0], a[1] * b[1], a[2] * b[2]];
        Grade { exposure: self.exposure + o.exposure, temperature: self.temperature + o.temperature, tint: self.tint + o.tint, saturation: self.saturation * o.saturation, sun: m(self.sun, o.sun), ambient: m(self.ambient, o.ambient) }
    }
}

/// Dungeon: cool, a little desaturated, a little green-blue, damp.
const DUNGEON: Grade = Grade { exposure: 0.0, temperature: -0.02, tint: -0.01, saturation: 0.93, sun: [0.9, 1.0, 1.0], ambient: [0.9, 1.0, 1.05] };
/// Palace: warm and golden, a bit more saturated.
const PALACE: Grade = Grade { exposure: 0.0, temperature: 0.025, tint: 0.0, saturation: 1.06, sun: [1.0, 0.94, 0.8], ambient: [1.05, 0.98, 0.86] };
/// Time of day at the start (dusk) and at the end (deep night); `exposure` is the full drop in stops.
const DUSK: Grade = Grade { exposure: 0.0, temperature: 0.02, tint: 0.0, saturation: 1.0, sun: [1.0, 0.88, 0.74], ambient: [1.0, 0.95, 0.9] };
const NIGHT: Grade = Grade { exposure: -0.5, temperature: -0.08, tint: 0.0, saturation: 0.9, sun: [0.55, 0.68, 1.0], ambient: [0.72, 0.85, 1.12] };
/// Colour the red pulse multiplies in at full strength.
const RED: [f32; 3] = [1.0, 0.4, 0.38];
/// The pulse: last `PULSE_SECS` seconds, its peak strength at 0 s left, its rate (Hz) at the start and at the end.
const PULSE_SECS: f32 = 300.0;
const PULSE_MAX: f32 = 0.7;
const PULSE_HZ: (f32, f32) = (0.4, 1.6);

/// The level whose grade `level` gets: the demo (0) looks like level 1.
fn demo_as_dungeon(level: usize) -> usize {
    level.max(1)
}

/// The mood of a level: the palace levels are golden, the rest is dungeon; level 0 (the demo) has none of its own (`atmosphere` grades it as level 1).
pub fn mood(level: usize) -> Grade {
    match level {
        0 => Grade::NEUTRAL,
        l if crate::kid::palace(l) => PALACE,
        _ => DUNGEON,
    }
}

/// Seconds left -> strength 0..1 of the red pulse (0 before the last 5 minutes, growing to 1).
pub fn pulse_amp(secs_left: f32) -> f32 {
    (1.0 - secs_left / PULSE_SECS).clamp(0.0, 1.0)
}

/// Pulse rate in Hz: faster the less time is left.
pub fn pulse_hz(secs_left: f32) -> f32 {
    PULSE_HZ.0 + (PULSE_HZ.1 - PULSE_HZ.0) * pulse_amp(secs_left)
}

/// The grade of `level` with the fraction `f` of the time left; `red` 0..1 is the pulse's current brightness (already times its
/// strength). Pure: the same input always gives the same colours.
pub fn grade(level: usize, f: f32, red: f32) -> Grade {
    let n = 1.0 - f.clamp(0.0, 1.0);
    let k = n * n * (3.0 - 2.0 * n); // smoothstep: the first minutes hardly change, then the night comes in
    let l = |a: f32, b: f32| a + (b - a) * k;
    let l3 = |a: [f32; 3], b: [f32; 3]| [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2])];
    let day = Grade { exposure: l(DUSK.exposure, NIGHT.exposure), temperature: l(DUSK.temperature, NIGHT.temperature), tint: 0.0, saturation: l(DUSK.saturation, NIGHT.saturation), sun: l3(DUSK.sun, NIGHT.sun), ambient: l3(DUSK.ambient, NIGHT.ambient) };
    let r = red.clamp(0.0, 1.0);
    let pulse = Grade { exposure: 0.0, temperature: 0.25 * r, tint: 0.06 * r, saturation: 1.0, sun: [1.0, 1.0 + (RED[1] - 1.0) * r, 1.0 + (RED[2] - 1.0) * r], ambient: [1.0, 1.0 + (RED[1] - 1.0) * r, 1.0 + (RED[2] - 1.0) * r] };
    day.then(mood(level)).then(pulse)
}

/// Base colours the multipliers apply to: the sun is white, the ambient is enhanced.rs `enhanced_ambient` (sRGB).
const SUN_BASE: [f32; 3] = [1.0; 3];
const AMBIENT_BASE: [f32; 3] = [0.78, 0.76, 0.86];

fn colour(base: [f32; 3], m: [f32; 3]) -> Color {
    Color::srgb((base[0] * m[0]).min(1.0), (base[1] * m[1]).min(1.0), (base[2] * m[2]).min(1.0))
}

/// Metallic steel in enhanced + atmosphere; the classic look (non-metallic, roughness 0.6) otherwise.
const STEEL: (f32, f32) = (0.8, 0.35);
/// Intensity (cd/m2) of the environment map on the camera; low, so only the metal reflects it visibly.
const ENV_INTENSITY: f32 = 150.0;
const ENV_SIZE: u32 = 64;

/// Environment colour (linear, HDR) seen in direction `d` from the middle of a room: dark ceiling, a warm torch-lit band around
/// the walls, cool floor bounce and a few bright warm torch spots (the glints on the steel).
fn env_colour(d: Vec3) -> Vec3 {
    let up = d.y.max(0.0);
    let down = (-d.y).max(0.0);
    let band = (-((d.y - 0.05) / 0.35).powi(2)).exp();
    let mut c = Vec3::splat(0.012) * up + Vec3::new(0.9, 0.5, 0.22) * (0.30 * band) + Vec3::new(0.05, 0.07, 0.11) * down;
    for s in [Vec3::new(0.7, 0.3, 0.7), Vec3::new(-0.7, 0.3, 0.7), Vec3::new(0.0, 0.5, 1.0), Vec3::new(1.0, 0.2, 0.2), Vec3::new(-1.0, 0.2, 0.2), Vec3::new(0.0, 0.3, -1.0), Vec3::new(0.4, 0.7, -0.6)] {
        c += Vec3::new(6.0, 3.0, 0.9) * ((d.dot(s.normalize()) - 1.0) / 0.0144).exp();
    }
    c
}

/// Positive f32 -> f16 bits (the cube map is Rgba16Float; truncated, which is plenty for a light map).
fn f16_bits(x: f32) -> u16 {
    let b = x.clamp(0.0, 60000.0).to_bits();
    let e = (b >> 23) as i32 - 127 + 15;
    if e <= 0 { 0 } else { ((e as u16) << 10) | ((b >> 13) & 0x3ff) as u16 }
}

/// The cube map image, generated in code (the release is one exe, no asset files).
fn env_cube() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
    let n = ENV_SIZE;
    let mut px = Vec::with_capacity((6 * n * n * 8) as usize);
    for face in 0..6 {
        for y in 0..n {
            for x in 0..n {
                let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                let d = match face {
                    0 => Vec3::new(1.0, -v, -u),
                    1 => Vec3::new(-1.0, -v, u),
                    2 => Vec3::new(u, 1.0, v),
                    3 => Vec3::new(u, -1.0, -v),
                    4 => Vec3::new(u, -v, 1.0),
                    _ => Vec3::new(-u, -v, -1.0),
                };
                let c = env_colour(d.normalize());
                for ch in [c.x, c.y, c.z, 1.0] {
                    px.extend(f16_bits(ch).to_le_bytes());
                }
            }
        }
    }
    Image { texture_view_descriptor: Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() }), ..Image::new(Extent3d { width: n, height: n, depth_or_array_layers: 6 }, TextureDimension::D2, px, TextureFormat::Rgba16Float, RenderAssetUsages::RENDER_WORLD) }
}

#[derive(Default)]
pub struct MoodState {
    /// the atmosphere is applied now (so leaving it resets the camera once)
    on: bool,
    /// pulse phase in cycles: integrated, so the rate can change without the brightness jumping
    phase: f32,
    cube: Option<Handle<Image>>,
    steel: Option<bool>,
}

/// Every frame (enhanced only): grade, sun / ambient colour and the steel gleam; resets them once when leaving.
#[allow(clippy::too_many_arguments)]
pub fn atmosphere(
    mode: Res<RenderMode>,
    set: Res<Settings>,
    screen: Res<State<Screen>>,
    g: Res<crate::Game>,
    time: Res<Time>,
    palette: Res<crate::room::Palette>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut cmds: Commands,
    mut cam: Query<(Entity, &mut AmbientLight, Option<&mut ColorGrading>), (With<Camera3d>, Without<crate::mirror::MirrorCam>)>,
    mut sun: Query<&mut DirectionalLight, Without<crate::mirror::TwinOf>>,
    mut st: Local<MoodState>,
) {
    let want = *mode == RenderMode::Enhanced && set.atmosphere;
    let Ok((cam_e, mut amb, grading)) = cam.single_mut() else { return };
    if want != st.steel.unwrap_or(false) {
        st.steel = Some(want);
        if let Some(mut m) = palette.steel().and_then(|h| mats.get_mut(h)) {
            (m.metallic, m.perceptual_roughness) = if want { STEEL } else { (0.0, 0.6) };
        }
    }
    if !want {
        if st.on {
            st.on = false;
            cmds.entity(cam_e).insert(ColorGrading::default()).remove::<(GeneratedEnvironmentMapLight, bevy::light::EnvironmentMapLight)>();
            // classic: apply() has put the classic ambient back, but not the sun's white
            for mut l in &mut sun {
                l.color = colour(SUN_BASE, [1.0; 3]);
            }
            if *mode == RenderMode::Enhanced {
                amb.color = colour(AMBIENT_BASE, [1.0; 3]);
            }
        }
        return;
    }
    if !st.on {
        st.on = true;
        let cube = st.cube.get_or_insert_with(|| imgs.add(env_cube())).clone();
        cmds.entity(cam_e).insert(GeneratedEnvironmentMapLight { environment_map: cube, intensity: ENV_INTENSITY, ..default() });
    }
    // the demo (level 0) is graded like level 1, the dungeon it leads into (no timer pulse: it has no time limit)
    let level = matches!(screen.get(), Screen::Playing | Screen::Paused).then_some(demo_as_dungeon(g.level));
    let secs = g.world.progress.rem_min as f32 * 60.0 + g.world.progress.rem_tick as f32 / 12.0;
    let red = match level {
        Some(_) if g.level != 0 && pulse_amp(secs) > 0.0 => {
            st.phase = (st.phase + time.delta_secs() * pulse_hz(secs)).fract();
            (0.5 - 0.5 * (st.phase * std::f32::consts::TAU).cos()) * pulse_amp(secs) * PULSE_MAX
        }
        _ => 0.0,
    };
    let gr = level.map_or(Grade::NEUTRAL, |l| grade(l, time_frac(g.world.progress.rem_min, g.world.progress.rem_tick), red));
    let cg = ColorGrading { global: ColorGradingGlobal { exposure: gr.exposure, temperature: gr.temperature, tint: gr.tint, post_saturation: gr.saturation, ..default() }, ..default() };
    match grading {
        Some(mut c) => {
            if *c != cg {
                *c = cg;
            }
        }
        None => {
            cmds.entity(cam_e).insert(cg);
        }
    }
    let (sc, ac) = (colour(SUN_BASE, gr.sun), colour(AMBIENT_BASE, gr.ambient));
    if amb.color != ac {
        amb.color = ac;
    }
    for mut l in &mut sun {
        if l.color != sc {
            l.color = sc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_is_graded_like_the_dungeon() {
        assert_eq!((demo_as_dungeon(0), demo_as_dungeon(1), demo_as_dungeon(7)), (1, 1, 7));
        assert_eq!(grade(demo_as_dungeon(0), 1.0, 0.0).exposure, grade(1, 1.0, 0.0).exposure);
    }

    #[test]
    fn time_fraction_runs_from_one_to_zero() {
        assert_eq!(time_frac(60, 719), 1.0);
        assert_eq!(time_frac(0, 0), 0.0);
        assert!((time_frac(30, 0) - 0.5).abs() < 1e-6);
        assert_eq!(time_frac(-1, 0), 0.0);
        assert!(time_frac(10, 100) > time_frac(10, 99));
    }

    #[test]
    fn dungeon_is_cool_and_palace_warm() {
        let (d, p) = (mood(1), mood(4));
        assert!(d.temperature < 0.0 && p.temperature > 0.0 && d.saturation < 1.0 && p.saturation > 1.0);
        assert!(d.sun[0] < d.sun[2] && p.sun[0] > p.sun[2]);
        for l in [4, 5, 6, 10, 11, 14] {
            assert_eq!(mood(l), PALACE);
        }
        for l in [1, 2, 3, 7, 8, 9, 12, 13] {
            assert_eq!(mood(l), DUNGEON);
        }
        assert_eq!(mood(0), Grade::NEUTRAL);
    }

    #[test]
    fn night_is_darker_cooler_and_bluer_than_dusk() {
        let (dusk, night) = (grade(1, 1.0, 0.0), grade(1, 0.0, 0.0));
        assert!(night.exposure < dusk.exposure && night.exposure >= -0.55);
        assert!(night.temperature < dusk.temperature);
        assert!(night.sun[2] / night.sun[0] > dusk.sun[2] / dusk.sun[0]);
        assert!(night.ambient[2] / night.ambient[0] > dusk.ambient[2] / dusk.ambient[0]);
    }

    #[test]
    fn red_pulse_only_in_the_last_five_minutes_and_faster_at_the_end() {
        assert_eq!(pulse_amp(301.0), 0.0);
        assert_eq!(pulse_amp(3600.0), 0.0);
        assert!(pulse_amp(299.0) > 0.0 && pulse_amp(10.0) > pulse_amp(200.0) && pulse_amp(0.0) == 1.0);
        assert!(pulse_hz(10.0) > pulse_hz(250.0) && pulse_hz(900.0) == PULSE_HZ.0);
        let (calm, red) = (grade(1, 0.5, 0.0), grade(1, 0.5, 1.0));
        assert!(red.sun[1] < calm.sun[1] && red.ambient[2] < calm.ambient[2] && red.temperature > calm.temperature);
    }

    #[test]
    fn colours_are_constant_per_time() {
        for f in [0.0, 0.3, 1.0] {
            assert_eq!(grade(5, f, 0.2), grade(5, f, 0.2));
        }
        assert_ne!(grade(5, 0.2, 0.0), grade(5, 0.8, 0.0));
    }

    #[test]
    fn environment_cube_is_6_faces_of_f16() {
        assert_eq!(env_cube().data.map(|d| d.len()), Some((6 * ENV_SIZE * ENV_SIZE * 8) as usize));
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(0.0), 0);
    }
}
