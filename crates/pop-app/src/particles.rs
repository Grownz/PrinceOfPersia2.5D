//! Sparks where two swords meet: point-sized, glowing white-yellow particles fly radially out of the contact point on
//! ballistic paths, land on the floor the prince stands on and burn out within 2 s. Triggered by the sword-clash sound
//! (id 10, `audio::fixed`). In enhanced mode every spark is also a small light, as strong as a torch together at the start,
//! fading exponentially.
use crate::kid::{GuardSprite, KidSprite};
use crate::settings::RenderMode;
use crate::Game;
use bevy::prelude::*;
use pop_core::level::TILE_H_PX;

/// The drawn swords (centre, size in world units) of the kid [0] and the guard [1], written by `kid::render`.
#[derive(Resource, Default)]
pub struct Swords(pub [Option<(Vec2, Vec2)>; 2]);

/// Where the blades cross: the middle of the two sword pictures' overlap (their middle if they do not quite touch).
fn crossing(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> Vec2 {
    let lo = (a.0 - a.1.abs() / 2.0).max(b.0 - b.1.abs() / 2.0);
    let hi = (a.0 + a.1.abs() / 2.0).min(b.0 + b.1.abs() / 2.0);
    if lo.x <= hi.x && lo.y <= hi.y { (lo + hi) / 2.0 } else { (a.0 + b.0) / 2.0 }
}

/// Set by `audio::fixed` when the clash sound plays; consumed by `clash_sparks`.
#[derive(Resource, Default)]
pub struct Clash(pub bool);

const SPARKS: usize = 20;
const LIFE: f32 = 2.0;
/// Gravity in world units / s² (the kid is about 1.5 units tall, i.e. roughly 1.8 m: 9.8 m/s² is about 8).
const GRAVITY: f32 = 8.0;
/// The prince's sprite height (a fencing stance is about 47 px of 63 per 2 units; estimated). Sparks rise at most half of it.
const KID_H: f32 = 1.5;
/// Edge of a spark: half a figure voxel (1/32 wide), i.e. an eighth of its volume.
const SIZE: f32 = 1.0 / 64.0;
/// A torch's light (`room::torch_light`) shared by all sparks at the start; the decay per `LIFE` is e^-DECAY.
const DECAY: f32 = 5.0;

#[derive(Component)]
pub struct Spark {
    vel: Vec3,
    age: f32,
    floor: f32,
    mat: Handle<StandardMaterial>,
}

/// Tiny xorshift: 0..1 per call (sparks need no quality randomness).
fn rnd(s: &mut u32) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    (*s >> 8) as f32 / (1u32 << 24) as f32
}

/// The fastest launch speed whose highest point (straight up) is half the prince's sprite height: v² / 2g = KID_H / 2.
pub fn max_speed() -> f32 {
    (GRAVITY * KID_H).sqrt()
}

#[allow(clippy::too_many_arguments)]
pub fn clash_sparks(
    mut clash: ResMut<Clash>,
    mut cmds: Commands,
    mut meshes: Local<Option<Handle<Mesh>>>,
    mut assets: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    g: Res<Game>,
    swords: Res<Swords>,
    kid: Query<&Transform, With<KidSprite>>,
    guard: Query<&Transform, (With<GuardSprite>, Without<KidSprite>)>,
) {
    if !std::mem::take(&mut clash.0) {
        return;
    }
    let (Ok(k), Ok(gd)) = (kid.single(), guard.single()) else { return };
    let mid = (k.translation + gd.translation) / 2.0;
    let at = match swords.0 {
        [Some(a), Some(b)] => crossing(a, b).extend(mid.z + 0.15),
        _ => mid + Vec3::new(0.0, 0.0, 0.15),
    };
    // the floor the prince stands on: world y of his feet (row 2 floor = 0)
    let floor = (181.0 - g.world.kid.y as f32) * crate::room::TILE_H / TILE_H_PX as f32;
    let vmax = max_speed();
    let mesh = meshes.get_or_insert_with(|| assets.add(Cuboid::new(SIZE, SIZE, SIZE))).clone();
    let mut s = (time.elapsed_secs_f64().to_bits() as u32) | 1;
    for _ in 0..SPARKS {
        let (a, v) = (rnd(&mut s) * std::f32::consts::TAU, vmax * (0.35 + 0.65 * rnd(&mut s)));
        let vel = Vec3::new(a.cos() * v, a.sin() * v, (rnd(&mut s) - 0.5) * 0.6);
        let mat = mats.add(StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
        let light = PointLight { color: Color::srgb(1.0, 0.8, 0.45), intensity: 0.0, range: 3.0, shadow_maps_enabled: false, ..default() };
        cmds.spawn((Spark { vel, age: 0.0, floor, mat: mat.clone() }, Mesh3d(mesh.clone()), MeshMaterial3d(mat), light, Transform::from_translation(at), bevy::light::NotShadowCaster));
    }
}

pub fn sparks(mut cmds: Commands, time: Res<Time>, mode: Res<RenderMode>, mut mats: ResMut<Assets<StandardMaterial>>, mut q: Query<(Entity, &mut Spark, &mut Transform, &mut PointLight)>) {
    let dt = time.delta_secs();
    let base = crate::room::torch_light().intensity * crate::enhanced::ENHANCED_TORCH_GAIN / SPARKS as f32;
    for (e, mut s, mut t, mut l) in &mut q {
        s.age += dt;
        if s.age >= LIFE {
            cmds.entity(e).despawn();
            continue;
        }
        if t.translation.y > s.floor {
            s.vel.y -= GRAVITY * dt;
            t.translation += s.vel * dt;
        }
        if t.translation.y <= s.floor {
            t.translation.y = s.floor;
            s.vel = Vec3::ZERO; // it lies where it fell
        }
        // brightness fades exponentially (light and glow), the point itself goes out linearly; white-yellow, overbright
        let life = s.age / LIFE;
        let f = (-DECAY * life).exp();
        l.intensity = if *mode == RenderMode::Enhanced { base * f } else { 0.0 };
        if let Some(mut m) = mats.get_mut(&s.mat) {
            m.base_color = Color::linear_rgba(1.0 + 8.0 * f, 0.8 + 6.0 * f, 0.4 + 2.0 * f, 1.0 - life);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_up_at_top_speed_peaks_at_half_the_sprite_height() {
        let v = max_speed();
        assert!((v * v / (2.0 * GRAVITY) - KID_H / 2.0).abs() < 1e-4);
    }
}
