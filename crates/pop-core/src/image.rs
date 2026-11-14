//! Image and palette resources.
//! Image: u16 height, u16 width, u16 flags, data. flags: bits 12..14 = bits per pixel - 1,
//! bits 8..11 = compression (0 raw, 1 RLE left-right, 2 RLE up-down, 3 LZG left-right, 4 LZG up-down).
//! Decompressed data is planar: per row, `bpp` bit planes of `(width+7)/8` bytes each.
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    /// one palette index per pixel, row-major
    pub pixels: Vec<u8>,
}

/// 6-bit VGA palette entries -> RGB8. Palette resource: u16 id, u8, u8 count, then `count` * (r,g,b) at offset 4.
pub fn palette(res: &[u8]) -> Option<Vec<[u8; 3]>> {
    let n = *res.get(3)? as usize;
    let c = res.get(4..4 + 3 * n)?;
    Some(c.chunks(3).map(|v| [0, 1, 2].map(|i| (v[i] as u32 * 255 / 63) as u8)).collect())
}

fn rd16(b: &[u8], o: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]) as usize)
}

pub fn decode(res: &[u8]) -> Option<Image> {
    let (height, width, flags) = (rd16(res, 0)?, rd16(res, 2)?, rd16(res, 4)?);
    let (bpp, comp) = (((flags >> 12) & 7) + 1, (flags >> 8) & 15);
    let stride = (width * bpp + 7) / 8;
    let len = stride * height;
    let src = &res[6..];
    let mut out = match comp {
        0 => src.get(..len)?.to_vec(),
        1 | 2 => rle(src, len)?,
        3 | 4 => lzg(src, len)?,
        _ => return None,
    };
    if comp == 2 || comp == 4 {
        // stream order was column-major over a `stride` x `height` byte array
        let mut t = vec![0; len];
        for (k, &b) in out.iter().enumerate() {
            t[(k % height) * stride + k / height] = b;
        }
        out = t;
    }
    let mut pixels = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            pixels[y * width + x] = if bpp == 4 {
                out[y * stride + x / 2] >> (4 - 4 * (x % 2)) & 15
            } else {
                out[y * stride + x / 8] >> (7 - x % 8) & 1
            };
        }
    }
    Some(Image { width, height, bpp, pixels })
}

fn rle(src: &[u8], len: usize) -> Option<Vec<u8>> {
    let (mut out, mut i) = (Vec::with_capacity(len), 0);
    while out.len() < len {
        let c = *src.get(i)? as i8;
        i += 1;
        if c >= 0 {
            out.extend_from_slice(src.get(i..i + c as usize + 1)?);
            i += c as usize + 1;
        } else {
            let v = *src.get(i)?;
            i += 1;
            out.extend(std::iter::repeat(v).take(-(c as i32) as usize));
        }
    }
    out.truncate(len);
    Some(out)
}

fn lzg(src: &[u8], len: usize) -> Option<Vec<u8>> {
    let (mut win, mut wp) = ([0u8; 1024], 1024 - 66);
    let (mut out, mut i, mut mask) = (Vec::with_capacity(len), 0usize, 0u16);
    while out.len() < len {
        if mask <= 1 {
            mask = *src.get(i)? as u16 | 0x100;
            i += 1;
        }
        let lit = mask & 1 != 0;
        mask >>= 1;
        if lit {
            let b = *src.get(i)?;
            i += 1;
            out.push(b);
            win[wp] = b;
            wp = (wp + 1) & 1023;
        } else {
            let (a, b) = (*src.get(i)? as usize, *src.get(i + 1)? as usize);
            i += 2;
            let (n, pos) = ((a >> 2) + 3, (a & 3) << 8 | b);
            for k in 0..n {
                let b = win[(pos + k) & 1023];
                out.push(b);
                win[wp] = b;
                wp = (wp + 1) & 1023;
            }
        }
    }
    out.truncate(len);
    Some(out)
}
