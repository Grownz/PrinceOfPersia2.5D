//! Cinematic fights, blood (option `cinematic_fights`, enhanced modes only). A sword hit (`World::hits`) sprays droplets from the
//! attacker's blade tip away from the victim; they fly on ballistic paths and stay on the floor as flat spots. A killed figure
//! also gets a pool that grows once the body lies still. Every landed hit makes the attacker's sword bloodier (0 clean, 1
//! flecked, 2 covered; `kid::bloody` paints it), a sheathed sword is clean again.
//! Limit: blood belongs to the shown room and is gone when the room (or level) changes.
use crate::particles::Swords;
use crate::settings::{RenderMode, Settings};
use crate::Game;
use bevy::prelude::*;
use pop_core::level::TILE_H_PX;

/// What `kid::render` knows for the blood: each fighter's drawn image centre x and bottom y (kid [0], guard [1]) and the
/// blood level of their swords.
#[derive(Resource, Default)]
pub struct Gore {
    pub bodies: [Option<Vec2>; 2],
    pub sword: [u8; 2],
    shared: Option<Shared>,
}

const GRAVITY: f32 = 8.0;
const DROPS: usize = 14;
const DROPS_KILL: usize = 26;
/// Most droplets and spots alive at once; the oldest go first.
const MAX_DROPS: usize = 500;
/// Final radius of a pool (x; it is squashed in depth) and its time constant in seconds.
const POOL_R: f32 = 0.34;
const POOL_TAU: f32 = 2.5;

#[derive(Component)]
pub struct Blood;

/// A droplet in the air (`landed` false) or a spot on the floor.
#[derive(Component)]
pub struct Drop {
    vel: Vec3,
    floor: f32,
    born: f32,
    size: f32,
    landed: bool,
}

/// A pool under a killed figure (`guard`: the guard, else the kid). It follows the falling body until that lies still.
#[derive(Component)]
pub struct Pool {
    guard: bool,
    floor: f32,
    dead_for: Option<f32>,
}

fn rnd(s: &mut u32) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    (*s >> 8) as f32 / (1u32 << 24) as f32
}

/// Blood radius of a pool `t` seconds after the body came to rest: grows fast, then slowly (exponential approach).
fn pool_radius(t: f32) -> f32 {
    POOL_R * (1.0 - (-t / POOL_TAU).exp())
}

/// World y of the feet of a figure at original y `y` (row 2 floor = 0).
fn floor_of(y: u8) -> f32 {
    (181.0 - y as f32) * crate::room::TILE_H / TILE_H_PX as f32
}

