//! Headless tests of the tile mechanics (docs/tiles.md) on synthetic levels and a smoke test on all shipped levels.
use crate::dat::Dat;
use crate::kid::Keys;
use crate::level::*;
use crate::sim::{Data, World};
use std::sync::Arc;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
const NONE: Keys = Keys { left: false, right: false, up: false, down: false, shift: false };
const RIGHT: Keys = Keys { left: false, right: true, up: false, down: false, shift: false };

fn data() -> Arc<Data> {
    let exe = crate::exe::Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
    let frames = exe.kid_frames();
    let sizes = crate::image::image_sizes(&Dat::open(format!("{DIR}KID.DAT")).unwrap(), 401, 220);
    let gsizes = ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"].iter().map(|f| crate::image::image_sizes(&Dat::open(format!("{DIR}{f}")).unwrap(), 751, 40)).collect();
    Arc::new(Data { gframes: exe.guard_frames(), exe, frames, sizes, gsizes })
}

/// Room 1 with floor on rows 1 and 2; the kid starts at (col, 1) facing right. `tiles` = (col, row, type) overrides.
fn world(start_col: usize, tiles: &[(usize, usize, u8)], links: &[(u8, u8)]) -> World {
    World::new(data(), synth(1, 1..3, start_col, 1, tiles, links), 2)
}

/// Level with floor on `rows` of `room`; the kid starts there at (col, row) facing right.
fn synth(room: usize, rows: std::ops::Range<usize>, start_col: usize, start_row: usize, tiles: &[(usize, usize, u8)], links: &[(u8, u8)]) -> Level {
    let mut b = vec![0u8; 2305];
    let o = (room - 1) * 30;
    for r in rows {
        for c in 0..10 {
            b[o + r * 10 + c] = 1;
        }
    }
    for &(c, r, t) in tiles {
        b[o + r * 10 + c] = t;
    }
    for g in 2119..2143 {
        b[g] = 30;
    }
    (b[2112], b[2113], b[2114]) = (room as u8, (10 * start_row + start_col) as u8, 255);
    for &(i, v) in links {
        b[1440 + i as usize] = v; // doorlinks1
    }
    Level::parse(&b).unwrap()
}

/// n logic ticks; a frame longer than 6 steps (the sword frame length) fails instead of hanging
fn tick(w: &mut World, keys: Keys, n: usize) {
    for _ in 0..n {
        let mut steps = 1;
        while !w.step(keys) {
            steps += 1;
            assert!(steps <= crate::sim::SWORD_SUBSTEPS, "a logic frame took more than {} steps", crate::sim::SWORD_SUBSTEPS);
        }
    }
}

#[test]
fn button_opens_gate_for_285_ticks() {
    // raise button (15) at col 3 row 1 (link 0) -> gate (4) at col 6 row 1 (tile 16 of room 1); the kid stands on the button
    let mut b_links = vec![];
    b_links.push((0u8, 0x80 | 0x20 | 16)); // last entry, room low bits = 1, tile 16
    let mut w = world(3, &[(3, 1, 15), (6, 1, 4)], &b_links);
    assert_eq!(w.level.tile_at(1, 16).modifier, 0, "gates start closed (raw 2 -> 0)");
    tick(&mut w, NONE, 1);
    // first tick: the kid presses the plate (trob added), the gate starts opening
    assert!(w.trobs.iter().any(|t| t.tilepos == 13 && t.ty == 1), "button trob missing: {:?}", w.trobs);
    assert!(w.trobs.iter().any(|t| t.tilepos == 16 && t.ty == 1), "gate trob missing: {:?}", w.trobs);
    let mut max = 0;
    for _ in 0..60 {
        tick(&mut w, NONE, 1);
        max = max.max(w.level.tile_at(1, 16).modifier);
    }
    assert!(max >= 188, "gate never opened: {max}");
    // the kid keeps standing on the plate: the gate is held at 238 (restarted every tick)
    assert!(w.level.tile_at(1, 16).modifier >= 188);
    assert!(w.sounds.contains(&3) && w.sounds.contains(&5));
}

