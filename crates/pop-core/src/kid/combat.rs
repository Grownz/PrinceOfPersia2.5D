//! Sword fight and guard AI (docs/combat.md sections 3-5). Child module of `kid` (shares the private character code).
use super::*;

/// The other fighter, as seen by this character (docs/combat.md 0: the `Opp` record). Copied in by the world each tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Opp {
    pub present: bool,
    pub room: u8,
    pub x: u8,
    pub y: u8,
    pub facing_left: bool,
    pub frame: u8,
    pub action: u8,
    pub row: i32,
    pub sword: u8,
    pub alive: bool,
    pub charid: u8,
}

impl Opp {
    pub fn of(k: &Kid) -> Opp {
        Opp { present: true, room: k.room, x: k.x, y: k.y, facing_left: k.facing_left, frame: k.frame, action: k.action, row: k.row, sword: k.sword, alive: k.alive, charid: k.charid }
    }
}

/// State both fighters read and write (docs/combat.md 3.4, 4.1): the world copies it in and out around each tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shared {
    /// can_guard_see_kid: 0 not seen, 1 seen but blocked, 2 seen and reachable
    pub can_see: u8,
    /// set to 15 when the kid presses strike; guards of skill >= 1 do not advance while it runs
    pub kid_sword_strike: u8,
    /// the kid sheathed his sword in front of a guard: no automatic drawing until the guard is out of sight
    pub offguard: bool,
    pub holding_sword: bool,
    /// the kid made a noise (sequence op, bump, button): a sleeping guard turns around
    pub guard_notice: bool,
    /// the kid fell with his sword drawn: the guard follows
    pub droppedout: bool,
    pub notice_timer: u8,
    /// guard_refrac (0000:436E): no advance or strike while it runs (set after a hit, or 9 when the kid sheathes)
    pub refrac: u8,
    pub level: usize,
    /// the room the kid is in (= drawn room)
    pub drawn_room: u8,
}

/// prandom (0C83:0412): the original's linear congruential generator.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self, max: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        (self.0 >> 16) % (max + 1)
    }
}

