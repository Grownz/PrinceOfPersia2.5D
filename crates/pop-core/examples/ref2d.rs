//! throwaway: 2D reference composite of a kid frame against tiles (original px). usage: ref2d <dir> <out.png> <frame,x,y,left>... 
use pop_core::{dat::Dat, exe::Exe, image};
fn crc32(data: &[u8]) -> u32 { let mut c = 0xFFFF_FFFFu32; for &b in data { c ^= b as u32; for _ in 0..8 { c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 }; } } !c }
fn adler32(data: &[u8]) -> u32 { let (mut a, mut b) = (1u32, 0u32); for &x in data { a = (a + x as u32) % 65521; b = (b + a) % 65521; } (b << 16) | a }
fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) { out.extend((data.len() as u32).to_be_bytes()); let mut body = ty.to_vec(); body.extend(data); out.extend(&body); out.extend(crc32(&body).to_be_bytes()); }
fn zlib_stored(raw: &[u8]) -> Vec<u8> { let mut z = vec![0x78, 0x01]; let mut parts = raw.chunks(65535).peekable(); while let Some(c) = parts.next() { z.push(parts.peek().is_none() as u8); let n = c.len() as u16; z.extend(n.to_le_bytes()); z.extend((!n).to_le_bytes()); z.extend(c); } z.extend(adler32(raw).to_be_bytes()); z }
fn png(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> { let mut raw = Vec::new(); for row in rgb.chunks(w * 3) { raw.push(0); raw.extend(row); } let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]; let mut ihdr = vec![]; ihdr.extend((w as u32).to_be_bytes()); ihdr.extend((h as u32).to_be_bytes()); ihdr.extend([8, 2, 0, 0, 0]); chunk(&mut out, b"IHDR", &ihdr); chunk(&mut out, b"IDAT", &zlib_stored(&raw)); chunk(&mut out, b"IEND", &[]); out }
const OX: i32 = 64; // canvas x of screen x 0
struct Canvas { w: usize, h: usize, px: Vec<u8> }
impl Canvas {
    fn blit(&mut self, im: &image::Image, pal: &[[u8; 3]], x: i32, bottom: i32, mirror: bool) {
        let top = bottom - im.height as i32 + 1;
        for yy in 0..im.height { for xx in 0..im.width {
            let p = im.pixels[yy * im.width + if mirror { im.width - 1 - xx } else { xx }];
            if p == 0 { continue; }
            let (cx, cy) = (x + OX + xx as i32, top + yy as i32);
            if cx < 0 || cy < 0 || cx >= self.w as i32 || cy >= self.h as i32 { continue; }
            let c = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
            let o = (cy as usize * self.w + cx as usize) * 3; self.px[o..o + 3].copy_from_slice(&c);
        } }
    }
    fn vline(&mut self, x: i32, col: [u8; 3]) { for y in 0..self.h { let o = (y * self.w + (x + OX) as usize) * 3; self.px[o..o + 3].copy_from_slice(&col); } }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let exe = Exe::open(format!("{dir}/PRINCE.EXE")).unwrap();
    let frames = exe.kid_frames();
    let kd = Dat::open(format!("{dir}/KID.DAT")).unwrap();
    let kpal = image::palette(kd.get(400).unwrap()).unwrap();
    let td = Dat::open(format!("{dir}/VDUNGEON.DAT")).unwrap();
    let tpal = image::palette(td.get(200).unwrap()).unwrap();
    let tile = |id: u16| image::decode(td.get(id).unwrap()).unwrap();
    let (w, h) = ((320 + OX as usize) + 32, 200usize);
    let mut c = Canvas { w, h, px: vec![0; w * h * 3] };
    // room 2 (level 1): row0 / row1 / row2; col -1 = room 6 col 9
    let rows: [[u8; 11]; 3] = [[3, 0, 0, 20, 20, 20, 20, 0, 0, 20, 20], [20, 19, 1, 3, 19, 1, 3, 1, 1, 3, 1], [20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20]];
    for r in 0..3 { for k in 0..11 { // k = col + 1
        let col = k as i32 - 1; let x = 32 * col; let y = 55 + 63 * r as i32;
        match rows[r][k] {
            20 => { c.blit(&tile(368), &tpal, x, y, false); c.blit(&tile(367), &tpal, x, y + 3, false); }
            0 => {}
            t => { c.blit(&tile(241), &tpal, x, y, false); c.blit(&tile(243), &tpal, x, y + 3, false); if t == 3 { c.blit(&tile(293), &tpal, x, y, false); } }
        }
    } }
    for x in (0..320).step_by(32) { c.vline(x, [255, 255, 0]); } // tile image columns 32c
    for x in (16..320).step_by(32) { c.vline(x, [0, 120, 255]); } // logic col edges 32c+16
    for arg in &a[3..] {
        let v: Vec<i32> = arg.split(',').map(|s| s.parse().unwrap()).collect();
        let (fr, x, y, left) = (v[0] as usize, v[1] as u8, v[2] as u8, v[3] != 0);
        let f = frames[fr];
        let sx = (2 * (x as i32 + if left { -(f.dx as i32) } else { f.dx as i32 }) - 116 + ((((f.flags ^ if left { 0xFF } else { 0 }) & 0x80) == 0) as i32)) * 320 / 280;
        let im = image::decode(kd.get(401 + f.image as u16).unwrap()).unwrap();
        let bottom = (y as i32 + f.dy as i32) & 255;
        let lx = if left { sx } else { sx - im.width as i32 };
        println!("frame {fr} img {} {}x{} x {x} y {y} left {left}: sx {sx} left edge {lx} right edge {} bottom {bottom}", f.image, im.width, im.height, lx + im.width as i32);
        let top = bottom - im.height as i32 + 1;
        let first = (0..im.height).find(|&r| (0..im.width).any(|xx| im.pixels[r * im.width + xx] != 0)).unwrap();
        let cols: Vec<usize> = (0..im.width).filter(|&xx| im.pixels[first * im.width + xx] != 0).collect();
        let (c0, c1) = (cols[0] as i32, *cols.last().unwrap() as i32);
        let (h0, h1) = if left { (lx + c0, lx + c1) } else { (lx + im.width as i32 - 1 - c1, lx + im.width as i32 - 1 - c0) };
        println!("  top row {} (y {}) hands at screen x {h0}..{h1}", first, top + first as i32);
        c.blit(&im, &kpal, lx, bottom, !left);
    }
    let s = 3; let (sw, sh) = (w * s, h * s); let mut big = vec![0u8; sw * sh * 3];
    for y in 0..sh { for x in 0..sw { let o = ((y / s) * w + x / s) * 3; big[(y * sw + x) * 3..][..3].copy_from_slice(&c.px[o..o + 3]); } }
    std::fs::write(&a[2], png(sw, sh, &big)).unwrap();
}
