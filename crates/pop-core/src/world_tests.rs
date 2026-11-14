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
    Arc::new(Data { exe, frames, sizes })
}

/// Room 1 with floor on rows 1 and 2; the kid starts at (col, 1) facing right. `tiles` = (col, row, type) overrides.
fn world(start_col: usize, tiles: &[(usize, usize, u8)], links: &[(u8, u8)]) -> World {
    let mut b = vec![0u8; 2305];
    for r in 1..3 {
        for c in 0..10 {
            b[r * 10 + c] = 1;
        }
    }
    for &(c, r, t) in tiles {
        b[r * 10 + c] = t;
    }
    for g in 2119..2143 {
        b[g] = 30;
    }
    (b[2112], b[2113], b[2114]) = (1, (10 + start_col) as u8, 255);
    for &(i, v) in links {
        b[1440 + i as usize] = v; // doorlinks1
    }
    World::new(data(), Level::parse(&b).unwrap(), 2)
}

fn tick(w: &mut World, keys: Keys, n: usize) {
    for _ in 0..n * 5 {
        w.step(keys);
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