// Skill tables (docs/combat.md 3.4), word per skill 0..11; the test is `table[skill] > prandom(255)`.
pub const STRIKEPROB: [u8; 12] = [75, 100, 75, 75, 75, 50, 100, 220, 0, 60, 40, 60];
pub const RESTRIKEPROB: [u8; 12] = [0, 0, 0, 5, 5, 175, 20, 10, 0, 255, 255, 150];
pub const BLOCKPROB: [u8; 12] = [0, 150, 150, 200, 200, 255, 200, 250, 0, 255, 255, 255];
pub const IMPBLOCKPROB: [u8; 12] = [0, 75, 75, 100, 100, 145, 100, 250, 0, 145, 255, 175];
pub const ADVPROB: [u8; 12] = [255, 200, 200, 200, 255, 255, 200, 0, 0, 255, 100, 100];
pub const REFRACTIMER: [u8; 12] = [20, 20, 20, 20, 10, 10, 10, 10, 0, 10, 0, 0];
pub const EXTRASTRENGTH: [u8; 12] = [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
/// tbl_guard_hp (ds:0EEE), byte per level 0..15
pub const GUARD_HP_LEVEL: [u8; 16] = [4, 3, 3, 3, 3, 4, 5, 4, 4, 5, 5, 5, 4, 6, 0, 0];

/// Hit points of a guard of `skill` on `level` (get_guard_hp, 0000:3738).
pub fn guard_hp(skill: u8, level: usize) -> i32 {
    EXTRASTRENGTH[(skill as usize).min(11)] as i32 + GUARD_HP_LEVEL[level.min(15)] as i32
}

// move helpers (0000:3E15..3E64): they set the same control flags the kid's keys do (forward is relative to facing)
impl Ctrl {
    pub(crate) fn move_forward(&mut self) {
        self.x = -1;
        self.fwd = -1;
    }
    pub(crate) fn move_backward(&mut self) {
        self.bwd = -1;
        self.x = 1;
    }
    pub(crate) fn move_up(&mut self) {
        self.y = -1;
        self.up = -1;
    }
    pub(crate) fn move_down(&mut self) {
        self.down = -1;
        self.y = 1;
    }
    pub(crate) fn move_down_back(&mut self) {
        self.down = -1;
        self.move_backward();
    }
    pub(crate) fn move_down_forw(&mut self) {
        self.down = -1;
        self.move_forward();
    }
    pub(crate) fn move_shift(&mut self) {
        self.shift = -1;
        self.shift2 = -1;
    }
}

impl Kid {
    /// char_opp_dist (0000:85A6): positive = the opponent is in front. 999 = no opponent in this room.
    pub fn opp_dist(&self) -> i32 {
        let o = &self.opp;
        if !o.present || o.room != self.room {
            return 999;
        }
        let mut d = o.x as i32 - self.x as i32;
        if self.facing_left {
            d = -d;
        }
        if d >= 0 && self.facing_left != o.facing_left {
            d += 13;
        }
        d
    }

    fn facing_each_other(&self) -> bool {
        self.facing_left != self.opp.facing_left
    }

    // ---------------------------------------------------------------- guard AI (3.2, 3.3)

    /// autocontrol_opponent (0000:3E6A) for guards and skeletons: returns the control flags for this tick.
    /// (The shadow, mouse and the demo are driven by the world.)
    pub fn autocontrol(&mut self, env: &Env, rng: &mut Rng) -> Ctrl {
        self.tick_timers();
        let mut c = Ctrl::default();
        if self.charid == 4 {
            self.sword = 2;
        }
        if self.sword < 2 {
            self.guard_inactive(&mut c);
        } else {
            self.guard_active(env, rng, &mut c);
        }
        c
    }

    /// The timers autocontrol_opponent counts down once per opponent tick (0000:3E85..3EA6).
    pub(crate) fn tick_timers(&mut self) {
        if self.justblocked > 0 {
            self.justblocked -= 1;
        }
        if self.sh.kid_sword_strike > 0 {
            self.sh.kid_sword_strike -= 1;
        }
        if self.sh.refrac > 0 {
            self.sh.refrac -= 1;
        }
    }

    /// autocontrol_guard_active without the timers (the level 12 shadow uses it).
    pub(crate) fn autocontrol_active(&mut self, env: &Env, rng: &mut Rng) -> Ctrl {
        let mut c = Ctrl::default();
        self.guard_active(env, rng, &mut c);
        c
    }

    /// A fighter with the defaults of enter_guard (0000:3806): action 1, first frame of `seq` shown.
    #[allow(clippy::too_many_arguments)]
    pub fn new_char(env: &Env, charid: u8, room: u8, x: u8, row: i32, facing_left: bool, skill: u8, hp: i32, sword: u8, seq: usize) -> Kid {
        let mut k = Kid {
            charid,
            room,
            x,
            y: Y_LAND[(row + 1).clamp(0, 4) as usize] as u8,
            row,
            facing_left,
            frame: 166,
            action: 1,
            alive: true,
            hp,
            hp_max: hp,
            skill,
            sword,
            controlled: true,
            bump_left: -1,
            bump_right: -1,
            ..Kid::default()
        };
        k.start_seq(env, seq);
        k.play_seq(env);
        k.refresh_snap(env);
        k
    }

    /// autocontrol_guard_inactive (0000:3F6A): a sheathed guard notices the kid and draws
    fn guard_inactive(&mut self, c: &mut Ctrl) {
        if !self.opp.alive {
            return;
        }
        let d = self.opp_dist();
        let see = if self.opp.row == self.row && (-8..=-1).contains(&d) {
            true
        } else if self.sh.guard_notice {
            self.sh.guard_notice = false;
            if d >= 0 {
                true
            } else {
                if d < -4 {
                    c.move_down(); // turn around
                }
                return;
            }
        } else if d < 0 {
            return;
        } else {
            true
        };
        if see && self.sh.can_see != 0 && (self.sh.level != 13 || self.sh.notice_timer == 0) {
            c.move_down_forw();
        }
    }

    /// autocontrol_guard_active (0000:3FD0)
    fn guard_active(&mut self, env: &Env, rng: &mut Rng, c: &mut Ctrl) {
        if self.frame == 166 || self.frame < 150 || self.sh.can_see == 1 {
            return;
        }
        if self.sh.can_see == 0 {
            if self.sh.droppedout {
                self.guard_follows_kid_down(env, c);
            } else if self.charid != 4 {
                c.move_down_back(); // put the sword away
            }
            return;
        }
        let d = self.opp_dist();
        let of = self.opp.frame;
        if d >= 12 && (102..=117).contains(&of) && self.opp.action == 5 {
            return;
        }
        if d < 35 {
            if (self.sword < 2 && d < 8) || d < 12 {
                if !self.facing_each_other() { c.move_backward() } else { c.move_forward() }
            } else {
                self.kid_in_sight(rng, c, d);
            }
        } else if self.sh.refrac == 0 {
            let strike_window = ((7..=14).contains(&of) && d < 40) || ((34..=43).contains(&of) && d < 50);
            let in_ranges = (7..=14).contains(&of) || (34..=43).contains(&of);
            if self.facing_each_other() && strike_window {
                c.move_shift();
            } else if self.facing_each_other() && in_ranges {
                // facing and coming, but still far: wait
            } else {
                self.kid_far(env, c);
            }
        }
    }

    /// autocontrol_guard_kid_in_sight (0000:4187)
    fn kid_in_sight(&mut self, rng: &mut Rng, c: &mut Ctrl, d: i32) {
        if self.opp.sword == 2 {
            self.kid_armed(rng, c, d);
        } else if self.sh.refrac == 0 {
            if d < 29 { c.move_shift() } else { c.move_forward() }
        }
    }

    /// autocontrol_guard_kid_armed (0000:41B5)
    fn kid_armed(&mut self, rng: &mut Rng, c: &mut Ctrl, d: i32) {
        if d < 10 || d >= 29 {
            self.guard_advance(rng, c);
        } else {
            self.guard_block(rng, c);
            if self.sh.refrac == 0 {
                if d < 12 || d >= 29 {
                    self.guard_advance(rng, c);
                } else {
                    self.guard_strike(rng, c);
                }
            }
        }
    }

    fn skill_idx(&self) -> usize {
        (self.skill as usize).min(11)
    }

    /// guard_advance (0000:41E9)
    fn guard_advance(&mut self, rng: &mut Rng, c: &mut Ctrl) {
        if self.skill == 0 || self.sh.kid_sword_strike == 0 {
            let r = rng.next(255);
            if ADVPROB[self.skill_idx()] as u32 > r {
                c.move_forward();
            }
        }
    }

    /// guard_block (0000:4211): parry the kid's strike frames
    fn guard_block(&mut self, rng: &mut Rng, c: &mut Ctrl) {
        if matches!(self.opp.frame, 152 | 153 | 162) {
            let r = rng.next(255);
            let p = if self.justblocked > 0 { IMPBLOCKPROB } else { BLOCKPROB }[self.skill_idx()];
            if p as u32 > r {
                c.move_up();
            }
        }
    }

    /// guard_strike (0000:4267)
    fn guard_strike(&mut self, rng: &mut Rng, c: &mut Ctrl) {
        if matches!(self.opp.frame, 169 | 151) {
            return;
        }
        let r = rng.next(255);
        let p = if matches!(self.frame, 161 | 150) { RESTRIKEPROB } else { STRIKEPROB }[self.skill_idx()];
        if p as u32 > r {
            c.move_shift();
        }
    }

    /// autocontrol_guard_kid_far (0000:40BF): walk on while there is floor ahead, else back off
    fn kid_far(&mut self, env: &Env, c: &mut Ctrl) {
        let (col, d) = (self.col(env), self.dir());
        if is_floor(self.tile(env, col + d, self.row)) || is_floor(self.tile(env, col + 2 * d, self.row)) {
            c.move_forward();
        } else {
            c.move_backward();
        }
    }

    /// guard_follows_kid_down (0000:40EC)
    fn guard_follows_kid_down(&mut self, env: &Env, c: &mut Ctrl) {
        if self.action == 2 || self.action == 6 {
            return;
        }
        let (col, d, row) = (self.col(env), self.dir(), self.row);
        let front = self.tile(env, col + d, row);
        let below = self.tile(env, col + d, row + 1);
        let stop = wall_type(front) != 0
            || (!is_floor(front) && (below == Tile::Spikes || below == Tile::LooseFloor || wall_type(below) != 0 || !is_floor(below) || row + 1 != self.opp.row));
        if stop {
            self.sh.droppedout = false;
            c.move_backward();
        } else {
            c.move_forward();
        }
    }

    // ---------------------------------------------------------------- sword control (4)

    /// control_guard_inactive (0000:857B)
    pub(crate) fn control_guard_inactive(&mut self, env: &Env, c: &mut Ctrl) {
        if self.frame == 166 && c.down == -1 {
            if c.fwd == -1 {
                self.draw_sword(env, c);
            } else {
                c.down = 1;
                self.start_seq(env, 80);
            }
        }
    }

    /// draw_sword (0000:6A89)
    pub(crate) fn draw_sword(&mut self, env: &Env, c: &mut Ctrl) {
        let r = c.release_arrows();
        c.shift2 = r;
        c.fwd = r;
        if self.charid == 0 {
            self.events.push(Event::Sound(19));
            self.sh.offguard = false;
            self.start_seq(env, 55);
        } else if self.charid == 1 {
            self.start_seq(env, 55);
        } else {
            self.start_seq(env, 90);
        }
        self.sword = 2;
    }

    /// control_with_sword (0000:6AD3)
    pub(crate) fn control_with_sword(&mut self, env: &Env, c: &mut Ctrl) {
        if self.action >= 2 {
            return;
        }
        if self.tile(env, self.col(env), self.row) == Tile::LooseFloor || self.sh.can_see >= 2 {
            let d = self.opp_dist();
            if (0..90).contains(&d) {
                self.swordfight(env, c);
                return;
            } else if d < -4 {
                self.start_seq(env, 60);
                return;
            } else if d < 0 {
                self.swordfight(env, c);
                return;
            }
        }
        // no opponent in view, or too far
        if self.charid == 0 && self.alive {
            self.sh.holding_sword = false;
        }
        if self.charid >= 2 {
            self.swordfight(env, c);
        } else if self.frame == 171 {
            self.sword = 0;
            self.start_seq(env, 92); // sheathe by himself
        }
    }

    /// swordfight (0000:6B47)
    fn swordfight(&mut self, env: &Env, c: &mut Ctrl) {
        if self.frame == 161 && c.shift2 != -1 {
            self.start_seq(env, 57); // step back after being parried
            return;
        }
        if c.shift2 == -1 {
            if self.charid == 0 {
                self.sh.kid_sword_strike = 15;
            }
            if self.sword_strike(env) {
                c.shift2 = 1;
                return;
            }
        }
        if c.down == -1 {
            // down held: sheathe from the en garde frames, otherwise nothing (no parry or step)
            if matches!(self.frame, 158 | 170 | 171) {
                c.down = 1;
                self.sword = 0;
                if self.charid == 0 {
                    self.sh.offguard = true;
                    self.sh.refrac = 9;
                    self.sh.holding_sword = false;
                    self.start_seq(env, 93); // fast sheathe
                } else if self.charid == 1 {
                    self.start_seq(env, 92);
                } else {
                    self.start_seq(env, 87);
                }
            }
        } else if c.up == -1 {
            self.parry(env, c);
        } else if c.fwd == -1 {
            self.forward_with_sword(env, c);
        } else if c.bwd == -1 {
            self.back_with_sword(env, c);
        }
    }

    /// sword_strike (0000:6C1C): returns true if a strike sequence was started
    fn sword_strike(&mut self, env: &Env) -> bool {
        if matches!(self.frame, 157 | 158 | 170 | 171 | 165) {
            self.start_seq(env, if self.charid == 0 { 75 } else { 58 });
            true
        } else if matches!(self.frame, 150 | 161) {
            self.start_seq(env, 66); // riposte after a parry
            true
        } else {
            false
        }
    }

    fn forward_with_sword(&mut self, env: &Env, c: &mut Ctrl) {
        if matches!(self.frame, 158 | 170 | 171) {
            c.fwd = 1;
            self.start_seq(env, if self.charid == 0 { 86 } else { 56 });
        }
    }

    fn back_with_sword(&mut self, env: &Env, c: &mut Ctrl) {
        if matches!(self.frame, 158 | 170 | 171) {
            c.bwd = 1;
            self.start_seq(env, 57);
        }
    }

    /// parry (0000:6C7B)
    fn parry(&mut self, env: &Env, c: &mut Ctrl) {
        let (mut seq, mut go, mut immediate) = (62, false, false);
        match self.frame {
            158 | 170 | 171 | 168 | 165 => {
                if self.charid != 0 && self.opp_dist() >= 32 {
                    self.back_with_sword(env, c);
                    return;
                }
                if self.charid == 0 {
                    match self.opp.frame {
                        168 => {}
                        153 => {
                            go = true;
                            immediate = true;
                        }
                        _ => go = true, // 151, 152, 162 and anything else
                    }
                } else if self.opp.frame == 152 {
                    go = true;
                }
            }
            167 => {
                seq = 61;
                go = true;
            }
            _ => {}
        }
        if go {
            c.up = 1;
            self.start_seq(env, seq);
            if immediate {
                self.play_seq(env);
            }
        }
    }

    // ---------------------------------------------------------------- damage (5)

    /// hurt_by_sword (0000:42C1)
    pub fn hurt_by_sword(&mut self, env: &Env) {
        if !self.alive {
            return;
        }
        let mut dead = false;
        if self.sword != 2 {
            dead = self.take_hp(100);
        } else if self.charid != 4 {
            dead = self.take_hp(1);
        }
        if !dead {
            self.start_seq(env, 74);
            self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
            self.fall_y = 0;
        } else {
            let (col, d) = (self.col(env), self.dir());
            let behind = self.tile(env, col - d, self.row);
            let dist = self.dist_weight(env);
            if behind != Tile::Empty || dist < 4 {
                self.start_seq(env, 85);
                if self.charid != 0 && self.facing_left {
                    // on or next to a gate (INFERRED reading of the gate special case)
                    let t = self.tile(env, col, self.row);
                    if t == Tile::Gate {
                        self.x = (X0 + COL_W * col + 7) as u8;
                        self.forward(10);
                    }
                }
                self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
                self.fall_y = 0;
            } else {
                self.forward(dist - 20);
                self.row += 1;
                self.start_seq(env, 81); // pushed off the ledge
            }
        }
        self.events.push(Event::Sound(if self.charid == 0 { 13 } else { 12 }));
        self.play_seq(env);
    }

    /// bump_into_opponent (0000:5122): an unarmed kid cannot walk through an armed guard
    pub(crate) fn bump_into_opponent(&mut self, env: &Env) {
        if self.sh.can_see >= 2 && self.sword == 0 && self.opp.present && self.opp.sword != 0 && self.opp.action < 2 && self.facing_each_other() && self.opp_dist().abs() <= 15 {
            self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
            self.fall_y = 0;
            self.start_seq(env, 47);
            self.play_seq(env);
        }
    }
}
