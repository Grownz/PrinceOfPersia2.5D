//! cargo run -p pop-core --example level_dump -- <Original_data dir> <outdir>
//! Writes levelN.bin (N = 0..=15) = fg[720] bg[720] links[24*4] start room, tile, dir: the runtime level arrays as this
//! crate parses them; levelN.guards = one "room tile dir" line per guard. tools/level_ram/level_ram.py compares them with the original's RAM.
use pop_core::{dat::Dat, level::Level};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = Dat::open(format!("{}/LEVELS.DAT", a[1])).unwrap();
    std::fs::create_dir_all(&a[2]).unwrap();
    for n in 0..=15u16 {
        let l = Level::parse(d.get(2000 + n).unwrap()).unwrap();
        std::fs::write(format!("{}/level{n}.raw", a[2]), d.get(2000 + n).unwrap()).unwrap();
        let mut v = [l.fg.as_slice(), l.bg.as_slice()].concat();
        v.extend(l.links.iter().flatten());
        v.extend([l.start_room, l.start_tile, l.start_dir]);
        std::fs::write(format!("{}/level{n}.bin", a[2]), v).unwrap();
        let g: String = l.guards.iter().map(|g| format!("{} {} {}
", g.room, g.tile, g.dir)).collect();
        std::fs::write(format!("{}/level{n}.guards", a[2]), g).unwrap();
    }
}
