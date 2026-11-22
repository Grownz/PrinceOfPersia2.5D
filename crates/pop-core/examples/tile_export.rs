//! cargo run -p pop-core --example tile_export -- <Original_data dir> <outdir>
//! Writes every decodable image of VDUNGEON.DAT / VPALACE.DAT as <set>_<id>.bin (u16 width, u16 height, u8 bpp, one palette
//! index per pixel) and palettes 200 and 360 as <set>_pal<id>.txt (one "r g b" per line, 8-bit), for tools/room_shots/compare.py.
use pop_core::{dat::Dat, image};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    std::fs::create_dir_all(&a[2]).unwrap();
    for set in ["VDUNGEON", "VPALACE"] {
        let d = Dat::open(format!("{}/{set}.DAT", a[1])).unwrap();
        for id in d.ids() {
            let r = d.get(id).unwrap();
            if (id == 200 || id == 360) && r.len() < 200 {
                let p = image::palette(r).unwrap();
                let t: String = p.iter().map(|c| format!("{} {} {}\n", c[0], c[1], c[2])).collect();
                std::fs::write(format!("{}/{set}_pal{id}.txt", a[2]), t).unwrap();
                continue;
            }
            let w = |o: usize| r.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
            let (h, wd, fl) = (w(0), w(2), w(4));
            if !((1..=400).contains(&h) && (1..=400).contains(&wd) && matches!(fl >> 12, 0 | 0xb) && (fl >> 8) & 15 <= 4) {
                continue;
            }
            if let Some(im) = image::decode(r) {
                let mut o = (im.width as u16).to_le_bytes().to_vec();
                o.extend((im.height as u16).to_le_bytes());
                o.push(im.bpp as u8);
                o.extend(&im.pixels);
                std::fs::write(format!("{}/{set}_{id}.bin", a[2], ), o).unwrap();
            }
        }
    }
}
