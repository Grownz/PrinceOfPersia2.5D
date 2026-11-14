//! Deterministic game simulation. The original game runs its logic at 12 ticks per second.
//! We step at 60 Hz (`SUBSTEPS` = 5 steps per logic tick): the original logic runs unchanged on every
//! 5th step, the steps in between only advance `alpha()` so rendering can interpolate and input can
//! be sampled at 60 Hz. Mechanics therefore stay identical to the original.
//!
//! One logic tick (docs/tiles.md 2.1): falling loose floors (mobs), then all moving tiles (trobs), then the
//! kid. What the kid triggers in tick N first moves in tick N+1. Rules and addresses: docs/tiles.md.
//! Not simulated: torch/potion/sword animation (visual only, they also consume random numbers in the original),
//! guards, the shadow, level doors (entering), skeleton, mirror, potions.
use crate::exe::{Exe, Frame};
use crate::kid::{Env, Event, Keys, Kid, Y_LAND};
use crate::level::{Level, Tile, COLS, ROWS, TILES_PER_ROOM};
use std::sync::Arc;

pub const TICKS_PER_SEC: u32 = 12;
pub const SUBSTEPS: u32 = 5;
pub const STEP_HZ: f64 = (TICKS_PER_SEC * SUBSTEPS) as f64;

const MAX_TROBS: usize = 30;
const MAX_MOBS: usize = 14;
/// y_loose_land (ds:2298) and y_something (ds:228E), indexed by row + 1
const Y_LOOSE_LAND: [u8; 5] = [2, 65, 128, 191, 254];
const Y_SOMETHING: [i32; 5] = [-1, 62, 125, 188, 251];
const DOOR_DELTA: [i8; 3] = [-1, 4, 4];
const GATE_CLOSE: [u8; 9] = [0, 0, 0, 20, 40, 60, 80, 100, 120];
const LEVELDOOR_CLOSE: [u8; 8] = [0, 0, 0, 0, 5, 17, 99, 0];
const LOOSE_SOUND: [u8; 12] = [0, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0];

/// Static game data shared by all worlds.
pub struct Data {
    pub exe: Exe,
    pub frames: Vec<Frame>,
    /// (height, width) of kid image i (KID.DAT resource 401 + i)
    pub sizes: Vec<(u16, u16)>,
}

/// A moving tile ("trob"): `ty` < 0 = finished. Meaning of `ty` depends on the tile type (docs/tiles.md 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trob {
    pub tilepos: u8,
    pub room: u8,
    pub ty: i8,
}

/// A falling loose floor ("mob"). `xh` = column * 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mob {
    pub xh: u8,
    pub y: u8,
    pub room: u8,
    pub speed: i8,
    pub ty: u8,
    pub row: u8,
}

pub struct World {
    /// original logic ticks (12 Hz)
    pub tick: u64,
    sub: u32,
    pub data: Arc<Data>,
    /// runtime tile state (the level as loaded, modified by the mechanics)
    pub level: Level,
    pub level_no: usize,
    pub kid: Kid,
    pub trobs: Vec<Trob>,
    pub mobs: Vec<Mob>,
    /// sound ids triggered since the consumer last cleared this list (docs/tiles.md section 9)
    pub sounds: Vec<u8>,
    /// 0 = exit door closed, 1 = open (set when the door finished opening)
    pub leveldoor_open: u8,
    /// bumped whenever tiles, trobs or mobs changed; renderers use it to know when to rebuild
    pub version: u32,
    dirty: bool,
    seed: u32,
    last_loose_sound: u8,
}

impl World {
    pub fn new(data: Arc<Data>, level: Level, level_no: usize) -> World {
        let kid = {
            let env = Env { exe: &data.exe, frames: &data.frames, level: &level, sizes: &data.sizes };
            Kid::spawn(&env, level_no)
        };
        let mut w = World { tick: 0, sub: 0, data, level, level_no, kid, trobs: vec![], mobs: vec![], sounds: vec![], leveldoor_open: 0, version: 0, dirty: true, seed: 0x1234_5678, last_loose_sound: 0 };
        // level start (do_startpos): level 1 presses the button at room 5 tile 2; the entry door closes; chompers wake up
        if level_no == 1 {
            w.trigger_button(false, 0, -1, 5, 2);
        }
        let room = w.kid.room;
        if let Some(idx) = (0..TILES_PER_ROOM).find(|&i| (1..=24).contains(&room) && w.level.tile_at(room as usize, i).kind == Tile::ExitLeft) {
            w.set_mod(room, idx as u8, 43);
            w.add_trob(room, idx as u8, 3);
        }
        w.start_chompers(room, w.kid.row);
        w.apply_kid_events();
        w
    }

