//! The kid: controls, sequence player, physics, wall collisions, room changes, grab/hang/climb,
//! ported from PRINCE.EXE v1.1. docs/mechanics.md has the evidence (addresses) for every rule; rules we
//! had to reconstruct are marked INFERRED in comments. Not yet: combat, traps, gates/buttons, level doors, items.
use crate::exe::{Exe, Frame, Op};
use crate::level::{Level, Tile};

mod combat;
pub use combat::{guard_hp, Opp, Rng, Shared, REFRACTIMER};

/// y of the floor of row -1..=3 (`ds:22B0`); a kid standing on row r has y = Y_LAND[r + 1].
pub const Y_LAND: [i32; 5] = [-8, 55, 118, 181, 244];
/// x of the left edge of column 0, and column width.
pub const X0: i32 = 58;
pub const COL_W: i32 = 14;

/// Raw key state for one tick.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Keys {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub shift: bool,
}

/// Things the kid does to the level; `sim::World` applies them right after the kid tick (docs/tiles.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// kid (or its body when `dead`) is on a button tile
    Button { room: u8, tilepos: u8, dead: bool },
    /// make_loose_fall(1)
    LooseFall { room: u8, tilepos: u8 },
    /// do_knock(room, row): shake the loose floors of a row
    Knock { room: u8, row: i32 },
    StartSpike { room: u8, tilepos: u8 },
    /// spikes impaled the kid: the spike tile gets modifier 255
    Spiked { room: u8, tilepos: u8 },
    /// chomper closed on the kid: modifier |= 0x80
    Chomped { room: u8, tilepos: u8 },
    StartChompers { room: u8, row: i32 },
    /// sound id (docs/tiles.md section 9; sequence sound ops are mapped to ids)
    Sound(u8),
    /// the sword on this tile was picked up: it becomes floor
    Pickup { room: u8, tilepos: u8 },
}

/// Everything the kid needs to look at besides itself.
pub struct Env<'a> {
    pub exe: &'a Exe,
    pub frames: &'a [Frame],
    pub level: &'a Level,
    /// (height, width) in pixels of KID.DAT image `i` (resource 401 + i), see `image::image_sizes`
    pub sizes: &'a [(u16, u16)],
    /// guard frame table (frames 150..190) and sizes of the guard-type images (chtab 5) of this level
    pub gframes: &'a [Frame],
    pub gsizes: &'a [(u16, u16)],
}

const NO_FRAME: Frame = Frame { image: 255, sword: 0, dx: 0, dy: 0, flags: 0 };

impl<'a> Env<'a> {
    /// Environment without guard tables (kid only).
    pub fn new(exe: &'a Exe, frames: &'a [Frame], level: &'a Level, sizes: &'a [(u16, u16)]) -> Env<'a> {
        Env { exe, frames, level, sizes, gframes: &[], gsizes: &[] }
    }

    /// load_frame (0000:6E96): the frame table depends on the character id (docs/combat.md 0).
    pub fn frame_of(&self, charid: u8, frame: u8) -> Frame {
        let f = frame as usize;
        let kid = |i: usize| self.frames.get(i).copied().unwrap_or(NO_FRAME);
        let guard = |i: usize| i.checked_sub(150).and_then(|j| self.gframes.get(j)).copied();
        match charid {
            1 => guard(f).unwrap_or_else(|| kid(f)),
            2 | 4 => {
                let f2 = if (102..=106).contains(&f) { f + 70 } else { f };
                guard(f2).unwrap_or_else(|| kid(f2))
            }
            _ => kid(f),
        }
    }

    /// Image size (height, width): chtab = 2 + (sword >> 6); 5 = guard-type images, otherwise the kid images.
    pub fn size_of(&self, f: &Frame) -> (u16, u16) {
        let t = if f.sword >> 6 == 3 { self.gsizes } else { self.sizes };
        t.get(f.image as usize).copied().unwrap_or((0, 0))
    }
}

/// Control state as seen inside `control()`: x is already flipped so that -1 = forward.
/// Flags: -1 held (fires), 0 released, 1 consumed (stays until the key is let go).
#[derive(Clone, Copy, Default, Debug)]
pub struct Ctrl {
    pub x: i8,
    pub y: i8,
    pub shift: i8,
    pub fwd: i8,
    pub bwd: i8,
    pub up: i8,
    pub down: i8,
    pub shift2: i8,
}

/// What drives a character this tick: the keyboard (kid) or the control flags set by the AI (guards, shadow).
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Keys(Keys),
    Moves(Ctrl),
}

impl Ctrl {
    /// `release_arrows` (0000:7B48): clears the four flags and returns 1, which the caller stores in the flag it just used.
    pub(crate) fn release_arrows(&mut self) -> i8 {
        self.fwd = 0;
        self.bwd = 0;
        self.up = 0;
        self.down = 0;
        1
    }
}

/// The original keeps all collision data in 10-byte arrays, one byte per tile column; columns of neighbouring rooms are
/// stored at their normalised index together with the room they were read from (docs/collisions.md 1.3).
#[derive(Clone, Debug)]
struct CollData {
    /// rows: 0 = curr_row, 1 = curr_row + 1, 2 = curr_row - 1 (flags are never cleared, rooms are reset to -1 each tick)
    flags: [[u8; 10]; 3],
    rooms: [[i8; 10]; 3],
    /// previous tick, matched to curr_row by move_coll_to_prev
    prev_flags: [u8; 10],
    prev_rooms: [i8; 10],
    coll_row: i8,
    prev_coll_row: i8,
}