#[test]
fn loose_floor_falls_ten_ticks_after_being_pressed() {
    // loose floor (11) at col 3 row 1: the kid stands on it
    let mut w = world(3, &[(3, 1, 11)], &[]);
    let mut fall_tick = None;
    for t in 1..=30 {
        tick(&mut w, NONE, 1);
        if fall_tick.is_none() && w.level.tile_at(1, 13).kind == Tile::Empty {
            fall_tick = Some(t);
        }
    }
    let t = fall_tick.expect("loose floor never fell");
    assert!((10..=12).contains(&t), "fell at tick {t}"); // pressed in tick 1, falls 10 ticks later
    tick(&mut w, NONE, 30);
    assert!(w.kid.alive);
    assert_eq!(w.kid.row, 2, "kid should have dropped to the next row");
}

#[test]
fn spikes_cycle_and_kill_a_runner() {
    // spikes (2) at col 6 row 1; the kid runs at them
    let mut w = world(2, &[(6, 1, 2)], &[]);
    let mut died = false;
    for _ in 0..40 {
        tick(&mut w, RIGHT, 1);
        died |= !w.kid.alive;
    }
    assert!(died, "runner should have been impaled (seq {} frame {})", w.kid.seq, w.kid.frame);
    assert_eq!(w.level.tile_at(1, 16).modifier, 255, "impaling leaves the spikes stuck out");
}

#[test]
fn spike_cycle_lasts_23_ticks() {
    let mut w = world(0, &[(8, 1, 2)], &[]);
    w.kid.alive = false; // keep the kid out of the way: only the tile cycle is of interest
    w.kid.controlled = false;
    w.set_mod_for_test(1, 18, 0);
    w.start_spike_for_test(1, 18);
    let mut out = vec![];
    for _ in 0..30 {
        tick(&mut w, NONE, 1);
        out.push(w.level.tile_at(1, 18).modifier);
    }
    // rises 1..4, out (>= 0x81) for ticks 5..19, retracts, done after 23 ticks
    assert_eq!(&out[..4], &[1, 2, 3, 4]);
    assert_eq!(out[4], 0x8F);
    assert_eq!(out[22], 0);
    assert!(w.trobs.is_empty());
}

#[test]
fn chompers_start_staggered() {
    let w = world(0, &[(1, 1, 18), (2, 1, 18), (3, 1, 18)], &[]);
    let m: Vec<u8> = (11..14).map(|i| w.level.tile_at(1, i).modifier).collect();
    assert_eq!(m, [15, 12, 9]);
}

/// The kid crouches on a potion of `kind` and drinks it (docs/edge_cases.md 5.1). Returns the world after the
/// pick-up frame and every flash seen on the way.
fn drink(kind: u8, hp: i32) -> (World, Vec<(u8, u8)>) {
    let mut w = world(3, &[], &[]);
    tick(&mut w, NONE, 10);
    let col = w.kid.col(&crate::sim::mk_env(&w.data, &w.level, w.guard_type));
    w.set_tile_for_test(1, 10 + col as u8, Tile::Potion);
    w.set_mod_for_test(1, 10 + col as u8, kind << 5);
    w.kid.hp = hp;
    let down = Keys { down: true, ..NONE };
    for _ in 0..20 {
        if w.kid.frame == 109 {
            break;
        }
        tick(&mut w, down, 1);
    }
    tick(&mut w, Keys { shift: true, ..down }, 1);
    assert_eq!(w.kid.seq, 78, "drinking");
    let mut flashes = vec![];
    for _ in 0..30 {
        tick(&mut w, NONE, 1);
        if w.flash.1 > 0 {
            flashes.push(w.flash);
        }
    }
    assert_eq!(w.level.tile_at(1, 10 + col as usize).kind, Tile::Floor);
    (w, flashes)
}

