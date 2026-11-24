//! Texture atlas for the tile images. Bevy 0.20 makes `StandardMaterial` bindless: every loaded texture sits in a binding array that each
//! frame touches level by level, so the 881 separate tile textures cost 60 ms per frame with mip maps (docs/tile_filters.md). Here the
//! images of a tile set are packed into a few pages; each material keeps its own `StandardMaterial` and selects its rectangle with
//! `uv_transform`.
//!
//! * Growing: `insert` takes any image at any time; when no page has room a new page starts, so more textures (new image ids found in the
//!   DAT files) need no change here, only a `texture(..)` call in `TileSet::load`.
//! * No bleeding: every image gets a gutter of replicated edge pixels around it (`gutter` source pixels). Bilinear filtering, anisotropy and
//!   the mip levels (`gutter_for`) then only ever read the image's own colours, never a neighbour's.
//! * Pixel sizes: the colour image of a source pixel is `cs` x `cs` pixels (1 for the plain look, 4 enlarged) and its normal map `ns` x
//!   `ns` (0: none). Everything is laid out in source pixels, so both page sets share one layout and one `uv_transform`.
//!
//! Pure (no Bevy types), so it is tested without a GPU.

/// Gutter in source pixels that keeps `levels` mip levels free of neighbours: level k reaches 2^k pixels over an edge; `min_scale` is the
/// smallest pixels-per-source-pixel of the page sets (the normal map's 2 when there is one).
pub fn gutter_for(levels: u32, min_scale: usize) -> usize {
    (((1usize << (levels.max(1) - 1)) + min_scale - 1) / min_scale).max(1)
}

struct Shelf {
    y: usize,
    h: usize,
    x: usize,
}

struct Page {
    shelves: Vec<Shelf>,
    next_y: usize,
    colour: Vec<u8>,
    normal: Vec<u8>,
    /// used extent in source pixels (the finished page is cropped to it)
    used: (usize, usize),
}

/// Where an image's own pixels (without gutter) are: page and rectangle in source pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub page: usize,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

pub struct Atlas {
    /// page edge in source pixels
    units: usize,
    cs: usize,
    ns: usize,
    gutter: usize,
    pages: Vec<Page>,
}

/// A packed page, cropped to what is used: RGBA8 colour (`colour_size` pixels) and the normal map RGBA8 (`normal_size`) if any.
pub struct PageOut {
    pub colour: Vec<u8>,
    pub colour_size: (usize, usize),
    pub normal: Option<(Vec<u8>, (usize, usize))>,
}

pub struct Finished {
    /// the pages (take them to build the images, `uv` stays usable)
    pub pages: Vec<PageOut>,
    used: Vec<(usize, usize)>,
}

impl Finished {
    /// `uv_transform` of a slot as (scale, offset): `uv * scale + offset` maps an image's 0..1 onto its rectangle of the page.
    pub fn uv(&self, s: &Slot) -> ([f32; 2], [f32; 2]) {
        let (uw, uh) = self.used[s.page];
        ([s.w as f32 / uw as f32, s.h as f32 / uh as f32], [s.x as f32 / uw as f32, s.y as f32 / uh as f32])
    }
}

/// Writes `src` (`cw` x `ch` RGBA pixels) with its top-left at (`ox`, `oy`) into a page of edge `edge` pixels and fills `g` pixels around
/// it with the nearest edge pixel.
fn put(buf: &mut [u8], edge: usize, (ox, oy): (usize, usize), (cw, ch): (usize, usize), g: usize, src: &[u8]) {
    assert_eq!(src.len(), cw * ch * 4);
    for dy in -(g as isize)..(ch + g) as isize {
        let sy = dy.clamp(0, ch as isize - 1) as usize;
        for dx in -(g as isize)..(cw + g) as isize {
            let sx = dx.clamp(0, cw as isize - 1) as usize;
            let d = ((oy as isize + dy) as usize * edge + (ox as isize + dx) as usize) * 4;
            buf[d..d + 4].copy_from_slice(&src[(sy * cw + sx) * 4..][..4]);
        }
    }
}

impl Atlas {
    /// `page_px`: page edge in colour pixels (the normal pages are proportional); `cs` / `ns` as above; `gutter` in source pixels.
    pub fn new(page_px: usize, cs: usize, ns: usize, gutter: usize) -> Self {
        Atlas { units: page_px / cs, cs, ns, gutter, pages: vec![] }
    }

    /// Number of pages so far.
    pub fn pages(&self) -> usize {
        self.pages.len()
    }

