//! Engine-independent core: original data formats and (later) the game simulation.
//! No graphics/audio deps, so mechanics can be tested headless against the original.
pub mod dat;
pub mod level;
pub mod sim;

#[cfg(test)]
mod tests {
    use super::*;
    const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");

    #[test]
    fn level1_loads() {
        let d = dat::Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
        println!("LEVELS.DAT ids/sizes: {:?}", d.ids().map(|i| (i, d.get(i).unwrap().len())).collect::<Vec<_>>());
        let r = d.get(2001).or_else(|| d.get(2000)).expect("level resource");
        assert_eq!(r.len(), level::LEVEL_SIZE);
        let l = level::Level::parse(r).unwrap();
        assert!((1..=24).contains(&l.start_room));
        println!("start room {} pos {} dir {}", l.start_room, l.start_pos, l.start_dir);
    }
}
