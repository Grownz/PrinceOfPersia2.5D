//! Traces that stay (rides on the setting `atmosphere`, enhanced mode): blood stains and rubble piles.
//! Blood: every pool under a killed figure and every splatter of a sword hit is recorded in `Stains` (room, place, size, seed, game
//! tick) for the current level (a new level or restart empties it; leaving and re-entering a room keeps it, up to `PER_ROOM` per room,
//! the oldest merged into its neighbour). `sync_stains` lays one flat quad per stain of the rooms in view; one multiply shader
//! (shaders/traces.wesl) draws it with hash-noise edges, growing while it is new and drying to brown with its age.
//! Rubble: the room builder (room.rs) spawns a bare `Rubble` marker over each fallen-loose-floor tile (`Tile::Debris`, which the sim
//! already keeps); `sync` dresses it with a flat pebble pile drawn by the same shader. No per-frame CPU work, no shadows.
//! Skipped: footprints (they would need a per-step system and a ring buffer of entities).
use crate::room::{SLAB_FRONT, TILE_H};
use crate::settings::{RenderMode, Settings};
use crate::Game;
use bevy::asset::embedded_asset;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use pop_core::level::COLS;

/// Stains kept per room; over that the oldest is merged into the nearest.
pub const PER_ROOM: usize = 24;
/// Largest radius a merged pool / splatter grows to (world units, x).
pub const MAX_POOL: f32 = 0.5;
pub const MAX_SPLAT: f32 = 0.22;
/// Rubble piles per room.
pub const RUBBLE_MAX: u8 = 8;
/// Logic ticks per second (the game's 12 Hz clock).
const TICK_HZ: f32 = 12.0;
/// Game seconds when a stain starts to dry and when it is dry (mirrored in the shader).
#[allow(dead_code)]
const DRY_START: f32 = 8.0;
#[allow(dead_code)]
const DRY_FULL: f32 = 150.0;
/// A stain lies this far above the floor slab's top (the weathering decal is at 0.004, a puddle at 0.011).
const LIFT: f32 = 0.009;
const BACK_FRONT: f32 = -0.9;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stain {
    pub room: u8,
    pub x: f32,
    /// y of the floor line it lies on (world units, room frame)
    pub floor: f32,
    pub z: f32,
    pub r: f32,
    pub seed: u32,
    pub tick: u64,
    pub pool: bool,
}

/// All stains of the current level.
#[derive(Resource, Default)]
pub struct Stains {
    list: Vec<Stain>,
    world: (usize, u32),
    /// bumped on every change
    pub version: u32,
}

impl Stains {
    /// A new level or restart (`(level, generation)` changed) forgets everything.
    pub fn sync_world(&mut self, w: (usize, u32)) {
        if std::mem::replace(&mut self.world, w) != w {
            self.list.clear();
            self.version = self.version.wrapping_add(1);
        }
    }

    pub fn add(&mut self, s: Stain) {
        self.version = self.version.wrapping_add(1);
        if self.list.iter().filter(|o| o.room == s.room).count() >= PER_ROOM {
            let oldest = (0..self.list.len()).filter(|&i| self.list[i].room == s.room).min_by_key(|&i| self.list[i].tick).unwrap_or(0);
            let old = self.list.remove(oldest);
            let near = (0..self.list.len()).filter(|&i| self.list[i].room == s.room).min_by(|&a, &b| dist(&self.list[a], &old).total_cmp(&dist(&self.list[b], &old)));
            if let Some(j) = near {
                let o = &mut self.list[j];
                o.pool |= old.pool;
                o.r = (o.r * o.r + old.r * old.r).sqrt().min(if o.pool { MAX_POOL } else { MAX_SPLAT });
            }
        }
        self.list.push(s);
    }

    pub fn in_room(&self, room: u8) -> impl Iterator<Item = &Stain> {
        self.list.iter().filter(move |s| s.room == room)
    }
}

fn dist(a: &Stain, b: &Stain) -> f32 {
    Vec2::new(a.x - b.x, (a.floor - b.floor) * 2.0).length()
}

/// 0 wet .. 1 dry for a stain `ticks` logic ticks old (the shader computes the same from its age in seconds).
#[allow(dead_code)] // the shader computes the same from its age; kept as the tested reference
pub fn dryness(ticks: u64) -> f32 {
    ((ticks as f32 / TICK_HZ - DRY_START) / (DRY_FULL - DRY_START)).clamp(0.0, 1.0)
}

