//! Engine-independent core: original data formats and (later) the game simulation.
//! No graphics/audio deps, so mechanics can be tested headless against the original.
pub mod dat;
pub mod exe;
pub mod image;
pub mod kid;
pub mod level;
pub mod sim;

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
        let (_, l) = levels().remove(1);
        assert_eq!(l.links[0], [5, 0, 0, 2]);
        assert_eq!((l.start_room, l.start_tile, l.start_dir), (1, 0, 255));
        assert_eq!(l.guards.iter().map(|g| (g.room, g.tile)).collect::<Vec<_>>(), [(3, 17), (21, 6)]);
    }

    #[test]
    fn every_button_targets_a_gate_or_exit() {
        let mut n = 0;
        for (id, l) in levels() {
            for room in 1..=ROOMS {
                for t in 0..TILES_PER_ROOM {
                    let tr = l.tile_at(room, t);
                    if matches!(tr.kind, Tile::DropButton | Tile::RaiseButton) {
                        for k in l.door_links(tr.modifier) {
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
    fn room_links_valid() {
        // resource 2015 deviates (links padded with 255); it is not a regular level.
        for (_, l) in levels().into_iter().take(15) {
            assert!(l.links.iter().flatten().all(|&r| r as usize <= ROOMS));
        }
    }

    #[test]
    fn all_images_decode() {
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
    fn test_level(floor: [&str; 3], start: (usize, usize)) -> Level {
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
        b[2114] = 0;
        Level::parse(&b).unwrap()
    }

    struct Rig {
        exe: super::exe::Exe,
        frames: Vec<super::exe::Frame>,
        level: Level,
    }
    impl Rig {
        fn new(floor: [&str; 3], start: (usize, usize)) -> Rig {
            let exe = super::exe::Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
            let frames = exe.kid_frames();
            Rig { exe, frames, level: test_level(floor, start) }
        }
        fn env(&self) -> super::kid::Env<'_> {
            super::kid::Env { exe: &self.exe, frames: &self.frames, level: &self.level }
        }
    }
    use super::kid::{Keys, Kid};
    const RIGHT: Keys = Keys { left: false, right: true, up: false, down: false, shift: false };
    const NONE: Keys = Keys { left: false, right: false, up: false, down: false, shift: false };

    #[test]
    fn kid_stands_idle() {
        let r = Rig::new(["", "##########", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn(&env);
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
        let r = Rig::new(["", "##########", "##########"], (1, 1));
        let env = r.env();
        let mut k = Kid::spawn(&env);
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
        let r = Rig::new(["", "##########", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn(&env);
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
        let r = Rig::new(["", "##########", "##########"], (2, 1));
        let env = r.env();
        let mut k = Kid::spawn(&env);
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
        // row 1 floor only on columns 0..=4, row 2 floor everywhere below
        let r = Rig::new(["", "#####", "##########"], (3, 1));
        let env = r.env();
        let mut k = Kid::spawn(&env);
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
}
