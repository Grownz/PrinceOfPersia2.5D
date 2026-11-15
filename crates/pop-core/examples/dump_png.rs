//! cargo run -p pop-core --example dump_png -- <Original_data dir> <outdir>
//! Writes every decodable image of VDUNGEON.DAT and VPALACE.DAT as an indexed PNG (palette 200, index 0 transparent),
//! one contact sheet per DAT (4x scale, 8 per row, ordered by id) with <stem>_sheet_index.txt (grid index -> id),
//! and prints the histogram of wall modifiers from LEVELS.DAT. No external crates: stored-deflate PNG writer.
use pop_core::{dat::Dat, image::{self, Image}, level::{Level, Tile}};
use std::collections::BTreeMap;

/// Levels with the palace flag set (ds:1CE, read from PRINCE.EXE).
const PALACE_LEVELS: [usize; 6] = [4, 5, 6, 10, 11, 14];

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let mut body = ty.to_vec();
    body.extend(data);
    out.extend(&body);
    out.extend(crc32(&body).to_be_bytes());
}

/// zlib stream of stored (uncompressed) deflate blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let mut parts = raw.chunks(65535).peekable();
    if raw.is_empty() {
        z.extend([1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(c) = parts.next() {
        z.push(parts.peek().is_none() as u8);
        let n = c.len() as u16;
        z.extend(n.to_le_bytes());
        z.extend((!n).to_le_bytes());
        z.extend(c);
    }
    z.extend(adler32(raw).to_be_bytes());
    z
}

/// 8-bit indexed PNG. `transparent0`: palette index 0 is transparent (tRNS).
fn png(w: usize, h: usize, px: &[u8], plte: &[[u8; 3]], transparent0: bool) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w + 1) * h);
    for row in px.chunks(w) {
        raw.push(0); // filter type 0
        raw.extend(row);
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, 3, 0, 0, 0]); // 8 bit, colour type 3 (indexed)
    chunk(&mut out, b"IHDR", &ihdr);
    let pl: Vec<u8> = plte.iter().flatten().copied().collect();
    chunk(&mut out, b"PLTE", &pl);
    if transparent0 {
        chunk(&mut out, b"tRNS", &[0]);
    }
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, out) = (&a[1], &a[2]);
    std::fs::create_dir_all(out).unwrap();
    let levels = Dat::open(format!("{dir}/LEVELS.DAT")).unwrap();
    let mut wall_hist: [BTreeMap<u8, usize>; 2] = [BTreeMap::new(), BTreeMap::new()]; // [dungeon, palace]

    for name in ["VDUNGEON.DAT", "VPALACE.DAT"] {
        let dat = Dat::open(format!("{dir}/{name}")).unwrap();
        let pal = image::palette(dat.get(200).unwrap()).unwrap();
        let stem = name.trim_end_matches(".DAT");
        let mut imgs: Vec<(u16, Image)> = dat
            .ids()
            .filter_map(|id| image::decode(dat.get(id)?).map(|im| (id, im)))
            .collect();
        imgs.sort_by_key(|(id, _)| *id);
        for (id, im) in &imgs {
            let p = png(im.width, im.height, &im.pixels, &pal, true);
            std::fs::write(format!("{out}/{stem}_{id}.png"), p).unwrap();
        }

        // contact sheet: cell = largest image of the set at 4x plus 8 px gutter; checker background (indices 16, 17)
        let base = pal.len() as u8;
        let cw = imgs.iter().map(|(_, i)| i.width).max().unwrap() * 4 + 8;
        let ch = imgs.iter().map(|(_, i)| i.height).max().unwrap() * 4 + 8;
        let cols = 8;
        let rows = (imgs.len() + cols - 1) / cols;
        let (sw, sh) = (cols * cw, rows * ch);
        let mut sheet = vec![0u8; sw * sh];
        let mut index = String::new();
        for (k, (id, im)) in imgs.iter().enumerate() {
            let (gx, gy) = ((k % cols) * cw, (k / cols) * ch);
            writeln!(index, "grid {k} (row {}, col {}) = id {id}  {}x{} bpp{}", k / cols, k % cols, im.width, im.height, im.bpp).unwrap();
            for y in 0..ch {
                for x in 0..cw {
                    sheet[(gy + y) * sw + gx + x] = base + ((x / 8 + y / 8) & 1) as u8;
                }
            }
            for y in 0..im.height * 4 {
                for x in 0..im.width * 4 {
                    let p = im.pixels[(y / 4) * im.width + x / 4];
                    if p != 0 {
                        sheet[(gy + 4 + y) * sw + gx + 4 + x] = p;
                    }
                }
            }
        }
        let mut plte = pal.clone();
        plte.push([48, 48, 56]);
        plte.push([84, 84, 96]);
        std::fs::write(format!("{out}/{stem}_sheet.png"), png(sw, sh, &sheet, &plte, false)).unwrap();
        std::fs::write(format!("{out}/{stem}_sheet_index.txt"), index).unwrap();
        println!("{stem}: {} images, sheet {sw}x{sh}", imgs.len());
    }

    for lv in 0..16usize {
        let Some(l) = levels.get(2000 + lv as u16).and_then(Level::parse) else { continue };
        let set = PALACE_LEVELS.contains(&lv) as usize;
        for room in 1..=pop_core::level::ROOMS {
            for idx in 0..pop_core::level::TILES_PER_ROOM {
                let t = l.tile_at(room, idx);
                if t.kind == Tile::Wall {
                    *wall_hist[set].entry(t.modifier).or_insert(0) += 1;
                }
            }
        }
    }
    for (set, name) in [(0, "dungeon"), (1, "palace")] {
        let line: Vec<String> = wall_hist[set].iter().map(|(m, c)| format!("{m}:{c}")).collect();
        println!("wall modifier histogram {name} (mod:count): {}", line.join(" "));
    }
}

use std::fmt::Write as _;
