use pop_core::{dat::Dat, image};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = Dat::open(format!("{}/PV.DAT", a[1])).unwrap();
    let im = image::decode(d.get(951).unwrap()).unwrap();
    let (x0, x1, y0, y1): (usize, usize, usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap());
    for y in y0..y1 {
        let row: String = (x0..x1).map(|x| format!("{:x}", im.pixels[y * im.width + x])).collect();
        println!("{y:3} {row}");
    }
}