/// Size of the quad a stain of radius `r` is drawn in: (width, depth) in world units.
pub fn stain_quad(r: f32, pool: bool) -> Vec2 {
    if pool { Vec2::new(2.6, 1.7) * r } else { Vec2::new(8.0, 5.0) * r }
}

/// The pile of a fallen loose floor on tile `tilepos` (row * 10 + column) of a room: (centre in room coordinates, quad size).
pub fn rubble_place(tilepos: usize) -> (Vec3, Vec2) {
    let (col, row) = (tilepos % COLS, tilepos / COLS);
    let y = (2 - row) as f32 * TILE_H + LIFT - 0.003;
    (Vec3::new(col as f32 + 0.5, y, (BACK_FRONT + SLAB_FRONT) / 2.0), Vec2::new(1.3, 1.0))
}

/// A bare marker the room builder puts on a fallen-floor tile; `sync` gives it mesh and material while the effects are on.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Rubble {
    pub seed: u32,
    pub size: Vec2,
}

#[derive(Component)]
pub struct StainEnt;

#[derive(Clone, Copy, Default, ShaderType)]
pub struct TraceU {
    kind: Vec4,
    size: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct TraceMaterial {
    #[uniform(0)]
    u: TraceU,
}

impl Material for TraceMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(bevy::asset::AssetPath::parse("embedded://pop_app/shaders/traces.wesl"))
    }
    // out = rgb * what is there, like the weathering: the traces take the scene's light
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    // both sides: the palace wall mirror (mirror.rs) draws the same material on the reflected quad, whose winding is reversed
    fn specialize(_: &bevy::pbr::MaterialPipeline, d: &mut bevy::render::render_resource::RenderPipelineDescriptor, _: &bevy::mesh::MeshVertexBufferLayoutRef, _: bevy::pbr::MaterialPipelineKey<Self>) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        d.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Resource)]
pub struct Shared {
    quad: Handle<Mesh>,
}

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    cmds.insert_resource(Shared { quad: meshes.add(Rectangle::new(1.0, 1.0)) });
}

fn wanted(mode: &RenderMode, set: &Settings) -> bool {
    *mode == RenderMode::Enhanced && set.atmosphere
}

/// Whether blood.rs records stains and hides its room-bound pools.
pub fn persistent(mode: &RenderMode, set: &Settings) -> bool {
    wanted(mode, set)
}

fn material(kind: f32, seed: u32, age: f32, t0: f32, size: Vec2, r: f32) -> TraceMaterial {
    TraceMaterial { u: TraceU { kind: Vec4::new(kind, seed as f32, age, t0), size: Vec4::new(size.x, size.y, r, 0.0) } }
}

