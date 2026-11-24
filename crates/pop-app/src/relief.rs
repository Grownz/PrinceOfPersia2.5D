//! Tile relief (spike): a normal map for the 4-bit tile images, derived from their palette indices scaled 2x (Scale2x / EPX: a
//! pixel only changes when two neighbours agree, so the bevels come out rounded). The colour image itself is not scaled. The height is the brightness of the
//! palette colour (dark mortar low, light stone high), softened by one 3x3 blur, then Sobel. Only at load time, on the CPU.

/// Scale2x on palette indices; `px` is `w * h`, the result `2w * 2h`.
pub fn scale2x(w: usize, h: usize, px: &[u8]) -> Vec<u8> {
    let at = |x: isize, y: isize| px[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let (p, a, b, c, d) = (at(x, y), at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
            let e = if c == a && c != d && a != b { a } else { p };
            let f = if a == b && a != c && b != d { b } else { p };
            let g = if d == c && d != b && c != a { c } else { p };
            let i = if b == d && b != a && d != c { d } else { p };
            let o = (y as usize * 2) * w * 2 + x as usize * 2;
            out[o] = e;
            out[o + 1] = f;
            out[o + w * 2] = g;
            out[o + w * 2 + 1] = i;
        }
    }
    out
}

/// MMPX (Morgan McGuire, Mara Gagiu 2021; port of the MIT-licensed `mmpx` crate 0.2.0 with its two row-start neighbour reads fixed)
/// on palette indices, 2x: keeps the palette and single-pixel features, rounds diagonals. `luma(index)` is the colour's luma.
pub fn mmpx(w: usize, h: usize, px: &[u8], luma: &dyn Fn(u8) -> u32) -> Vec<u8> {
    let at = |x: isize, y: isize| px[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let n = |dx: isize, dy: isize| at(x + dx, y + dy);
            let (a, b, c, d, e, f, g, hh, i) = (n(-1, -1), n(0, -1), n(1, -1), n(-1, 0), n(0, 0), n(1, 0), n(-1, 1), n(0, 1), n(1, 1));
            let (p, q, r, s) = (n(0, -2), n(-2, 0), n(2, 0), n(0, 2));
            let (bl, dl, el, fl, hl) = (luma(b), luma(d), luma(e), luma(f), luma(hh));
            let (mut j, mut k, mut l, mut m) = (e, e, e, e);
            if d == b && d != hh && d != f && (el >= dl || e == a) && (e == a || e == c || e == g) && (el < dl || a != d || e != p || e != q) { j = d; }
            if b == f && b != d && b != hh && (el >= bl || e == c) && (e == a || e == c || e == i) && (el < bl || c != b || e != p || e != r) { k = b; }
            if hh == d && hh != f && hh != b && (el >= hl || e == g) && (e == a || e == g || e == i) && (el < hl || g != hh || e != s || e != q) { l = hh; }
            if f == hh && f != b && f != d && (el >= fl || e == i) && (e == c || e == g || e == i) && (el < fl || i != hh || e != r || e != s) { m = f; }
            if e != f && e == c && e == i && e == d && e == q && f == b && f == hh && f != n(3, 0) { k = f; m = f; }
            if e != d && e == a && e == g && e == f && e == r && d == b && d == hh && d != n(-3, 0) { j = d; l = d; }
            if e != hh && e == g && e == i && e == b && e == p && hh == d && hh == f && hh != n(0, 3) { l = hh; m = hh; }
            if e != b && e == a && e == c && e == hh && e == s && b == d && b == f && b != n(0, -3) { j = b; k = b; }
            if bl < el && e == g && e == hh && e == i && e == s && e != a && e != d && e != c && e != f { j = b; k = b; }
            if hl < el && e == a && e == b && e == c && e == p && e != d && e != g && e != i && e != f { l = hh; m = hh; }
            if fl < el && e == a && e == d && e == g && e == q && e != b && e != c && e != i && e != hh { k = f; m = f; }
            if dl < el && e == c && e == f && e == i && e == r && e != b && e != a && e != g && e != hh { j = d; l = d; }
            if hh != b {
                if hh != a && hh != e && hh != c {
                    if hh == g && hh == f && hh == r && hh != d && hh != n(2, -1) { l = m; }
                    if hh == i && hh == d && hh == q && hh != f && hh != n(-2, -1) { m = l; }
                }
                if b != i && b != g && b != e {
                    if b == a && b == f && b == r && b != d && b != n(2, 1) { j = k; }
                    if b == c && b == d && b == q && b != f && b != n(-2, 1) { k = j; }
                }
            }
            if f != d {
                if d != i && d != e && d != c {
                    if d == a && d == hh && d == s && d != b && d != n(1, 2) { j = l; }
                    if d == g && d == b && d == p && d != hh && d != n(1, -2) { l = j; }
                }
                if f != e && f != a && f != g {
                    if f == c && f == hh && f == s && f != b && f != n(-1, 2) { k = m; }
                    if f == i && f == b && f == p && f != hh && f != n(-1, -2) { m = k; }
                }
            }
            let o = (y as usize * 2) * w * 2 + x as usize * 2;
            (out[o], out[o + 1], out[o + w * 2], out[o + w * 2 + 1]) = (j, k, l, m);
        }
    }
    out
}

