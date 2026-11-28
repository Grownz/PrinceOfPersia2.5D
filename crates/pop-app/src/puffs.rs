//! Dust puffs and impact feel (rides on the setting `atmosphere`, enhanced mode).
//! Puffs: events the sim already has (`World::impacts`: loose floor lets go / crashes, gate slam, chomper; the kid's landing
//! sequences) spawn one small billboard quad each, drawn by shaders/puff.wesl as an fbm cloud that swells and fades in about 0.6 to
//! 0.9 s (a gate slam: two sprays along the floor), tinted by the nearest torch or the sun. Twelve pooled entities, hidden when unused;
//! the age is the shader clock minus the spawn time in the uniform, so nothing runs per frame but one check of the oldest puff.
//! Shake: a hard landing, a loose floor crashing near the kid and a gate slam kick the camera by a few pixels for 0.25 s
//! (`Shake`, applied additively by `camera::transform`; the offset is a pure function of the time since the kick, so it never drifts).
//! No hit-stop: sword hits do not freeze anything.
use crate::settings::{RenderMode, Settings};
use crate::Game;
use bevy::asset::embedded_asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use pop_core::level::COLS;
use pop_core::sim::ImpactKind;

/// Live puffs at most (pooled entities).
pub const POOL: usize = 12;
/// Puffs stand this far in front of the room's back wall plane (the figures are at 0.25).
const PUFF_Z: f32 = 0.3;
pub const SHAKE_SECS: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PuffKind {
    Soft,
    Hard,
    Fatal,
    Fall,
    Crash,
    Slam,
    Chomp,
}

/// Size (world units, the quad), strength (alpha), life (seconds), spray along the floor, camera kick (world units, at 1280x720 about
/// 125 px per unit).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spec {
    pub size: Vec2,
    pub strength: f32,
    pub life: f32,
    pub spray: bool,
    pub shake: f32,
}

pub fn spec(k: PuffKind) -> Spec {
    let s = |w, h, strength, life, spray, shake| Spec { size: Vec2::new(w, h), strength, life, spray, shake };
    match k {
        PuffKind::Soft => s(0.9, 0.7, 0.4, 0.6, false, 0.0),
        PuffKind::Hard => s(1.5, 1.2, 0.65, 0.75, false, 0.025),
        PuffKind::Fatal => s(1.9, 1.5, 0.75, 0.85, false, 0.035),
        PuffKind::Fall => s(1.0, 0.8, 0.4, 0.7, false, 0.0),
        PuffKind::Crash => s(2.5, 2.0, 0.85, 0.9, false, 0.03),
        PuffKind::Slam => s(3.4, 0.9, 0.65, 0.7, true, 0.015),
        PuffKind::Chomp => s(1.0, 0.8, 0.4, 0.6, false, 0.0),
    }
}

/// The puff of a kid's landing sequence (17 soft, 20 hard with damage, 22 fatal, 63 soft with the sword drawn).
pub fn landing(seq: usize) -> Option<PuffKind> {
    match seq {
        17 | 63 => Some(PuffKind::Soft),
        20 => Some(PuffKind::Hard),
        22 => Some(PuffKind::Fatal),
        _ => None,
    }
}

pub fn of_impact(k: ImpactKind) -> PuffKind {
    match k {
        ImpactKind::LooseFall => PuffKind::Fall,
        ImpactKind::Crash => PuffKind::Crash,
        ImpactKind::GateSlam => PuffKind::Slam,
        ImpactKind::Chomp => PuffKind::Chomp,
    }
}

/// Camera kick of a puff `dist` world units from the kid (full strength up to 2, none from 7; the kid's own landing has dist 0).
pub fn kick(k: PuffKind, dist: f32) -> f32 {
    spec(k).shake * (1.0 - (dist - 2.0) / 5.0).clamp(0.0, 1.0)
}

/// Where a puff happened.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum At {
    Kid,
    Tile(u8, u8),
}

/// Events collected on the logic ticks, spawned (and shown) on the next frame.
#[derive(Resource, Default)]
pub struct Pending(Vec<(PuffKind, At)>);

/// Camera shake: the biggest kick still going and the seconds since it started.
#[derive(Resource, Default)]
pub struct Shake {
    amp: f32,
    t: f32,
}