    /// One 60 Hz step; returns true if a logic tick ran.
    pub fn step(&mut self, keys: Keys) -> bool {
        self.sub += 1;
        if self.sub < SUBSTEPS {
            return false;
        }
        self.sub = 0;
        self.tick += 1;
        self.logic_tick(keys);
        true
    }

    /// True if the next `step` runs a logic tick (lets callers snapshot state for interpolation first).
    pub fn next_is_tick(&self) -> bool {
        self.sub + 1 >= SUBSTEPS
    }

    /// Sequence viewer: play sequence `seq` without control and physics.
    pub fn kid_start_seq(&mut self, seq: usize) {
        let env = Env { exe: &self.data.exe, frames: &self.data.frames, level: &self.level, sizes: &self.data.sizes };
        self.kid.controlled = false;
        self.kid.start_seq(&env, seq);
    }

    /// Progress from the last logic tick to the next, in [0, 1), for render interpolation.
    pub fn alpha(&self) -> f32 {
        self.sub as f32 / SUBSTEPS as f32
    }

    fn logic_tick(&mut self, keys: Keys) {
        self.do_mobs();
        self.process_trobs();
        {
            let env = Env { exe: &self.data.exe, frames: &self.data.frames, level: &self.level, sizes: &self.data.sizes };
            self.kid.tick(&env, keys);
        }
        self.apply_kid_events();
        if self.dirty {
            self.dirty = false;
            self.version = self.version.wrapping_add(1);
        }
    }

    fn apply_kid_events(&mut self) {
        let evs = std::mem::take(&mut self.kid.events);
        for e in evs {
            match e {
                Event::Button { room, tilepos, dead } => {
                    if dead { self.died_on_button(room, tilepos) } else { self.trigger_button(true, 0, -1, room, tilepos) }
                }
                Event::LooseFall { room, tilepos } => self.make_loose_fall(room, tilepos, 1),
                Event::Knock { room, row } => self.do_knock(room, row),
                Event::StartSpike { room, tilepos } => self.start_anim_spike(room, tilepos),
                Event::Spiked { room, tilepos } => self.set_mod(room, tilepos, 255),
                Event::Chomped { room, tilepos } => {
                    let m = self.modifier(room, tilepos);
                    self.set_mod(room, tilepos, m | 0x80);
                }
                Event::StartChompers { room, row } => self.start_chompers(room, row),
                Event::Sound(id) => self.sounds.push(id),
            }
        }
    }

    // ------------------------------------------------------------------ helpers

    fn kind(&self, room: u8, idx: u8) -> Tile {
        if !(1..=24).contains(&room) || idx as usize >= TILES_PER_ROOM {
            return Tile::Wall;
        }
        self.level.tile_at(room as usize, idx as usize).kind
    }

    fn modifier(&self, room: u8, idx: u8) -> u8 {
        if !(1..=24).contains(&room) || idx as usize >= TILES_PER_ROOM {
            return 0;
        }
        self.level.tile_at(room as usize, idx as usize).modifier
    }

    fn set_mod(&mut self, room: u8, idx: u8, m: u8) {
        if (1..=24).contains(&room) && (idx as usize) < TILES_PER_ROOM {
            self.level.set_modifier(room as usize, idx as usize, m);
            self.dirty = true;
        }
    }

    fn set_tile(&mut self, room: u8, idx: u8, t: Tile) {
        if (1..=24).contains(&room) && (idx as usize) < TILES_PER_ROOM {
            self.level.set_tile(room as usize, idx as usize, t);
            self.dirty = true;
        }
    }

    fn sound(&mut self, id: u8) {
        self.sounds.push(id);
    }

    /// door_sound_if_visible (simplified: only the room the kid is in counts as visible)
    fn sound_if_visible(&mut self, room: u8, id: u8) {
        if room == self.kid.room {
            self.sounds.push(id);
        }
    }

    /// prandom (0C83:0412)
    fn prandom(&mut self, max: u32) -> u32 {
        self.seed = self.seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        (self.seed >> 16) % (max + 1)
    }