    /// Finds room for a `rw` x `rh` rectangle (gutter included): the shelf with the least spare height that fits, else a new shelf (heights
    /// round up to 8 so shelves get reused), else a new page.
    fn place(&mut self, rw: usize, rh: usize) -> (usize, usize, usize) {
        let units = self.units;
        let mut best: Option<(usize, usize)> = None;
        for (pi, p) in self.pages.iter().enumerate() {
            for (si, s) in p.shelves.iter().enumerate() {
                if s.h >= rh && s.x + rw <= units && best.is_none_or(|(bp, bs)| s.h < self.pages[bp].shelves[bs].h) {
                    best = Some((pi, si));
                }
            }
        }
        if let Some((pi, si)) = best {
            let s = &mut self.pages[pi].shelves[si];
            let at = (pi, s.x, s.y);
            s.x += rw;
            return at;
        }
        let sh = ((rh + 7) / 8 * 8).min(units);
        let pi = match self.pages.iter().position(|p| p.next_y + sh <= units) {
            Some(pi) => pi,
            None => {
                let (ec, en) = (units * self.cs, units * self.ns);
                self.pages.push(Page { shelves: vec![], next_y: 0, colour: vec![0; ec * ec * 4], normal: vec![0; en * en * 4], used: (0, 0) });
                self.pages.len() - 1
            }
        };
        let p = &mut self.pages[pi];
        p.shelves.push(Shelf { y: p.next_y, h: sh, x: rw });
        let at = (pi, 0, p.next_y);
        p.next_y += sh;
        at
    }

    /// Packs an image of `w` x `h` source pixels: `colour` is `w*cs` x `h*cs` RGBA, `normal` (needed when `ns` > 0) `w*ns` x `h*ns`.
    pub fn insert(&mut self, w: usize, h: usize, colour: &[u8], normal: Option<&[u8]>) -> Slot {
        let g = self.gutter;
        let (rw, rh) = (w + 2 * g, h + 2 * g);
        assert!(rw <= self.units && rh <= self.units, "image {w} x {h} does not fit a page of {} source pixels", self.units);
        let (page, rx, ry) = self.place(rw, rh);
        let (x, y) = (rx + g, ry + g);
        let (cs, ns, units) = (self.cs, self.ns, self.units);
        let p = &mut self.pages[page];
        p.used = (p.used.0.max(rx + rw), p.used.1.max(ry + rh));
        put(&mut p.colour, units * cs, (x * cs, y * cs), (w * cs, h * cs), g * cs, colour);
        if ns > 0 {
            put(&mut p.normal, units * ns, (x * ns, y * ns), (w * ns, h * ns), g * ns, normal.expect("normal pages need normal pixels"));
        }
        Slot { page, x, y, w, h }
    }

