//! The kid: controls, sequence player, physics, wall collisions, room changes, grab/hang/climb,
//! ported from PRINCE.EXE v1.1. docs/mechanics.md has the evidence (addresses) for every rule; rules we
//! had to reconstruct are marked INFERRED in comments. Not yet: combat, traps, gates/buttons, level doors, items.
use crate::exe::{Exe, Frame, Op};
use crate::level::{Level, Tile};

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

/// Everything the kid needs to look at besides itself.
pub struct Env<'a> {
    pub exe: &'a Exe,
    pub frames: &'a [Frame],
    pub level: &'a Level,
    /// (height, width) in pixels of KID.DAT image `i` (resource 401 + i), see `image::image_sizes`
    pub sizes: &'a [(u16, u16)],
}

/// Control state as seen inside `control()`: x is already flipped so that -1 = forward.
/// Flags: -1 held (fires), 0 released, 1 consumed (stays until the key is let go).
#[derive(Clone, Copy, Default)]
struct Ctrl {
    x: i8,
    y: i8,
    shift: i8,
    fwd: i8,
    bwd: i8,
    up: i8,
    down: i8,
    shift2: i8,
}

impl Ctrl {
    /// `release_arrows` (0000:7B48): clears the four flags and returns 1, which the caller stores in the flag it just used.
    fn release_arrows(&mut self) -> i8 {
        self.fwd = 0;
        self.bwd = 0;
        self.up = 0;
        self.down = 0;
        1
    }
}

/// Collision data of one tile column for the "box starts overlapping a wall" test (check_collisions).
type RowFlags = [u8; 10];

#[derive(Clone, Debug, Default)]
pub struct Kid {
    // original Char fields
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
    /// collision data of last tick for rows (row-1, row, row+1): (room, row, flags)
    prev_coll: Option<(u8, i32, [RowFlags; 3])>,
    bump_left: i32,
    bump_right: i32,
    /// false = only play the current sequence (sequence viewer)
    pub controlled: bool,
}

fn floor_div(a: i32, b: i32) -> i32 {
    a.div_euclid(b)
}