/// How the tile colour images are enlarged (test, `POP_UPSCALE`): `nearest` as before (1x, nearest sampling); the others make a
/// 4x image sampled linearly (fractional screen scales stay crisp): `epx` and `mmpx` filter twice, `sharp` repeats pixels
/// (sharp bilinear).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upscale {
    Nearest,
    Epx,
    Mmpx,
    Sharp,
}

impl Upscale {
    pub fn from_env() -> Self {
        match std::env::var("POP_UPSCALE").as_deref() {
            Ok("epx") => Upscale::Epx,
            Ok("mmpx") => Upscale::Mmpx,
            Ok("sharp") => Upscale::Sharp,
            _ => Upscale::Nearest,
        }
    }

    /// The enlarged indices and their size (1x for `Nearest`).
    pub fn apply(self, w: usize, h: usize, px: &[u8], pal: &[[u8; 3]]) -> (usize, usize, Vec<u8>) {
        let luma = |p: u8| pal.get(p as usize).map_or(0, |c| (2126 * c[0] as u32 + 7152 * c[1] as u32 + 722 * c[2] as u32) / 10000);
        match self {
            Upscale::Nearest => (w, h, px.to_vec()),
            Upscale::Epx => (w * 4, h * 4, scale2x(w * 2, h * 2, &scale2x(w, h, px))),
            Upscale::Mmpx => (w * 4, h * 4, mmpx(w * 2, h * 2, &mmpx(w, h, px, &luma), &luma)),
            Upscale::Sharp => (w * 4, h * 4, (0..w * h * 16).map(|i| px[i / (w * 16) * w + i % (w * 4) / 4]).collect()),
        }
    }
}

/// What a mip chain averages: sRGB colour with alpha (in linear light, weighted by alpha so cut-out edges stay clean) or a tangent-space
/// normal map (the vectors averaged and renormalised).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MipKind {
    Colour,
    Normal,
}

/// Number of mip levels of a `w` x `h` image (down to 1 x 1).
pub fn mip_levels(w: usize, h: usize) -> u32 {
    usize::BITS - w.max(h).leading_zeros()
}

fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear light to an sRGB byte through an 8192-entry table (a `powf` per channel made the mip chains twice as slow).
fn linear_to_srgb(l: f32) -> u8 {
    static ENC: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let t = ENC.get_or_init(|| {
        (0..8192).map(|i| {
            let c = i as f32 / 8191.0;
            ((if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }) * 255.0).round() as u8
        }).collect()
    });
    t[(l.clamp(0.0, 1.0) * 8191.0 + 0.5) as usize]
}