#[test]
fn potions_5_1() {
    let (w, f) = drink(2, 3); // life: one more heart, full health
    assert_eq!((w.kid.hp_max, w.kid.hp), (4, 4));
    assert!(w.sounds.contains(&30) && f.contains(&(4, 4)));
    let (w, f) = drink(1, 2); // heal
    assert_eq!(w.kid.hp, 3);
    assert!(w.sounds.contains(&33) && f == [(4, 2), (4, 1)]);
    let (w, f) = drink(1, 3); // heal at full health: nothing
    assert!(!w.sounds.contains(&33) && f.is_empty());
    let (w, f) = drink(5, 3); // hurt: -1 and the red hurt flash
    assert_eq!(w.kid.hp, 2);
    assert!(w.sounds.contains(&13) && f == [(12, 1)]);
    let (w, f) = drink(3, 3); // feather
    assert!(w.shared.feather > 0 && w.sounds.contains(&39) && f.contains(&(2, 3)));
    let (w, _) = drink(4, 3); // invert: display only
    assert!(w.upside);
    let (w, _) = drink(0, 3); // empty flask
    assert_eq!((w.kid.hp, w.kid.hp_max, w.upside), (3, 3, false));
}

#[test]
fn have_sword_except_on_level_1() {
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    for n in [0usize, 1, 2] {
        let w = World::new(data(), Level::parse(lv.get(2000 + n as u16).unwrap()).unwrap(), n);
        assert_eq!(w.kid.have_sword, n != 1, "level {n}");
    }
}

#[test]
fn all_shipped_levels_run_without_panicking() {
    let d = data();
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    for n in 0..=14usize {
        let mut w = World::new(d.clone(), Level::parse(lv.get(2000 + n as u16).unwrap()).unwrap(), n);
        for t in 0..400 {
            let keys = if t % 40 < 20 { RIGHT } else { NONE };
            w.step(keys);
            w.sounds.clear();
        }
        assert!(w.trobs.len() <= 30 && w.mobs.len() <= 14);
    }
}

// ------------------------------------------------------------ docs/edge_cases.md items 8-13

use crate::sim::{Exit, Progress};

#[test]
fn death_counter_timeline_and_restart_3_2() {
    let mut w = world(3, &[], &[]);
    tick(&mut w, NONE, 3);
    w.sounds.clear();
    w.kid.room = 0; // fell out of the level: fell_out sets frame 185, the counter starts in the same tick
    let mut trace = vec![];
    for _ in 0..9 {
        tick(&mut w, NONE, 1);
        trace.push((w.death_count, w.sounds.contains(&24), w.text_timer));
        w.restart_input(true, false);
        if w.death_count <= 6 {
            assert_eq!(w.exit, None, "Shift/Enter only after the counter passed 6");
        }
        w.exit = None;
    }
    assert!(!w.kid.alive && w.kid.frame == 185);
    let counts: Vec<u8> = trace.iter().map(|t| t.0).collect();
    assert_eq!(counts, [1, 2, 3, 4, 5, 6, 7, 8, 8]);
    assert!(trace[6].1 && !trace[5].1, "death music (24) when the counter goes 6 -> 7");
    assert_eq!(trace[7].2, 288, "Press Button to Continue at 7 -> 8");
    assert!(w.continue_text());
    w.restart_input(true, false);
    assert_eq!(w.exit, Some(Exit::Restart));
    // nobody presses a key: back to the title when the 288-tick timer has run down to 1
    w.exit = None;
    tick(&mut w, NONE, 285);
    assert_eq!(w.exit, None);
    tick(&mut w, NONE, 1);
    assert_eq!((w.exit, w.text_timer), (Some(Exit::Title), 1));
    // Ctrl+A works any time
    let mut w = world(3, &[], &[]);
    w.restart_input(false, true);
    assert_eq!(w.exit, Some(Exit::Restart));
    // time out
    let mut w = world(3, &[], &[]);
    w.progress.rem_min = 0;
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::Title), "rem_min 0 on level < 13 is a time out");
}

