//! Guards, the shadow, the skeleton and the sword fight at world level (docs/combat.md sections 1, 2, 5, 7).
use super::*;
use crate::kid::{guard_hp, is_floor, norm, tile_n, Ctrl, Input, Opp, Rng, REFRACTIMER};

/// tbl_guard_type (ds:02F0) per level: 0 guard, 1 fat guard, 2 skeleton, 3 vizier, 4 shadow, -1 none
pub const GUARD_TYPE: [i8; 16] = [0, 0, 0, 2, 0, 0, 1, 0, 0, 0, 0, 0, 4, 3, -1, -1];

/// One guard slot per room: the level tail guard fields while they are not in play (docs/combat.md 2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardSlot {
    /// 0..29 = a guard waits here, >= 30 = none
    pub tile: u8,
    pub x: u8,
    /// 0 right, 255 left
    pub dir: u8,
    pub skill: u8,
    pub colour: u8,
    /// a dead guard lies here: (seq number, raw sequence pointer `Kid::seq_ptr`) saved by leave_guard
    pub dead: Option<(usize, (u16, usize))>,
}

pub const NO_SLOT: GuardSlot = GuardSlot { tile: 0xFF, x: 0, dir: 0, skill: 0, colour: 0, dead: None };

/// State of the scripted shadows (docs/combat.md 7.1)
#[derive(Clone, Copy, Debug, Default)]
pub struct ShadowState {
    pub demo_time: u8,
    pub demo_index: usize,
    pub initialized: bool,
    pub clear: bool,
    pub unite: bool,
    pub level6_done: bool,
    /// united with the level 12 shadow (42 ticks counting down, then -1 = hidden floor trick)
    pub united: i32,
}

pub fn mk_env<'a>(data: &'a Data, level: &'a Level, gtype: i8) -> Env<'a> {
    let gs: &[(u16, u16)] = if gtype >= 0 { data.gsizes.get(gtype as usize).map_or(&[], |v| v.as_slice()) } else { &[] };
    Env { exe: &data.exe, frames: &data.frames, level, sizes: &data.sizes, gframes: &data.gframes, gsizes: gs }
}

/// The level-5 shadow's scripted moves (ds:0F16): (time, move)
const SHADOW5_MOVES: [(i16, i16); 8] = [(0, 0), (1, 1), (14, 0), (18, 6), (29, 7), (45, 2), (49, 1), (255, -2)];

/// do_auto_moves (0000:4633): the index advances by at most one entry per call. Returns the move applied.
fn do_auto_moves(table: &[(i16, i16)], st: &mut ShadowState, c: &mut Ctrl) -> i16 {
    if st.demo_time >= 254 {
        return -2;
    }
    st.demo_time += 1;
    let i = if table[st.demo_index.min(table.len() - 1)].0 <= st.demo_time as i16 {
        st.demo_index += 1;
        st.demo_index - 1
    } else {
        st.demo_index.saturating_sub(1)
    };
    let m = table[i.min(table.len() - 1)].1;
    match m {
        0 => *c = Ctrl::default(),
        1 => c.move_forward(),
        2 => c.move_backward(),
        3 => c.move_up(),
        4 => c.move_down(),
        5 => {
            c.move_up();
            c.move_forward();
        }
        6 => c.move_shift(),
        7 => (c.shift, c.shift2) = (0, 0),
        _ => {} // -1 sets a flag nothing reads; others do nothing
    }
    m
}

/// autocontrol_shadow (0000:3F11) for the levels 5, 6 and 12 (level 4's mirror shadow is not simulated).
fn shadow_moves(g: &mut Kid, env: &Env, level_no: usize, st: &mut ShadowState, rng: &mut Rng) -> Ctrl {
    let mut c = Ctrl::default();
    match level_no {
        5 => {
            // 0000:470E
            if g.room == 24 {
                if st.demo_time == 0 {
                    if env.level.modifier_wrapped(24, 1, 0) < 80 {
                        return c; // waits for the gate; x is not checked meanwhile
                    }
                    st.demo_index = 0;
                }
                do_auto_moves(&SHADOW5_MOVES, st, &mut c);
                if g.x < 15 {
                    st.clear = true;
                }
            }
        }
        6 => {
            if g.room == 1 && g.opp.frame == 43 && g.opp.x < 128 {
                c.move_shift();
                c.move_forward();
            }
        }
        12 => {
            if g.room == 15 {
                let d = g.opp_dist();
                if g.sword == 2 {
                    if !g.sh.offguard || g.sh.refrac == 0 {
                        return g.autocontrol_active(env, rng);
                    }
                    c.move_down();
                } else if g.opp.sword >= 2 || !g.sh.offguard {
                    if g.sh.can_see < 2 || d >= 90 {
                        if d < 0 {
                            c.move_backward();
                        }
                    } else if g.frame == 15 {
                        c.move_down_forw();
                    }
                } else if d < 10 {
                    st.unite = true;
                } else if g.sh.can_see == 2 && ((3..=14).contains(&g.opp.frame) || (127..=132).contains(&g.opp.frame)) {
                    c.move_forward();
                }
            }
        }
        _ => {}
    }
    c
}

