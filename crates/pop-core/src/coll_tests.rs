//! Collision test vectors from docs/collisions.md section 7 (hand-traced from the original code).
use crate::dat::Dat;
use crate::kid::{Env, Keys, Kid};
use crate::level::*;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
const NONE: Keys = Keys { left: false, right: false, up: false, down: false, shift: false };
const R: Keys = Keys { left: false, right: true, up: false, down: false, shift: false };
const RU: Keys = Keys { left: false, right: true, up: true, down: false, shift: false };
const L: Keys = Keys { left: true, right: false, up: false, down: false, shift: false };

struct Rig {
    exe: crate::exe::Exe,
    frames: Vec<crate::exe::Frame>,
    sizes: Vec<(u16, u16)>,
    level: Level,
}

impl Rig {
    fn env(&self) -> Env<'_> {
        Env { exe: &self.exe, frames: &self.frames, level: &self.level, sizes: &self.sizes }
    }
}

/// Level with floor everywhere in `rooms` (1-based), links all 0, then `tiles` (room, col, row, type) and `links` (room, dir, target).
fn rig(rooms: usize, tiles: &[(usize, usize, usize, u8)], links: &[(usize, usize, u8)], mods: &[(usize, usize, usize, u8)]) -> Rig {
    let mut b = vec![0u8; 2305];
    for r in 0..rooms {
        for t in 0..30 {
            b[r * 30 + t] = 1;
        }
    }
    for &(room, col, row, t) in tiles {
        b[(room - 1) * 30 + row * 10 + col] = t;
    }
    for &(room, dir, to) in links {
        b[1952 + (room - 1) * 4 + dir] = to;
    }
    for g in 2119..2143 {
        b[g] = 30;
    }
    (b[2112], b[2113], b[2114]) = (1, 11, 255);
    let mut level = Level::parse(&b).unwrap();
    for &(room, col, row, m) in mods {
        level.set_modifier(room, row * 10 + col, m);
    }
    let exe = crate::exe::Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
    let frames = exe.kid_frames();
    let sizes = crate::image::image_sizes(&Dat::open(format!("{DIR}KID.DAT")).unwrap(), 401, 220);
    Rig { exe, frames, sizes, level }
}

/// state after each tick: (x, y, frame, seq, room)
type Trace = Vec<(u8, u8, u8, usize, u8)>;

fn run(r: &Rig, mut k: Kid, ticks: usize, keys: impl Fn(usize) -> Keys) -> Trace {
    let env = r.env();
    let mut out = vec![];
    for t in 0..ticks {
        k.tick(&env, keys(t));
        out.push((k.x, k.y, k.frame, k.seq, k.room));
    }
    out
}

fn stand(r: &Rig, x: u8, left: bool) -> Kid {
    let env = r.env();
    let mut k = Kid::spawn_standing(&env);
    k.force(&env, x, 118, 1, left, 15, 0, 2, (0, 0));
    k
}

fn at(t: &Trace, i: usize) -> (u8, u8, u8, usize) {
    let e = t[i];
    (e.0, e.1, e.2, e.3)
}

#[test]
fn vector_a_run_right_into_wall() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 86, false), 28, |_| R);
    assert_eq!(at(&t, 0).3, 1, "t0 starts the run");
    assert_eq!((t[4].0, t[4].2), (94, 5));
    assert_eq!((t[13].0, t[13].2), (122, 14));
    assert_eq!(t[21].0, 148, "no bump yet at t21: {:?}", t[21]);
    assert_eq!((t[22].0, t[22].2, t[22].3), (145, 50, 47), "bump at t22: {:?}", t[22]);
    assert_eq!((t[25].0, t[25].2), (145, 15));
}

#[test]
fn vector_b_run_left_into_wall() {
    let r = rig(1, &[(1, 1, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 184, true), 36, |_| L);
    assert_eq!((t[30].0, t[30].2), (92, 7), "{:?}", t[30]);
    assert_eq!((t[31].0, t[31].2, t[31].3), (96, 50, 47), "{:?}", t[31]);
}

#[test]
fn vector_c_other_phase() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 90, false), 25, |_| R);
    assert_eq!((t[20].0, t[20].2), (149, 13), "{:?}", t[20]);
    assert_eq!((t[21].0, t[21].2, t[21].3), (145, 50, 47), "{:?}", t[21]);
}

#[test]
fn vector_d_start_next_to_wall() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 139, false), 10, |_| R);
    assert_eq!(t[0].3, 1);
    assert_eq!((t[4].0, t[4].2), (147, 5), "{:?}", t[4]);
    assert_eq!((t[5].0, t[5].2, t[5].3), (145, 50, 47), "{:?}", t[5]);
}

#[test]
fn vector_e_safe_step_does_not_bump() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 143, false), 30, |i| if i < 12 { R } else { NONE });
    assert_eq!(t[0].3, 34, "safe step 28 + 6: {:?}", t[0]);
    assert!(t.iter().all(|e| !matches!(e.3, 45 | 46 | 47)), "no bump expected: {t:?}");
    // `seq` keeps the number of the last started sequence; the kid ends standing (frame 15)
    assert_eq!((t.last().unwrap().0, t.last().unwrap().2), (149, 15));
}