    /// add_trob (0000:9052): an existing entry only gets its type replaced.
    fn add_trob(&mut self, room: u8, tilepos: u8, ty: i8) {
        self.dirty = true;
        if let Some(t) = self.trobs.iter_mut().find(|t| t.tilepos == tilepos && t.room == room) {
            t.ty = ty;
        } else if self.trobs.len() < MAX_TROBS {
            self.trobs.push(Trob { tilepos, room, ty });
        }
    }

    fn add_mob(&mut self, m: Mob) {
        if self.mobs.len() < MAX_MOBS {
            self.mobs.push(m);
        }
    }

    // ------------------------------------------------------------------ triggers (docs/tiles.md 3.4, 4)

    /// trigger_button (0000:924B) for the button tile at (room, tilepos). `button_type` 0 = the tile's own type,
    /// `modifier` -1 = the tile's own link index.
    fn trigger_button(&mut self, playsound: bool, button_type: u8, modifier: i32, room: u8, tilepos: u8) {
        let bt = if button_type == 0 { self.kind(room, tilepos) as u8 } else { button_type };
        let m = if modifier == -1 { self.modifier(room, tilepos) } else { modifier as u8 };
        let t = self.level.link_timer(m);
        if t != 0x1F {
            self.level.set_link_timer(m, 5);
            if t < 2 {
                self.add_trob(room, tilepos, 1);
                if playsound {
                    self.sound(3);
                }
            }
            self.do_trigger_list(m, bt);
        }
    }

    /// do_trigger_list (0000:8FDD)
    fn do_trigger_list(&mut self, index: u8, bt: u8) {
        for link in self.level.door_links(index) {
            let (room, idx) = (link.room, link.tile);
            let r = match self.kind(room, idx) {
                Tile::Gate => self.trigger_gate(room, idx, bt),
                Tile::ExitLeft => if self.modifier(room, idx) != 0 { -1 } else { 1 },
                _ => -1,
            };
            if r >= 0 {
                self.add_trob(room, idx, r);
            }
        }
    }

    /// trigger_gate (0000:8F24)
    fn trigger_gate(&mut self, room: u8, idx: u8, bt: u8) -> i8 {
        let m = self.modifier(room, idx);
        match bt {
            15 => {
                if m == 255 {
                    -1
                } else if m >= 188 {
                    self.set_mod(room, idx, 238);
                    -1
                } else {
                    self.set_mod(room, idx, m.wrapping_add(3) & 0xFC);
                    1
                }
            }
            14 => {
                if m < 188 {
                    2
                } else {
                    self.set_mod(room, idx, 255);
                    -1
                }
            }
            _ => if m != 0 { 3 } else { -1 },
        }
    }

    /// died_on_button (0000:92D1)
    fn died_on_button(&mut self, room: u8, tilepos: u8) {
        let bt = self.kind(room, tilepos);
        let old = self.modifier(room, tilepos);
        let bt = if bt == Tile::RaiseButton {
            self.set_tile(room, tilepos, Tile::Floor);
            self.set_mod(room, tilepos, 0);
            14
        } else {
            self.set_tile(room, tilepos, Tile::StuckButton);
            bt as u8
        };
        self.trigger_button(true, bt, old as i32, room, tilepos);
    }

    // ------------------------------------------------------------------ trobs

    fn process_trobs(&mut self) {
        let mut i = 0;
        while i < self.trobs.len() {
            let mut t = self.trobs[i];
            self.animate_tile(&mut t);
            if i < self.trobs.len() {
                self.trobs[i] = t;
            }
            i += 1;
        }
        self.trobs.retain(|t| t.ty >= 0);
    }

    /// animate_tile (0000:86A3): dispatch on the current tile type, then write the modifier back.
    fn animate_tile(&mut self, t: &mut Trob) {
        let (room, tp) = (t.room, t.tilepos);
        let mut m = self.modifier(room, tp);
        match self.kind(room, tp) {
            Tile::Spikes => self.animate_spike(t, &mut m),
            Tile::Gate => self.animate_door(t, &mut m),
            Tile::DropButton | Tile::RaiseButton => {
                let timer = self.level.link_timer(m);
                if timer == 0 {
                    t.ty = -1;
                } else {
                    let nt = timer - 1;
                    self.level.set_link_timer(m, nt);
                    if nt < 2 {
                        t.ty = -1;
                    }
                }
                self.dirty = true;
            }
            Tile::LooseFloor => self.animate_loose(t, &mut m),
            Tile::ExitLeft => self.animate_leveldoor(t, &mut m),
            Tile::Chomper => self.animate_chomper(t, &mut m),
            _ => t.ty = -1, // empty, stuck button, debris, torches, ...
        }
        // the original writes the modifier back unconditionally (a fallen loose floor sets it in remove_loose)
        if self.modifier(room, tp) != m {
            self.set_mod(room, tp, m);
        }
    }