impl World {
    fn guard_env(&self) -> Env<'_> {
        mk_env(&self.data, &self.level, self.guard_type)
    }

    /// Slots from the level tail, as pos_guards (0000:518B) leaves them: x = 72 + 14 * col.
    pub(super) fn init_guard_slots(level: &Level, level_no: usize) -> [GuardSlot; 24] {
        let mut slots = [NO_SLOT; 24];
        if GUARD_TYPE[level_no.min(15)] < 0 {
            return slots;
        }
        for g in &level.guards {
            let col = (g.tile % 10) as i32;
            slots[g.room as usize - 1] = GuardSlot { tile: g.tile, x: (72 + 14 * col) as u8, dir: g.dir, skill: if g.skill >= 12 { 3 } else { g.skill }, colour: g.colour, dead: None };
        }
        slots
    }

    /// A fighter with the original defaults of enter_guard (0000:3806): action 1, first frame played once.
    #[allow(clippy::too_many_arguments)]
    fn new_fighter(&self, charid: u8, room: u8, x: u8, row: i32, facing_left: bool, skill: u8, hp: i32, sword: u8, seq: usize) -> Kid {
        let env = self.guard_env();
        let mut g = Kid::new_char(&env, charid, room, x, row, facing_left, skill, hp, sword, seq);
        g.sh = self.shared;
        g
    }

    /// enter_guard (0000:3806): a guard waiting in the kid's room steps into play.
    pub(super) fn enter_guard(&mut self, room: u8) {
        if !(1..=24).contains(&room) || GUARD_TYPE[self.level_no.min(15)] < 0 {
            return;
        }
        let slot = self.slots[room as usize - 1];
        if slot.tile >= 30 {
            return;
        }
        let charid = if self.level_no == 3 { 4 } else { 2 };
        let row = (slot.tile / 10) as i32;
        let skill = slot.skill;
        let hp = guard_hp(skill, self.level_no);
        let (sword, seq) = if slot.dead.is_some() { (0, 0) } else if charid == 4 { (2, 63) } else { (0, 77) };
        let mut g = self.new_fighter(charid, room, slot.x, row, slot.dir == 255, skill, hp, sword, seq);
        if let Some((seq, ptr)) = slot.dead {
            // 0000:38D4: the saved pointer is restored as-is and played ONE step; the frame decides dead or alive
            // (a body left mid-death comes back alive with full HP, docs/edge_cases.md 2.2)
            let env = self.guard_env();
            g.seq = seq;
            g.set_seq_ptr(&env, ptr);
            g.play_seq(&env);
            (g.action, g.fall_x, g.fall_y) = (1, 0, 0);
            if matches!(g.frame, 185 | 177 | 178) {
                g.alive = false;
                g.hp = 0;
            }
        }
        g.colour = slot.colour;
        self.shared.droppedout = false;
        self.guard = Some(g);
    }

    /// leave_guard (0000:39E9): the guard stays behind in its room. An alive guard comes back fresh.
    fn leave_guard(&mut self) {
        let Some(g) = self.guard.take() else { return };
        if g.charid == 1 || g.charid == 24 {
            self.guard = Some(g); // the shadow's record stays active
            return;
        }
        if (1..=24).contains(&g.room) {
            let dead = if g.alive { None } else { Some((g.seq, g.seq_ptr())) };
            let colour = g.colour;
            self.slots[g.room as usize - 1] = GuardSlot { tile: 10 * g.row.clamp(0, 2) as u8, x: g.x, dir: if g.facing_left { 255 } else { 0 }, skill: g.skill, colour, dead };
        }
    }

    /// exit_room (0000:3ABB) guard part: after the kid changed room the guard follows or is left behind.
    pub(super) fn guard_after_room_change(&mut self, dir: u8) {
        let kroom = self.kid.room;
        let Some(g) = &self.guard else { return };
        let target_slot = if (1..=24).contains(&kroom) { self.slots[kroom as usize - 1] } else { NO_SLOT };
        let free = target_slot.tile >= 30 || target_slot.dead.is_some();
        let follows = g.alive
            && g.sword == 2
            && free
            && match dir {
                0 => g.x < 91,
                1 => g.x >= 165,
                2 => g.row < 0,
                _ => g.row >= 3,
            };
        if follows {
            // follow_guard (0000:3A92): both rooms lose their guard slot, the guard moves with the kid
            let groom = g.room;
            if (1..=24).contains(&groom) {
                self.slots[groom as usize - 1].tile = 0xFF;
            }
            if (1..=24).contains(&kroom) {
                self.slots[kroom as usize - 1].tile = 0xFF;
            }
            let links = if (1..=24).contains(&groom) { self.level.links[groom as usize - 1] } else { [0; 4] };
            let g = self.guard.as_mut().unwrap();
            g.room = links[dir as usize];
            match dir {
                0 => g.x = (g.x as i32 + 140) as u8,
                1 => g.x = (g.x as i32 - 140) as u8,
                2 => {
                    g.y = g.y.wrapping_add(189);
                    g.row = (g.y as i32 + 60) / 63 % 4 - 1;
                }
                _ => {
                    g.y = g.y.wrapping_sub(189);
                    g.row = (g.y as i32 + 60) / 63 % 4 - 1;
                }
            }
        } else {
            self.leave_guard();
        }
    }

    /// check_the_end / check_shadow (0000:0FE8, 0000:3758): the kid entered a room.
    pub(super) fn check_shadow(&mut self) {
        self.shared.offguard = false;
        let room = self.kid.room;
        match self.level_no {
            6 if room == 1 => {
                if !self.shadow.level6_done {
                    self.shadow.level6_done = true;
                    self.sounds.push(25); // the music plays only once
                }
                self.init_shadow(15, 81, 118, 1, 2);
            }
            5 if room == 24 && self.level.tile_at(24, 3).kind == Tile::Potion => {
                self.init_shadow(15, 55, 55, 0, 2);
            }
            12 if room == 15 && self.shadow.united == 0 && self.level.tile_at(15, 1).kind != Tile::Sword => {
                self.shadow.initialized = false;
                self.init_shadow(15, 81, 232, 0, 7);
            }
            _ => self.enter_guard(room),
        }
    }

    /// do_init_shad (0000:36F4): the shadow of the prince
    fn init_shadow(&mut self, frame: u8, x: u8, y: u8, row: i32, seq: usize) {
        let room = self.kid.room;
        let mut g = self.new_fighter(1, room, x, row, false, 3, 4, 0, seq);
        let env = self.guard_env();
        g.frame = frame;
        g.y = y;
        g.start_seq(&env, seq); // do_init_shad only sets the sequence pointer: the first frame comes next tick
        self.shadow.demo_time = 0; // demo_index is reset by the level-5 autocontrol when the gate opens
        self.guard = Some(g);
    }

    /// check_skel (0000:4513): level 3, the skeleton wakes up when the kid comes close to it
    pub(super) fn check_skel(&mut self) {
        if self.level_no != 3 || self.guard.is_some() || self.kid.room != 1 || self.leveldoor_open == 0 {
            return;
        }
        let col = {
            let env = self.guard_env();
            self.kid.col(&env)
        };
        if (col == 2 || col == 3) && self.level.tile_at(1, 15).kind == Tile::Skeleton {
            self.set_tile(1, 15, Tile::Floor);
            let g = self.new_fighter(4, 1, 142, 1, true, 2, 3, 2, 88);
            self.sounds.push(44);
            self.guard = Some(g);
        }
    }

    /// can_guard_see_kid (check_can_guard_see_kid, 0000:51D1): 0 not seen, 1 seen but blocked, 2 seen and reachable.
    pub(super) fn can_guard_see_kid(&self) -> u8 {
        let Some(g) = &self.guard else { return 0 };
        if g.charid == 24 || (g.charid == 1 && self.level_no != 12) {
            return 0;
        }
        if self.kid.frame == 0 || (219..=228).contains(&self.kid.frame) || !g.alive || !self.kid.alive || g.room != self.kid.room || g.row != self.kid.row {
            return 0;
        }
        let env = self.guard_env();
        let (kc, gc) = (self.kid.col(&env), g.col(&env));
        let (mut l, mut r) = (65 + 14 * kc, 65 + 14 * gc);
        if l > r {
            std::mem::swap(&mut l, &mut r);
        }
        let (room, row) = (self.kid.room as i32, self.kid.row);
        let tile_at = |x: i32| {
            let (rm, c, rw) = norm(&self.level, room, (x - 7 - 58).div_euclid(14), row);
            (tile_n(&self.level, rm, c, rw), self.level.modifier_wrapped(rm, c, rw))
        };
        if tile_at(l).0 == Tile::Chomper {
            l += 14;
        }
        if tile_at(r).0 == Tile::Gate {
            r -= 14;
        }
        let mut result = 2;
        let mut x = l;
        while x <= r {
            let (t, m) = tile_at(x);
            if matches!(t, Tile::Wall | Tile::DoortopWithFloor | Tile::Doortop) {
                return 0;
            }
            if t == Tile::LooseFloor || t == Tile::Chomper || (t == Tile::Gate && m < 112) || !is_floor(t) {
                result = 1;
            }
            x += 14;
        }
        result
    }

    /// play_guard_frame (0000:0F73) with play_guard (0000:7ACD): autocontrol, control, movement and floor checks.
    pub(super) fn play_guard_frame(&mut self) {
        let Some(mut g) = self.guard.take() else { return };
        g.opp = Opp::of(&self.kid);
        g.sh = self.shared;
        let (mut killed, mut clear) = (false, false);
        let level_no = self.level_no;
        let opp = g.opp;
        if super::check_killed_shadow(level_no, &mut g, &opp) {
            self.flash = (15, 5); // the kid died: the level 12 shadow dies with him
        }
        if g.alive && g.hp <= 0 {
            g.alive = false;
            killed = true;
        }
        if !g.alive && g.charid == 1 {
            clear = true;
        }
        if !g.alive && g.charid != 1 {
            g.tick_timers();
        }
        if !clear {
            // level 12: the shadow keeps dropping in until the kid has moved left of x 150
            if level_no == 12 && g.charid == 1 && g.room == 15 && !self.shadow.initialized {
                if g.opp.x >= 150 {
                    let env = mk_env(&self.data, &self.level, self.guard_type);
                    let (x, y) = (81u8, 232u8);
                    g = Kid::new_char(&env, 1, g.room, x, 0, false, 3, 4, 0, 7);
                    g.y = y;
                    g.sh = self.shared;
                    g.opp = Opp::of(&self.kid);
                } else {
                    self.shadow.initialized = true;
                }
            }
            let env = mk_env(&self.data, &self.level, self.guard_type);
            let moves = if !g.alive {
                Ctrl::default()
            } else if g.charid == 1 {
                g.tick_timers();
                shadow_moves(&mut g, &env, level_no, &mut self.shadow, &mut self.rng)
            } else {
                g.autocontrol(&env, &mut self.rng)
            };
            if self.shadow.unite {
                self.kid.copy_char_from(&g); // savekid in the autocontrol: the kid takes over the shadow's record
            }
            g.tick_input(&env, Input::Moves(moves));
            clear |= std::mem::take(&mut g.clear);
        }
        self.shared = g.sh;
        let evs = std::mem::take(&mut g.events);
        for e in evs {
            self.apply_event(e);
        }
        if killed {
            self.on_guard_killed(g.charid);
        }
        if self.shadow.clear {
            self.shadow.clear = false;
            clear = true;
        }
        if self.shadow.unite {
            // united with the shadow: a new heart, full health (add_life, 0000:1217), Char.charid = 0
            self.shadow.unite = false;
            self.kid.hp_max = (self.kid.hp_max + 1).min(10);
            self.kid.hp_delta = self.kid.hp_max - self.kid.hp;
            self.shadow.united = 42;
            self.flash = (15, 18);
            g.charid = 0;
            clear = true;
        }
        self.guard_rec = Opp::of(&g); // saveshad: the record outlives clear_char
        if clear {
            return; // clear_char: the guard record is gone
        }
        self.guard = Some(g);
    }

    /// on_guard_killed (0000:833C)
    fn on_guard_killed(&mut self, charid: u8) {
        if charid == 1 {
            return;
        }
        if self.level_no == 13 {
            self.leveldoor_open = 2;
            self.flash = (15, 18); // 0000:835A
            self.sounds.push(43);
        } else {
            self.sounds.push(37);
        }
    }

    /// check_guard_fallout (0000:395D): a guard that fell out of the room is gone.
    pub(super) fn check_guard_fallout(&mut self) {
        let Some(g) = &self.guard else { return };
        if g.y < 211 {
            return;
        }
        let (charid, action, room) = (g.charid, g.action, g.room);
        if charid == 1 {
            if action == 4 {
                self.guard = None;
            }
            return;
        }
        if charid == 4 {
            let below = if (1..=24).contains(&room) { self.level.links[room as usize - 1][3] } else { 0 };
            if below == 3 {
                // the skeleton lands in room 3 and waits there
                self.slots[2] = GuardSlot { tile: 15, x: 133, dir: 0, skill: 2, colour: 0, dead: None };
                self.guard = None;
                return;
            }
        }
        self.on_guard_killed(charid);
        if (1..=24).contains(&self.kid.room) {
            self.slots[self.kid.room as usize - 1].tile = 0xFF;
        }
        self.guard = None;
    }

    /// check_sword_hurting (0000:440E): the guard strikes first, then the kid.
    pub(super) fn check_sword_hurting(&mut self) {
        if self.kid.frame == 0 || (219..=228).contains(&self.kid.frame) {
            return;
        }
        let Some(g) = self.guard.as_mut() else { return };
        let env = mk_env(&self.data, &self.level, self.guard_type);
        check_hurting(g, &mut self.kid, &env, &mut self.sounds);
        check_hurting(&mut self.kid, g, &env, &mut self.sounds);
    }

    /// check_sword_hurt (0000:43C8)
    pub(super) fn check_sword_hurt(&mut self) {
        let Some(g) = self.guard.as_mut() else { return };
        let env = mk_env(&self.data, &self.level, self.guard_type);
        if g.action == 99 {
            if self.kid.action == 99 {
                self.kid.action = 1; // the kid wins a double hit
            }
            g.hurt_by_sword(&env);
            self.shared.refrac = REFRACTIMER[(g.skill as usize).min(11)];
        } else if self.kid.action == 99 {
            self.kid.hurt_by_sword(&env);
        }
    }

    /// do_delta_hp (0000:12A6)
    pub(super) fn do_delta_hp(&mut self) {
        if let Some(g) = &mut self.guard {
            if self.level_no == 12 && g.charid == 1 && g.hp_delta != 0 {
                self.kid.hp_delta = g.hp_delta; // damage to the shadow hurts the kid (overwrites)
            }
            g.hp = (g.hp + g.hp_delta).clamp(0, g.hp_max);
        }
        self.kid.hp = (self.kid.hp + self.kid.hp_delta).clamp(0, self.kid.hp_max);
    }
}

