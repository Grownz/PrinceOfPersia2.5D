//! Structural audit and golden hashes of the levels in LEVELS.DAT (needs the user's own game files, skipped otherwise).
//!
//! * `hard` = invariants every regular level (1..=14) keeps; a failure means a parser or interpretation bug.
//! * `soft` = oddities the shipped data really has (one-way room links, rooms the editor left behind, a start tile in the
//!   air). Only printed (`cargo test -p pop-core level_audit_report -- --nocapture`), never asserted.
//! * golden hashes: one FNV-1a hash per level room over the parsed tiles, modifiers, links and guards, in
//!   `level_golden.txt` (hashes only, no game data). They catch any change of the parser, not a mistake that was
//!   already there when the file was written. Rewrite with `POP_BLESS=1 cargo test -p pop-core level_golden`.
//!   Level 0 (demo) and resource 2015 (not a regular level) are only hashed, not held to `hard`.
use crate::level::*;
use std::collections::VecDeque;

fn walkable(t: Tile) -> bool {
    !matches!(t, Tile::Wall | Tile::Empty)
}

fn rooms(l: &Level) -> Vec<usize> {
    (1..=ROOMS).filter(|&r| (0..TILES_PER_ROOM).any(|i| l.tile_at(r, i).kind != Tile::Empty)).collect()
}

/// Invariants of a regular level.
pub fn hard(l: &Level) -> Vec<String> {
    let mut f = Vec::new();
    for r in 1..=ROOMS {
        if l.links[r - 1].iter().any(|&n| n as usize > ROOMS) {
            f.push(format!("room {r}: link out of range {:?}", l.links[r - 1]));
        }
    }
    let (sr, st) = (l.start_room as usize, l.start_tile as usize);
    if !(1..=ROOMS).contains(&sr) || st >= TILES_PER_ROOM {
        f.push(format!("start {sr}/{st} out of range"));
    }
    for g in &l.guards {
        let (r, t) = (g.room as usize, g.tile as usize);
        if !(1..=ROOMS).contains(&r) || t >= TILES_PER_ROOM {
            f.push(format!("guard {g:?} out of range"));
        } else if !walkable(l.tile_at(r, t).kind) {
            f.push(format!("guard in room {r} tile {t} stands on {:?}", l.tile_at(r, t).kind));
        }
        if !matches!(g.dir, 0 | 255) || g.skill > 11 {
            f.push(format!("guard {g:?}: direction or skill out of range"));
        }
    }
    let mut exits = 0;
    for r in rooms(l) {
        for i in 0..TILES_PER_ROOM {
            let t = l.tile_at(r, i);
            match t.kind {
                Tile::ExitLeft => exits += 1,
                Tile::Unknown31 => f.push(format!("room {r} tile {i}: unidentified tile type 31")),
                Tile::DropButton | Tile::RaiseButton => {
                    let ks = l.door_links(t.modifier);
                    if ks.len() > 16 {
                        f.push(format!("room {r} tile {i}: door link chain of {} entries", ks.len()));
                    }
                    for k in ks.iter().filter(|k| k.room != 0) {
                        // room 0: the level 6 quirk, see lib.rs level_tail_and_used_rooms
                        let target = l.tile_at(k.room as usize, k.tile as usize).kind;
                        if !matches!(target, Tile::Gate | Tile::ExitLeft) {
                            f.push(format!("room {r} tile {i}: button targets {}/{} which is {target:?}", k.room, k.tile));
                        }
                    }
                }
                Tile::ExitRight if i % COLS == 0 || l.tile_at(r, i - 1).kind != Tile::ExitLeft => {
                    f.push(format!("room {r} tile {i}: right exit half without its left half"));
                }
                _ => {}
            }
        }
    }
    if exits == 0 {
        f.push("no level exit".into());
    }
    f
}

