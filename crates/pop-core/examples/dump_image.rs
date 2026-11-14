//! cargo run -p pop-core --example dump_image -- <dir> <image.DAT> <id> <pal.DAT> <palid> <out.ppm>
use pop_core::{dat::Dat, image};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = Dat::open(format!("{}/{}", a[1], a[2])).unwrap();
    let im = image::decode(img.get(a[3].parse().unwrap()).unwrap()).expect("decode");
    let pal = Dat::open(format!("{}/{}", a[1], a[4])).unwrap();
    let pal = image::palette(pal.get(a[5].parse().unwrap()).unwrap()).unwrap();
    let mut o = format!("P6 {} {} 255\n", im.width, im.height).into_bytes();
    for &p in &im.pixels {
        o.extend(pal.get(p as usize).unwrap_or(&[255, 0, 255]));
    }
    std::fs::write(&a[6], o).unwrap();
}