impl Shake {
    /// A kick of `amp`: it replaces the current one only when it is stronger than what is left of it.
    pub fn kick(&mut self, amp: f32) {
        if amp > self.amp * shake_decay(self.t) {
            *self = Shake { amp, t: 0.0 };
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.t += dt;
        if self.t >= SHAKE_SECS {
            *self = Shake::default();
        }
    }

    pub fn offset(&self) -> Vec2 {
        shake_offset(self.amp, self.t)
    }
}

/// 1 at the kick, 0 at `SHAKE_SECS` (quadratic ease-out).
pub fn shake_decay(t: f32) -> f32 {
    let k = (1.0 - t / SHAKE_SECS).clamp(0.0, 1.0);
    k * k
}

/// The camera offset `t` seconds after a kick of `amp`: damped wobble, mostly vertical, exactly zero before and from `SHAKE_SECS` on.
pub fn shake_offset(amp: f32, t: f32) -> Vec2 {
    if !(0.0..SHAKE_SECS).contains(&t) {
        return Vec2::ZERO;
    }
    amp * shake_decay(t) * Vec2::new(0.45 * (t * 83.0).sin(), (t * 61.0).sin())
}

/// Dust colour: the grey-brown dust lit by the sun's colour (dim ambient plus the sun) and, near a torch (`torch` 0..1), the torch's glow.
pub fn tint(sun: Vec3, torch: f32) -> Vec3 {
    Vec3::new(0.62, 0.56, 0.5) * (sun * 0.18 + Vec3::splat(0.1)) + Vec3::new(1.0, 0.6, 0.3) * torch * 0.55
}

#[derive(Clone, Copy, Default, ShaderType)]
pub struct PuffU {
    tint: Vec4,
    p: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct PuffMaterial {
    #[uniform(0)]
    u: PuffU,
}

impl Material for PuffMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(bevy::asset::AssetPath::parse("embedded://pop_app/shaders/puff.wesl"))
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

/// The pooled puffs: entity, material, spawn time and life of each; `next` is where the next one goes (the oldest), `tail` the oldest
/// that may still be showing.
#[derive(Resource)]
pub struct Puffs {
    slots: Vec<(Entity, Handle<PuffMaterial>)>,
    born: [f32; POOL],
    life: [f32; POOL],
    next: usize,
    tail: usize,
    live: usize,
}

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<PuffMaterial>>) {
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let slots = (0..POOL)
        .map(|_| {
            let m = mats.add(PuffMaterial { u: PuffU::default() });
            let e = cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d(m.clone()), Transform::default(), Visibility::Hidden, bevy::light::NotShadowCaster)).id();
            (e, m)
        })
        .collect();
    cmds.insert_resource(Puffs { slots, born: [0.0; POOL], life: [0.0; POOL], next: 0, tail: 0, live: 0 });
}

fn wanted(mode: &RenderMode, set: &Settings) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere
}

/// FixedUpdate, after the world step: takes the sim's tile impacts (always, so the list cannot pile up) and the kid's landing, for the
/// next frame. The landing is the kid's sequence changing into a landing one.
pub fn collect(mut g: ResMut<Game>, mode: Res<RenderMode>, set: Res<Settings>, mut pend: ResMut<Pending>, mut prev: Local<(u32, usize)>) {
    let impacts = std::mem::take(&mut g.world.impacts);
    let seq = g.world.kid.seq;
    let before = std::mem::replace(&mut *prev, (g.gen, seq));
    if !wanted(&mode, &set) {
        pend.0.clear();
        return;
    }
    for i in impacts {
        pend.0.push((of_impact(i.kind), At::Tile(i.room, i.tilepos)));
    }
    if before.0 == g.gen && before.1 != seq && (g.world.kid.alive || seq == 22) {
        if let Some(k) = landing(seq) {
            pend.0.push((k, At::Kid));
        }
    }
    let n = pend.0.len();
    if n > POOL {
        pend.0.drain(..n - POOL);
    }
}