/// Oddities that may be the data's own.
pub fn soft(l: &Level) -> Vec<String> {
    let mut f = Vec::new();
    let used = rooms(l);
    for r in 1..=ROOMS {
        for (d, &n) in l.links[r - 1].iter().enumerate().filter(|(_, &n)| (1..=ROOMS as u8).contains(&n)) {
            let back = l.links[n as usize - 1][d ^ 1];
            if back as usize != r {
                f.push(format!("room {r}: link {d} -> {n}, but room {n} links back to {back}"));
            }
            if !used.contains(&(n as usize)) {
                f.push(format!("room {r}: link {d} -> {n} which is empty"));
            }
        }
    }
    let (sr, st) = (l.start_room as usize, l.start_tile as usize);
    if (1..=ROOMS).contains(&sr) && st < TILES_PER_ROOM {
        if !walkable(l.tile_at(sr, st).kind) {
            f.push(format!("start tile {sr}/{st} is {:?}", l.tile_at(sr, st).kind));
        }
        let mut seen = [false; ROOMS + 1];
        let mut q = VecDeque::from([sr]);
        seen[sr] = true;
        while let Some(r) = q.pop_front() {
            for &n in l.links[r - 1].iter().filter(|&&n| (1..=ROOMS as u8).contains(&n)) {
                if !std::mem::replace(&mut seen[n as usize], true) {
                    q.push_back(n as usize);
                }
            }
        }
        f.extend(used.iter().filter(|&&r| !seen[r]).map(|r| format!("room {r} holds tiles but no room link leads there from the start room")));
    }
    f
}

/// FNV-1a over everything the game reads of one room.
fn room_hash(l: &Level, room: usize) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut eat = |b: u8| h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    for i in 0..TILES_PER_ROOM {
        eat(l.fg_byte(room, i));
        eat(l.tile_at(room, i).modifier);
    }
    l.links[room - 1].iter().for_each(|&b| eat(b));
    for g in l.guards.iter().filter(|g| g.room as usize == room) {
        [g.tile, g.dir, g.skill, g.colour].into_iter().for_each(&mut eat);
    }
    if l.start_room as usize == room {
        [l.start_tile, l.start_dir].into_iter().for_each(&mut eat);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::Dat;

    const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/level_golden.txt");

    fn levels() -> Vec<(u16, Level)> {
        let d = Dat::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/LEVELS.DAT")).unwrap();
        (2000..=2015).map(|id| (id, Level::parse(d.get(id).unwrap()).unwrap())).collect()
    }

    #[test]
    fn level_audit_report() {
        if crate::no_game_data() {
            return;
        }
        for (id, l) in levels() {
            for m in hard(&l).into_iter().map(|m| ("hard", m)).chain(soft(&l).into_iter().map(|m| ("soft", m))) {
                println!("level {} {}: {}", id - 2000, m.0, m.1);
            }
        }
    }

    #[test]
    fn regular_levels_keep_the_invariants() {
        if crate::no_game_data() {
            return;
        }
        let bad: Vec<String> = levels().iter().filter(|(id, _)| (2001..=2014).contains(id)).flat_map(|(id, l)| hard(l).into_iter().map(move |m| format!("level {}: {m}", id - 2000))).collect();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn level_golden() {
        if crate::no_game_data() {
            return;
        }
        let now: String = levels().iter().flat_map(|(id, l)| (1..=ROOMS).map(move |r| format!("{} {r} {:016x}\n", id - 2000, room_hash(l, r)))).collect();
        if std::env::var_os("POP_BLESS").is_some() {
            std::fs::write(GOLDEN, &now).unwrap();
            return;
        }
        let want = std::fs::read_to_string(GOLDEN).expect("level_golden.txt missing: POP_BLESS=1 cargo test -p pop-core level_golden");
        let diff: Vec<String> = now.lines().zip(want.lines()).filter(|(a, b)| a != b).map(|(a, b)| format!("now {a} / golden {b}")).collect();
        assert!(diff.is_empty() && now.lines().count() == want.lines().count(), "level data changed (level room hash):\n{}", diff.join("\n"));
    }
}