/// tile_is_floor (0000:7364)
fn is_floor(t: Tile) -> bool {
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
        env.frames.get(self.frame as usize).copied().unwrap_or(Frame { image: 255, sword: 0, dx: 0, dy: 0, flags: 0 })
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

    fn here(&self, env: &Env) -> Tile {
        self.tile(env, self.col(env), self.row)
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
        let (h, w) = env.sizes.get(f.image as usize).copied().unwrap_or((0, 0));
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

    /// is_obstacle (0000:5726) for the tile at `col` in the current row.
    fn is_obstacle(&self, env: &Env, col: i32, height: i32) -> bool {
        use Tile::*;
        let t = self.tile(env, col, self.row);
        let m = self.tile_mod(env, col, self.row) as i32;
        match t {
            Potion => false,
            Gate => (m >> 2) + 6 < height, // can_bump_into_gate (0000:59FF)
            Chomper => m == 2,
            Mirror => !(self.frame >= 39 && self.frame <= 43 && self.facing_left), // jumping through the mirror is not simulated
            _ => wall_type(t) != 0,
        }
    }

    /// dist_from_wall_forward (0000:5DB8): uses the collision box, not the foot. -1 = no wall.
    fn dist_from_wall(&self, env: &Env, col: i32, t: Tile) -> i32 {
        let wt = wall_type(t);
        if wt == 0 {
            return -1;
        }
        let b = self.coll_box(env);
        if t == Tile::Gate && !self.is_obstacle(env, col, b.height) {
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
        if self.room == 0 {
            return; // outside the level: the original skips the tick
        }
        if self.controlled && self.alive {
            self.grab_timer = (self.grab_timer - 1).max(0);
            self.shift = keys.shift;
            self.control_kid(env, keys);
        }
        self.play_seq(env);
        if !self.controlled {
            return;
        }
        // fall_accel
        if self.action == 4 {
            self.fall_y = (self.fall_y as i32 + 3).min(33) as i8;
        }
        // fall_speed (every tick)
        self.y = self.y.wrapping_add(self.fall_y as u8);
        if self.action == 4 {
            self.forward(self.fall_x as i32);
        }
        self.check_collisions(env);
        self.check_bumped(env);
        self.check_action(env);
        self.check_room_change(env);
    }

    fn control_kid(&mut self, env: &Env, k: Keys) {
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
    fn control(&mut self, env: &Env, c: &mut Ctrl) {
        if self.action == 5 || self.action == 4 {
            c.release_arrows();
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

    fn running_jump(&mut self, env: &Env, c: &mut Ctrl) {
        if self.frame < 7 {
            return;
        }
        let dir = self.dir();
        let x4 = self.x as i32 + dir * 4;
        let col0 = floor_div(x4 - 7 - X0, COL_W);
        for i in 1..=2 {
            let t = self.tile(env, col0 + dir * i, self.row);
            if t == Tile::Spikes || !is_floor(t) {
                let mut adj = (self.distance_to_edge(x4) + COL_W * i - COL_W) as u8;
                if adj.wrapping_add(8) >= 10 {
                    if adj < 128 {
                        return;
                    }
                    adj = (-3i8) as u8;
                }
                self.forward(adj as i8 as i32 + 4);
                break;
            }
        }
        c.up = c.release_arrows();
        self.start_seq(env, 4);
    }

    fn control_crouched(&mut self, env: &Env, c: &mut Ctrl) {
        if c.y != 1 {
            self.start_seq(env, 49);
        } else if c.fwd == -1 {
            c.fwd = 1;
            self.start_seq(env, 79);
        }
    }

    // ---------------------------------------------------------------- sequences and physics

    /// play_seq (0000:6F90): run ops until the next frame (exactly one frame per tick).
    fn play_seq(&mut self, env: &Env) {
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
                Op::Up => self.row -= 1,
                Op::Down => self.row += 1,
                Op::Jmp(t) => {
                    self.cur = env.exe.ops_from(t).unwrap_or_default();
                    self.pc = 0;
                }
                // feather fall is never active; sounds, items, knock and level end are not simulated yet
                Op::JmpIfFeather(_) | Op::Sound(_) | Op::GetItem(_) | Op::KnockDown | Op::KnockUp | Op::EndLevel | Op::Nop => {}
            }
        }
    }

    // ---------------------------------------------------------------- wall collisions (docs 9)

    /// left_wall_x / right_wall_x of the tile at `col` in `row` (0000:55CE / 0000:5607).
    fn wall_xs(&self, env: &Env, col: i32, row: i32) -> (i32, i32) {
        let wt = wall_type(self.tile(env, col, row));
        if wt == 0 {
            return (0xFF, 0);
        }
        let tile_left = X0 + COL_W * col + 7;
        (WALL_DIST_FROM_LEFT[wt] + tile_left, tile_left - WALL_DIST_FROM_RIGHT[wt] + 13)
    }

    /// check_collisions (0000:53AC): remember which tile columns the box overlaps; a bump fires on the tick the overlap starts.
    fn check_collisions(&mut self, env: &Env) {
        self.bump_left = -1;
        self.bump_right = -1;
        if self.action == 7 {
            return;
        }
        let b = self.coll_box(env);
        let m7 = |x: i32| floor_div(x - 7 - X0, COL_W);
        let (lo, hi) = (m7(b.left_coll) - 1, (m7(b.right_coll) + 2).min(11));
        let mut now = [[0u8; 10]; 3];
        for (ri, rows) in now.iter_mut().enumerate() {
            let r = self.row - 1 + ri as i32;
            for col in lo.max(0)..=hi.min(9) {
                let (lw, rw) = self.wall_xs(env, col, r);
                rows[col as usize] = (if lw < b.right_coll { 0x0F } else { 0 }) | (if rw > b.left_coll { 0xF0 } else { 0 });
            }
        }
        if let Some((proom, prow, prev)) = &self.prev_coll {
            let idx = self.row - prow + 1;
            if *proom == self.room && (0..3).contains(&idx) {
                let (p, n) = (&prev[idx as usize], &now[1]);
                for col in (0..10).rev() {
                    if p[col] & 0x0F == 0 && n[col] & 0x0F != 0 {
                        self.bump_left = col as i32;
                    }
                    if p[col] & 0xF0 == 0 && n[col] & 0xF0 != 0 {
                        self.bump_right = col as i32;
                    }
                }
            }
        }
        self.prev_coll = Some((self.room, self.row, now));
    }

    /// check_bumped (0000:5645)
    fn check_bumped(&mut self, env: &Env) {
        if self.action == 2 || self.action == 6 || (135..=148).contains(&self.frame) {
            return;
        }
        let b = self.coll_box(env);
        if self.bump_left >= 0 {
            if !self.facing_left && self.is_obstacle(env, self.bump_left, b.height) {
                let (lw, _) = self.wall_xs(env, self.bump_left, self.row);
                self.bumped(env, lw - b.right_coll, true, self.bump_left);
            }
        } else if self.bump_right >= 0 && self.facing_left && self.is_obstacle(env, self.bump_right, b.height) {
            let (_, rw) = self.wall_xs(env, self.bump_right, self.row);
            self.bumped(env, rw - b.left_coll, false, self.bump_right);
        }
    }

    /// bumped (0000:57F0). The "look one column further" step is our reading of the disassembly summary (INFERRED).
    fn bumped(&mut self, env: &Env, delta: i32, push_left: bool, col: i32) {
        if !self.alive || self.frame == 177 {
            return;
        }
        self.x = (self.x as i32 + delta) as u8;
        let t = self.tile(env, col, self.row);
        let tc = if push_left {
            if t == Tile::Wall { col - 1 } else { col }
        } else if matches!(t, Tile::Doortop | Tile::DoortopWithFloor | Tile::Wall) {
            col + 1
        } else {
            col
        };
        if is_floor(self.tile(env, tc, self.row)) { self.bumped_floor(env) } else { self.bumped_fall(env) }
    }

    fn bumped_fall(&mut self, env: &Env) {
        let _ = env;
        self.forward(-4);
        if self.action == 4 {
            self.fall_x = 0;
        } else {
            self.start_seq(env, 45);
        }
    }

    fn bumped_floor(&mut self, env: &Env) {
        let land = Y_LAND[(self.row + 1).clamp(0, 4) as usize];
        if (land - self.y as i32) as u8 >= 15 {
            self.bumped_fall(env);
            return;
        }
        self.y = land as u8;
        if self.fall_y >= 22 {
            self.forward(-5);
            return;
        }
        self.fall_y = 0;
        if self.alive {
            let hard = matches!(self.frame, 24 | 25 | 40..=42 | 102..=106);
            self.start_seq(env, if hard { 46 } else { 47 });
            self.play_seq(env);
        }
    }

    /// in_wall (0000:7809): push the kid out of a wall tile.
    fn in_wall(&mut self, env: &Env) {
        let mut d = self.dist_weight(env);
        if d >= 8 || self.tile(env, self.col(env) + self.dir(), self.row) == Tile::Wall {
            d = 6 - d;
        } else {
            d += 4;
        }
        self.forward(d);
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

    /// check_on_floor (0000:7551): only for frames with flag 0x40.
    fn check_on_floor(&mut self, env: &Env) {
        if self.frame_rec(env).flags & 0x40 == 0 {
            return;
        }
        if self.here(env) == Tile::Wall {
            self.in_wall(env);
        }
        if !is_floor(self.here(env)) && self.here(env) != Tile::Wall {
            self.start_fall(env);
        }
    }

    /// start_fall (0000:75F5)
    fn start_fall(&mut self, env: &Env) {
        self.row += 1;
        let seq = match self.frame {
            9 => 7,
            13 => 19,
            26 => 18,
            44 => 21,
            81..=85 => {
                self.forward(5);
                19
            }
            _ => 7,
        };
        self.start_seq(env, seq);
        self.play_seq(env);
        if self.here(env) == Tile::Wall {
            self.in_wall(env);
        }
        let front = self.tile(env, self.col(env) + self.dir(), self.row);
        if front == Tile::Wall {
            if self.frame == 44 && self.dist_weight(env) < 6 {
                self.start_seq(env, 104);
            } else {
                self.forward(-1);
            }
        }
    }

    /// do_fall (0000:5E98) and land (0000:5EF8)
    fn do_fall(&mut self, env: &Env) {
        let land = Y_LAND[(self.row + 1).clamp(0, 4) as usize];
        if (self.y as i32) < land {
            self.check_grab(env);
            return;
        }
        let t = self.here(env);
        if t == Tile::Wall {
            self.in_wall(env);
        } else if is_floor(t) {
            self.y = land as u8;
            let fy = self.fall_y;
            let seq = if fy < 22 {
                17
            } else if fy < 33 {
                self.hp -= 1;
                if self.hp <= 0 { self.alive = false; 22 } else { 20 }
            } else {
                self.hp = 0;
                self.alive = false;
                22
            };
            self.start_seq(env, seq);
            self.play_seq(env);
            self.fall_y = 0;
        } else {
            self.row += 1;
        }
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
        self.prev_coll = None;
    }
}