impl Default for CollData {
    fn default() -> Self {
        CollData { flags: [[0; 10]; 3], rooms: [[-1; 10]; 3], prev_flags: [0; 10], prev_rooms: [-1; 10], coll_row: 0, prev_coll_row: -1 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Kid {
    // original Char fields
    /// 0 kid, 1 shadow, 2 guard (also fat guard and Jaffar), 4 skeleton
    pub charid: u8,
    /// 2 = sword drawn
    pub sword: u8,
    pub skill: u8,
    /// guard colour 0..7 (palette slice of PRINCE.DAT resource 10; 0 = the file's own palette)
    pub colour: u8,
    pub hp_max: i32,
    /// set by take_hp, applied once per tick by the world (do_delta_hp)
    pub hp_delta: i32,
    /// guard timers (docs/combat.md 3.4)
    pub refrac: u8,
    pub justblocked: u8,
    pub have_sword: bool,
    /// the other fighter and the state shared between them, copied in by the world before each tick
    pub opp: Opp,
    pub sh: Shared,
    /// direction of the last room change (0 left, 1 right, 2 up, 3 down), consumed by the world
    pub leave_dir: Option<u8>,
    pub x: u8,
    pub y: u8,
    pub facing_left: bool,
    pub frame: u8,
    pub action: u8,
    pub room: u8,
    pub row: i32,
    pub fall_x: i8,
    pub fall_y: i8,
    repeat: u8,
    pub seq: usize,
    cur: Vec<Op>,
    pc: usize,
    pub hp: i32,
    pub alive: bool,
    /// edge-triggered flags in raw (left/right) terms; see `Ctrl`
    left_f: i8,
    right_f: i8,
    up_f: i8,
    down_f: i8,
    shift2_f: i8,
    /// raw shift key, read by check_grab
    shift: bool,
    grab_timer: i32,
    cd: CollData,
    bump_left: i32,
    bump_right: i32,
    /// Frame record, tile column and collision box as the original last loaded them (0000:0F25..0F2F, in_wall, start_fall).
    /// Bumps and gate pushes change x/frame without reloading these: the rest of the tick sees the old values (4.6).
    snap_rec: Frame,
    snap_col: i32,
    snap_box: CollBox,
    /// the obstacle tile found by check_bumped: (room, normalised col, row), and coll_tile_left_x
    ot: (i32, i32, i32),
    coll_tile_left_x: i32,
    /// false = only play the current sequence (sequence viewer)
    pub controlled: bool,
    /// set by sequence ops 0xF4 (-1) / 0xF5 (+1), consumed by check_knock
    knock: i8,
    /// effects on the level, drained by `sim::World`
    pub events: Vec<Event>,
}

fn floor_div(a: i32, b: i32) -> i32 {
    a.div_euclid(b)
}

/// get_tile normalisation (0000:6D99): out-of-range columns/rows move into the neighbour room; room 0 stays 0.
pub(crate) fn norm(level: &Level, mut room: i32, mut col: i32, mut row: i32) -> (i32, i32, i32) {
    for _ in 0..8 {
        let l = if (1..=24).contains(&room) { level.links[room as usize - 1] } else { [0; 4] };
        if col < 0 {
            col += 10;
            room = l[0] as i32;
        } else if col >= 10 {
            col -= 10;
            room = l[1] as i32;
        } else if row < 0 {
            row += 3;
            room = l[2] as i32;
        } else if row >= 3 {
            row -= 3;
            room = l[3] as i32;
        } else {
            break;
        }
    }
    (room, col, row)
}

/// Tile at a normalised position; room 0 (outside the level) reads as a wall.
pub(crate) fn tile_n(level: &Level, room: i32, col: i32, row: i32) -> Tile {
    if (1..=24).contains(&room) { level.tile_at(room as usize, (row * 10 + col) as usize).kind } else { Tile::Wall }
}

fn mod_n(level: &Level, room: i32, col: i32, row: i32) -> u8 {
    if (1..=24).contains(&room) { level.tile_at(room as usize, (row * 10 + col) as usize).modifier } else { 0 }
}

/// can_bump_into_gate (0000:59FF)
fn gate_bumpable(modifier: u8, height: i32) -> bool {
    (modifier as i32 >> 2) + 6 < height
}

/// tile_is_floor (0000:7364)
pub(crate) fn is_floor(t: Tile) -> bool {
    use Tile::*;
    !matches!(t, Empty | BigPillarTop | Doortop | Wall | LatticePillar | LatticeSupport | SmallLattice | LatticeLeft)
}

/// wall_type (0000:7D0B): 0 = not wall-like
fn wall_type(t: Tile) -> usize {
    use Tile::*;
    match t {
        Gate | DoortopWithFloor | Doortop => 1,
        Mirror => 2,
        Chomper => 3,
        Wall => 4,
        _ => 0,
    }
}
const WALL_DIST_FROM_LEFT: [i32; 6] = [0, 10, 0, -1, 0, 0];
const WALL_DIST_FROM_RIGHT: [i32; 6] = [0, 0, 10, 13, 0, 0];

/// y_to_row_mod4 (0000:716F)
fn row_from_y(y: u8) -> i32 {
    (y as i32 + 60) / 63 % 4 - 1
}

/// Collision box (set_char_collision, 0000:745F). All x in logic units.
#[derive(Clone, Copy, Debug, Default)]
struct CollBox {
    left: i32,
    right: i32,
    left_coll: i32,
    right_coll: i32,
    height: i32,
}

impl Kid {
    /// Kid at the level start as in the original (do_startpos, docs/mechanics.md section 8): x = 72 + 14*col,
    /// direction = NOT start_dir, first sequence 7 (fall) on level 1, 84 (run) on level 13, otherwise 5 (turn),
    /// played once so that its first frame is shown immediately.
    pub fn spawn(env: &Env, level_no: usize) -> Kid {
        let mut k = Kid::spawn_standing(env);
        k.start_seq(env, match level_no { 1 => 7, 13 => 84, _ => 5 });
        k.play_seq(env);
        k
    }

    /// Kid standing (sequence 2) at the start position, without the level-specific first sequence.
    pub fn spawn_standing(env: &Env) -> Kid {
        let l = env.level;
        let (col, row) = ((l.start_tile % 10) as i32, (l.start_tile / 10) as i32);
        let mut k = Kid {
            x: (X0 + COL_W * col + 14) as u8,
            y: Y_LAND[row as usize + 1] as u8,
            facing_left: l.start_dir == 0, // direction = NOT start_dir
            frame: 15,
            room: l.start_room,
            row,
            alive: true,
            hp: 3,
            hp_max: 3,
            controlled: true,
            bump_left: -1,
            bump_right: -1,
            ..Kid::default()
        };
        k.start_seq(env, 2);
        k
    }

    fn dir(&self) -> i32 {
        if self.facing_left { -1 } else { 1 }
    }

    pub fn start_seq(&mut self, env: &Env, seq: usize) {
        self.seq = seq;
        self.cur = env.exe.sequence(seq).unwrap_or_default();
        self.pc = 0;
    }

    fn frame_rec(&self, env: &Env) -> Frame {
        env.frame_of(self.charid, self.frame)
    }

    /// char_dx_forward-style move: x += dir * d
    fn forward(&mut self, d: i32) {
        self.x = (self.x as i32 + self.dir() * d) as u8;
    }

    /// Foot position (dx_weight, 0000:6F31): char_dx_forward(frame.dx - weight) added to x.
    pub fn foot_x(&self, env: &Env) -> i32 {
        let f = self.frame_rec(env);
        self.x as i32 + self.dir() * (f.dx as i32 - (f.flags & 0x1f) as i32)
    }

    /// determine_col: get_tile_div_mod_m7(foot x)
    pub fn col(&self, env: &Env) -> i32 {
        floor_div(self.foot_x(env) - 7 - X0, COL_W)
    }

    fn tile(&self, env: &Env, col: i32, row: i32) -> Tile {
        env.level.tile_wrapped(self.room as i32, col, row)
    }

    fn tile_mod(&self, env: &Env, col: i32, row: i32) -> u8 {
        env.level.modifier_wrapped(self.room as i32, col, row)
    }

    /// distance_to_edge(x) (0000:78D0): distance from x to the tile edge in front, 0..=13.
    fn distance_to_edge(&self, x: i32) -> i32 {
        let m = (x - 7 - X0).rem_euclid(COL_W);
        if self.facing_left { m } else { COL_W - 1 - m }
    }

    fn dist_weight(&self, env: &Env) -> i32 {
        self.distance_to_edge(self.foot_x(env))
    }

    /// Collision box from the image size (7.4): obj_x as in the drawing rule, half = (w+1)/2.
    fn coll_box(&self, env: &Env) -> CollBox {
        let f = self.frame_rec(env);
        let (h, w) = env.size_of(&f);
        let dir_byte: u8 = if self.facing_left { 0xFF } else { 0 };
        let dxs = if self.facing_left { -(f.dx as i32) } else { f.dx as i32 };
        let obj_x = 2 * (self.x as i32 + dxs) - 116 + (((f.flags ^ dir_byte) & 0x80) == 0) as i32;
        let half = (w as i32 + 1) / 2;
        let mut left = obj_x / 2 + X0;
        if !self.facing_left {
            left -= half;
        }
        let right = left + half;
        let thin = if f.flags & 0x20 != 0 { 4 } else { 0 };
        CollBox { left, right, left_coll: left + thin, right_coll: right - thin, height: h as i32 }
    }

    /// dist_from_wall_forward (0000:5DB8): uses the collision box, not the foot. -1 = no wall.
    fn dist_from_wall(&self, env: &Env, col: i32, t: Tile) -> i32 {
        let wt = wall_type(t);
        if wt == 0 {
            return -1;
        }
        let b = self.coll_box(env);
        if t == Tile::Gate && !gate_bumpable(self.tile_mod(env, col, self.row), b.height) {
            return -1;
        }
        let tile_left = X0 + COL_W * col + 7;
        if self.facing_left {
            WALL_DIST_FROM_RIGHT[wt] + b.left_coll - tile_left - 13
        } else {
            WALL_DIST_FROM_LEFT[wt] + tile_left - b.right_coll
        }
    }

    /// get_edge_distance (0000:5A24): returns (edge_type, distance, tile looked at).
    fn edge_distance(&self, env: &Env) -> (i32, i32, Tile) {
        let col = self.col(env);
        let here = self.tile(env, col, self.row);
        if wall_type(here) != 0 {
            let d = self.dist_from_wall(env, col, here);
            if d >= 0 {
                return if d < 14 { (1, d, here) } else { (2, 11, here) };
            }
        }
        let fc = col + self.dir();
        let front = self.tile(env, fc, self.row);
        let dw = self.dist_weight(env);
        use Tile::*;
        if (front == Doortop && !self.facing_left) || front == LooseFloor {
            return (0, dw, front);
        }
        if wall_type(front) != 0 {
            let d = self.dist_from_wall(env, fc, front);
            if d >= 0 {
                return if d < 14 { (1, d, front) } else { (2, 11, front) };
            }
        }
        if matches!(front, DropButton | Sword | Potion) {
            return if dw != 0 { (0, dw, front) } else { (2, 11, front) };
        }
        if is_floor(front) { (2, 11, front) } else { (0, dw, front) }
    }

    // ------------------------------------------------------------------ tick

    /// One logic tick: control (before the sequence step, same tick), play_seq, gravity, collisions, floor checks,
    /// then the room-change test (0000:0ED4 order, docs/mechanics.md 3.1 and 10).
    pub fn tick(&mut self, env: &Env, keys: Keys) {
        self.tick_input(env, Input::Keys(keys));
    }

    pub fn tick_input(&mut self, env: &Env, input: Input) {
        if self.room == 0 {
            return; // outside the level: the original skips the tick
        }
        if self.charid != 0 {
            self.guard_tick(env, input);
            return;
        }
        if self.controlled {
            self.grab_timer = (self.grab_timer - 1).max(0);
            if let Input::Keys(k) = input {
                self.shift = k.shift;
            }
            // control_kid (0000:7A19): hit points at 0 end the life (docs/combat.md 6)
            if self.alive && self.hp <= 0 {
                self.alive = false;
            }
            self.control_input(env, input);
        }
        self.play_seq(env);
        if !self.controlled {
            return;
        }
        self.fall_accel_speed();
        self.refresh_snap(env); // 0000:0F25..0F2F: load_frame_to_obj, load_fram_det_col, set_char_collision
        self.bump_into_opponent(env);
        self.check_collisions(env);
        self.check_bumped(env);
        self.check_gate_push(env);
        self.check_action(env);
        self.check_press(env);
        self.check_spike_below(env);
        self.check_spiked(env);
        self.check_chomped_kid(env);
        self.check_knock();
        self.check_room_change(env);
    }

    /// fall_accel (0000:72B8) and fall_speed (0000:72EA)
    fn fall_accel_speed(&mut self) {
        if self.action == 4 {
            self.fall_y = (self.fall_y as i32 + 3).min(33) as i8;
        }
        self.y = self.y.wrapping_add(self.fall_y as u8);
        if self.action == 4 {
            self.forward(self.fall_x as i32);
        }
    }

    /// play_guard_frame (0000:0F73) for a guard, skeleton or shadow: control, sequence step and, if the character is
    /// in the drawn room and inside the visible x range, physics and floor checks.
    fn guard_tick(&mut self, env: &Env, input: Input) {
        let Input::Moves(mut c) = input else { return };
        self.control(env, &mut c);
        if self.room != self.sh.drawn_room {
            return; // a guard in another room thinks but does not move
        }
        self.play_seq(env);
        if !(44..211).contains(&self.x) {
            return;
        }
        self.fall_accel_speed();
        self.refresh_snap(env);
        self.check_action(env);
        self.check_press(env);
        self.check_spike_below(env);
        self.check_spiked(env);
    }

    /// take_hp (0000:73F9): sets the hit point delta; returns true if this kills.
    pub fn take_hp(&mut self, n: i32) -> bool {
        if n >= self.hp {
            self.hp_delta = -self.hp;
            true
        } else {
            self.hp_delta = -n;
            false
        }
    }

    fn control_input(&mut self, env: &Env, input: Input) {
        let k = match input {
            Input::Moves(mut c) => {
                self.control(env, &mut c);
                return;
            }
            Input::Keys(k) => k,
        };
        let raw_x: i8 = if k.left { -1 } else if k.right { 1 } else { 0 };
        let raw_y: i8 = if k.up { -1 } else if k.down { 1 } else { 0 };
        let shift: i8 = if k.shift { -1 } else { 0 };
        // read_user_control: edge-triggered flags
        let upd = |f: &mut i8, on: bool| {
            if *f >= 0 {
                *f = if on { if *f == 0 { -1 } else { *f } } else { 0 };
            }
        };
        upd(&mut self.left_f, raw_x == -1);
        upd(&mut self.right_f, raw_x == 1);
        upd(&mut self.up_f, raw_y == -1);
        upd(&mut self.down_f, raw_y == 1);
        upd(&mut self.shift2_f, shift < 0);
        // user_control: controls are mirrored when facing right, so that inside control() -1 = forward
        let facing_right = !self.facing_left;
        let mut c = Ctrl { x: if facing_right { -raw_x } else { raw_x }, y: raw_y, shift, up: self.up_f, down: self.down_f, shift2: self.shift2_f, ..Ctrl::default() };
        (c.fwd, c.bwd) = if facing_right { (self.right_f, self.left_f) } else { (self.left_f, self.right_f) };
        self.control(env, &mut c);
        (self.right_f, self.left_f) = if facing_right { (c.fwd, c.bwd) } else { (c.bwd, c.fwd) };
        (self.up_f, self.down_f, self.shift2_f) = (c.up, c.down, c.shift2);
    }

    /// control() dispatcher (0000:607B), alive kid without sword.
    pub(crate) fn control(&mut self, env: &Env, c: &mut Ctrl) {
        if !self.alive {
            // dead and standing: the dying sequence
            if matches!(self.frame, 15 | 166 | 158 | 171) {
                self.start_seq(env, 71);
            }
            return;
        }
        if self.action == 5 || self.action == 4 {
            c.release_arrows();
            return;
        }
        if self.sword == 2 {
            self.control_with_sword(env, c);
            return;
        }
        if self.charid >= 2 {
            self.control_guard_inactive(env, c);
            return;
        }
        match self.frame {
            15 | 50..=52 => self.control_standing(env, c),
            48 => {
                if c.shift >= 0 && c.x == -1 && c.y >= 0 {
                    self.start_seq(env, 43);
                }
            }
            0..=3 => {
                if c.y == -1 && c.x == -1 {
                    self.standing_jump(env, c);
                }
            }
            67..=69 => {
                if c.x == -1 || c.fwd == -1 {
                    self.standing_jump(env, c);
                }
            }
            4..=14 => self.control_running(env, c),
            87..=99 => self.control_hanging(env, c),
            109 => self.control_crouched(env, c),
            _ => {}
        }
    }

    fn control_standing(&mut self, env: &Env, c: &mut Ctrl) {
        // the kid draws his sword by himself when a reachable guard is close (docs/combat.md 4.1)
        if self.charid == 0 && self.have_sword && !(self.sh.offguard && c.shift >= 0) {
            if self.sh.can_see >= 2 {
                let d = self.opp_dist();
                if (-10..90).contains(&d) {
                    self.sh.holding_sword = true;
                    let busy = self.opp.charid == 1 && (self.opp.action == 3 || (107..=117).contains(&self.opp.frame));
                    if busy {
                        self.sh.offguard = false;
                    } else if (-6..0).contains(&d) {
                        self.back_pressed(env, c);
                        return;
                    } else {
                        self.draw_sword(env, c);
                        return;
                    }
                }
            } else {
                self.sh.offguard = false;
            }
        }
        if c.shift < 0 {
            if c.bwd == -1 {
                self.back_pressed(env, c);
            } else if c.up == -1 {
                self.up_pressed(env, c);
            } else if c.down == -1 {
                self.down_pressed(env, c);
            } else if c.x == -1 && c.fwd == -1 {
                self.safe_step(env, c);
            }
        } else if c.fwd == -1 {
            if c.up == -1 { self.standing_jump(env, c) } else { self.forward_pressed(env, c) }
        } else if c.bwd == -1 {
            self.back_pressed(env, c);
        } else if c.up == -1 {
            if c.fwd == -1 { self.standing_jump(env, c) } else { self.up_pressed(env, c) }
        } else if c.down == -1 {
            self.down_pressed(env, c);
        } else if c.x == -1 {
            self.forward_pressed(env, c);
        }
    }

    fn forward_pressed(&mut self, env: &Env, c: &mut Ctrl) {
        let (et, d, tile) = self.edge_distance(env);
        if et == 1 && tile != Tile::Chomper && d < 8 {
            if c.fwd == -1 {
                self.safe_step(env, c);
            }
        } else {
            self.start_seq(env, 1);
        }
    }

    fn back_pressed(&mut self, env: &Env, c: &mut Ctrl) {
        c.bwd = c.release_arrows();
        if self.have_sword && self.sh.can_see >= 2 && self.opp_dist() <= 0 && self.dist_weight(env) >= 2 {
            self.sword = 2;
            self.start_seq(env, 89); // turn and draw
            return;
        }
        self.start_seq(env, 5);
    }

    fn safe_step(&mut self, env: &Env, c: &mut Ctrl) {
        c.shift2 = 1;
        c.fwd = 1;
        let (et, d, _) = self.edge_distance(env);
        if d != 0 {
            self.repeat = 1;
            self.start_seq(env, 28 + d as usize);
        } else if et != 1 && self.repeat != 0 {
            self.repeat = 0;
            self.start_seq(env, 44);
        } else {
            self.start_seq(env, 39);
        }
    }

    fn standing_jump(&mut self, env: &Env, c: &mut Ctrl) {
        c.fwd = 1;
        c.up = 1;
        self.start_seq(env, 3);
    }

    fn up_pressed(&mut self, env: &Env, c: &mut Ctrl) {
        // level doors are not implemented
        if c.x == -1 {
            self.standing_jump(env, c);
        } else {
            self.check_jump_up(env, c);
        }
    }

    // --- jump up / grab (docs/mechanics.md 11)

    /// can_grab (0000:7C9D)
    fn can_grab(&self, env: &Env, target: (i32, i32), through: (i32, i32)) -> bool {
        let (tt, th) = (self.tile(env, target.0, target.1), self.tile(env, through.0, through.1));
        let tm = self.tile_mod(env, target.0, target.1);
        !(th == Tile::Wall
            || (th == Tile::Doortop && !self.facing_left)
            || is_floor(th)
            || (tt == Tile::LooseFloor && tm != 0)
            || (tt == Tile::DoortopWithFloor && self.facing_left)
            || !is_floor(tt))
    }

    fn check_jump_up(&mut self, env: &Env, c: &mut Ctrl) {
        c.up = c.release_arrows();
        let (col, row, d) = (self.col(env), self.row, self.dir());
        if self.can_grab(env, (col + d, row - 1), (col, row - 1)) {
            self.grab_up_with_floor_behind(env);
        } else if self.can_grab(env, (col, row - 1), (col - d, row - 1)) {
            self.jump_up_or_grab(env, c);
        } else {
            self.jump_up(env, c);
        }
    }

    fn jump_up_or_grab(&mut self, env: &Env, c: &mut Ctrl) {
        let d = self.dist_weight(env);
        if d < 6 {
            self.jump_up(env, c);
        } else if !is_floor(self.tile(env, self.col(env) - self.dir(), self.row)) {
            self.grab_up_no_floor_behind(env);
        } else {
            self.forward(d - 14);
            self.grab_up_with_floor_behind(env);
        }
    }

    fn grab_up_no_floor_behind(&mut self, env: &Env) {
        let d = self.dist_weight(env);
        self.forward(d - 10);
        self.start_seq(env, 16);
    }

    fn jump_up(&mut self, env: &Env, c: &mut Ctrl) {
        c.up = c.release_arrows();
        let (et, d, _) = self.edge_distance(env);
        if d < 4 && et == 1 {
            self.forward(d - 3);
        }
        // tile at (get_tile_div_mod(dx_weight - 6), row - 1): plain division without the -7 of determine_col
        let col = floor_div(self.foot_x(env) - 6 - X0, COL_W);
        let t = self.tile(env, col, self.row - 1);
        self.start_seq(env, if t != Tile::Wall && !is_floor(t) { 28 } else { 14 });
    }

    fn grab_up_with_floor_behind(&mut self, env: &Env) {
        let d = self.dist_weight(env);
        let (et, gd, _) = self.edge_distance(env);
        if d < 4 && gd < 4 && et != 1 {
            self.forward(d);
            self.start_seq(env, 8);
        } else {
            self.forward(d - 4);
            self.start_seq(env, 24);
        }
    }

    /// check_grab (0000:7755): while rising/falling past a ledge with Shift held.
    fn check_grab(&mut self, env: &Env) {
        if !self.shift || self.fall_y >= 32 || !self.alive || Y_LAND[(self.row + 1).clamp(0, 4) as usize] > self.y as i32 + 25 {
            return;
        }
        let saved = self.x;
        self.forward(-8);
        let (col, row, d) = (self.col(env), self.row, self.dir());
        if !self.can_grab(env, (col + d, row - 1), (col, row - 1)) {
            self.x = saved;
            return;
        }
        let w = self.dist_weight(env);
        self.forward(w);
        self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
        self.fall_y = 0;
        self.start_seq(env, 15);
        self.play_seq(env);
        self.grab_timer = 12;
    }

    fn control_hanging(&mut self, env: &Env, c: &mut Ctrl) {
        if !self.alive {
            self.hang_fall(env, c);
            return;
        }
        let (col, row) = (self.col(env), self.row);
        if self.grab_timer == 0 && c.y == -1 {
            self.can_climb_up(env, c);
        } else if c.shift < 0 {
            let here = self.tile(env, col, row);
            if self.action != 6 && (here == Tile::Wall || (self.facing_left && matches!(here, Tile::DoortopWithFloor | Tile::Doortop))) {
                self.start_seq(env, 25); // hang against the wall
            } else if !is_floor(self.tile(env, col, row - 1)) {
                self.hang_fall(env, c);
            }
        } else {
            self.hang_fall(env, c);
        }
    }

    fn can_climb_up(&mut self, env: &Env, c: &mut Ctrl) {
        c.up = c.release_arrows();
        c.shift2 = 1;
        let (col, row) = (self.col(env), self.row);
        let above = self.tile(env, col, row - 1);
        let m = self.tile_mod(env, col, row - 1) as i32;
        let blocked = (matches!(above, Tile::Mirror | Tile::Chomper) && !self.facing_left) || (above == Tile::Gate && self.facing_left && (m >> 2) < 6);
        self.start_seq(env, if blocked { 73 } else { 10 });
    }

    fn hang_fall(&mut self, env: &Env, c: &mut Ctrl) {
        c.down = c.release_arrows();
        let (col, row, d) = (self.col(env), self.row, self.dir());
        let (behind, here) = (self.tile(env, col - d, row), self.tile(env, col, row));
        if !is_floor(behind) && !is_floor(here) {
            self.start_seq(env, 23);
        } else {
            if here == Tile::Wall || (self.facing_left && matches!(here, Tile::DoortopWithFloor | Tile::Doortop)) {
                self.forward(-7);
            }
            self.start_seq(env, 11);
        }
    }

    fn down_pressed(&mut self, env: &Env, c: &mut Ctrl) {
        c.down = 1;
        let (col, d) = (self.col(env), self.dir());
        let front = self.tile(env, col + d, self.row);
        let dist = self.dist_weight(env);
        if !is_floor(front) && dist < 3 {
            self.forward(5); // shuffle to the edge; no sequence
            return;
        }
        // climb down (INFERRED reading of 0000:6333: behind is empty, the kid's own tile is the floor it grabs)
        let behind = self.tile(env, col - d, self.row);
        let here = self.tile(env, col, self.row);
        let gate_ok = !self.facing_left || behind != Tile::Gate || (self.tile_mod(env, col - d, self.row) as i32 >> 2) >= 6;
        if !is_floor(behind) && dist >= 8 && self.can_grab(env, (col, self.row), (col - d, self.row)) && gate_ok && is_floor(here) {
            self.forward(dist - 9);
            self.start_seq(env, 68);
        } else {
            c.down = c.release_arrows();
            self.start_seq(env, 50);
        }
    }

    fn control_running(&mut self, env: &Env, c: &mut Ctrl) {
        if c.x == 0 && (self.frame == 7 || self.frame == 11) {
            c.fwd = c.release_arrows();
            self.start_seq(env, 13);
        } else if c.x > 0 {
            c.bwd = c.release_arrows();
            self.start_seq(env, 6);
        } else if c.y == -1 && c.up == -1 {
            self.running_jump(env, c);
        } else if c.down == -1 {
            c.down = 1;
            self.start_seq(env, 26);
        }
    }

    fn control_crouched(&mut self, env: &Env, c: &mut Ctrl) {
        // pick up a sword lying under the kid (get_item, 0000:65AA): Shift while crouching
        if c.shift2 == -1 {
            let (col, row) = (self.col(env), self.row);
            if self.tile(env, col, row) == Tile::Sword {
                if let Some((room, tilepos)) = self.locate(env, col, row) {
                    self.events.push(Event::Pickup { room, tilepos });
                }
                c.shift2 = 1;
                self.start_seq(env, 91);
                return;
            }
        }
        if c.y != 1 {
            self.start_seq(env, 49);
        } else if c.fwd == -1 {
            c.fwd = 1;
            self.start_seq(env, 79);
        }
    }

    // ---------------------------------------------------------------- sequences and physics

    /// play_seq (0000:6F90): run ops until the next frame (exactly one frame per tick).
    pub(crate) fn play_seq(&mut self, env: &Env) {
        for _ in 0..256 {
            let Some(&op) = self.cur.get(self.pc) else { return };
            self.pc += 1;
            match op {
                Op::Frame(n) => {
                    self.frame = n;
                    return;
                }
                Op::Dx(d) => self.forward(d as i32),
                Op::Dy(d) => self.y = self.y.wrapping_add(d as u8),
                Op::Flip => self.facing_left = !self.facing_left,
                Op::Action(a) => self.action = a,
                Op::SetFall(x, y) => {
                    self.fall_x = x;
                    self.fall_y = y;
                }
                Op::Up => {
                    self.row -= 1;
                    self.events.push(Event::StartChompers { room: self.room, row: self.row });
                }
                Op::Down => {
                    self.row += 1;
                    self.events.push(Event::StartChompers { room: self.room, row: self.row });
                }
                Op::KnockDown => self.knock = -1,
                Op::KnockUp => self.knock = 1,
                Op::Sound(n) => {
                    // F2 arg: 1 = footstep (23), 2 = bump (8), 3 = drink (18); 0 and 4 (music) have no effect id here
                    if let Some(id) = match n { 1 => Some(23), 2 => Some(8), 3 => Some(18), _ => None } {
                        self.events.push(Event::Sound(id));
                    }
                }
                Op::Jmp(t) => {
                    self.cur = env.exe.ops_from(t).unwrap_or_default();
                    self.pc = 0;
                }
                // feather fall is never active; items and level end are not simulated yet
                Op::GetItem(1) => {
                    self.have_sword = true;
                    self.events.push(Event::Sound(37));
                }
                Op::JmpIfFeather(_) | Op::GetItem(_) | Op::EndLevel | Op::Nop => {}
            }
        }
    }

    // ---------------------------------------------------------------- wall collisions (docs 9)


    // ---------------------------------------------------------------- snapshot helpers

    /// 0000:0F25..0F2F: reload frame record, tile column and box.
    fn refresh_snap(&mut self, env: &Env) {
        self.snap_rec = self.frame_rec(env);
        self.snap_col = self.col(env);
        self.snap_box = self.coll_box(env);
    }

    /// load_fram_det_col (0000:6E80): reload record and column, not the box.
    fn load_fram_det_col(&mut self, env: &Env) {
        self.snap_rec = self.frame_rec(env);
        self.snap_col = self.col(env);
    }

    /// distance_to_edge_weight with the stored frame record (dx_weight reads `rec`).
    fn dist_weight_snap(&self) -> i32 {
        let f = self.snap_rec;
        self.distance_to_edge(self.x as i32 + self.dir() * (f.dx as i32 - (f.flags & 0x1f) as i32))
    }

    fn tile_at_char(&self, env: &Env) -> Tile {
        self.tile(env, self.snap_col, self.row)
    }

    // ---------------------------------------------------------------- wall collisions (docs/collisions.md)

    /// move_coll_to_prev (0000:5487)
    fn move_coll_to_prev(&mut self) {
        let (r, p) = (self.cd.coll_row as i32, self.cd.prev_coll_row as i32);
        let src = if r == p || r + 3 == p || r - 3 == p { 0 } else if r + 1 == p || r - 2 == p { 2 } else { 1 };
        self.cd.prev_flags = self.cd.flags[src];
        self.cd.prev_rooms = self.cd.rooms[src];
        self.cd.rooms = [[-1; 10]; 3];
    }

    /// get_row_collision_data (0000:552D) for `row`, stored in slot `k`.
    fn row_collision_data(&mut self, env: &Env, row: i32, k: usize, left: i32, right: i32) {
        let b = self.snap_box;
        let mut tile_left = X0 + COL_W * left + 7;
        for col in left..=right {
            let (room, c, r) = norm(env.level, self.room as i32, col, row);
            let wt = wall_type(tile_n(env.level, room, c, r));
            let lw = if wt != 0 { WALL_DIST_FROM_LEFT[wt] + tile_left } else { 0xFF };
            let rw = if wt != 0 { tile_left - WALL_DIST_FROM_RIGHT[wt] + 13 } else { 0 };
            self.cd.flags[k][c as usize] = (if lw < b.right_coll { 0x0F } else { 0 }) | (if rw > b.left_coll { 0xF0 } else { 0 });
            self.cd.rooms[k][c as usize] = room as i8;
            tile_left += COL_W;
        }
    }

    /// check_collisions (0000:53AC): a bump fires on the tick on which a column's overlap bit turns on and the column
    /// was filled from the same room last tick.
    fn check_collisions(&mut self, env: &Env) {
        self.bump_left = -1;
        self.bump_right = -1;
        if self.action == 7 {
            return; // arrays and prev_coll_row stay as they are
        }
        let b = self.snap_box;
        self.cd.coll_row = self.row as i8;
        self.move_coll_to_prev();
        self.cd.prev_coll_row = self.cd.coll_row;
        let m7 = |x: i32| floor_div(x - 7 - X0, COL_W);
        let (left, right) = (m7(b.left_coll) - 1, (m7(b.right_coll) + 2).min(11));
        for (k, row) in [(0, self.row), (1, self.row + 1), (2, self.row - 1)] {
            self.row_collision_data(env, row, k, left, right);
        }
        for col in (0..10).rev() {
            let (r, pr) = (self.cd.rooms[0][col], self.cd.prev_rooms[col]);
            if r < 0 || pr != r {
                continue;
            }
            let (p, n) = (self.cd.prev_flags[col], self.cd.flags[0][col]);
            if p & 0x0F == 0 && n & 0x0F != 0 {
                self.bump_left = col as i32;
            }
            if p & 0xF0 == 0 && n & 0xF0 != 0 {
                self.bump_right = col as i32;
            }
        }
    }

    /// xpos_in_drawn_room (0000:57AD): tiles of side rooms are shifted by 140 (drawn room = the kid's room).
    fn xpos_in_drawn_room(&self, env: &Env, room: i32, x: i32) -> i32 {
        let link = |r: i32, d: usize| if (1..=24).contains(&r) { env.level.links[r as usize - 1][d] as i32 } else { 0 };
        let drawn = self.room as i32;
        if room == drawn {
            return x;
        }
        let (l, r, b) = (link(drawn, 0), link(drawn, 1), link(drawn, 3));
        let bl = if b != 0 { link(b, 0) } else { link(l, 3) };
        let br = if b != 0 { link(b, 1) } else { link(r, 3) };
        if room == l || room == bl {
            x - 140
        } else if room == r || room == br {
            x + 140
        } else {
            x
        }
    }

    /// is_obstacle_at_col (0000:56EB) + is_obstacle (0000:5726): reads the tile of the room stored for that column.
    fn obstacle_at_col(&mut self, env: &Env, col: i32) -> bool {
        let mut row = self.row;
        if row < 0 {
            row += 3;
        }
        if row >= 3 {
            row -= 3;
        }
        let room = self.cd.rooms[0][col as usize] as i32;
        let (r, c, rw) = norm(env.level, room, col, row);
        self.ot = (r, c, rw);
        let (t, m) = (tile_n(env.level, r, c, rw), mod_n(env.level, r, c, rw));
        let obstacle = match t {
            Tile::Potion => false,
            Tile::Gate => gate_bumpable(m, self.snap_box.height),
            Tile::Chomper => m == 2,
            // jumping through the mirror (frames 39..43 facing left) is not simulated: it only skips the bump
            Tile::Mirror => !((39..44).contains(&self.frame) && self.facing_left),
            _ => true,
        };
        if obstacle {
            self.coll_tile_left_x = self.xpos_in_drawn_room(env, r, X0 + COL_W * c) + 7;
        }
        obstacle
    }

    /// left_wall_x / right_wall_x of the obstacle tile found by `obstacle_at_col`.
    fn obstacle_wall_x(&self, env: &Env, left: bool) -> i32 {
        let wt = wall_type(tile_n(env.level, self.ot.0, self.ot.1, self.ot.2));
        if left {
            if wt != 0 { WALL_DIST_FROM_LEFT[wt] + self.coll_tile_left_x } else { 0xFF }
        } else if wt != 0 {
            self.coll_tile_left_x - WALL_DIST_FROM_RIGHT[wt] + 13
        } else {
            0
        }
    }

    /// check_bumped (0000:5645)
    fn check_bumped(&mut self, env: &Env) {
        if self.action == 2 || self.action == 6 || (135..149).contains(&self.frame) {
            return;
        }
        let b = self.snap_box;
        if self.bump_left >= 0 {
            // kid left of the obstacle, moving right (sword == 2 would also qualify)
            if !self.facing_left && self.obstacle_at_col(env, self.bump_left) {
                let delta = self.obstacle_wall_x(env, true) - b.right_coll;
                self.bumped(env, delta, true);
            }
        } else if self.bump_right >= 0 && self.facing_left && self.obstacle_at_col(env, self.bump_right) {
            let delta = self.obstacle_wall_x(env, false) - b.left_coll;
            self.bumped(env, delta, false);
        }
    }

    /// bumped (0000:57F0): push back, then decide by the tile on the kid's side of the obstacle.
    fn bumped(&mut self, env: &Env, delta: i32, push_left: bool) {
        if !self.alive || self.frame == 177 {
            return;
        }
        self.x = (self.x as i32 + delta) as u8;
        let (mut room, mut col, row) = self.ot;
        let mut t = tile_n(env.level, room, col, row);
        if push_left {
            if t == Tile::Wall {
                let (r, c, rw) = norm(env.level, room, col - 1, row);
                t = tile_n(env.level, r, c, rw);
            }
        } else if matches!(t, Tile::Doortop | Tile::DoortopWithFloor | Tile::Wall) {
            col += 1;
            if room == 0 && col == 10 {
                room = self.room as i32;
                col = 0;
            }
            let (r, c, rw) = norm(env.level, room, col, row);
            t = tile_n(env.level, r, c, rw);
        }
        if is_floor(t) { self.bumped_floor(env) } else { self.bumped_fall(env) }
    }

    /// bumped_fall (0000:588C)
    fn bumped_fall(&mut self, env: &Env) {
        self.forward(-4);
        if self.action == 4 {
            self.fall_x = 0;
        } else {
            self.start_seq(env, 45);
            self.play_seq(env);
        }
        self.events.push(Event::Sound(8)); // bumped_sound, both branches
    }

    /// bumped_floor (0000:58C8), kid without sword
    fn bumped_floor(&mut self, env: &Env) {
        let land = Y_LAND[(self.row + 1).clamp(0, 4) as usize];
        if (land as u16).wrapping_sub(self.y as u16) >= 15 {
            self.bumped_fall(env);
            return;
        }
        self.y = land as u8;
        if self.fall_y >= 22 {
            self.forward(-5);
            return;
        }
        self.fall_y = 0;
        let hard = matches!(self.frame, 24 | 25 | 40..=42 | 102..=106);
        self.start_seq(env, if hard { 46 } else { 47 });
        self.play_seq(env);
        self.events.push(Event::Sound(8));
    }

    /// in_wall (0000:7809): push the kid out of a wall tile, then reload record and column.
    fn in_wall(&mut self, env: &Env) {
        let mut d = self.dist_weight_snap();
        if d >= 8 || self.tile(env, self.snap_col + self.dir(), self.row) == Tile::Wall {
            d = 6 - d;
        } else {
            d += 4;
        }
        self.forward(d);
        self.load_fram_det_col(env);
    }

    /// check_gate_push (0000:5BDB): a gate closing on the kid pushes it 5 units per tick.
    fn check_gate_push(&mut self, env: &Env) {
        if !(self.action == 7 || self.frame == 15 || (108..111).contains(&self.frame)) {
            return;
        }
        let (mut room, mut col, mut row) = norm(env.level, self.room as i32, self.snap_col, self.row);
        let orig = col;
        if tile_n(env.level, room, col, row) != Tile::Gate {
            (room, col, row) = norm(env.level, room, col - 1, row);
            if tile_n(env.level, room, col, row) != Tile::Gate {
                return;
            }
        }
        let c = col as usize;
        let m = mod_n(env.level, room, col, row);
        if self.cd.flags[0][c] & self.cd.prev_flags[c] == 0xFF && gate_bumpable(m, self.snap_box.height) {
            self.events.push(Event::Sound(8));
            self.x = (self.x as i32 + if orig <= col { -5 } else { 5 }) as u8;
        }
    }

    /// check_chomped_kid (0000:5B13): uses the same flag arrays, no early exit.
    fn check_chomped_kid(&mut self, env: &Env) {
        if !self.alive {
            return;
        }
        let row = self.row;
        for col in 0..10usize {
            if self.cd.flags[0][col] != 0xFF || self.cd.rooms[0][col] < 0 {
                continue;
            }
            let (room, c, r) = norm(env.level, self.cd.rooms[0][col] as i32, col as i32, row);
            if tile_n(env.level, room, c, r) != Tile::Chomper || mod_n(env.level, room, c, r) & 0x7F != 2 {
                continue;
            }
            if (1..=24).contains(&room) {
                self.events.push(Event::Chomped { room: room as u8, tilepos: (r * 10 + c) as u8 });
            }
            if self.frame == 178 || room != self.room as i32 {
                continue;
            }
            self.x = (X0 + COL_W * c + 7) as u8;
            self.forward(7 - (!self.facing_left) as i32);
            self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
            self.take_hp(100);
            self.events.push(Event::Sound(46));
            self.start_seq(env, 54);
            self.play_seq(env);
        }
    }

    /// running_jump (0000:6963): decides where a running jump starts relative to a pit or spikes ahead.
    fn running_jump(&mut self, env: &Env, c: &mut Ctrl) {
        if self.frame < 7 {
            return;
        }
        let dir = self.dir();
        let x4 = self.x as i32 + dir * 4;
        let mut col = floor_div(x4 - 7 - X0, COL_W);
        for i in 0..2 {
            col += dir;
            let t = self.tile(env, col, self.row);
            if t == Tile::Spikes || !is_floor(t) {
                let mut adj = self.distance_to_edge(x4) + COL_W * i - COL_W;
                // the unsigned compare of the original accepts only -8..=-1; too close (< -8) becomes -3, >= 0 returns
                if !(-8..=-1).contains(&adj) {
                    if adj >= 0 {
                        return;
                    }
                    adj = -3;
                }
                self.forward(adj + 4);
                break;
            }
        }
        c.up = c.release_arrows();
        self.start_seq(env, 4);
    }

    // ---------------------------------------------------------------- tile interaction (docs/tiles.md)

    /// (room, tilepos) of a tile relative to the kid room, following room links.
    fn locate(&self, env: &Env, col: i32, row: i32) -> Option<(u8, u8)> {
        env.level.wrap(self.room as i32, col, row).map(|(r, i)| (r as u8, i as u8))
    }

    /// check_press (0000:7E2E): standing on / hanging from / jumping under tiles that react.
    fn check_press(&mut self, env: &Env) {
        let (col, row) = (self.snap_col, self.row);
        let (c, r) = if (87..100).contains(&self.frame) || (135..141).contains(&self.frame) {
            (col, row - 1)
        } else if matches!(self.action, 7 | 5) || self.action < 2 {
            if self.frame == 79 && self.tile(env, col, row - 1) == Tile::LooseFloor {
                if let Some((room, tilepos)) = self.locate(env, col, row - 1) {
                    self.events.push(Event::LooseFall { room, tilepos });
                }
                (col, row - 1)
            } else if self.snap_rec.flags & 0x40 == 0 {
                return;
            } else {
                (col, row)
            }
        } else {
            return;
        };
        let Some((room, tilepos)) = self.locate(env, c, r) else { return };
        match self.tile(env, c, r) {
            Tile::DropButton | Tile::RaiseButton => self.events.push(Event::Button { room, tilepos, dead: !self.alive }),
            Tile::LooseFloor => self.events.push(Event::LooseFall { room, tilepos }),
            _ => {}
        }
    }

    /// check_spike_below (0000:7EE1): spikes under the image box of the kid start to rise.
    fn check_spike_below(&mut self, env: &Env) {
        let b = self.snap_box;
        let m7 = |x: i32| floor_div(x - 7 - X0, COL_W);
        let (lo, hi) = (m7(b.left), m7(b.right));
        if hi < 0 {
            return;
        }
        for col in lo..=hi {
            let mut row = self.row;
            while row <= 2 {
                let t = self.tile(env, col, row);
                if t == Tile::Spikes {
                    if let Some((room, tilepos)) = self.locate(env, col, row) {
                        self.events.push(Event::StartSpike { room, tilepos });
                    }
                    break;
                } else if !is_floor(t) {
                    row += 1;
                } else {
                    break;
                }
            }
        }
    }

    /// is_spike_harmful (0000:9B4E)
    fn spike_harm(m: u8) -> u8 {
        if m == 0 || m == 255 {
            0
        } else if (m as i8) < 0 {
            1
        } else if m >= 5 {
            0
        } else {
            2
        }
    }

    /// check_spiked (0000:7394)
    fn check_spiked(&mut self, env: &Env) {
        if !self.alive {
            return;
        }
        let (col, row) = (self.snap_col, self.row);
        if self.tile(env, col, row) != Tile::Spikes {
            return;
        }
        let h = Self::spike_harm(self.tile_mod(env, col, row));
        let f = self.frame;
        if (h >= 2 && ((7..=14).contains(&f) || (34..=39).contains(&f))) || ((f == 43 || f == 26) && h != 0) {
            self.spiked(env, col, row);
        }
    }

    /// spiked (0000:601F)
    fn spiked(&mut self, env: &Env, col: i32, row: i32) {
        if let Some((room, tilepos)) = self.locate(env, col, row) {
            self.events.push(Event::Spiked { room, tilepos });
        }
        self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
        self.x = (X0 + COL_W * col + 10) as u8;
        self.forward(8);
        self.fall_y = 0;
        self.events.push(Event::Sound(48));
        self.take_hp(100);
        self.start_seq(env, 51);
        self.play_seq(env);
    }

    /// check_knock (0000:4F7A): landings shake the row of the kid, bumping the ceiling shakes the row above.
    fn check_knock(&mut self) {
        if self.knock != 0 {
            let row = self.row - (self.knock > 0) as i32;
            self.events.push(Event::Knock { room: self.room, row });
            self.knock = 0;
        }
    }

    // ---------------------------------------------------------------- floor checks and falling

    /// check_action (0000:7309)
    fn check_action(&mut self, env: &Env) {
        match self.action {
            4 => self.do_fall(env),
            3 if (102..=105).contains(&self.frame) => self.check_grab(env),
            0 | 1 | 7 => self.check_on_floor(env),
            5 if self.frame == 109 => self.check_on_floor(env),
            _ => {}
        }
    }


    /// check_on_floor (0000:7551): only for frames with flag 0x40 (stored record, stale after a bump).
    fn check_on_floor(&mut self, env: &Env) {
        if self.snap_rec.flags & 0x40 == 0 {
            return;
        }
        let mut t = self.tile_at_char(env);
        if t == Tile::Wall {
            self.in_wall(env);
            t = self.tile_at_char(env);
        }
        if !is_floor(t) {
            self.start_fall(env); // (the level 12 hidden floor is not simulated)
        }
    }

    /// start_fall (0000:75F5)
    fn start_fall(&mut self, env: &Env) {
        let f0 = self.frame;
        self.row += 1;
        self.events.push(Event::StartChompers { room: self.room, row: self.row });
        let seq = match f0 {
            9 => 7,
            13 => 19,
            26 => 18,
            44 => 21,
            81..=85 => {
                self.forward(5);
                self.load_fram_det_col(env);
                19
            }
            150..=179 if self.charid >= 2 => {
                // guards pushed off a ledge (INFERRED: 83 when moving forward, else 82)
                if self.fall_x > 0 { 83 } else { 82 }
            }
            150..=179 => {
                self.sh.droppedout = true;
                if self.facing_left && self.dist_weight_snap() <= 7 {
                    self.forward(-5);
                }
                81
            }
            _ => 7,
        };
        self.sword = 0;
        self.start_seq(env, seq);
        self.play_seq(env);
        self.load_fram_det_col(env);
        if self.tile_at_char(env) == Tile::Wall {
            self.in_wall(env);
            return;
        }
        if self.tile(env, self.snap_col + self.dir(), self.row) == Tile::Wall {
            if f0 == 44 && self.dist_weight_snap() < 6 {
                self.start_seq(env, 104);
                self.play_seq(env);
            } else {
                self.forward(-1);
            }
            self.load_fram_det_col(env);
        }
    }

    /// do_fall (0000:5E98) and land (0000:5EF8)
    fn do_fall(&mut self, env: &Env) {
        let land = Y_LAND[(self.row + 1).clamp(0, 4) as usize];
        // unsigned 16-bit compare: row -1 (y_land -8) never lands
        if land as u16 > self.y as u16 {
            self.check_grab(env);
            return;
        }
        let mut t = self.tile_at_char(env);
        if t == Tile::Wall {
            self.in_wall(env);
            t = self.tile_at_char(env);
        }
        if !is_floor(t) {
            self.row += 1;
            return;
        }
        self.y = land as u8;
        // spikes at the foot (or just behind it) kill if they are not retracted
        let (col, row, d) = (self.snap_col, self.row, self.dir());
        let spike_col = if t == Tile::Spikes {
            Some(col)
        } else if self.alive && self.dist_weight_snap() >= 12 && self.tile(env, col - d, row) == Tile::Spikes {
            Some(col - d)
        } else {
            None
        };
        if let Some(sc) = spike_col {
            if Self::spike_harm(self.tile_mod(env, sc, row)) != 0 {
                self.spiked(env, sc, row);
                return;
            }
        }
        // a wall or empty tile in front, close by: step back
        if !is_floor(self.tile(env, col + d, row)) && self.dist_weight_snap() < 3 {
            self.forward(-3);
        }
        self.events.push(Event::StartChompers { room: self.room, row: self.row });
        let fy = self.fall_y;
        let mut seq = if fy < 22 {
            17
        } else if fy < 33 {
            if self.take_hp(1) { 22 } else { 20 }
        } else {
            self.take_hp(100);
            22
        };
        // fighters (guards, or a drawn sword) land in the en garde sequence (0000:5F85)
        if (self.charid != 0 || self.sword == 2) && seq != 22 {
            seq = 63;
        }
        self.start_seq(env, seq);
        self.play_seq(env);
        self.fall_y = 0;
    }

    // ---------------------------------------------------------------- room change (docs 10)

    /// leave_room (0000:3BF8) and goto_other_room (0000:3B7A). The original moves even when the link is 0 (room 0).
    fn check_room_change(&mut self, env: &Env) {
        let b = self.coll_box(env);
        let ys = self.y as i8;
        let dir = if !matches!(self.action, 5 | 4 | 3) && (-16..=9).contains(&ys) {
            Some(2)
        } else if self.y >= 211 {
            Some(3)
        } else if (135..=149).contains(&self.frame) || (110..=119).contains(&self.frame) || (150..=162).contains(&self.frame) || (166..=168).contains(&self.frame) || self.action == 7 {
            None
        } else if self.facing_left {
            if b.left <= 54 { Some(0) } else if b.left >= 198 { Some(1) } else { None }
        } else {
            let edge = self.tile(env, 9, self.row);
            if !matches!(edge, Tile::DoortopWithFloor | Tile::Doortop) && b.right >= 201 {
                Some(1)
            } else if b.right <= 57 {
                Some(0)
            } else {
                None
            }
        };
        let Some(dir) = dir else { return };
        let links = &env.level.links[(self.room as usize).wrapping_sub(1).min(23)];
        self.leave_dir = Some(dir as u8);
        self.room = links[dir];
        match dir {
            0 => self.x = (self.x as i32 + 140) as u8,
            1 => self.x = (self.x as i32 - 140) as u8,
            2 => {
                self.y = self.y.wrapping_add(189);
                self.row = row_from_y(self.y);
            }
            _ => {
                self.y = self.y.wrapping_sub(189);
                self.row = row_from_y(self.y);
            }
        }
        self.events.push(Event::StartChompers { room: self.room, row: self.row });
    }
}

#[cfg(test)]
impl Kid {
    /// Put the kid into a given state (docs/collisions.md test vectors).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn force(&mut self, env: &Env, x: u8, y: u8, row: i32, facing_left: bool, frame: u8, action: u8, seq: usize, fall: (i8, i8)) {
        (self.x, self.y, self.row, self.facing_left, self.frame, self.action) = (x, y, row, facing_left, frame, action);
        (self.fall_x, self.fall_y) = fall;
        self.start_seq(env, seq);
    }
}
