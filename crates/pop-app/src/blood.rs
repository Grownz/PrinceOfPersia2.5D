//! Cinematic fights, blood (option `cinematic_fights`, enhanced modes only). A sword hit (`World::hits`) sprays droplets from the
//! attacker's blade tip away from the victim; they fly on ballistic paths and stay on the floor as flat spots. A killed figure
//! also gets a pool that grows once the body lies still. Every landed hit makes the attacker's sword bloodier (0 clean, 1
//! flecked, 2 covered; `kid::bloody` paints it), a sheathed sword is clean again.
//! Limit: droplets belong to the shown room and are gone when it changes. With `atmosphere` on (enhanced) the pools and one splatter
//! per hit are handed to traces.rs instead, which keeps them for the level (the pool disc here then stays invisible).
use crate::particles::Swords;
use crate::settings::{RenderMode, Settings};
use crate::traces::{Stain, Stains};
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

use crate::room::{SLAB_BACK, SLAB_FRONT};
/// Spray origin depth: the middle of the floor tiles (z -0.95..0.5), so the drops fall on them, not past the front lip.
const SPRAY_Z: f32 = (SLAB_BACK + SLAB_FRONT) / 2.0;
const GRAVITY: f32 = 8.0;
const DROPS: usize = 14;
const DROPS_KILL: usize = 26;
/// Most droplets and spots alive at once; the oldest go first.
const MAX_DROPS: usize = 500;
/// Final radius of a pool (x; it is squashed in depth) and its time constant in seconds.
const POOL_R: f32 = 0.34;
const POOL_TAU: f32 = 2.5;
/// Radius of a pool that stays (traces.rs): wider than the body that lies on it.
const KEPT_POOL_R: f32 = 0.5;

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
    /// persistent stains (traces.rs): the pool has been handed over
    recorded: bool,
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
pub fn floor_of(y: u8) -> f32 {
    (181.0 - y as f32) * crate::room::TILE_H / TILE_H_PX as f32
}

/// Whether a drop landing at world `x`, floor height `floor` hits a rendered floor tile of the shown room (not a gap, a wall
/// block, a balcony or another room); the rest of the spray is gone.
pub(crate) fn on_floor(level: &pop_core::level::Level, room: usize, x: f32, floor: f32) -> bool {
    use pop_core::level::Tile::*;
    let (col, row) = (x.floor(), 2 - (floor / crate::room::TILE_H).round() as i32);
    if !(0.0..10.0).contains(&col) || !(0..3).contains(&row) || !(1..=pop_core::level::ROOMS).contains(&room) {
        return false;
    }
    let t = level.tile_at(room, row as usize * 10 + col as usize).kind;
    !matches!(t, Empty | BigPillarTop | Doortop | Wall | LatticeSupport | SmallLattice | LatticeLeft | LatticeRight | BalconyLeft | BalconyRight | Spikes)
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
    mut stains: ResMut<Stains>,
) {
    let persist = crate::traces::persistent(&mode, &set);
    stains.sync_world((g.level, g.gen));
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
            (Some((c, size)), _) => Vec3::new(c.x + dir * size.x.abs() / 2.0, c.y, SPRAY_Z),
            (None, Some(b)) => Vec3::new(b.x, b.y + 0.6, SPRAY_Z),
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
            let tf = Transform::from_xyz(x, floor + 0.008, 0.25).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::ZERO);
            let pool = Pool { guard: h.guard, floor, dead_for: None, recorded: false };
            if persist {
                // the pool is drawn by traces.rs once the body lies still; this entity only waits under the body
                cmds.spawn((Blood, pool, tf));
            } else {
                cmds.spawn((Blood, pool, Mesh3d(sh.disc.clone()), MeshMaterial3d(sh.mat.clone()), tf, bevy::light::NotShadowCaster));
            }
        } else if persist {
            // a splatter that stays: away from the attacker, a little in front of or behind the victim
            let x = gore.bodies[vic].map_or(at.x, |b| b.x) + dir * (0.25 + 0.6 * rnd(&mut s));
            if on_floor(&g.world.level, g.room, x, floor) {
                let z = -0.4 + 0.7 * rnd(&mut s);
                stains.add(Stain { room: g.room as u8, x, floor, z, r: 0.11 + 0.08 * rnd(&mut s), seed: (rnd(&mut s) * 1000.0) as u32, tick: g.world.tick, pool: false });
            }
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
    mut stains: ResMut<Stains>,
    mode: Res<RenderMode>,
    set: Res<Settings>,
) {
    let persist = crate::traces::persistent(&mode, &set);
    let dt = time.delta_secs();
    let Some(sh) = gore.shared.as_ref() else { return };
    for (e, mut d, mut t, mut m) in &mut drops {
        if d.landed {
            continue;
        }
        d.vel.y -= GRAVITY * dt;
        t.translation += d.vel * dt;
        if t.translation.y <= d.floor {
            if !on_floor(&g.world.level, g.room, t.translation.x, d.floor) {
                cmds.entity(e).despawn();
                continue;
            }
            t.translation.y = d.floor + 0.004;
            t.translation.z = t.translation.z.clamp(SLAB_BACK + 0.05, SLAB_FRONT - 0.05);
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
        if persist && !p.recorded && p.dead_for.is_some() {
            p.recorded = true;
            if on_floor(&g.world.level, g.room, t.translation.x, p.floor) {
                stains.add(Stain { room: g.room as u8, x: t.translation.x, floor: p.floor, z: 0.15, r: KEPT_POOL_R, seed: (t.translation.x * 997.0) as u32 ^ g.world.tick as u32, tick: g.world.tick, pool: true });
            }
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