    /// The pages, each cropped to its used extent.
    pub fn finish(self) -> Finished {
        let (cs, ns, units) = (self.cs, self.ns, self.units);
        let crop = |buf: &[u8], edge: usize, (w, h): (usize, usize)| -> Vec<u8> { (0..h).flat_map(|y| buf[y * edge * 4..][..w * 4].iter().copied()).collect() };
        let used = self.pages.iter().map(|p| p.used).collect();
        let pages = self
            .pages
            .iter()
            .map(|p| PageOut {
                colour: crop(&p.colour, units * cs, (p.used.0 * cs, p.used.1 * cs)),
                colour_size: (p.used.0 * cs, p.used.1 * cs),
                normal: (ns > 0).then(|| (crop(&p.normal, units * ns, (p.used.0 * ns, p.used.1 * ns)), (p.used.0 * ns, p.used.1 * ns))),
            })
            .collect();
        Finished { pages, used }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// an image whose pixel (x, y) is [x, y, tag, 255]
    fn img(w: usize, h: usize, scale: usize, tag: u8) -> Vec<u8> {
        (0..w * scale * h * scale).flat_map(|i| [(i % (w * scale)) as u8, (i / (w * scale)) as u8, tag, 255]).collect()
    }

    fn px(f: &Finished, page: usize, x: usize, y: usize) -> [u8; 4] {
        let o = &f.pages[page];
        o.colour[(y * o.colour_size.0 + x) * 4..][..4].try_into().unwrap()
    }

    #[test]
    fn gutters_keep_the_mips_clear() {
        assert_eq!(gutter_for(1, 4), 1);
        assert_eq!(gutter_for(4, 4), 2);
        assert_eq!(gutter_for(4, 2), 4);
    }

    #[test]
    fn rectangles_never_overlap_and_stay_inside_their_page() {
        let mut a = Atlas::new(256, 1, 0, 1);
        let sizes: Vec<(usize, usize)> = (0..300).map(|i| (8 + i * 7 % 41, 10 + i * 13 % 53)).collect();
        let slots: Vec<Slot> = sizes.iter().enumerate().map(|(i, &(w, h))| a.insert(w, h, &img(w, h, 1, i as u8), None)).collect();
        for (i, s) in slots.iter().enumerate() {
            assert!(s.x >= 1 && s.y >= 1 && s.x + s.w + 1 <= 256 && s.y + s.h + 1 <= 256, "{s:?}");
            for t in &slots[..i] {
                if s.page == t.page {
                    // gutter included on both sides: the rectangles with their gutters are disjoint
                    let apart = s.x + s.w + 1 <= t.x - 1 || t.x + t.w + 1 <= s.x - 1 || s.y + s.h + 1 <= t.y - 1 || t.y + t.h + 1 <= s.y - 1;
                    assert!(apart, "{s:?} overlaps {t:?}");
                }
            }
        }
        assert!(a.pages() > 1, "300 images do not fit one page of 256 x 256");
    }

    #[test]
    fn the_atlas_grows_by_pages_and_slots_read_back_their_image() {
        let mut a = Atlas::new(128, 2, 0, 1);
        // a page is 64 source pixels wide; each image takes 22 x 22 with its gutter: 2 per row, 2 shelves of 24 = 4 per page
        let slots: Vec<Slot> = (0..10).map(|i| a.insert(20, 20, &img(20, 20, 2, i), None)).collect();
        assert_eq!((slots[3].page, slots[4].page, slots[9].page), (0, 1, 2));
        let f = a.finish();
        for (i, s) in slots.iter().enumerate() {
            // the image's own pixel (x, y) (cs = 2) sits at its slot origin
            assert_eq!(px(&f, s.page, s.x * 2 + 3, s.y * 2 + 5), [3, 5, i as u8, 255]);
        }
    }

    #[test]
    fn a_gutter_repeats_the_edge_and_never_a_neighbour() {
        let mut a = Atlas::new(64, 1, 0, 2);
        let s1 = a.insert(8, 8, &img(8, 8, 1, 1), None);
        let s2 = a.insert(8, 8, &img(8, 8, 1, 2), None);
        assert_eq!((s1.page, s1.y), (s2.page, s2.y));
        let f = a.finish();
        // two pixels left of / right of / above / below image 1 are its own edge pixels (tag 1)
        for d in 1..=2 {
            assert_eq!(px(&f, 0, s1.x - d, s1.y + 3), [0, 3, 1, 255]);
            assert_eq!(px(&f, 0, s1.x + 7 + d, s1.y + 3), [7, 3, 1, 255]);
            assert_eq!(px(&f, 0, s1.x + 3, s1.y - d), [3, 0, 1, 255]);
            assert_eq!(px(&f, 0, s1.x + 3, s1.y + 7 + d), [3, 7, 1, 255]);
        }
        // the corner gutter is the corner pixel; the pixels of image 2 next to image 1's gutter are image 2's own gutter
        assert_eq!(px(&f, 0, s1.x - 2, s1.y - 2), [0, 0, 1, 255]);
        assert_eq!(px(&f, 0, s2.x - 1, s2.y + 3)[2], 2);
    }

    #[test]
    fn uv_maps_zero_to_one_onto_the_image_rectangle_of_the_cropped_page() {
        let mut a = Atlas::new(128, 4, 2, 2);
        let s = a.insert(10, 6, &img(10, 6, 4, 9), Some(&img(10, 6, 2, 9)));
        let f = a.finish();
        let (scale, offset) = f.uv(&s);
        let o = &f.pages[0];
        // the page is cropped to the used rectangle: 14 x 10 source pixels (gutter 2 each side), colour 4x, normal 2x
        assert_eq!(o.colour_size, (56, 40));
        assert_eq!(o.normal.as_ref().unwrap().1, (28, 20));
        // uv (0,0) and (1,1) land on the first and last pixel of the image in both pages
        for (size, k) in [(o.colour_size, 4.0), (o.normal.as_ref().unwrap().1, 2.0)] {
            let at = |u: f32, v: f32| ((u * scale[0] + offset[0]) * size.0 as f32, (v * scale[1] + offset[1]) * size.1 as f32);
            let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
            assert!(near(at(0.0, 0.0), (2.0 * k, 2.0 * k)) && near(at(1.0, 1.0), (12.0 * k, 8.0 * k)));
        }
    }
}