    /// animate_spike (0000:8AC7)
    fn animate_spike(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 || *m == 255 {
            return;
        }
        if *m & 0x80 != 0 {
            *m -= 1;
            if *m & 0x7F == 0 {
                *m = 6;
            } else {
                return;
            }
        } else {
            *m += 1;
            if *m == 5 {
                *m = 0x8F;
            } else if *m == 9 {
                *m = 0;
                t.ty = -1;
            }
        }
    }

    /// animate_door (0000:8B16)
    fn animate_door(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        let room = t.room;
        if t.ty >= 3 {
            if t.ty < 8 {
                t.ty += 1;
            }
            let sp = GATE_CLOSE[t.ty as usize];
            if *m < sp {
                *m = 0;
                t.ty = -1;
                self.sound(6);
            } else {
                *m -= sp;
            }
        } else if *m == 255 {
            t.ty = -1;
            self.sound_if_visible(room, 7);
        } else {
            *m = m.wrapping_add(DOOR_DELTA[t.ty as usize] as u8);
            if t.ty == 0 {
                if *m == 0 {
                    t.ty = -1;
                    self.sound_if_visible(room, 7);
                } else if *m < 188 && *m & 3 == 3 {
                    self.sound_if_visible(room, 4);
                }
            } else if *m < 188 {
                if *m & 7 == 0 {
                    self.sound(5);
                }
            } else if t.ty < 2 {
                *m = 238;
                t.ty = 0;
                self.sound(7);
            } else {
                *m = 255;
                t.ty = -1;
                self.sound_if_visible(room, 7);
            }
        }
    }

    /// animate_loose (0000:9395)
    fn animate_loose(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        *m = m.wrapping_add(1);
        if *m & 0x80 != 0 {
            if self.level_no == 13 {
                return;
            }
            if *m >= 0x84 {
                *m = 0;
                t.ty = -1;
            }
            self.loose_shake(*m, *m == 0);
        } else if *m >= 11 {
            // falls: the tile becomes empty (remove_loose) and a mob takes over
            self.set_tile(t.room, t.tilepos, Tile::Empty);
            *m = 0;
            t.ty = -1;
            let row = t.tilepos / COLS as u8;
            self.add_mob(Mob { xh: (t.tilepos % COLS as u8) * 4, y: Y_LOOSE_LAND[row as usize + 1], room: t.room, speed: 0, ty: 0, row });
        } else {
            self.loose_shake(*m, false);
        }
    }

    /// loose_shake (0000:944D): shake sounds 20..22, never the same twice in a row. (The extra prandom(1000) for the
    /// digital sample rate is not consumed here.)
    fn loose_shake(&mut self, m: u8, force: bool) {
        if force || LOOSE_SOUND.get((m & 0x7F) as usize).copied().unwrap_or(0) != 0 {
            let mut s;
            loop {
                s = self.prandom(2) as u8 + 20;
                if s != self.last_loose_sound {
                    break;
                }
            }
            self.last_loose_sound = s;
            self.sound(s);
        }
    }

    /// animate_leveldoor (0000:8BE5)
    fn animate_leveldoor(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        if t.ty >= 3 {
            t.ty = (t.ty + 1).min(7);
            let sp = LEVELDOOR_CLOSE[t.ty as usize];
            if *m < sp {
                *m = 0;
                t.ty = -1;
                self.sound(14);
            } else {
                *m -= sp;
            }
        } else {
            *m = m.wrapping_add(1);
            if *m >= 43 {
                t.ty = -1;
                if self.leveldoor_open == 0 || self.leveldoor_open == 2 {
                    self.leveldoor_open = 1;
                }
            } else {
                self.sound(15);
            }
        }
    }

    /// animate_chomper (0000:8A3C)
    fn animate_chomper(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        let blood = *m & 0x80;
        let mut f = (*m & 0x7F) + 1;
        if f > 15 {
            f = 1;
        }
        *m = blood | f;
        if f == 2 {
            self.sound_if_visible(t.room, 47);
        }
        if (t.room != self.kid.room || (t.tilepos / COLS as u8) as i32 != self.kid.row || (self.kid.alive && blood == 0)) && f >= 6 {
            t.ty = -1;
        }
    }