#[test]
fn restart_and_next_level_progress_3_4() {
    let mut w = world(3, &[], &[]);
    tick(&mut w, NONE, 719);
    assert_eq!((w.progress.rem_min, w.progress.rem_tick), (59, 719), "720 ticks per minute, starting at 719");
    tick(&mut w, NONE, 10);
    w.kid.hp_max = 5; // a life potion drunk on this level
    w.kid.hp = 4;
    w.upside = true;
    let lvl = w.level.clone();
    // restart: hit points from hitp_beg_lev, time kept
    let p = w.progress_after(Exit::Restart);
    assert_eq!((p.level, p.hitp_beg_lev, p.rem_min, p.rem_tick), (2, 3, 59, 709));
    let r = World::from_progress(w.data.clone(), lvl.clone(), p);
    assert_eq!((r.kid.hp, r.kid.hp_max, r.kid.have_sword, r.upside, r.progress.rem_tick), (3, 3, true, false, 709));
    // next level: hitp_beg_lev = hitp_max, checkpoint cleared; seamless keeps the hit points
    w.progress.checkpoint = true;
    let p = w.progress_after(Exit::NextLevel { seamless: false });
    assert_eq!((p.level, p.hitp_beg_lev, p.checkpoint, p.carry_hp), (3, 5, false, None));
    let n = World::from_progress(w.data.clone(), lvl.clone(), p);
    assert_eq!((n.kid.hp, n.kid.hp_max), (5, 5));
    let p = w.progress_after(Exit::NextLevel { seamless: true });
    let n = World::from_progress(w.data.clone(), lvl.clone(), p);
    assert_eq!((n.kid.hp, n.kid.hp_max), (4, 5));
    assert_eq!(w.progress_after(Exit::Title), Progress::new_game(0));
    assert_eq!(w.progress_after(Exit::NewGame).level, 1);
    // level 3 checkpoint: start room 2, tile 6, start_dir 0xFF (NOT + turn -> facing left); tile (7, 4, 0) cleared
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let l3 = Level::parse(lv.get(2003).unwrap()).unwrap();
    let w = World::from_progress(data(), l3.clone(), Progress { checkpoint: true, ..Progress::new_game(3) });
    assert_eq!((w.kid.room, w.kid.row, w.kid.facing_left, w.level.tile_at(7, 4).kind), (2, 0, true, Tile::Empty));
}