/// RGBA8 `rgba` (`w` x `h`) followed by its smaller levels up to `levels` in all (2 x 2 box filter, edges clamped), the layout of an
/// `Image` with `mip_level_count` = `levels` (at most `mip_levels(w, h)`).
pub fn mip_chain(w: usize, h: usize, rgba: Vec<u8>, kind: MipKind, levels: u32) -> Vec<u8> {
    let lut: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    let mut out = rgba;
    let (mut start, mut cw, mut ch) = (0, w, h);
    for _ in 1..levels {
        if cw == 1 && ch == 1 {
            break;
        }
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut level = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                let texels: [&[u8]; 4] = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| {
                    let (sx, sy) = ((x * 2 + dx).min(cw - 1), (y * 2 + dy).min(ch - 1));
                    &out[start + (sy * cw + sx) * 4..][..4]
                });
                match kind {
                    MipKind::Colour => {
                        let a: f32 = texels.iter().map(|t| t[3] as f32).sum();
                        let mut px = [0u8; 4];
                        for c in 0..3 {
                            // weighted by alpha; fully transparent texels keep the plain average
                            let (sum, wt) = texels.iter().fold((0.0, 0.0), |(s, wt), t| {
                                let k = if a > 0.0 { t[3] as f32 } else { 1.0 };
                                (s + lut[t[c] as usize] * k, wt + k)
                            });
                            px[c] = linear_to_srgb(sum / wt);
                        }
                        px[3] = (a / 4.0).round() as u8;
                        level.extend_from_slice(&px);
                    }
                    MipKind::Normal => {
                        let mut n = [0.0f32; 3];
                        for t in texels {
                            for c in 0..3 {
                                n[c] += t[c] as f32 / 255.0 * 2.0 - 1.0;
                            }
                        }
                        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        let n = if l < 1e-3 { [0.0, 0.0, 1.0] } else { n.map(|v| v / l) };
                        level.extend(n.map(|v| ((v * 0.5 + 0.5) * 255.0).round() as u8));
                        level.push(255);
                    }
                }
            }
        }
        start = out.len();
        out.extend_from_slice(&level);
        (cw, ch) = (nw, nh);
    }
    out
}

/// Height 0..1 from palette indices: the colour's brightness, blurred 3x3 (edges clamped).
fn height(w: usize, h: usize, px: &[u8], pal: &[[u8; 3]]) -> Vec<f32> {
    let lum: Vec<f32> = px.iter().map(|&p| pal.get(p as usize).map_or(0.5, |c| (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) / 255.0)).collect();
    let at = |x: isize, y: isize| lum[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    (0..w * h).map(|i| {
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| at(x + dx, y + dy)).sum::<f32>() / 9.0
    }).collect()
}

