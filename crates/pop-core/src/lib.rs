//! Engine-independent core: original data formats and (later) the game simulation.
//! No graphics/audio deps, so mechanics can be tested headless against the original.
pub mod dat;
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
}