#[test]
fn level_exits_4_1() {
    // the open exit door: up -> seq 70 (go up the stairs), its op F1 ends the level
    let mut w = world(5, &[(5, 1, 16)], &[]);
    w.level.start_room = 2;
    w.leveldoor_open = 1;
    tick(&mut w, NONE, 6);
    tick(&mut w, Keys { up: true, ..NONE }, 1);
    // x = x_bump[col + 5] + 10 = 138, then seq 70's first dx -5 (facing left) in the same tick
    assert_eq!((w.kid.seq, w.kid.x, w.kid.facing_left), (70, 58 + 14 * 5 + 10 + 5, true));
    for _ in 0..40 {
        if w.exit.is_some() {
            break;
        }
        tick(&mut w, NONE, 1);
    }
    assert_eq!(w.exit, Some(Exit::NextLevel { seamless: false }));
    // seq 70's op F2 4: end-level music 41 (level 2); on level 4 the shadow music 32 instead, on 13/15 none
    assert!(w.sounds.contains(&41) && !w.sounds.contains(&32));
    for (lvl, want) in [(4, Some(32)), (13, None), (15, None)] {
        let mut w = World::new(data(), synth(1, 1..3, 5, 1, &[(5, 1, 16)], &[]), lvl);
        w.level.start_room = 2;
        w.leveldoor_open = 1;
        tick(&mut w, NONE, 6);
        tick(&mut w, Keys { up: true, ..NONE }, 1);
        tick(&mut w, NONE, 20);
        assert_eq!(w.kid.seq, 70, "level {lvl}");
        let music: Vec<u8> = w.sounds.iter().copied().filter(|s| [32, 41].contains(s)).collect();
        assert_eq!(music, want.into_iter().collect::<Vec<_>>(), "level {lvl}");
    }
    // closed door: no stairs
    let mut w = world(5, &[(5, 1, 16)], &[]);
    w.level.start_room = 2;
    tick(&mut w, NONE, 6);
    tick(&mut w, Keys { up: true, ..NONE }, 1);
    assert_ne!(w.kid.seq, 70);
    // level 6: falling out of room 1 leads to level 7 (no room change)
    let mut w = World::new(data(), synth(1, 1..3, 3, 1, &[(3, 1, 0), (3, 2, 0)], &[]), 6);
    for _ in 0..30 {
        tick(&mut w, NONE, 1);
        if w.exit.is_some() {
            break;
        }
    }
    assert_eq!((w.exit, w.kid.room), (Some(Exit::NextLevel { seamless: false }), 1));
    // level 12 room 23: seamless; level 0 room 24: demo over
    let mut w = World::new(data(), synth(23, 0..3, 3, 1, &[], &[]), 12);
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::NextLevel { seamless: true }));
    let mut w = World::new(data(), synth(24, 0..3, 3, 1, &[], &[]), 0);
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::Title));
}

#[test]
fn mouse_level_8_4_2() {
    let mut w = World::new(data(), synth(16, 0..3, 2, 1, &[], &[]), 8);
    w.leveldoor_open = 1; // the exit door just finished opening
    tick(&mut w, NONE, 148);
    assert!(w.guard.is_none(), "counter at 149");
    tick(&mut w, NONE, 1);
    assert_eq!(w.leveldoor_open, 150);
    let g = w.guard.as_ref().expect("mouse at 150");
    assert_eq!((g.charid, g.room, g.row, g.y, g.facing_left, g.hp, g.seq, g.frame, g.x), (24, 16, 0, 55, true, 1, 105, 186, 195));
    let mut xs = vec![195];
    let mut seqs = vec![];
    while let Some(g) = &w.guard {
        assert!((186..=188).contains(&g.frame));
        seqs.push(g.seq);
        tick(&mut w, NONE, 1);
        if let Some(g) = &w.guard {
            xs.push(g.x);
        }
        assert!(xs.len() < 80, "mouse never left: {xs:?}");
    }
    assert_eq!(&xs[..9], &[195, 192, 188, 183, 180, 176, 171, 168, 164], "runs left 12 units per 3 ticks");
    assert_eq!(xs.iter().filter(|&&x| x == 164).count(), 11, "stops below 166 for 11 ticks: {xs:?}");
    let back = xs.iter().position(|&x| x == 172).unwrap();
    assert!(xs[back..].windows(2).all(|p| p[1] > p[0]) && *xs.last().unwrap() >= 200, "runs right and vanishes at 200: {xs:?}");
    assert!(seqs.contains(&107));
    assert!(w.leveldoor_open > 150, "the counter runs on and the door stays open");
}

