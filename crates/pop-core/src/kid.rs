//! The kid: controls, sequence player and ground/fall physics, ported from PRINCE.EXE v1.1.
//! See docs/mechanics.md for the evidence (addresses) of every rule. Implemented: standing, start run,
//! running, stop, turn, run turn, standing/running jump, crouch/stand up, safe steps, falling and landing.
//! Not yet: climbing, hanging, jump up, combat, traps, gates/buttons, bumping into walls, level doors.
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

impl Kid {
    /// Kid at the level's start position, standing.
    pub fn spawn(env: &Env) -> Kid {
        let l = env.level;
        let (col, row) = ((l.start_tile % 10) as i32, (l.start_tile / 10) as i32);
        let mut k = Kid {
            // provisional: the original start x is not verified; this puts the standing kid's foot mid-tile
            x: (X0 + COL_W * col + 17) as u8,
            y: Y_LAND[row as usize + 1] as u8,
            facing_left: l.start_dir == 255,
            frame: 15,
            room: l.start_room,
            row,
            alive: true,
            hp: 3,
            controlled: true,
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

    /// distance_to_edge(x) (0000:78D0): distance from x to the tile edge in front, 0..=13.
    fn distance_to_edge(&self, x: i32) -> i32 {
        let m = (x - 7 - X0).rem_euclid(COL_W);
        if self.facing_left { m } else { COL_W - 1 - m }
    }

    /// dist_from_wall_forward (0000:5DB8) is not decoded yet; this is our reconstruction from the offset tables.
    fn dist_from_wall(&self, col: i32, wt: usize, foot: i32) -> i32 {
        let left = X0 + COL_W * col;
        if self.facing_left {
            foot - (left + COL_W - 1 - WALL_DIST_FROM_RIGHT[wt])
        } else {
            left + WALL_DIST_FROM_LEFT[wt] - foot
        }
    }

    /// get_edge_distance (0000:5A24): returns (edge_type, distance, tile looked at).
    fn edge_distance(&self, env: &Env) -> (i32, i32, Tile) {
        let (col, foot) = (self.col(env), self.foot_x(env));
        let here = self.tile(env, col, self.row);
        let wt = wall_type(here);
        if wt != 0 {
            let d = self.dist_from_wall(col, wt, foot);
            if d >= 0 {
                return if d < 14 { (1, d, here) } else { (2, 11, here) };
            }
        }
        let fc = col + self.dir();
        let front = self.tile(env, fc, self.row);
        let dw = self.distance_to_edge(foot);
        use Tile::*;
        if (front == Doortop && !self.facing_left) || front == LooseFloor {
            return (0, dw, front);
        }
        let wt = wall_type(front);
        if wt != 0 {
            let d = self.dist_from_wall(fc, wt, foot);
            if d >= 0 {
                return if d < 14 { (1, d, front) } else { (2, 11, front) };
            }
        }
        if matches!(front, DropButton | Sword | Potion) {
            return if dw != 0 { (0, dw, front) } else { (2, 11, front) };
        }
        if is_floor(front) { (2, 11, front) } else { (0, dw, front) }
    }

    // ------------------------------------------------------------------ input

    /// One tick of input handling (control_kid, 0000:7A19) followed by the same-tick physics.
    pub fn tick(&mut self, env: &Env, keys: Keys) {
        if self.controlled && self.alive {
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
            self.x = (self.x as i32 + self.dir() * self.fall_x as i32) as u8;
        }
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
            109 => self.control_crouched(env, c),
            _ => {} // hanging (87..=99) and others: not implemented
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
        // level doors and check_jump_up (jump up / grab) are not implemented
        if c.x == -1 {
            self.standing_jump(env, c);
        }
    }

    fn down_pressed(&mut self, env: &Env, c: &mut Ctrl) {
        c.down = 1;
        let (col, foot) = (self.col(env), self.foot_x(env));
        let front = self.tile(env, col + self.dir(), self.row);
        if !is_floor(front) && self.distance_to_edge(foot) < 3 {
            self.x = (self.x as i32 + self.dir() * 5) as u8; // shuffle to the edge; no sequence
        } else {
            // climb down (needs hanging) is not implemented
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
                let adj = (self.distance_to_edge(x4) + COL_W * i - COL_W) as u8;
                let mut adj = adj;
                if adj.wrapping_add(8) >= 10 {
                    if adj < 128 {
                        return;
                    }
                    adj = (-3i8) as u8;
                }
                self.x = (self.x as i32 + dir * (adj as i8 as i32 + 4)) as u8;
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
            let dir = self.dir();
            match op {
                Op::Frame(n) => {
                    self.frame = n;
                    return;
                }
                Op::Dx(d) => self.x = (self.x as i32 + dir * d as i32) as u8,
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

    /// check_action (0000:7309)
    fn check_action(&mut self, env: &Env) {
        match self.action {
            4 => self.do_fall(env),
            0 | 1 | 7 => self.check_on_floor(env),
            5 if self.frame == 109 => self.check_on_floor(env),
            _ => {} // 2, 3 (grab), 6: not implemented
        }
    }

    /// check_on_floor (0000:7551): only for frames with flag 0x40.
    fn check_on_floor(&mut self, env: &Env) {
        if self.frame_rec(env).flags & 0x40 == 0 {
            return;
        }
        let t = self.tile(env, self.col(env), self.row);
        if !is_floor(t) && t != Tile::Wall {
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
                self.x = (self.x as i32 + self.dir() * 5) as u8;
                19
            }
            _ => 7,
        };
        self.start_seq(env, seq);
        self.play_seq(env);
        let (col, front) = (self.col(env), self.col(env) + self.dir());
        let _ = col;
        if self.tile(env, front, self.row) == Tile::Wall {
            self.x = (self.x as i32 - self.dir()) as u8;
        }
    }

    /// do_fall (0000:5E98) and land (0000:5EF8)
    fn do_fall(&mut self, env: &Env) {
        if (self.y as i32) < Y_LAND[(self.row + 1).clamp(0, 4) as usize] {
            return; // check_grab not implemented
        }
        let t = self.tile(env, self.col(env), self.row);
        if is_floor(t) || t == Tile::Wall {
            // land (a wall counts as floor here: in_wall is not decoded yet)
            self.y = Y_LAND[(self.row + 1).clamp(0, 4) as usize] as u8;
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

    /// Room change (goto_other_room, 0000:3B7A). The trigger conditions are our inference: the foot leaves the
    /// room's column range, or the row leaves 0..=2.
    fn check_room_change(&mut self, env: &Env) {
        let l = &env.level.links[(self.room as usize).wrapping_sub(1).min(23)];
        let col = self.col(env);
        if col < 0 && l[0] != 0 {
            self.room = l[0];
            self.x = (self.x as i32 + 140) as u8;
        } else if col > 9 && l[1] != 0 {
            self.room = l[1];
            self.x = (self.x as i32 - 140) as u8;
        } else if self.row < 0 && l[2] != 0 {
            self.room = l[2];
            self.row += 3;
            self.y = self.y.wrapping_add(189);
        } else if self.row > 2 && l[3] != 0 {
            self.room = l[3];
            self.row -= 3;
            self.y = self.y.wrapping_sub(189);
        }
    }
}