/// check_hurting (0000:444A): `att` strikes `vic` on its frames 153/154.
fn check_hurting(att: &mut Kid, vic: &mut Kid, env: &Env, sounds: &mut Vec<u8>) {
    att.opp = Opp::of(vic);
    vic.opp = Opp::of(att);
    if att.sword != 2 || att.row != vic.row || !matches!(att.frame, 153 | 154) {
        return;
    }
    let d = att.opp_dist();
    if (0..29).contains(&d) && matches!(vic.frame, 161 | 150) {
        // parried
        vic.frame = 161;
        if att.charid != 0 {
            att.justblocked = 4;
        }
        att.start_seq(env, 69);
        att.play_seq(env);
    } else if att.frame == 154 {
        let min = if vic.sword < 2 { 8 } else { 12 };
        if (min..29).contains(&d) {
            vic.action = 99; // marked as hurt, applied by check_sword_hurt
        } else if vic.frame != 161 {
            sounds.push(11); // sword swish, a miss
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::Dat;
    use crate::kid::Keys;

    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");

    fn data() -> Arc<Data> {
        let exe = crate::exe::Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
        let frames = exe.kid_frames();
        let kid = Dat::open(format!("{DIR}KID.DAT")).unwrap();
        let sizes = crate::image::image_sizes(&kid, 401, 220);
        let gsizes = ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"].iter().map(|f| crate::image::image_sizes(&Dat::open(format!("{DIR}{f}")).unwrap(), 751, 40)).collect();
        Arc::new(Data { gframes: exe.guard_frames(), exe, frames, sizes, gsizes })
    }

    /// Room 1 (rows 1 and 2 floor) with one guard on row 1, column `gcol` facing left; the kid starts at column 2 facing right.
    fn world(level_no: usize, gcol: usize, skill: u8) -> World {
        let mut b = vec![0u8; 2305];
        for r in 1..3 {
            for c in 0..10 {
                b[r * 10 + c] = 1;
            }
        }
        for g in 2119..2143 {
            b[g] = 30;
        }
        b[2119] = (10 + gcol) as u8; // guard tile in room 1
        b[2143] = 255; // facing left
        b[2215] = skill;
        (b[2112], b[2113], b[2114]) = (1, 12, 0); // direction = NOT start_dir, and the opening turn flips it again: the kid ends facing start_dir ... for 0 that is right, toward the guard
        World::new(data(), Level::parse(&b).unwrap(), level_no)
    }

    /// n logic ticks (5 or 6 steps each)
    fn tick(w: &mut World, keys: Keys, n: usize) {
        for _ in 0..n {
            while !w.step(keys) {}
        }
    }

    const NONE: Keys = Keys { left: false, right: false, up: false, down: false, shift: false };

    #[test]
    fn guard_is_placed_like_enter_guard() {
        let w = world(2, 7, 1);
        let g = w.guard.as_ref().expect("guard in the start room");
        assert_eq!((g.charid, g.room, g.row, g.x, g.facing_left, g.sword, g.alive), (2, 1, 1, 170, true, 0, true));
        assert_eq!(g.frame, 166, "inactive guard stands on frame 166");
        assert_eq!((g.hp, g.hp_max), (3, 3), "level 2 guard: 0 + tbl_guard_hp[2]");
        assert_eq!(guard_hp(4, 4), 4, "skill 4 gets one extra hit point");
    }

    #[test]
    fn debug_draw_trace() {
        let mut w = world(2, 7, 1);
        for t in 0..40 {
            tick(&mut w, NONE, 1);
            let g = w.guard.as_ref().unwrap();
            println!("t{t} kid f{} s{} seq{} x{} row{} alive{} have{} | guard f{} s{} seq{} x{} | can_see {} dist {}", w.kid.frame, w.kid.sword, w.kid.seq, w.kid.x, w.kid.row, w.kid.alive, w.kid.have_sword, g.frame, g.sword, g.seq, g.x, w.shared.can_see, w.kid.opp_dist());
        }
    }

    #[test]
    fn guard_and_kid_draw_their_swords() {
        let mut w = world(2, 7, 1);
        tick(&mut w, NONE, 40);
        assert_eq!(w.kid.sword, 2, "the kid draws when a reachable guard is within 90");
        assert_eq!(w.guard.as_ref().unwrap().sword, 2, "the guard draws when it sees the kid");
        assert!(w.sounds.contains(&19));
    }

    #[test]
    fn kid_strike_and_hit_reduces_hit_points() {
        // attacker (kid) on frame 154 at distance 20 from a guard in en garde: the guard is marked hurt, then loses 1 HP
        let mut w = world(2, 5, 3);
        tick(&mut w, NONE, 30);
        let env_gt = w.guard_type;
        let g = w.guard.as_mut().unwrap();
        g.sword = 2;
        g.frame = 158;
        g.x = (w.kid.x as i32 + 5) as u8; // opp_dist adds 13 when facing each other: 18, inside 12..29
        g.facing_left = true;
        w.kid.sword = 2;
        w.kid.facing_left = false;
        w.kid.frame = 154;
        w.kid.action = 1;
        let (hp0, _) = (w.guard.as_ref().unwrap().hp, env_gt);
        w.check_sword_hurting();
        assert_eq!(w.guard.as_ref().unwrap().action, 99, "hit marks the victim");
        w.check_sword_hurt();
        assert_eq!(w.guard.as_ref().unwrap().hp_delta, -1);
        w.do_delta_hp();
        assert_eq!(w.guard.as_ref().unwrap().hp, hp0 - 1);
    }

    #[test]
    fn parry_blocks_the_hit() {
        let mut w = world(2, 5, 3);
        tick(&mut w, NONE, 30);
        let g = w.guard.as_mut().unwrap();
        g.sword = 2;
        g.frame = 150; // parrying
        g.x = (w.kid.x as i32 + 5) as u8;
        g.facing_left = true;
        w.kid.sword = 2;
        w.kid.facing_left = false;
        w.kid.frame = 154;
        w.check_sword_hurting();
        let g = w.guard.as_ref().unwrap();
        assert_eq!(g.frame, 161, "the parried fighter shows frame 161");
        assert_ne!(g.action, 99);
        assert_eq!(w.kid.seq, 69, "the attacker's blow was parried");
    }

    #[test]
    fn a_kid_without_sword_dies_from_one_hit() {
        let mut w = world(2, 5, 3);
        tick(&mut w, NONE, 2);
        w.kid.sword = 0;
        w.kid.have_sword = false;
        let k = &mut w.kid;
        k.action = 99;
        w.check_sword_hurt();
        // the guard is not hurt, the kid is hurt by the sword: take_hp(100)
        assert_eq!(w.kid.hp_delta, -3);
        assert_eq!(w.kid.seq, 85, "stabbed to death (seq 85)");
    }

    #[test]
    fn a_fight_runs_without_panics_and_someone_gets_hurt() {
        let mut w = world(2, 5, 3);
        let mut hurt = false;
        for t in 0..900 {
            // kid: approach, strike, parry now and then
            let keys = match t % 24 {
                0..=7 => Keys { right: true, ..NONE },
                8..=10 => Keys { shift: true, ..NONE },
                11..=14 => Keys { up: true, ..NONE },
                _ => NONE,
            };
            tick(&mut w, keys, 1);
            w.sounds.retain(|&s| s != 0);
            hurt |= w.sounds.iter().any(|&s| s == 12 || s == 13);
            w.sounds.clear();
            if !w.kid.alive || w.guard.as_ref().is_none_or(|g| !g.alive) {
                break;
            }
        }
        assert!(hurt || w.kid.hp < w.kid.hp_max || w.guard.as_ref().is_none_or(|g| g.hp < g.hp_max), "no hit in 900 ticks");
    }

    #[test]
    fn guard_left_behind_comes_back_fresh() {
        let mut w = world(2, 7, 1);
        // the guard is hurt, then the kid leaves the room to the right
        w.guard.as_mut().unwrap().hp = 1;
        w.kid.room = 2;
        w.guard_after_room_change(1);
        assert!(w.guard.is_none());
        assert_eq!(w.slots[0].tile, 10);
        w.kid.room = 1;
        w.enter_guard(1);
        let g = w.guard.as_ref().unwrap();
        assert_eq!((g.hp, g.sword), (3, 0), "full hit points, sword sheathed");
    }

    #[test]
    fn sheathing_makes_the_kid_offguard_and_makes_the_guard_wait() {
        let mut w = world(2, 7, 1);
        tick(&mut w, NONE, 40);
        assert_eq!(w.kid.sword, 2);
        w.kid.frame = 158; // en garde
        w.kid.action = 0;
        tick(&mut w, Keys { down: true, ..NONE }, 1);
        assert_eq!(w.kid.sword, 0, "sheathed");
        assert!(w.shared.offguard, "offguard stops the automatic drawing");
        assert!((8..=9).contains(&w.shared.refrac), "guard_refrac = 9 (one tick already counted): {}", w.shared.refrac);
    }

    #[test]
    fn shadow_united_counts_down_to_minus_one() {
        let mut w = world(12, 7, 1);
        w.shadow.united = 3;
        tick(&mut w, NONE, 5);
        assert_eq!(w.shadow.united, -1);
    }

    // ------------------------------------------------------------ docs/edge_cases.md test vectors

    #[test]
    fn frame_length_is_six_steps_while_the_sword_is_drawn() {
        let mut w = world(2, 7, 1);
        tick(&mut w, NONE, 40);
        assert_eq!(w.kid.sword, 2);
        let mut steps = 0;
        for _ in 0..30 {
            let s = w.kid.sword;
            let mut n = 1;
            while !w.step(NONE) {
                n += 1;
            }
            assert_eq!(n, if s == 2 { 6 } else { 5 });
            steps += n;
        }
        assert_eq!(steps, 180, "30 ticks with the sword drawn = 180 steps at 60 Hz");
        w.step(NONE);
        assert!((w.alpha() - 1.0 / 6.0).abs() < 1e-6);
    }

    /// The level-1 guard of `world` put en garde (frame 171 loop of seq 63) at (x, row 1).
    fn en_garde(w: &World, x: u8, left: bool) -> Kid {
        let env = w.guard_env();
        let mut g = w.guard.clone().unwrap();
        g.force(&env, x, 118, 1, left, 171, 1, 63, (0, 0));
        for _ in 0..3 {
            g.play_seq(&env);
        }
        g.sword = 2;
        g.sh.drawn_room = g.room;
        g.sh.can_see = 0;
        g
    }

    fn gtick(w: &World, g: &mut Kid) {
        let env = w.guard_env();
        g.tick_input(&env, Input::Moves(Ctrl::default()));
    }

    /// First x for which the guard (after the next play_seq) stands in `col` and `ok(left_coll, right_coll, kid)` holds.
    fn find_x(w: &World, mk: impl Fn(u8) -> Kid, col: i32, ok: impl Fn(i32, i32, &Kid) -> bool) -> Kid {
        let env = w.guard_env();
        (40u8..220)
            .map(&mk)
            .find(|g| {
                let mut p = g.clone();
                p.play_seq(&env);
                let (l, r) = p.box_lr(&env);
                p.col(&env) == col && ok(l, r, &p)
            })
            .expect("no x found")
    }

    /// A guard retreating (seq 57) on f160: the next play_seq shows f157 (the box of en garde f171 cannot reach a
    /// doortop behind it when facing right).
    fn retreating(w: &World, x: u8, left: bool) -> Kid {
        let env = w.guard_env();
        let mut g = en_garde(w, x, left);
        g.start_seq(&env, 57);
        g.play_seq(&env);
        g
    }

    #[test]
    fn guard_bumped_vectors_1_1() {
        // A: facing left, own column 3 is a wall: dist = 0 + 107 - right_coll, shoved forward by |dist|, seq 65
        let mut w = world(2, 7, 1);
        w.set_tile(1, 13, Tile::Wall);
        let mut g = find_x(&w, |x| en_garde(&w, x, true), 3, |_, r, _| (-12..0).contains(&(107 - r)));
        let x0 = g.x as i32;
        let r = {
            let env = w.guard_env();
            let mut p = g.clone();
            p.play_seq(&env);
            p.box_lr(&env).1
        };
        gtick(&w, &mut g);
        assert_eq!((g.seq, g.frame, g.x as i32), (65, 160, x0 + (107 - r)));
        // E: sword sheathed: nothing
        let mut g = find_x(&w, |x| en_garde(&w, x, true), 3, |_, r, _| (-12..0).contains(&(107 - r)));
        g.sword = 0;
        gtick(&w, &mut g);
        assert_ne!(g.seq, 65);

        // B: facing right, own column 3 floor, column 2 doortop: dist = 0 + left_coll - 93 - 13
        let mut w = world(2, 7, 1);
        w.set_tile(1, 12, Tile::DoortopWithFloor);
        let mut g = find_x(&w, |x| retreating(&w, x, false), 3, |l, _, _| (-12..0).contains(&(l - 93 - 13)));
        let (x0, l) = {
            let env = w.guard_env();
            let mut p = g.clone();
            p.play_seq(&env);
            (p.x as i32, p.box_lr(&env).0)
        };
        gtick(&w, &mut g);
        assert_eq!((g.seq, g.x as i32), (65, x0 - (l - 106)));

        // C: same, but column 2 is a plain wall: walls count only in the own column
        let mut w = world(2, 7, 1);
        w.set_tile(1, 12, Tile::Wall);
        let mut g = find_x(&w, |x| retreating(&w, x, false), 3, |l, _, _| (-12..0).contains(&(l - 93 - 13)));
        gtick(&w, &mut g);
        assert_eq!((g.seq, g.frame), (57, 157), "no bump");
    }

    #[test]
    fn guard_chomped_vector_1_2() {
        let mut w = world(2, 7, 1);
        w.set_tile(1, 14, Tile::Chomper);
        w.set_mod(1, 14, 2);
        for (left, want_x) in [(false, 127), (true, 114)] {
            let fresh = |x: u8| {
                let env = w.guard_env();
                let mut g = w.guard.clone().unwrap();
                g.force(&env, x, 118, 1, left, 166, 1, 77, (0, 0));
                g.sh.drawn_room = g.room;
                g
            };
            let mut g = find_x(&w, fresh, 4, |l, r, _| 120 < r && 121 > l);
            gtick(&w, &mut g);
            assert_eq!((g.x, g.y, g.seq, g.frame, g.hp_delta), (want_x, 118, 54, 178, -g.hp));
            assert!(g.events.contains(&Event::Sound(46)));
            assert!(g.events.contains(&Event::Chomped { room: 1, tilepos: 14 }));
        }
    }

    #[test]
    fn guard_falls_off_a_ledge_1_3() {
        // backing off (fall_x -1): seq 82; one row down, landing en garde (seq 63) with fall_y 21
        let mut w = world(2, 7, 1);
        let mut g = en_garde(&w, 130, true);
        g.fall_x = -1;
        let col = {
            let env = w.guard_env();
            let mut p = g.clone();
            p.play_seq(&env);
            p.col(&env)
        };
        w.set_tile(1, 10 + col as u8, Tile::Empty);
        let mut trace = vec![];
        for _ in 0..6 {
            gtick(&w, &mut g);
            trace.push((g.frame, g.y));
        }
        assert_eq!(trace, [(102, 117), (103, 123), (104, 132), (105, 144), (106, 162), (158, 181)]);
        assert_eq!((g.seq, g.sword, g.row, g.fall_y), (63, 2, 2, 0));

        // fall_x 0 (never moved): seq 83 and droppedout cleared; the skeleton takes the kid's branch (81, droppedout set)
        for (charid, fx, seq, dropped) in [(2, 0, 83, false), (4, -1, 81, true)] {
            let mut g = en_garde(&w, 130, true);
            (g.charid, g.fall_x, g.sh.droppedout) = (charid, fx, !dropped);
            gtick(&w, &mut g);
            assert_eq!((g.seq, g.sh.droppedout, g.sword), (seq, dropped, 0));
        }
    }

    #[test]
    fn guard_landing_from_two_rows_is_fatal() {
        let w = world(2, 7, 1);
        for (charid, seq, dhp) in [(2, 22, -3), (4, 20, -1)] {
            let env = w.guard_env();
            let mut g = w.guard.clone().unwrap();
            g.force(&env, 120, 100, 1, false, 106, 4, 12, (0, 27));
            (g.charid, g.sh.drawn_room) = (charid, g.room);
            gtick(&w, &mut g);
            assert_eq!((g.seq, g.hp_delta), (seq, dhp), "charid {charid}: fall_y 30 lands");
        }
    }

    #[test]
    fn death_placement_vectors_1_4() {
        // guard facing left, killed en garde in column 5; behind = column 6
        for (own, behind, dist9, want_dx, want_seq) in
            [(Tile::Floor, Tile::Gate, false, None, 85), (Tile::Gate, Tile::Floor, false, Some(125), 85), (Tile::Floor, Tile::Floor, false, Some(-1), 85), (Tile::Floor, Tile::Empty, true, Some(-2), 81)]
        {
            let mut w = world(2, 7, 1);
            w.set_tile(1, 15, own);
            w.set_tile(1, 16, behind);
            let mut g = find_x(&w, |x| en_garde(&w, x, true), 5, |_, _, p| {
                let env = w.guard_env();
                let d = (p.foot_x(&env) - 65).rem_euclid(14);
                if dist9 { d == 9 } else { d >= 4 }
            });
            let env = w.guard_env();
            g.play_seq(&env);
            let (x0, row0) = (g.x as i32, g.row);
            g.hp = 1;
            g.hurt_by_sword(&env);
            let want_x = match want_dx {
                None => 139,
                Some(-1) => x0,
                Some(-2) => x0 + 11,
                Some(x) => x,
            };
            assert_eq!((g.x as i32, g.seq), (want_x, want_seq), "own {own:?} behind {behind:?}");
            assert_eq!(g.row, if want_seq == 81 { row0 + 1 } else { row0 });
        }
    }

    #[test]
    fn dead_guards_come_back_on_their_resting_frame() {
        // (seq, plays, frame): resting pointers 0x20CF, 0x20DA, 0x20E2, 0x20BA; one play_seq after re-entry shows the frame
        for (seq, plays, frame) in [(71, 10, 185), (51, 6, 177), (54, 5, 178), (22, 8, 185)] {
            let mut w = world(2, 7, 1);
            {
                let env = mk_env(&w.data, &w.level, w.guard_type);
                let g = w.guard.as_mut().unwrap();
                g.start_seq(&env, seq);
                for _ in 0..plays {
                    g.play_seq(&env);
                }
                (g.alive, g.hp, g.x, g.row) = (false, 0, 150, 1);
                assert_eq!(g.frame, frame);
            }
            w.kid.room = 2;
            w.guard_after_room_change(1);
            assert!(w.guard.is_none() && w.slots[0].dead.is_some());
            assert_eq!((w.slots[0].tile, w.slots[0].x), (10, 150));
            w.kid.room = 1;
            w.enter_guard(1);
            let g = w.guard.as_ref().unwrap();
            assert_eq!((g.frame, g.alive, g.x, g.seq), (frame, false, 150, seq), "seq {seq}");
        }
        // left mid-death (seq 71 on f180): back alive with full HP, the animation continues
        let mut w = world(2, 7, 1);
        {
            let env = mk_env(&w.data, &w.level, w.guard_type);
            let g = w.guard.as_mut().unwrap();
            g.start_seq(&env, 71);
            g.play_seq(&env);
            g.play_seq(&env);
            (g.alive, g.hp) = (false, 0);
        }
        w.kid.room = 2;
        w.guard_after_room_change(1);
        w.kid.room = 1;
        w.enter_guard(1);
        let g = w.guard.as_ref().unwrap();
        assert_eq!((g.frame, g.alive, g.hp), (181, true, 3));
    }

    #[test]
    fn check_killed_shadow_rule() {
        let mut k = Kid::default();
        (k.alive, k.hp) = (true, 3);
        let dead_shadow = Opp { charid: 1, alive: false, ..Opp::default() };
        assert!(!super::super::check_killed_shadow(11, &mut k, &dead_shadow));
        assert!(!super::super::check_killed_shadow(12, &mut k, &Opp { alive: true, ..dead_shadow }));
        assert!(!super::super::check_killed_shadow(12, &mut k, &Opp { charid: 2, ..dead_shadow }));
        assert!(super::super::check_killed_shadow(12, &mut k, &dead_shadow));
        assert_eq!(k.hp_delta, -3);
    }

    #[test]
    fn level5_shadow_auto_moves_5_4() {
        let mut st = ShadowState::default();
        let moves: Vec<i16> = (0..60).map(|_| do_auto_moves(&SHADOW5_MOVES, &mut st, &mut Ctrl::default())).collect();
        let want = |a: usize, b: usize, m: i16| moves[a - 1..b].iter().all(|&x| x == m);
        assert_eq!(moves[0], 0);
        assert!(want(2, 13, 1) && want(14, 17, 0) && want(18, 28, 6) && want(29, 44, 7) && want(45, 48, 2) && want(49, 60, 1), "{moves:?}");
        for _ in 0..300 {
            do_auto_moves(&SHADOW5_MOVES, &mut st, &mut Ctrl::default());
        }
        assert_eq!(st.demo_time, 254);
    }

    #[test]
    fn take_hp_sets_the_delta() {
        let mut w = world(2, 7, 1);
        assert!(!w.kid.take_hp(1));
        assert_eq!(w.kid.hp_delta, -1);
        assert!(w.kid.take_hp(100));
        assert_eq!(w.kid.hp_delta, -3);
    }
}