#[test]
fn vector_f_standing_jump_into_wall() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    // (a) far enough: no bump
    let t = run(&r, stand(&r, 110, false), 30, |i| if i == 0 { RU } else { NONE });
    assert!(t.iter().all(|e| !matches!(e.3, 45 | 46 | 47)), "(a) no bump expected");
    // docs/collisions.md says 148 here, but the dx ops of sequence 3 sum to 39 (verified in the EXE data): 110 + 39
    assert_eq!(t.last().unwrap().0, 149);
    // (b) closer: hard bump in the air at t9, second bump at t15
    let t = run(&r, stand(&r, 120, false), 20, |i| if i == 0 { RU } else { NONE });
    assert_eq!((t[8].0, t[8].2), (146, 24), "{:?}", t[8]);
    assert_eq!((t[9].0, t[9].1, t[9].2, t[9].3), (148, 114, 102, 46), "{:?}", t[9]);
    assert_eq!((t[15].0, t[15].2, t[15].3), (145, 50, 47), "{:?}", t[15]);
}

#[test]
fn vector_g_running_jump_into_wall_over_floor() {
    let r = rig(1, &[(1, 7, 1, 20)], &[], &[]);
    let t = run(&r, stand(&r, 86, false), 22, |i| if i == 8 { RU } else { R });
    assert_eq!((t[8].0, t[8].2, t[8].3), (105, 34, 4), "{:?}", t[8]);
    assert_eq!((t[18].0, t[18].2, t[18].3), (159, 50, 47), "{:?}", t[18]);
}

#[test]
fn vector_h_running_jump_into_wall_over_pit() {
    let r = rig(1, &[(1, 7, 1, 20), (1, 6, 1, 0)], &[], &[]);
    let t = run(&r, stand(&r, 86, false), 22, |i| if i == 8 { RU } else { R });
    assert_eq!((t[18].0, t[18].1, t[18].2, t[18].3), (160, 121, 102, 45), "{:?}", t[18]);
}

#[test]
fn vector_i_falling_past_a_wall() {
    let r = rig(1, &[(1, 6, 1, 20)], &[], &[]);
    let env = r.env();
    let mut k = Kid::spawn_standing(&env);
    k.force(&env, 144, 60, 1, false, 106, 4, 12, (2, 6));
    let t = run(&r, k, 6, |_| NONE);
    assert_eq!((t[2].0, t[2].1), (145, 96), "{:?}", t[2]);
    assert_eq!(t[4].3, 17, "lands with seq 17: {:?}", t[4]);
}

#[test]
fn vector_j_wall_in_right_neighbour_room() {
    let r = rig(2, &[(2, 0, 1, 20)], &[(1, 1, 2), (2, 0, 1)], &[]);
    let t = run(&r, stand(&r, 150, false), 22, |_| R);
    assert_eq!((t[18].0, t[18].4), (62, 2), "room change at t18: {:?}", t[18]);
    assert_eq!((t[19].0, t[19].2, t[19].3, t[19].4), (61, 50, 47, 2), "bump after the room change: {:?}", t[19]);
}

#[test]
fn vector_k_wall_in_left_neighbour_room() {
    let r = rig(2, &[(1, 9, 1, 20)], &[(1, 1, 2), (2, 0, 1)], &[]);
    let env = r.env();
    let mut k = Kid::spawn_standing(&env);
    k.force(&env, 120, 118, 1, true, 15, 0, 2, (0, 0));
    k.room = 2;
    let t = run(&r, k, 24, |_| L);
    assert_eq!((t[19].0, t[19].2, t[19].3, t[19].4), (68, 50, 47, 2), "{:?}", t[19]);
}

#[test]
fn vector_l_closed_gate_bumps_open_gate_does_not() {
    let r = rig(1, &[(1, 6, 1, 4)], &[], &[(1, 6, 1, 0)]);
    let t = run(&r, stand(&r, 86, false), 30, |_| R);
    assert_eq!((t[25].0, t[25].2, t[25].3), (155, 50, 47), "{:?}", t[25]);
    let r = rig(1, &[(1, 6, 1, 4)], &[], &[(1, 6, 1, 188)]);
    let t = run(&r, stand(&r, 86, false), 30, |_| R);
    assert!(t.iter().all(|e| e.3 != 47), "open gate must not bump");
    assert!(t[29].0 > 160);
}

#[test]
fn vector_m_closing_gate_pushes_the_standing_kid() {
    let r = rig(1, &[(1, 5, 1, 4)], &[], &[(1, 5, 1, 100)]);
    let t = run(&r, stand(&r, 146, false), 4, |_| NONE);
    assert_eq!(t[0].0, 146, "no push on the first tick (previous flags unknown)");
    assert_eq!(t[1].0, 141, "{:?}", t[1]);
    assert_eq!(t[2].0, 141);
    let t = run(&r, stand(&r, 152, false), 4, |_| NONE);
    assert_eq!(t[1].0, 157, "{:?}", t[1]);
}