#[test]
fn level4_mirror_shadow_4_3() {
    // room 4, row 0: a mirror at column 4; the kid runs left from column 9 and jumps (try every take-off tick)
    let mut found = false;
    'outer: for jump in 2..24 {
        let mut w = World::new(data(), synth(4, 0..3, 9, 0, &[(4, 0, 13)], &[]), 4);
        w.kid.facing_left = true;
        let left = Keys { left: true, ..NONE };
        for t in 0..40 {
            let k = if t == jump { Keys { up: true, ..left } } else { left };
            tick(&mut w, k, 1);
            if let Some(g) = w.guard.as_ref().filter(|g| g.charid == 1) {
                assert_eq!(g.x as i32, (2 * (58 + 14 * w.kid.curr_col() + 10) - w.kid.x as i32) & 0xFF, "mirror image about the kid's curr_col");
                assert_eq!((g.facing_left, g.hp, g.hp_max, w.kid.hp), (false, 3, 3, 1));
                assert!(w.sounds.contains(&45));
                assert_eq!(w.level.tile_at(4, 4).modifier, 0x56);
                tick(&mut w, left, 4); // the rest of the running jump probes the mirror again
                assert_eq!(w.sounds.iter().filter(|&&s| s == 45).count(), 1, "one shadow per jump");
                // autocontrol level 4: x < 80 in room 4 -> gone
                if let Some(g) = w.guard.as_mut() {
                    g.x = 70;
                }
                tick(&mut w, NONE, 1);
                assert!(w.guard.is_none());
                found = true;
                break 'outer;
            }
        }
    }
    assert!(found, "no take-off tick gets the kid through the mirror");
}

#[test]
fn demo_level_0_4_4() {
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let mut w = World::new(data(), Level::parse(lv.get(2000).unwrap()).unwrap(), 0);
    assert!(w.demo && w.kid.hp == 4 && w.kid.have_sword);
    let mut rooms = vec![w.kid.room];
    for _ in 0..1500 {
        tick(&mut w, NONE, 1);
        if rooms.last() != Some(&w.kid.room) {
            rooms.push(w.kid.room);
        }
        if w.exit.is_some() {
            break;
        }
    }
    // the move list walks the kid to the guard, the guard AI fights for him, then he runs right into room 24
    assert_eq!((w.exit, w.kid.room, w.progress.checkpoint, w.kid.alive), (Some(Exit::Title), 24, true, true), "rooms {rooms:?}");
    assert!(rooms.starts_with(&[1, 2]), "rooms {rooms:?}");
    // any key ends the demo
    let mut w = World::new(data(), Level::parse(lv.get(2000).unwrap()).unwrap(), 0);
    tick(&mut w, RIGHT, 1);
    assert_eq!(w.exit, Some(Exit::NewGame));
}

#[test]
fn level_chain_end_and_audit_fixes() {
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let lvl = |n: u16| Level::parse(lv.get(2000 + n).unwrap()).unwrap();
    let d = data();
    // every level hands over to the next one (12 -> 13 seamless); no shipped level ends on its first ticks
    let mut p = Progress::new_game(1);
    for n in 1..=14usize {
        assert_eq!(p.level, n);
        let mut w = World::from_progress(d.clone(), lvl(n as u16), p);
        tick(&mut w, NONE, 3);
        assert_eq!(w.exit, None, "level {n} start");
        p = w.progress_after(Exit::NextLevel { seamless: n == 12 });
    }
    // level 14: entering room 5 is the end (check_the_end)
    let mut w = World::from_progress(d.clone(), lvl(14), Progress::new_game(14));
    w.kid.room = 5;
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::End));
    assert_eq!(w.progress_after(Exit::End), Progress::new_game(0));
    // restart on level 0 clears the demo checkpoint (the guard-killed flag), elsewhere it is kept
    let w = World::from_progress(d.clone(), lvl(0), Progress { checkpoint: true, ..Progress::new_game(0) });
    assert!(!w.progress_after(Exit::Restart).checkpoint);
    let w = World::from_progress(d.clone(), lvl(3), Progress { checkpoint: true, ..Progress::new_game(3) });
    assert!(w.progress_after(Exit::Restart).checkpoint);
    // do_mouse leaves a live guard alone
    let mut w = World::new(d, synth(16, 0..3, 2, 1, &[], &[]), 8);
    w.leveldoor_open = 1;
    let mut g = w.kid.clone();
    g.charid = 2;
    w.guard = Some(g);
    tick(&mut w, NONE, 149);
    assert!(w.leveldoor_open >= 150 && w.guard.as_ref().is_some_and(|g| g.charid != 24));
}