    // ------------------------------------------------------------------ mechanics started by events

    /// start_anim_spike (0000:8EDB)
    fn start_anim_spike(&mut self, room: u8, tp: u8) {
        let m = self.modifier(room, tp);
        if (m as i8) <= 0 {
            if m == 0 {
                self.add_trob(room, tp, 1);
                self.sound(49);
            } else if m != 255 {
                self.set_mod(room, tp, 0x8F);
            }
        }
    }

    /// make_loose_fall (0000:94CD)
    fn make_loose_fall(&mut self, room: u8, tp: u8, m: u8) {
        if !(1..=24).contains(&room) || tp as usize >= TILES_PER_ROOM {
            return;
        }
        let cur = self.modifier(room, tp);
        if self.level.fg_byte(room as usize, tp as usize) & 0x20 == 0 && (cur as i8) <= 0 {
            self.set_mod(room, tp, m);
            self.add_trob(room, tp, 0);
        }
    }

    /// do_knock (0000:95D8): shake every loose floor of a row
    fn do_knock(&mut self, room: u8, row: i32) {
        if !(0..ROWS as i32).contains(&row) {
            return;
        }
        for col in 0..COLS as u8 {
            let tp = row as u8 * COLS as u8 + col;
            if self.kind(room, tp) == Tile::LooseFloor && self.modifier(room, tp) == 0 && self.level_no != 13 {
                self.set_mod(room, tp, 0x80);
                self.add_trob(room, tp, 1);
            }
        }
    }

    /// start_chompers (0000:950B)
    fn start_chompers(&mut self, room: u8, row: i32) {
        if !(0..ROWS as i32).contains(&row) || !(1..=24).contains(&room) {
            return;
        }
        let mut timing: u8 = 15;
        for col in 0..COLS as u8 {
            let tp = row as u8 * COLS as u8 + col;
            if self.kind(room, tp) != Tile::Chomper {
                continue;
            }
            let m = self.modifier(room, tp);
            if m & 0x7F == 0 || m & 0x7F >= 6 {
                if m == 0 || m >= 6 {
                    self.set_mod(room, tp, timing | (m & 0x80));
                    self.add_trob(room, tp, 1);
                }
                timing = timing.wrapping_sub(3);
                if timing < 6 {
                    timing = timing.wrapping_add(10);
                }
            }
        }
    }

    // ------------------------------------------------------------------ mobs (falling loose floors)

    fn do_mobs(&mut self) {
        let n = self.mobs.len();
        for i in 0..n {
            let mut m = self.mobs[i];
            self.move_mob(&mut m);
            self.check_loose_fall_on_kid(&m);
            self.mobs[i] = m;
            self.dirty = true;
        }
        self.mobs.retain(|m| m.speed != -1);
    }

    fn move_mob(&mut self, m: &mut Mob) {
        if m.ty == 0 {
            self.move_loose(m);
        }
        if m.speed <= 0 {
            m.speed = m.speed.wrapping_add(1);
        }
    }

    /// move_loose (0000:971E)
    fn move_loose(&mut self, m: &mut Mob) {
        if m.speed < 0 {
            return;
        }
        if m.speed < 29 {
            m.speed += 3;
        }
        m.y = m.y.wrapping_add(m.speed as u8);
        if m.room == 0 {
            if m.y >= 210 {
                m.speed = -2;
            }
            return;
        }
        if m.y >= 226 || (m.y as i32) < Y_SOMETHING[(m.row as usize + 1).min(4)] {
            return;
        }
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        let t = self.kind(m.room, idx);
        if t == Tile::LooseFloor {
            self.loose_fall(m);
        }
        if matches!(t, Tile::Empty | Tile::LooseFloor) {
            self.mob_down_a_row(m);
            return;
        }
        self.sound(2);
        self.do_knock(m.room, m.row as i32);
        m.y = Y_SOMETHING[m.row as usize + 1] as u8;
        m.speed = -2;
        self.loose_land(m);
    }