/// Every frame, after `blood::motion`: dresses the rubble markers while the effects are on (takes them off when they are not) and
/// keeps one quad per stain of the rooms in view (rebuilt when the stains, the shown room or the world change).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync(
    mut cmds: Commands,
    mode: Res<RenderMode>,
    set: Res<Settings>,
    g: Res<Game>,
    time: Res<Time>,
    shared: Option<Res<Shared>>,
    mut stains: ResMut<Stains>,
    mut mats: ResMut<Assets<TraceMaterial>>,
    bare: Query<(Entity, &Rubble), Without<Mesh3d>>,
    clothed: Query<Entity, (With<Rubble>, With<Mesh3d>)>,
    ents: Query<Entity, With<StainEnt>>,
    mut seen: Local<(u32, usize, u32, bool)>,
) {
    let Some(shared) = shared else { return };
    stains.sync_world((g.level, g.gen));
    let want = wanted(&mode, &set);
    if !want {
        if std::mem::take(&mut seen.3) {
            ents.iter().for_each(|e| cmds.entity(e).despawn());
            clothed.iter().for_each(|e| {
                cmds.entity(e).remove::<(Mesh3d, MeshMaterial3d<TraceMaterial>)>();
            });
        }
        return;
    }
    let t0 = time.elapsed_secs_wrapped();
    for (e, r) in &bare {
        cmds.entity(e).insert((Mesh3d(shared.quad.clone()), MeshMaterial3d(mats.add(material(2.0, r.seed, 0.0, t0, r.size, 0.0)))));
    }
    let now = (stains.version, g.room, g.gen, true);
    if *seen == now {
        return;
    }
    *seen = now;
    ents.iter().for_each(|e| cmds.entity(e).despawn());
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for (room, _, _, origin) in crate::room::rooms_around(&g) {
        for s in stains.in_room(room as u8) {
            let size = stain_quad(s.r, s.pool);
            let age = g.world.tick.saturating_sub(s.tick) as f32 / TICK_HZ;
            let m = mats.add(material(if s.pool { 0.0 } else { 1.0 }, s.seed, age, t0, size, s.r));
            let at = origin + Vec3::new(s.x, s.floor + LIFT, s.z);
            cmds.spawn((StainEnt, Mesh3d(shared.quad.clone()), MeshMaterial3d(m), Transform::from_translation(at).with_rotation(flat).with_scale(size.extend(1.0)), bevy::light::NotShadowCaster));
        }
    }
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "shaders/traces.wesl");
    app.add_plugins(MaterialPlugin::<TraceMaterial>::default()).init_resource::<Stains>().add_systems(Startup, setup);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stain(room: u8, x: f32, tick: u64, r: f32, pool: bool) -> Stain {
        Stain { room, x, floor: 0.0, z: 0.0, r, seed: tick as u32, tick, pool }
    }

    #[test]
    fn a_room_keeps_its_cap_and_merges_the_oldest_into_its_neighbour() {
        let mut s = Stains::default();
        for i in 0..PER_ROOM as u64 {
            s.add(stain(3, i as f32 * 0.4, 100 + i, 0.1, false));
        }
        s.add(stain(4, 1.0, 5, 0.1, true));
        assert_eq!((s.in_room(3).count(), s.in_room(4).count()), (PER_ROOM, 1));
        // one more in room 3: still the cap, the oldest (x 0.0, tick 100) is gone and its neighbour (x 0.4) took its area
        s.add(stain(3, 9.0, 999, 0.1, false));
        assert_eq!(s.in_room(3).count(), PER_ROOM);
        assert!(s.in_room(3).all(|o| o.tick != 100));
        let n = s.in_room(3).find(|o| o.tick == 101).unwrap();
        assert!((n.r - (0.1f32 * 0.1 * 2.0).sqrt()).abs() < 1e-5);
        assert_eq!(s.in_room(4).count(), 1, "other rooms are untouched");
        // merged radii stay under the cap
        for i in 0..200 {
            s.add(stain(3, 5.0, 2000 + i, 0.2, false));
        }
        assert!(s.in_room(3).all(|o| o.r <= MAX_SPLAT + 1e-6) && s.in_room(3).count() == PER_ROOM);
    }

    #[test]
    fn a_new_world_forgets_but_a_room_change_does_not() {
        let mut s = Stains::default();
        s.sync_world((1, 0));
        s.add(stain(3, 1.0, 1, 0.3, true));
        s.sync_world((1, 0));
        assert_eq!(s.in_room(3).count(), 1);
        s.sync_world((1, 1));
        assert_eq!(s.in_room(3).count(), 0);
        s.add(stain(3, 1.0, 1, 0.3, true));
        s.sync_world((2, 1));
        assert_eq!(s.in_room(3).count(), 0);
    }

    #[test]
    fn stains_dry_with_age() {
        assert_eq!(dryness(0), 0.0);
        assert_eq!(dryness((DRY_START * TICK_HZ) as u64), 0.0);
        let (a, b) = (dryness(600), dryness(1200));
        assert!(0.0 < a && a < b && b < 1.0);
        assert_eq!(dryness((DRY_FULL * TICK_HZ) as u64 + 1), 1.0);
    }

    #[test]
    fn rubble_lies_on_the_tile_it_fell_on() {
        // row 1, column 4: the middle floor line, centred over the tile, inside the floor's depth
        let (c, size) = rubble_place(14);
        assert_eq!((c.x, c.y > TILE_H && c.y < TILE_H + 0.02), (4.5, true));
        assert!(c.z - size.y / 2.0 >= BACK_FRONT - 0.5 && c.z + size.y / 2.0 <= SLAB_FRONT + 0.001);
        assert!(rubble_place(0).0.y > 2.0 * TILE_H && rubble_place(29).0.y < 0.02);
        assert_eq!(rubble_place(14), rubble_place(14));
    }

    #[test]
    fn quads_fit_their_stains() {
        let (p, s) = (stain_quad(0.34, true), stain_quad(0.1, false));
        assert!(p.x > 2.0 * 0.34 * 1.2 && s.x > 2.0 * 0.1 * 3.0);
    }
}