#[test]
fn save_file_round_trip() {
    let p = Progress { level: 7, rem_min: 41, rem_tick: 123, hitp_beg_lev: 5, ..Progress::new_game(7) };
    let t = p.save_text(5);
    assert_eq!(t, "{\"level\": 7, \"hitp_max\": 5, \"rem_min\": 41, \"rem_tick\": 123, \"hitp_beg_lev\": 5}
");
    assert_eq!(Progress::from_save(&t), Some(p));
    assert_eq!(Progress::from_save(&t.replace("41", "0")), None, "no time left");
    assert_eq!(Progress::from_save("{\"level\": 7}"), None);
}

#[test]
fn save_file_parsing_is_strict() {
    let ok = "{\"level\": 7, \"hitp_max\": 5, \"rem_min\": 41, \"rem_tick\": 123, \"hitp_beg_lev\": 5}";
    assert!(Progress::from_save(ok).is_some());
    // the key is matched exactly, not as a substring of another key or value
    assert!(Progress::from_save("{\"xlevel\": 3, \"level\": 7, \"rem_min\": 41, \"rem_tick\": 123, \"hitp_beg_lev\": 5}").is_none(), "unknown key");
    assert_eq!(Progress::from_save("{\"hitp_beg_lev\": 5, \"rem_tick\": 123, \"rem_min\": 41, \"level\": 7}").map(|p| p.level), Some(7), "any order");
    assert!(Progress::from_save(&ok.replace("\"rem_min\": 41", "\"rem_min\": 41, \"rem_min\": 42")).is_none(), "duplicate key");
    assert!(Progress::from_save(&ok.replace("41", "41x")).is_none(), "trailing junk in a value");
    assert!(Progress::from_save(&ok.replace("41", "-41")).is_none());
    assert!(Progress::from_save(&ok.replace("\"rem_tick\": 123", "\"rem_tick\": 720")).is_none());
    assert!(Progress::from_save(&ok.replace("\"hitp_beg_lev\": 5", "\"hitp_beg_lev\": 11")).is_none());
    assert!(Progress::from_save(&ok[..ok.len() - 1]).is_none(), "no closing brace");
    // writer and reader agree on the levels: 3..13 (0000:05EA); level 14 cannot be saved, so it is not loaded either
    for level in 0..=15 {
        let t = Progress { level, rem_min: 30, rem_tick: 5, hitp_beg_lev: 4, ..Progress::new_game(level) }.save_text(4);
        assert_eq!(Progress::from_save(&t).is_some(), (3..=13).contains(&level), "level {level}");
    }
}

#[test]
fn ending_wins_over_a_title_exit_in_the_same_tick() {
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let mut w = World::from_progress(data(), Level::parse(lv.get(2014).unwrap()).unwrap(), Progress::new_game(14));
    w.text_timer = 2; // reaches 1 at the start of the next tick: Exit::Title
    w.kid.room = 5; // and check_the_end fires later in the same tick
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::End));
    // otherwise the first exit stays
    let mut w = world(3, &[], &[]);
    w.text_timer = 2;
    w.progress.rem_min = 0;
    tick(&mut w, NONE, 1);
    assert_eq!(w.exit, Some(Exit::Title));
}

#[test]
fn clock_runs_in_the_demo_and_stops_on_level_13_after_jaffar() {
    // show_time (0000:C170) has no level-0 test: the demo's clock runs
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let mut w = World::new(data(), Level::parse(lv.get(2000).unwrap()).unwrap(), 0);
    tick(&mut w, NONE, 10);
    assert_eq!((w.progress.rem_min, w.progress.rem_tick), (60, 709));
    // level 13: only while leveldoor_open == 0 (Jaffar's death sets 2, the opened door 1)
    for (door, runs) in [(0u8, true), (2, false), (1, false)] {
        let mut w = World::new(data(), synth(1, 1..3, 3, 1, &[], &[]), 13);
        w.leveldoor_open = door;
        tick(&mut w, NONE, 5);
        assert_eq!(w.progress.rem_tick == 714, runs, "door {door}");
    }
}