/// Every frame: places the pending puffs (and kicks the camera), hides the oldest one when its time is up, steps the shake.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut cmds: Commands,
    time: Res<Time>,
    g: Res<Game>,
    mode: Res<RenderMode>,
    set: Res<Settings>,
    gore: Res<crate::blood::Gore>,
    sun: Query<&DirectionalLight, Without<crate::mirror::TwinOf>>,
    mut pend: ResMut<Pending>,
    puffs: Option<ResMut<Puffs>>,
    mut mats: ResMut<Assets<PuffMaterial>>,
    mut shake: ResMut<Shake>,
) {
    let Some(mut puffs) = puffs else { return };
    let now = time.elapsed_secs_wrapped();
    // the oldest puff is the only one to look at: they leave in the order they came
    while puffs.live > 0 {
        let t = puffs.tail;
        let age = (now - puffs.born[t]).rem_euclid(3600.0);
        if age < puffs.life[t] {
            break;
        }
        cmds.entity(puffs.slots[t].0).insert(Visibility::Hidden);
        puffs.tail = (t + 1) % POOL;
        puffs.live -= 1;
    }
    shake.step(time.delta_secs());
    if !wanted(&mode, &set) {
        pend.0.clear();
        *shake = Shake::default();
        return;
    }
    if pend.0.is_empty() {
        return;
    }
    let sun = sun.iter().next().map_or(Vec3::ONE, |l| {
        let c = l.color.to_linear();
        Vec3::new(c.red, c.green, c.blue) * (l.illuminance / 3300.0).clamp(0.0, 1.0).sqrt().max(0.25)
    });
    let kid = gore.bodies[0].map(|b| Vec2::new(b.x, crate::blood::floor_of(g.world.kid.y)));
    let origins = crate::room::rooms_around(&g);
    for (k, at) in std::mem::take(&mut pend.0) {
        let (room, pos) = match at {
            At::Kid => (g.room, kid),
            At::Tile(room, tp) => {
                let (col, row) = (tp as usize % COLS, tp as usize / COLS);
                (room as usize, origins.iter().find(|o| o.0 == room as usize).map(|o| Vec2::new(o.3.x + col as f32 + 0.5, o.3.y + (2 - row) as f32 * crate::room::TILE_H)))
            }
        };
        let Some(pos) = pos else { continue };
        let sp = spec(k);
        // the shake follows the kid's distance (the kid's own landing is at 0)
        if room == g.room {
            let d = kid.map_or(0.0, |kp| if at == At::Kid { 0.0 } else { (kp - pos).abs().max_element().max(0.0) });
            shake.kick(kick(k, d));
        }
        // lit by the nearest torch of its room within 4 units, else by the sun
        let torch = (0..30).filter(|&i| matches!(g.world.level.tile(room, i % COLS, i / COLS).kind, pop_core::level::Tile::Torch | pop_core::level::Tile::TorchDebris)).map(|i| Vec2::new((i % COLS) as f32 + 0.5, (2 - i / COLS) as f32 * crate::room::TILE_H + 1.3)).map(|t| (t - (pos - origins.iter().find(|o| o.0 == room).map_or(Vec2::ZERO, |o| o.3.truncate()))).length()).fold(f32::MAX, f32::min);
        let c = tint(sun, (1.0 - torch / 4.0).clamp(0.0, 1.0));
        let s = puffs.next;
        puffs.next = (s + 1) % POOL;
        if puffs.live == POOL {
            puffs.tail = puffs.next; // the oldest was overwritten
        } else {
            puffs.live += 1;
        }
        puffs.born[s] = now;
        puffs.life[s] = sp.life;
        let seed = ((pos.x * 31.0 + pos.y * 17.0 + g.world.tick as f32) % 97.0).abs();
        let (e, h) = puffs.slots[s].clone();
        if let Some(mut m) = mats.get_mut(&h) {
            m.u = PuffU { tint: c.extend(sp.strength), p: Vec4::new(now, sp.life, seed, sp.spray as u32 as f32) };
        }
        cmds.entity(e).insert((Visibility::Inherited, Transform::from_translation(Vec3::new(pos.x, pos.y + sp.size.y / 2.0, PUFF_Z)).with_scale(sp.size.extend(1.0))));
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/puff.wesl");
    app.add_plugins(MaterialPlugin::<PuffMaterial>::default()).init_resource::<Pending>().init_resource::<Shake>().add_systems(Startup, setup);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puffs_grow_with_the_event() {
        let area = |k| spec(k).size.x * spec(k).size.y;
        assert!(area(PuffKind::Soft) < area(PuffKind::Hard) && area(PuffKind::Hard) < area(PuffKind::Fatal) && area(PuffKind::Fatal) < area(PuffKind::Crash));
        assert!(area(PuffKind::Soft) < area(PuffKind::Fall) && area(PuffKind::Fall) < area(PuffKind::Crash));
        // the slam is a spray along the floor: much wider than tall, the others are clouds
        assert!(spec(PuffKind::Slam).spray && spec(PuffKind::Slam).size.x > 3.0 * spec(PuffKind::Slam).size.y);
        assert!(![PuffKind::Soft, PuffKind::Hard, PuffKind::Fatal, PuffKind::Fall, PuffKind::Crash, PuffKind::Chomp].iter().any(|k| spec(*k).spray));
        // all live 0.6 to 0.9 s
        for k in [PuffKind::Soft, PuffKind::Hard, PuffKind::Fatal, PuffKind::Fall, PuffKind::Crash, PuffKind::Slam, PuffKind::Chomp] {
            assert!((0.6..=0.9).contains(&spec(k).life), "{k:?}");
        }
    }

    #[test]
    fn landings_and_impacts_map_to_puffs() {
        assert_eq!((landing(17), landing(63), landing(20), landing(22)), (Some(PuffKind::Soft), Some(PuffKind::Soft), Some(PuffKind::Hard), Some(PuffKind::Fatal)));
        assert_eq!(landing(1), None);
        assert_eq!(of_impact(ImpactKind::Crash), PuffKind::Crash);
        assert_eq!(of_impact(ImpactKind::GateSlam), PuffKind::Slam);
    }

    #[test]
    fn only_the_big_events_shake_and_less_when_far() {
        assert_eq!((spec(PuffKind::Soft).shake, spec(PuffKind::Fall).shake, spec(PuffKind::Chomp).shake), (0.0, 0.0, 0.0));
        assert!(kick(PuffKind::Hard, 0.0) > 0.0 && kick(PuffKind::Crash, 1.0) == spec(PuffKind::Crash).shake);
        assert!(kick(PuffKind::Crash, 4.0) < kick(PuffKind::Crash, 2.0) && kick(PuffKind::Crash, 7.0) == 0.0 && kick(PuffKind::Crash, 20.0) == 0.0);
        // a few pixels at most (about 125 px per world unit at 1280x720)
        assert!([PuffKind::Hard, PuffKind::Fatal, PuffKind::Crash, PuffKind::Slam].iter().all(|k| spec(*k).shake * 125.0 < 5.0));
    }

    #[test]
    fn shake_decays_to_exactly_zero_and_never_drifts() {
        assert_eq!(shake_offset(0.03, 0.0), Vec2::ZERO);
        assert_eq!(shake_offset(0.03, SHAKE_SECS), Vec2::ZERO);
        assert_eq!(shake_offset(0.03, 5.0), Vec2::ZERO);
        assert_eq!(shake_offset(0.03, -1.0), Vec2::ZERO);
        // the envelope falls; the offset never exceeds it
        assert!(shake_decay(0.0) == 1.0 && shake_decay(0.1) > shake_decay(0.2) && shake_decay(SHAKE_SECS) == 0.0);
        for i in 0..250 {
            let t = i as f32 / 1000.0;
            assert!(shake_offset(0.03, t).abs().max_element() <= 0.03 * shake_decay(t) + 1e-6);
        }
        // stepping at any frame rate ends at zero and a stronger kick restarts it, a weaker one does not
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 240.0] {
            let mut s = Shake::default();
            s.kick(0.03);
            let mut steps = 0;
            while s.amp > 0.0 {
                s.step(dt);
                steps += 1;
                assert!(steps < 100);
            }
            assert_eq!(s.offset(), Vec2::ZERO);
        }
        let mut s = Shake::default();
        s.kick(0.03);
        s.step(0.1);
        s.kick(0.001);
        assert_eq!((s.amp, s.t > 0.0), (0.03, true));
        s.kick(0.05);
        assert_eq!((s.amp, s.t), (0.05, 0.0));
    }

    #[test]
    fn dust_takes_the_light_it_is_in() {
        let (day, night) = (Vec3::new(1.0, 0.88, 0.74), Vec3::new(0.55, 0.68, 1.0));
        let (d, n) = (tint(day, 0.0), tint(night, 0.0));
        assert!(n.z / n.x > d.z / d.x);
        let torch = tint(day, 1.0);
        assert!(torch.x > d.x && torch.x / torch.z > d.x / d.z, "a torch makes it warmer and brighter");
        assert_eq!(tint(day, 0.3), tint(day, 0.3));
    }
}