    /// loose_fall (0000:98C3): the floor the mob hits is knocked loose too (approximation: the copy is dropped
    /// when it would start below row 2).
    fn loose_fall(&mut self, m: &mut Mob) {
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        self.set_tile(m.room, idx, Tile::Empty);
        self.set_mod(m.room, idx, 0);
        m.speed >>= 1;
        if m.row < 2 {
            self.add_mob(Mob { xh: m.xh, y: m.y.wrapping_add(6), room: m.room, speed: m.speed, ty: 0, row: m.row + 1 });
        }
    }

    /// mob_down_a_row (0000:997F)
    fn mob_down_a_row(&mut self, m: &mut Mob) {
        m.row += 1;
        if m.row >= 3 {
            m.y = m.y.wrapping_sub(192);
            m.row = 0;
            m.room = if (1..=24).contains(&m.room) { self.level.links[m.room as usize - 1][3] } else { 0 };
        }
    }

    /// loose_land (0000:97E0)
    fn loose_land(&mut self, m: &Mob) {
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        match self.kind(m.room, idx) {
            Tile::RaiseButton => {
                self.set_tile(m.room, idx, Tile::Debris);
                self.trigger_button(true, 14, -1, m.room, idx);
            }
            Tile::DropButton => {
                self.trigger_button(true, 0, -1, m.room, idx);
                self.set_tile(m.room, idx, Tile::Debris);
            }
            Tile::Floor | Tile::Spikes | Tile::Potion => self.set_tile(m.room, idx, Tile::Debris),
            Tile::Torch | Tile::TorchDebris => self.set_tile(m.room, idx, Tile::TorchDebris),
            _ => {}
        }
    }

    /// check_loose_fall_on_kid (0000:9B89) / fell_on_your_head (0000:9BCB)
    fn check_loose_fall_on_kid(&mut self, m: &Mob) {
        let k = &self.kid;
        if m.room != k.room || !k.alive {
            return;
        }
        let col = {
            let env = Env { exe: &self.data.exe, frames: &self.data.frames, level: &self.level, sizes: &self.data.sizes };
            k.col(&env)
        };
        let ky = k.y as i32;
        if (m.xh >> 2) as i32 != col || !((m.y as i32) < ky && ky - 30 < m.y as i32) {
            return;
        }
        let (f, a) = (k.frame, k.action);
        if !((self.level_no == 13 || f < 5 || f >= 15) && (a < 2 || a == 7)) {
            return;
        }
        let env = Env { exe: &self.data.exe, frames: &self.data.frames, level: &self.level, sizes: &self.data.sizes };
        let k = &mut self.kid;
        k.y = Y_LAND[(k.row + 1).clamp(0, 4) as usize] as u8;
        k.hp -= 1;
        let dir = if k.facing_left { -1 } else { 1 };
        if k.hp <= 0 {
            k.alive = false;
            if f == 177 {
                k.x = (k.x as i32 - 12 * dir) as u8;
            }
            k.start_seq(&env, 22);
        } else if f != 109 {
            let behind = env.level.tile_wrapped(k.room as i32, col - dir, k.row);
            if behind == Tile::Empty {
                k.x = (k.x as i32 - 2 * dir) as u8;
            }
            k.start_seq(&env, 52);
        }
    }
}

#[cfg(test)]
impl World {
    pub(crate) fn set_mod_for_test(&mut self, room: u8, idx: u8, m: u8) {
        self.set_mod(room, idx, m);
    }
    pub(crate) fn start_spike_for_test(&mut self, room: u8, idx: u8) {
        self.start_anim_spike(room, idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kid::Keys;

    #[test]
    fn logic_runs_every_fifth_step() {
        // a World needs game data; the timing logic is tested through the counters only
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let exe = Exe::open(format!("{dir}PRINCE.EXE")).unwrap();
        let frames = exe.kid_frames();
        let data = Arc::new(Data { exe, frames, sizes: vec![(0, 0); 220] });
        let lv = crate::dat::Dat::open(format!("{dir}LEVELS.DAT")).unwrap();
        let level = Level::parse(lv.get(2002).unwrap()).unwrap();
        let mut w = World::new(data, level, 2);
        let ran: Vec<bool> = (0..10).map(|_| w.step(Keys::default())).collect();
        assert_eq!(ran, [false, false, false, false, true, false, false, false, false, true]);
        assert_eq!(w.tick, 2);
        assert_eq!(w.alpha(), 0.0);
        w.step(Keys::default());
        assert_eq!(w.alpha(), 0.2);
        assert_eq!(STEP_HZ, 60.0);
    }
}