#[test]
fn level8_counter_capped_at_255_behaves_like_the_word() {
    // the original counter is a word (0..65535); within the 60 minutes (< 43200 frames) it never wraps, and the only value
    // anything tests on level 8 is 150 (do_mouse) and != 0 (door open), so the port's saturating u8 gives the same result
    let mut w = World::new(data(), synth(16, 0..3, 2, 1, &[], &[]), 8);
    w.leveldoor_open = 1;
    let mut mice = 0;
    for _ in 0..600 {
        let had = w.guard.is_some();
        tick(&mut w, NONE, 1);
        mice += (!had && w.guard.as_ref().is_some_and(|g| g.charid == 24)) as u32;
    }
    assert_eq!((mice, w.leveldoor_open), (1, 255), "one mouse, the door stays open");
    assert!(w.shared.leveldoor_open);
}

/// Play-through harness (edge_cases.md 4.1): on every level the kid is teleported next to the way out and the level's exit
/// hook must fire. Door levels: start on the exit door (a level-door tile outside the start room), door open, Up -> seq 70
/// -> op F1. Level 6: falling out of room 1; level 12: room 23 (seamless); level 14: room 5 (the end).
fn reach_exit(n: usize) -> Result<Exit, String> {
    let lv = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let mut level = Level::parse(lv.get(2000 + n as u16).unwrap()).unwrap();
    let real_start = level.start_room;
    let (room, tile) = match n {
        6 => {
            // room 1, bottom row: a column with nothing to stand on (the kid drops out of the room)
            let c = (0..10).find(|&c| level.tile(1, c, 2).kind == Tile::Empty).ok_or("no gap in room 1")?;
            (1, 20 + c as u8)
        }
        12 => (23, 0),
        14 => (5, 0),
        _ => {
            let doors: Vec<(u8, u8)> = (1..=24u8).filter(|&r| r != real_start).flat_map(|r| (0..30u8).map(move |t| (r, t))).filter(|&(r, t)| level.tile_at(r as usize, t as usize).kind == Tile::ExitLeft).collect();
            *doors.first().ok_or(format!("no exit door outside room {real_start}"))?
        }
    };
    (level.start_room, level.start_tile, level.start_dir) = (room, tile, 0);
    let mut w = World::from_progress(data(), level, Progress::new_game(n));
    w.level.start_room = real_start; // up_pressed refuses the door of the start room
    if n != 6 && n != 12 && n != 14 {
        // the guard of the door room (levels 4, 10) would fight: this harness tests the exit, not the fight
        w.guard = None;
        w.slots[room as usize - 1].tile = 0xFF;
        w.leveldoor_open = 1;
        // tap Up until the kid is on the stairs (level 1 starts with a fall and a crouching landing)
        for _ in 0..20 {
            tick(&mut w, Keys { up: true, ..NONE }, 1);
            if w.kid.seq == 70 {
                break;
            }
            tick(&mut w, NONE, 1);
        }
    }
    for _ in 0..60 {
        if let Some(e) = w.exit {
            return Ok(e);
        }
        tick(&mut w, NONE, 1);
    }
    Err(format!("no exit (room {} seq {} frame {} alive {})", w.kid.room, w.kid.seq, w.kid.frame, w.kid.alive))
}

#[test]
fn play_through_every_level_reaches_its_exit() {
    let mut bad = vec![];
    for n in 1..=14 {
        let want = match n {
            12 => Exit::NextLevel { seamless: true },
            14 => Exit::End,
            _ => Exit::NextLevel { seamless: false },
        };
        match reach_exit(n) {
            Ok(e) if e == want => {}
            r => bad.push(format!("level {n}: {r:?}")),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
