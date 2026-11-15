//! cargo run -p pop-core --example dat_inventory -- <dir> <file.DAT> [<palid> <outdir>]
//! Lists every resource: id, payload size, decoded image (WxH, bpp, compression), or PAL (colour count), or raw header bytes.
//! With palid and outdir, also writes every decodable image as <outdir>/<stem>_<id>.ppm using palette palid from the same DAT.
use pop_core::{dat::Dat, image};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dat = Dat::open(format!("{}/{}", a[1], a[2])).unwrap();
    let pal = a.get(3).map(|p| image::palette(dat.get(p.parse().unwrap()).unwrap()).unwrap());
    for id in dat.ids() {
        let r = dat.get(id).unwrap();
        let info = match image::decode(r) {
            Some(im) => format!("img {}x{} bpp{} comp{}", im.width, im.height, im.bpp, (u16::from_le_bytes([r[4], r[5]]) >> 8) & 15),
            None => match image::palette(r) {
                Some(p) if r[3] > 0 => format!("PAL {} colours", p.len()),
                _ => format!("raw {:02x?}", &r[..r.len().min(8)]),
            },
        };
        println!("{:5} {:6} {}", id, r.len(), info);
    }
    if let (Some(pal), Some(out)) = (&pal, a.get(4)) {
        let stem = a[2].trim_end_matches(".DAT");
        for id in dat.ids() {
            if let Some(im) = image::decode(dat.get(id).unwrap()) {
                let mut o = format!("P6 {} {} 255\n", im.width, im.height).into_bytes();
                for &p in &im.pixels {
                    o.extend(pal.get(p as usize).unwrap_or(&[255, 0, 255]));
                }
                std::fs::write(format!("{}/{}_{}.ppm", out, stem, id), o).unwrap();
            }
        }
    }
}