#[derive(Clone)]
struct Shared {
    cube: Handle<Mesh>,
    disc: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

/// Takes the sword hits of the world (also when the option is off, so the list cannot pile up), spawns the blood and keeps
/// the swords' blood level.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    mut cmds: Commands,
    time: Res<Time>,
    set: Res<Settings>,
    mode: Res<RenderMode>,
    mut g: ResMut<Game>,
    mut gore: ResMut<Gore>,
    swords: Res<Swords>,
    mut seen: Local<(usize, u32)>,
    old: Query<Entity, With<Blood>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let hits = std::mem::take(&mut g.world.hits);
    if !set.cinematic_fights || *mode == RenderMode::Classic {
        gore.sword = [0; 2];
        for e in &old {
            cmds.entity(e).despawn();
        }
        return;
    }
    if *seen != (g.room, g.gen) {
        *seen = (g.room, g.gen);
        for e in &old {
            cmds.entity(e).despawn();
        }
    }
    if g.world.kid.sword == 0 {
        gore.sword[0] = 0;
    }
    if g.world.guard.as_ref().is_none_or(|k| k.sword == 0) {
        gore.sword[1] = 0;
    }
    let sh = gore
        .shared
        .get_or_insert_with(|| Shared {
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            disc: meshes.add(Circle::new(0.5)),
            mat: mats.add(StandardMaterial { base_color: Color::srgb(0.42, 0.015, 0.03), perceptual_roughness: 0.2, ..default() }),
        })
        .clone();
    let mut s = (time.elapsed_secs_f64().to_bits() as u32) | 1;
    for h in hits {
        if h.room as usize != g.room {
            continue;
        }
        let (att, vic) = ((!h.guard) as usize, h.guard as usize);
        let dir = if h.from_left { 1.0 } else { -1.0 }; // the spray goes away from the attacker
        let at = match (swords.0[att], gore.bodies[vic]) {
            (Some((c, size)), _) => Vec3::new(c.x + dir * size.x.abs() / 2.0, c.y, 0.4),
            (None, Some(b)) => Vec3::new(b.x, b.y + 0.6, 0.4),
            (None, None) => continue,
        };
        let floor = floor_of(h.y);
        for _ in 0..if h.dead { DROPS_KILL } else { DROPS } {
            let vel = Vec3::new(dir * (0.2 + 2.0 * rnd(&mut s)) - 0.3 * dir * rnd(&mut s), 0.4 + 2.2 * rnd(&mut s), (rnd(&mut s) - 0.5) * 0.8);
            let size = 0.02 + 0.03 * rnd(&mut s);
            cmds.spawn((Blood, Drop { vel, floor, born: time.elapsed_secs(), size, landed: false }, Mesh3d(sh.cube.clone()), MeshMaterial3d(sh.mat.clone()), Transform::from_translation(at).with_scale(Vec3::splat(size)), bevy::light::NotShadowCaster));
        }
        if h.dead {
            let x = gore.bodies[vic].map_or(at.x, |b| b.x);
            cmds.spawn((Blood, Pool { guard: h.guard, floor, dead_for: None }, Mesh3d(sh.disc.clone()), MeshMaterial3d(sh.mat.clone()), Transform::from_xyz(x, floor + 0.008, 0.25).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::ZERO), bevy::light::NotShadowCaster));
        }
        gore.sword[att] = (gore.sword[att] + 1).min(2);
    }
}

/// Moves the droplets, flattens them where they land, grows the pools, and drops the oldest spots over `MAX_DROPS`.
pub fn motion(
    mut cmds: Commands,
    time: Res<Time>,
    g: Res<Game>,
    gore: Res<Gore>,
    mut drops: Query<(Entity, &mut Drop, &mut Transform, &mut Mesh3d), Without<Pool>>,
    mut pools: Query<(&mut Pool, &mut Transform), Without<Drop>>,
) {
    let dt = time.delta_secs();
    let Some(sh) = gore.shared.as_ref() else { return };
    for (_, mut d, mut t, mut m) in &mut drops {
        if d.landed {
            continue;
        }
        d.vel.y -= GRAVITY * dt;
        t.translation += d.vel * dt;
        if t.translation.y <= d.floor {
            t.translation.y = d.floor + 0.004;
            d.landed = true;
            m.0 = sh.disc.clone();
            t.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
            t.scale = Vec3::splat(d.size * 2.5);
        }
    }
    if drops.iter().len() > MAX_DROPS {
        let mut all: Vec<(f32, Entity)> = drops.iter().map(|(e, d, _, _)| (d.born, e)).collect();
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, e) in all.iter().take(all.len() - MAX_DROPS) {
            cmds.entity(*e).despawn();
        }
    }
    for (mut p, mut t) in &mut pools {
        let k = if p.guard { g.world.guard.as_ref() } else { Some(&g.world.kid) };
        let still = k.is_none_or(|k| k.is_dead());
        if p.dead_for.is_none() && still {
            p.dead_for = Some(0.0);
        }
        match &mut p.dead_for {
            Some(s) => *s += dt,
            None => {
                // the body is still falling: the pool waits under it
                if let (Some(b), Some(k)) = (gore.bodies[p.guard as usize], k) {
                    t.translation.x = b.x;
                    p.floor = floor_of(k.y);
                    t.translation.y = p.floor + 0.008;
                }
            }
        }
        let r = p.dead_for.map_or(0.0, pool_radius);
        t.scale = Vec3::new(r * 2.0, r * 2.0 * 0.6, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pool_grows_slowly_to_its_size() {
        assert_eq!(pool_radius(0.0), 0.0);
        assert!(pool_radius(1.0) > 0.0 && pool_radius(1.0) < pool_radius(5.0));
        assert!(pool_radius(60.0) <= POOL_R && pool_radius(60.0) > 0.99 * POOL_R);
    }
}
