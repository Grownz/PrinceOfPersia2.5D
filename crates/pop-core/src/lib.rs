//! Engine-independent core: original data formats and (later) the game simulation.
//! No graphics/audio deps, so mechanics can be tested headless against the original.
pub mod cutscene;
pub mod dat;
pub mod exe;
pub mod image;
pub mod kid;
pub mod level;
pub mod sim;
#[cfg(test)]
mod world_tests;
#[cfg(test)]
mod coll_tests;

/// Tests that need the user's own game files (never in the repo, e.g. on CI) return early when they are absent.
#[cfg(test)]
pub(crate) fn no_game_data() -> bool {
    let missing = !std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PRINCE.EXE")).is_file();
    if missing {
        eprintln!("skipping: Original_data not found (game data tests need your own copy)");
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::{dat::Dat, level::*};
    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");

    fn levels() -> Vec<(u16, Level)> {
        let d = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
        (2000..=2015).map(|id| (id, Level::parse(d.get(id).unwrap()).unwrap())).collect()
    }

    #[test]
    fn level1_basics() {
        if crate::no_game_data() { return; }
        let (_, l) = levels().remove(1);
        assert_eq!(l.links[0], [5, 0, 0, 2]);
        assert_eq!((l.start_room, l.start_tile, l.start_dir), (1, 0, 255));
        assert_eq!(l.guards.iter().map(|g| (g.room, g.tile)).collect::<Vec<_>>(), [(3, 17), (21, 6)]);
    }

    #[test]
    fn every_button_targets_a_gate_or_exit() {
        if crate::no_game_data() { return; }
        let mut n = 0;
        for (id, l) in levels() {
            for room in 1..=ROOMS {
                for t in 0..TILES_PER_ROOM {
                    let tr = l.tile_at(room, t);
                    if matches!(tr.kind, Tile::DropButton | Tile::RaiseButton) {
                        for k in l.door_links(tr.modifier) {
                            if k.room == 0 {
                                // the pseudo room-25 alter zeroes doorlinks1[11] of level 6 (see below): a room-0 entry, inert
                                assert_eq!((id, tr.modifier), (2006, 11));
                                continue;
                            }
                            let kind = l.tile_at(k.room as usize, k.tile as usize).kind;
                            assert!(matches!(kind, Tile::Gate | Tile::ExitLeft), "level {id} room {room} tile {t} -> {k:?} is {kind:?}");
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 100);
    }

    #[test]
    fn level_tail_and_used_rooms() {
        if crate::no_game_data() { return; }
        let d = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
        // 2015 is one byte short (the unread last fill byte); every other level is 2305 bytes
        for id in 2000..=2015u16 {
            assert_eq!(d.get(id).unwrap().len(), if id == 2015 { 2304 } else { 2305 }, "{id}");
        }
        let ls = levels();
        // used_rooms 25 (level 6): load_alter_mod runs on a pseudo room 25 = room 1's modifiers over doorlinks1. Room 1
        // tile 11 has modifier 11 ("loose floor") -> doorlinks1[11] = 0: the drop button there gets a room-0 entry before
        // its real target (room 1, tile 12), which entry 12 repeats; trigger_1 on room 0 finds no gate (inert)
        let l6 = &ls[6].1;
        let ks: Vec<(u8, u8)> = l6.door_links(11).iter().map(|k| (k.room, k.tile)).collect();
        assert_eq!(ks, [(0, 0), (1, 12)]);
        // rooms above used_rooms keep their raw modifiers: level 10 (used 21) room 22's potion stays 1, not 1 << 5
        assert_eq!(ls[10].1.tile_at(22, 28).kind, Tile::Potion);
        assert_eq!(ls[10].1.tile_at(22, 28).modifier, 1);
    }

    #[test]
    fn room_links_valid() {
        if crate::no_game_data() { return; }
        // resource 2015 deviates (links padded with 255); it is not a regular level.
        for (_, l) in levels().into_iter().take(15) {
            assert!(l.links.iter().flatten().all(|&r| r as usize <= ROOMS));
        }
    }

    #[test]
    fn all_images_decode() {
        if crate::no_game_data() { return; }
        let mut n = 0;
        for e in std::fs::read_dir(DIR).unwrap().map(|e| e.unwrap().path()) {
            let name = e.file_name().unwrap().to_string_lossy().to_uppercase();
            if !name.ends_with(".DAT") || name.contains("SND") || name.starts_with("LEVELS") {
                continue;
            }
            let d = Dat::open(&e).unwrap();
            for id in d.ids() {
                let r = d.get(id).unwrap();
                let w = |o: usize| r.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
                let (h, wd, fl) = (w(0), w(2), w(4));
                // header filter: plausible size, 1 or 4 bpp, compression 0..=4
                if (1..=400).contains(&h) && (1..=400).contains(&wd) && matches!(fl >> 12, 0 | 0xb) && (fl >> 8) & 15 <= 4 {
                    let im = super::image::decode(r).unwrap_or_else(|| panic!("{name} resource {id} failed"));
                    assert_eq!(im.pixels.len(), wd * h);
                    n += 1;
                }
            }
        }
        assert!(n >= 1300, "only {n} images");
    }

    #[test]
    fn exe_tables() {
        if crate::no_game_data() { return; }
        use super::exe::*;
        let e = Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
        let f = e.kid_frames();
        assert_eq!(f.len(), KID_FRAMES);
        assert_eq!(f[0].image, 255);
        assert_eq!(f[1], Frame { image: 0, sword: 0, dx: 1, dy: 0, flags: 0xc4 }); // first start-run frame
        let ids = |file: &str| -> Vec<u16> {
            let d = Dat::open(format!("{DIR}{file}")).unwrap();
            d.ids().collect()
        };
        let (kid, guard) = (ids("KID.DAT"), ids("GUARD.DAT"));
        let has = |t: &[Frame], base: u16, ids: &[u16]| t.iter().filter(|f| f.image != 255).all(|f| ids.contains(&(base + f.image as u16)));
        assert!(has(&f, 401, &kid));
        assert_eq!(e.guard_frames().len(), GUARD_FRAMES);
        assert!(has(&e.guard_frames(), 751, &guard));
        assert_eq!(e.cutscene_frames().len(), CUT_FRAMES);
        // sequence 1 = start run: action 1 (run), frames 1..=4
        let s1 = e.sequence(1).unwrap();
        assert_eq!(&s1[..6], &[Op::Action(1), Op::Frame(1), Op::Frame(2), Op::Frame(3), Op::Frame(4), Op::Dx(8)]);
        for n in 1..=SEQUENCES {
            for op in e.sequence(n).unwrap() {
                if let Op::Frame(x) = op {
                    assert!((x as usize) < KID_FRAMES, "sequence {n}: frame {x}");
                }
            }
        }
    }

    /// Synthetic level: room 1 only. `floor[row]` = columns that have a floor tile; the kid starts at (col, row) facing right.
    fn test_bytes(floor: [&str; 3], start: (usize, usize)) -> Vec<u8> {
        let mut b = vec![0u8; 2305];
        for (r, row) in floor.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                b[r * 10 + c] = if ch == '#' { 1 } else { 0 };
            }
        }
        for g in 2119..2143 {
            b[g] = 30; // no guards
        }
        b[2112] = 1;
        b[2113] = (start.1 * 10 + start.0) as u8;
        b[2114] = 255; // start_dir 255 => direction right
        b
    }
    fn test_level(floor: [&str; 3], start: (usize, usize)) -> Level {
        Level::parse(&test_bytes(floor, start)).unwrap()
    }

    struct Rig {
        exe: super::exe::Exe,
        frames: Vec<super::exe::Frame>,
        level: Level,
        sizes: Vec<(u16, u16)>,
    }
    impl Rig {
        fn new(floor: [&str; 3], start: (usize, usize)) -> Rig {
            Rig::with_level(test_level(floor, start))
        }
        fn with_level(level: Level) -> Rig {
            let exe = super::exe::Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
            let frames = exe.kid_frames();
            let sizes = super::image::image_sizes(&Dat::open(format!("{DIR}KID.DAT")).unwrap(), 401, 220);
            Rig { exe, frames, level, sizes }
        }
        fn env(&self) -> super::kid::Env<'_> {
            super::kid::Env::new(&self.exe, &self.frames, &self.level, &self.sizes)
        }
    }
    use super::kid::{Keys, Kid};
    const RIGHT: Keys = Keys { left: false, right: true, up: false, down: false, shift: false };
    const NONE: Keys = Keys { left: false, right: false, up: false, down: false, shift: false };

    #[test]
    fn kid_stands_idle() {
        if crate::no_game_data() { return; }
        let r = Rig::new(["", "##########", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let x0 = k.x;
        for _ in 0..10 {
            k.tick(&env, NONE);
        }
        assert_eq!((k.frame, k.action, k.x, k.y), (15, 0, x0, 118));
        println!("spawn col {} foot {} x {}", k.col(&env), k.foot_x(&env), k.x);
        assert_eq!(k.col(&env), 3);
    }

    #[test]
    fn kid_runs_and_stops() {
        if crate::no_game_data() { return; }
        let r = Rig::new(["", "##########", "##########"], (1, 1));
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        k.tick(&env, RIGHT);
        assert_eq!((k.seq, k.frame, k.action), (1, 1, 1)); // control runs before play_seq: first frame in the same tick
        let x0 = k.x;
        for _ in 0..10 {
            k.tick(&env, RIGHT);
        }
        assert!(k.x > x0 && k.action == 1);
        let mut seqs = vec![];
        for _ in 0..40 {
            k.tick(&env, NONE);
            seqs.push(k.seq);
        }
        assert!(seqs.contains(&13), "stop sequence 13 not played: {seqs:?}");
        assert_eq!((k.frame, k.action), (15, 0));
    }

    #[test]
    fn kid_turns() {
        if crate::no_game_data() { return; }
        let r = Rig::new(["", "##########", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let left = Keys { left: true, ..NONE };
        k.tick(&env, left);
        assert_eq!(k.seq, 5);
        for _ in 0..12 {
            k.tick(&env, NONE);
        }
        assert!(k.facing_left);
        assert_eq!(k.frame, 15);
    }

    #[test]
    fn standing_jump_travels_39_units() {
        if crate::no_game_data() { return; }
        let r = Rig::new(["", "##########", "##########"], (2, 1));
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let x0 = k.x as i32;
        k.tick(&env, Keys { right: true, up: true, ..NONE });
        assert_eq!(k.seq, 3);
        for _ in 0..40 {
            k.tick(&env, NONE);
        }
        assert_eq!(k.frame, 15);
        assert_eq!(k.x as i32 - x0, 39); // sum of the dx ops of sequence 3 (docs/mechanics.md)
    }

    #[test]
    fn kid_falls_off_ledge_and_lands() {
        if crate::no_game_data() { return; }
        // row 1 floor only on columns 0..=4, row 2 floor everywhere below
        let r = Rig::new(["", "#####", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let (mut fell, mut ticks) = (false, 0);
        while ticks < 300 {
            k.tick(&env, if fell { NONE } else { RIGHT });
            fell |= k.action == 4;
            ticks += 1;
            if fell && k.action != 4 && k.row == 2 {
                break;
            }
        }
        assert!(fell, "kid never fell");
        assert_eq!(k.row, 2);
        assert_eq!(k.y, 181);
        assert!(k.alive);
    }

    #[test]
    fn kid_bumps_into_wall_and_does_not_pass() {
        if crate::no_game_data() { return; }
        let mut b = test_bytes(["", "##########", "##########"], (2, 1));
        b[16] = 20; // wall at col 6, row 1
        let r = Rig::with_level(Level::parse(&b).unwrap());
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let mut seqs = std::collections::HashSet::new();
        for _ in 0..100 {
            k.tick(&env, RIGHT);
            seqs.insert(k.seq);
            assert!(k.col(&env) <= 5, "kid got into the wall column: x {}", k.x);
        }
        assert!(seqs.contains(&47) || seqs.contains(&46) || seqs.contains(&45), "no bump sequence: {seqs:?}");
        assert!(k.alive);
    }

    #[test]
    fn kid_runs_into_next_room() {
        if crate::no_game_data() { return; }
        let mut b = test_bytes(["", "##########", "##########"], (6, 1));
        b[1952 + 1] = 2; // room 1: right neighbour = room 2
        for c in 0..10 {
            b[30 + 10 + c] = 1; // room 2, row 1 floor
        }
        let r = Rig::with_level(Level::parse(&b).unwrap());
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        for _ in 0..35 {
            k.tick(&env, RIGHT);
        }
        assert_eq!(k.room, 2);
        assert!(k.alive && k.row == 1);
        assert!((58..198).contains(&(k.x as i32)), "x {}", k.x);
    }

    #[test]
    fn kid_jumps_up_grabs_ledge_and_climbs() {
        if crate::no_game_data() { return; }
        let mut b = test_bytes(["", "##########", "##########"], (4, 1));
        for c in 5..10 {
            b[c] = 1; // ledge: row 0, columns 5..9
        }
        let r = Rig::with_level(Level::parse(&b).unwrap());
        let env = r.env();
        let mut k = Kid::spawn_standing(&env);
        let up = Keys { up: true, ..NONE };
        k.tick(&env, up);
        assert!(matches!(k.seq, 8 | 24), "grab sequence expected, got {}", k.seq);
        let hold = Keys { up: true, shift: true, ..NONE };
        let mut hung = false;
        let mut trace = vec![];
        for _ in 0..80 {
            k.tick(&env, if k.row == 0 { NONE } else { hold }); // (holding up after the climb would jump up again, as in the original)
            hung |= k.action == 2;
            if trace.last() != Some(&(k.seq, k.row)) {
                trace.push((k.seq, k.row));
            }
        }
        assert!(trace.iter().any(|t| t.0 == 10), "climb sequence 10 not played: {trace:?}");
        assert!(hung, "never hung");
        assert_eq!((k.row, k.y, k.frame), (0, 55, 15));
        assert!(k.alive);
    }
}

#[cfg(test)]
mod start_tests {
    use super::{dat::Dat, kid::*, level::*};

    #[test]
    fn level1_starts_with_a_fall() {
        if crate::no_game_data() { return; }
        let d = "../../Original_data/";
        let exe = super::exe::Exe::open(format!("{d}PRINCE.EXE")).unwrap();
        let frames = exe.kid_frames();
        let sizes = super::image::image_sizes(&Dat::open(format!("{d}KID.DAT")).unwrap(), 401, 220);
        let lv = Dat::open(format!("{d}LEVELS.DAT")).unwrap();
        let level = Level::parse(lv.get(2001).unwrap()).unwrap();
        let env = Env::new(&exe, &frames, &level, &sizes);
        let mut k = Kid::spawn(&env, 1);
        assert_eq!((k.seq, k.room, k.facing_left), (7, 1, false)); // level 1: seq 7, facing right
        let none = Keys::default();
        let mut soft = false;
        for _ in 0..40 {
            k.tick(&env, none);
            soft |= k.seq == 17; // soft landing, as worked out in docs/mechanics.md section 8
        }
        assert!(soft, "level 1 start should end in a soft landing (seq 17)");
        assert_eq!((k.row, k.y), (1, 118));
        assert_eq!(k.hp, 3);
    }
}