/// RGBA of an OpenGL-style tangent-space normal map (green up in the image) from the tile's palette indices; `strength` scales
/// the slope (about 3 shallow, 12 strong).
pub fn normal_map(w: usize, h: usize, px: &[u8], pal: &[[u8; 3]], strength: f32) -> Vec<u8> {
    let ht = height(w, h, px, pal);
    let at = |x: isize, y: isize| ht[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h as isize {
        for x in 0..w as isize {
            // y runs down the image: a rise towards the top tilts the normal down, so green = +dh/dy
            let (dx, dy) = ((at(x + 1, y) - at(x - 1, y)) * 0.5 * strength, (at(x, y + 1) - at(x, y - 1)) * 0.5 * strength);
            let n = [-dx, dy, 1.0];
            let l = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
            out.extend(n.map(|v| ((v / l * 0.5 + 0.5) * 255.0).round() as u8));
            out.push(255);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale2x_keeps_flat_areas_and_rounds_a_corner() {
        assert_eq!(scale2x(2, 2, &[7, 7, 7, 7]), vec![7; 16]);
        // an L-shaped corner: the 2 at (1,1) has 1 above and left, so its outer sub-pixel takes 1 (rounded corner)
        let out = scale2x(3, 3, &[1, 1, 1, 1, 2, 2, 1, 2, 2]);
        assert_eq!(out.len(), 36);
        assert_eq!((out[2 * 6 + 2], out[3 * 6 + 3]), (1, 2));
    }

    #[test]
    fn mmpx_keeps_a_single_pixel_and_matches_the_reference_port() {
        // the expected output is tools/tile_filters/compare.py's port (checked against the crate with its row-start reads)
        let px = [1u8, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 2, 2, 2, 1, 1, 2, 2, 2, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 3, 1];
        let luma = |p: u8| [0u32, 200, 50, 100][p as usize];
        let out = mmpx(6, 6, &px, &luma);
        let want: [[u8; 12]; 12] = [[1,1,1,1,1,1,1,1,1,1,1,1],[1,1,1,1,1,1,1,1,1,1,1,1],[1,1,1,1,1,1,2,2,1,1,1,1],[1,1,1,1,1,2,2,2,1,1,1,1],[1,1,1,1,2,2,2,2,2,2,1,1],[1,1,1,2,2,2,2,2,2,2,1,1],[1,1,2,2,2,2,2,2,2,1,1,1],[1,1,2,2,2,2,2,2,1,1,1,1],[1,1,1,1,2,2,2,1,1,1,1,1],[1,1,1,1,2,2,1,1,1,1,1,1],[1,1,1,1,1,1,1,1,3,3,1,1],[1,1,1,1,1,1,1,1,3,3,1,1]];
        assert_eq!(out, want.concat());
    }

    #[test]
    fn upscale_modes_make_4x_images() {
        let pal = [[0, 0, 0], [255, 255, 255]];
        let px = [0u8, 1, 1, 0];
        for u in [Upscale::Epx, Upscale::Mmpx, Upscale::Sharp] {
            let (w, h, o) = u.apply(2, 2, &px, &pal);
            assert_eq!((w, h, o.len()), (8, 8, 64));
        }
        assert_eq!(Upscale::Nearest.apply(2, 2, &px, &pal), (2, 2, px.to_vec()));
        // sharp: every pixel repeated 4x4
        let o = Upscale::Sharp.apply(2, 2, &px, &pal).2;
        assert_eq!((o[0], o[3], o[4], o[7], o[3 * 8], o[3 * 8 + 4], o[4 * 8 + 4], o[7 * 8 + 3]), (0, 0, 1, 1, 0, 1, 0, 1));
    }

    #[test]
    fn mip_chains_average_in_linear_light_and_renormalise_normals() {
        assert_eq!(mip_levels(128, 240), 8);
        assert_eq!(mip_levels(1, 1), 1);
        // black + white average to 188 in sRGB (0.5 linear), not 128
        let c = mip_chain(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255], MipKind::Colour, 9);
        assert_eq!(c.len(), 8 + 4);
        assert!((187..=189).contains(&c[8]) && c[11] == 255);
        // an opaque red and a transparent texel: the colour stays red, the alpha halves
        let c = mip_chain(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0], MipKind::Colour, 9);
        assert_eq!(&c[8..11], &[255, 0, 0]);
        assert!((127..=128).contains(&c[11]));
        // opposite tilts cancel to the flat normal
        let n = mip_chain(2, 1, vec![255, 128, 255, 255, 0, 128, 255, 255], MipKind::Normal, 9);
        // only as many levels as asked for
        assert_eq!(mip_chain(4, 4, vec![9; 64], MipKind::Colour, 2).len(), 64 + 16);
        assert_eq!(mip_chain(4, 4, vec![9; 64], MipKind::Colour, 1).len(), 64);
        assert!(n[8].abs_diff(128) <= 1 && n[10] >= 254);
    }

    #[test]
    fn a_ridge_tilts_the_normals_away_from_it() {
        // bright column in the middle of dark ones: left of it the normal leans left (red < 128), right of it right
        let pal = [[0, 0, 0], [255, 255, 255]];
        let px: Vec<u8> = (0..25).map(|i| u8::from(i % 5 == 2)).collect();
        let n = normal_map(5, 5, &px, &pal, 3.0);
        let r = |x: usize| n[(2 * 5 + x) * 4];
        assert!(r(1) < 128 && r(3) > 128 && r(2).abs_diff(128) < 4);
        assert_eq!(n[3], 255);
    }
}
