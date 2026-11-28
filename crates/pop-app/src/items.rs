//! Voxel items on the floor tiles: the sword (PRINCE.DAT 160 / 161), the skeleton pile (VDUNGEON 1230 / 1231) and the potions (PRINCE.DAT chtab 1: bottles
//! 162 small / 163 big, palace 164 / 165, bubble frames 166..172 drawn in the potion's colour). Built at runtime with the character voxel
//! pipeline (`voxel.rs`): the front projection of each model is the original image. Only with `tile_textures` = original;
//! anything missing keeps the procedural box (room.rs).
use crate::voxel::VoxelModel;
use bevy::prelude::*;
use pop_core::{dat::Dat, image};
use std::collections::HashMap;

/// A voxel model ready to place: mesh (origin = image bottom-left, one unit = one sprite pixel) and its size in pixels.
#[derive(Clone)]
pub struct Model {
    pub mesh: Handle<Mesh>,
    pub w: usize,
    pub h: usize,
}

pub struct Items {
    /// vertex-coloured material shared by the item meshes
    pub mat: Handle<StandardMaterial>,
    /// the same for the steel parts (gate bars, spikes, chomper blades, swords, exit door): mood.rs makes it metallic in enhanced mode
    pub steel: Handle<StandardMaterial>,
    /// skeleton pile per set [dungeon, palace] (the palace set has no skeleton images)
    pub skeleton: [Option<Model>; 2],
    /// potion bottles per set [dungeon, palace], each [small, big] (`bottle_image`)
    pub bottles: Option<[[Model; 2]; 2]>,
    /// bubble frames 1..7 (potion_fram_bubb), one mesh each
    pub bubbles: Vec<Model>,
    /// where the bubbles are inside each bubble frame: centroid of the set pixels, (x from the left, y from the bottom) in image px;
    /// the potion's light follows it
    pub bubble_at: Vec<[f32; 2]>,
    /// the sword lying on the floor: PRINCE.DAT 160 plain, 161 with the glint (`Level::anim_frame` 0 / 1); empty if missing
    pub sword: Vec<Model>,
    /// emissive liquid materials [red, green, blue]
    pub glow: [Handle<StandardMaterial>; 3],
    /// voxel columns per set [dungeon, palace] by image id (COLUMN_IDS); empty with `column_style` = flat
    pub columns: [HashMap<u16, Column>; 2],
    /// voxel objects per set (spikes, chompers, exit doors, torch holder, debris); empty with `object_style` = flat
    pub objects: [Objects; 2],
    /// torch flame frames (PRINCE.DAT 151..159) and their unlit, overbright material (blooms in enhanced mode)
    pub flames: Vec<Model>,
    /// visible (opaque) pixels of each flame frame: drives the torch light's flicker
    pub flame_px: Vec<usize>,
    /// centre of the set pixels of each flame frame (x from the left, y from the bottom, image px): the torch light follows it
    pub flame_at: Vec<[f32; 2]>,
    pub flame_mat: Handle<StandardMaterial>,
}

/// The voxel objects of one tile set (docs/texture_inventory.md 15). Spike frames (back-only part, right and fore pieces),
/// chomper blades and upper teeth and the torch holder by image id; exit frames [left, right] with their lintels, door
/// panels by slice count (`panel_slices`), the stairs and the debris heap. Anything missing: that piece stays flat.
#[derive(Default)]
pub struct Objects {
    pub ids: HashMap<u16, Model>,
    pub exit: [Option<Model>; 2],
    pub panels: Vec<Model>,
    pub stairs: Option<Model>,
    pub debris: Option<Model>,
    /// the gate by rise 1..=48 (`gate_rise`, `gate_sprite`; docs/texture_inventory.md 21)
    pub gate: Vec<Model>,
    /// image x (px) of the torch holder's rod axis (the flame stands on it)
    pub holder_axis: f32,
}

/// Chomper blade shear: image column x stands round((JAW_XREF - x) * JAW_SHEAR) voxels (1/32 unit) in front of column
/// JAW_XREF, so the blade's left edge is near the floor's front edge and column 13.5 (where the 2D fore slice ends) at
/// the figures' depth.
pub const JAW_SHEAR: f32 = 0.6;
pub const JAW_XREF: f32 = 31.0;

/// Exit door: slices of 233 stacked on the bottom piece 234 at modifier `m` (0 closed .. 43 open), as room.rs drew them
/// flat: the bottom edge m px above the floor, slices up to the frame top (EXIT_FRAME_H, 6-px slices).
pub fn panel_slices(m: u8) -> usize {
    let (mut by, mut n) = (m.min(43) as usize + 6, 0);
    while by < crate::tiles::EXIT_FRAME_H {
        by += 6;
        n += 1;
    }
    n
}

/// Gate bars (draw routine `B47F` / `B586`): the lowest bar tip stands `gate_rise(modifier)` px above the floor line,
/// modifier 0 closed .. 188 open (`(min(m, 188) >> 2) + 1`, 1..=48). The bars reach 60 px up (the cell top, `[4FCA]`).
pub fn gate_rise(modifier: u8) -> usize {
    (modifier.min(188) >> 2) as usize + 1
}

/// Rows the original's oblique drawing of the gate climbs per 2 image columns (rails and bar tips rise 1 row every 2 px).
pub const GATE_SLOPE: usize = 2;

/// The whole gate for rise `rise` as the original composes it (back pass, then the front tips on top): the 252 slices
/// stacked upwards from the tips, cut at 60 px over the floor line (the partial slices 253..259 are its lower rows), tips
/// 251 and the front tips 250 (with the post stripe) at the bottom. The source is an oblique drawing (image x is the corridor
/// depth, going right = back = up by 1 row per `GATE_SLOPE` px), so every column is moved down by x / GATE_SLOPE rows: the
/// rails become horizontal and the bar tips end on one line. Bottom edge `rise - 1` px over the floor line, top edge 60 px:
/// (w, h, pixels).
pub fn gate_sprite(back: &image::Image, slice: &image::Image, front: &image::Image, rise: usize) -> (usize, usize, Vec<u8>) {
    let w = slice.width;
    let hc = 61usize.saturating_sub(rise).max(front.height);
    let skew = w / GATE_SLOPE; // rows the last column drops (plus one for the tip line)
    let he = hc + skew;
    let mut e = vec![0u8; w * he];
    let put = |e: &mut [u8], im: &image::Image, bottom: usize, over: bool| {
        for (i, &p) in im.pixels.iter().enumerate().filter(|(_, &p)| p != 0 || !over) {
            e[(bottom - im.height + i / im.width) * w + i % im.width] = p;
        }
    };
    put(&mut e, back, he - 2, false);
    let mut top = he - front.height;
    while top > 0 {
        let n = top.min(slice.height);
        e[(top - n) * w..top * w].copy_from_slice(&slice.pixels[(slice.height - n) * w..slice.height * w]);
        top -= n;
    }
    put(&mut e, front, he, true);
    // the lowest tip (bar column 2, the nearest) ends on the last row; the post stripe's lower rows are cut
    let mut px = vec![0u8; w * hc];
    for j in 0..hc {
        for x in 0..w {
            let y = (j + skew) as isize - (x / GATE_SLOPE) as isize - 1;
            if y >= 0 && (y as usize) < he {
                px[j * w + x] = e[y as usize * w + x];
            }
        }
    }
    (w, hc, px)
}

/// Every 8-connected component of the image moved down until it touches the bottom row and stood upright (the 2D spikes
/// stand higher the further back they are and lean; in 3D they all stand on the floor, their depth is placed by room.rs).
pub fn settle(w: usize, h: usize, px: &[u8]) -> Vec<u8> {
    let mut comp = vec![usize::MAX; w * h];
    let mut out = vec![0u8; w * h];
    for s in (0..w * h).filter(|&i| px[i] != 0) {
        if comp[s] != usize::MAX {
            continue;
        }
        let (mut stack, mut cells) = (vec![s], vec![]);
        comp[s] = s;
        while let Some(i) = stack.pop() {
            cells.push(i);
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < w as isize && ny < h as isize {
                    let n = ny as usize * w + nx as usize;
                    if px[n] != 0 && comp[n] == usize::MAX {
                        comp[n] = s;
                        stack.push(n);
                    }
                }
            }
        }
        let bottom = cells.iter().map(|&i| i / w).max().unwrap_or(h - 1);
        // the 2D spikes lean (perspective); stood upright: every row of a one-run-per-row component is shifted so its centre
        // is over the bottom row's centre
        let centre = |y: usize| {
            let xs: Vec<usize> = cells.iter().filter(|&&i| i / w == y).map(|&i| i % w).collect();
            let (a, b) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
            ((a + b) as f32 / 2.0, b + 1 - a == xs.len())
        };
        let rows: Vec<usize> = (0..h).filter(|&y| cells.iter().any(|&i| i / w == y)).collect();
        let upright = rows.iter().all(|&y| centre(y).1);
        let cb = centre(bottom).0;
        for &i in &cells {
            let (x, y) = (i % w, i / w);
            let dx = if upright { (cb - centre(y).0).round() as isize } else { 0 };
            let nx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
            out[(y + h - 1 - bottom) * w + nx] = px[i];
        }
    }
    out
}

/// The back spike frame without the spikes the fore frame repeats (bottom-aligned, same x): what stands behind the figures.
pub fn spike_back_only(back: &image::Image, fore: &image::Image) -> Vec<u8> {
    let dy = back.height as isize - fore.height as isize;
    (0..back.width * back.height)
        .map(|i| {
            let (x, y) = (i % back.width, (i / back.width) as isize - dy);
            let f = if x < fore.width && y >= 0 && (y as usize) < fore.height { fore.pixels[y as usize * fore.width + x] } else { 0 };
            if f != 0 { 0 } else { back.pixels[i] }
        })
        .collect()
}

/// Images stacked top to bottom (each keeps its first `rows`), left-aligned, as wide as the widest: (w, h, pixels).
pub fn stack(parts: &[(&image::Image, usize)]) -> (usize, usize, Vec<u8>) {
    let w = parts.iter().map(|p| p.0.width).max().unwrap_or(0);
    let mut px = vec![];
    for &(im, rows) in parts {
        for y in 0..rows.min(im.height) {
            px.extend_from_slice(&im.pixels[y * im.width..][..im.width]);
            px.resize(px.len() + w - im.width, 0);
        }
    }
    (w, px.len() / w.max(1), px)
}

/// The rubble of a debris floor seen from above: the unsheared floor texture (`tiles::floor_pixels`) of the debris record
/// where it differs from the plain floor's, row 0 = back of the tile, the bottom rows the front lip (w, h, pixels).
pub fn debris_sprite(deb: [&image::Image; 3], floor: [&image::Image; 3]) -> (usize, usize, Vec<u8>) {
    let (w, h, a) = crate::tiles::floor_pixels(deb[0], deb[1], deb[2]);
    let (fw, fh, b) = crate::tiles::floor_pixels(floor[0], floor[1], floor[2]);
    let px = (0..w * h).map(|i| if (fw, fh) == (w, h) && a[i] == b[i] { 0 } else { a[i] }).collect();
    (w, h, px)
}


/// Build the voxel objects of one set from its DAT (`pal` = palette 200 colours). Returns them and the face count.
fn load_objects(dat: &Dat, pal: &[[u8; 3]], meshes: &mut Assets<Mesh>) -> (Objects, usize) {
    use crate::tiles::{floor_rows, Role, EXIT_FRAME_H};
    let img = |id: u16| dat.get(id).and_then(image::decode).filter(|i| i.bpp == 4);
    let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
    let mut faces = 0;
    let mut add = |v: VoxelModel| {
        let mesh = v.mesh(|p| {
            let [r, g, b] = rgb(p);
            Color::srgb_u8(r, g, b).to_linear().to_f32_array()
        });
        faces += mesh.count_vertices() / 4;
        Model { mesh: meshes.add(mesh), w: v.w, h: v.h }
    };
    let mut o = Objects::default();
    // spikes: back frames without the fore spikes, right and fore frames; each spike stands on the bottom row
    for k in 0..5u16 {
        let (back, right, fore) = (img(1328 + k), img(1334 + k), img(1339 + k));
        if let (Some(b), Some(f)) = (&back, &fore) {
            o.ids.insert(1328 + k, add(VoxelModel::revolve_runs(b.width, b.height, &settle(b.width, b.height, &spike_back_only(b, f)))));
        }
        for (id, im) in [(1334 + k, right), (1339 + k, fore)] {
            if let Some(im) = im {
                o.ids.insert(id, add(VoxelModel::revolve_runs(im.width, im.height, &settle(im.width, im.height, &im.pixels))));
            }
        }
    }
    // chomper blades (floor rows cut) and upper teeth: thin bevelled plates standing diagonally through the depth
    for id in (1301..=1305).chain(1311..=1313) {
        let Some(im) = img(id) else { continue };
        let h = im.height - if id <= 1305 { floor_rows(Role::JawBack, im.height) } else { 0 };
        o.ids.insert(id, add(VoxelModel::plate(im.width, h, &im.pixels[..im.width * h], 1.0, |_| 0).sheared(JAW_SHEAR, JAW_XREF)));
    }
    // torch holder: the rod through its cup, revolved
    if let Some(im) = img(346) {
        let (h, px, axis, _) = column_sprite(&im);
        o.ids.insert(346, add(VoxelModel::revolve(im.width, h, &px, axis)));
        o.holder_axis = axis;
    }
    // exit frames with their lintels on top: thick reliefs, the brighter ornament standing out
    let relief = |n: f32| move |p: u8| (crate::voxel::luma(rgb(p)) * n) as usize;
    for (i, (frame, lintel)) in [(237, 238), (239, 240)].into_iter().enumerate() {
        if let (Some(f), Some(l)) = (img(frame), img(lintel)) {
            let (w, h, px) = stack(&[(&l, l.height), (&f, EXIT_FRAME_H)]);
            o.exit[i] = Some(add(VoxelModel::plate(w, h, &px, 3.0, relief(2.5))));
        }
    }
    if let (Some(s), Some(b)) = (img(233), img(234)) {
        o.panels = (0..=panel_slices(0))
            .map(|n| {
                let mut parts = vec![(&s, s.height); n];
                parts.push((&b, b.height));
                let (w, h, px) = stack(&parts);
                add(VoxelModel::plate(w, h, &px, 1.0, relief(1.5)))
            })
            .collect();
    }
    // gate: one model per rise (the stack is cut at the cell top)
    if let (Some(t), Some(s), Some(f)) = (img(251), img(252), img(250)) {
        o.gate = (1..=48).map(|r| { let (w, h, px) = gate_sprite(&t, &s, &f, r); add(VoxelModel::plate(w, h, &px, 1.0, |_| 0)) }).collect();
    }
    o.stairs = img(344).map(|s| add(VoxelModel::plate(s.width, s.height, &s.pixels, 2.0, |_| 0)));
    // debris: the rubble as a heap on the floor (seen from above; room.rs lays the model down)
    if let (Some(a), Some(b), Some(c), Some(f0), Some(f1)) = (img(297), img(298), img(243), img(241), img(242)) {
        let (w, h, px) = debris_sprite([&a, &b, &c], [&f0, &f1, &c]);
        if px.iter().any(|&p| p != 0) {
            o.debris = Some(add(VoxelModel::extrude(w, h, &px, 0.12, |_, dt| ((dt - 1.0).clamp(0.0, 3.0).round() as usize, 0))));
        }
    }
    (o, faces)
}

/// A voxel column: mesh (origin = image bottom-left of the kept rows, one unit = one pixel), its rows and the mesh z of
/// its axis (voxels).
#[derive(Clone)]
pub struct Column {
    pub mesh: Handle<Mesh>,
    pub h: usize,
    pub az: f32,
}

/// The column images (both sets): pillar 295 (Pillar, LatticePillar), big pillar bottom 1288 and top 1291, door post 249
/// (Gate, Doortop). Their side pieces 292 / 293 / 294, 1286 / 1287 / 1289 / 1290 are 2D perspective parts of the same
/// column (the room keeps not drawing them).
pub const COLUMN_IDS: [u16; 4] = [295, 1288, 1291, 249];

/// A column image prepared for `VoxelModel::revolve`: bottom rows that reach the image's left edge are floor (the column
/// stands on the 3D slab) and are cut; per row only the opaque run through the shaft axis is kept (stray bits beside it,
/// e.g. a lattice end, are dropped). The axis is the most common row centre. Returns (rows kept, pixels, axis px, number
/// of pixels dropped from the kept rows).
pub fn column_sprite(im: &image::Image) -> (usize, Vec<u8>, f32, usize) {
    let w = im.width;
    let row = |y: usize| &im.pixels[y * w..][..w];
    let mut h = im.height;
    while h > 1 && row(h - 1)[0] != 0 {
        h -= 1;
    }
    let mut count = vec![0usize; 2 * w + 1];
    for y in 0..h {
        let r = row(y);
        if let (Some(a), Some(b)) = (r.iter().position(|&p| p != 0), r.iter().rposition(|&p| p != 0)) {
            count[a + b + 1] += 1;
        }
    }
    let axis = count.iter().enumerate().max_by_key(|&(_, &c)| c).map_or(0, |(i, _)| i) as f32 / 2.0;
    let (mut px, mut dropped) = (vec![0u8; w * h], 0);
    for y in 0..h {
        let r = row(y);
        let p = ((axis - 0.5).floor().max(0.0) as usize).min(w - 1);
        let Some(c) = [p, p + 1].into_iter().find(|&c| c < w && r[c] != 0) else {
            dropped += r.iter().filter(|&&p| p != 0).count();
            continue;
        };
        let (mut a, mut b) = (c, c);
        while a > 0 && r[a - 1] != 0 {
            a -= 1;
        }
        while b + 1 < w && r[b + 1] != 0 {
            b += 1;
        }
        px[y * w + a..=y * w + b].copy_from_slice(&r[a..=b]);
        dropped += r.iter().filter(|&&p| p != 0).count() - (b + 1 - a);
    }
    (h, px, axis, dropped)
}

/// Image column where the front face of a block column ends and its side face starts: the boundary with the most rows
/// changing colour across it.
pub fn block_split(im: &image::Image) -> usize {
    let w = im.width;
    (1..w).max_by_key(|&c| (0..im.height).filter(|&y| { let r = &im.pixels[y * w..]; r[c - 1] != 0 && r[c] != 0 && r[c - 1] != r[c] }).count()).unwrap_or(w)
}

/// The voxel model of a column image: a square block when its top row reaches the left edge (only the dungeon pillar 295:
/// a front face of one colour band and a side face of another, the 2D view of a square stone pillar), else round.
pub fn column_model(im: &image::Image) -> VoxelModel {
    if im.pixels[0] != 0 {
        VoxelModel::block(im.width, im.height, &im.pixels, block_split(im))
    } else {
        let (h, px, axis, _) = column_sprite(im);
        VoxelModel::revolve(im.width, h, &px, axis)
    }
}

/// Potion kind (modifier >> 5) -> (big bottle, bubble colour 0 red / 1 green / 2 blue; None = empty, no bubbles).
/// Traced in PRINCE.EXE (docs/texture_inventory.md 16): draw_tile_fore `AA45` big bottle for mod & 0xE0 in 0x40..0x9F
/// (kinds 2..4); draw_tile_anim `A857` no bubbles for kind 0, colour EGA 12 (kinds 1, 2, 7), 10 (3, 4), 9 (5, 6).
pub fn potion_look(kind: u8) -> (bool, Option<usize>) {
    ((2..5).contains(&kind), match kind { 0 => None, 3 | 4 => Some(1), 5 | 6 => Some(2), _ => Some(0) })
}

/// PRINCE.DAT bottle image: chtab 1 image 12 small / 13 big, + 2 on palace levels (`AA8E`: tbl_level_type ds:01CE).
pub fn bottle_image(kind: u8, palace: bool) -> u16 {
    162 + potion_look(kind).0 as u16 + 2 * palace as u16
}

/// Bottle image left edge in the tile (fore_x 2 bytes + xl 6, `AA9D`) and the bubbles' offset from it (xh + 3, xl 1,
/// `A884`), screen px. The bubbles' bottom row is 11 + 4 * big px above the bottle's (y main_y - 14 - 4 * big against
/// main_y - 3), i.e. right on top of the 11 / 15-px bottle.
pub const POTION_X: f32 = 22.0;
pub const BUBBLE_DX: f32 = 3.0;

/// Linear brightness factor of the flame voxels (unlit): above 1, so the flames read as light and bloom.
pub const FLAME_GAIN: f32 = 1.5;

/// Bubble colours: EGA 12 / 10 / 9 (light red, green, blue), the colours of the mono blit (blitter 0x40 + colour, `A8D7`).
pub const GLOW: [[u8; 3]; 3] = [[255, 85, 85], [85, 255, 85], [85, 85, 255]];

/// The skeleton as drawn on screen without its floor: the skeleton floor (`skel` = [base, right piece]) and the plain floor
/// (`floor` = [241, 242]) are each composed as the original draws them (base in the tile's column, right piece in the next,
/// both bottom-aligned); pixels where they differ are the bones. Cropped to the bones' bounding box: (w, h, pixels).
pub fn bone_sprite(skel: [&image::Image; 2], floor: [&image::Image; 2]) -> (usize, usize, Vec<u8>) {
    let w = 32 + skel[1].width.max(floor[1].width);
    let h = skel.iter().chain(&floor).map(|i| i.height).max().unwrap_or(0);
    let compose = |pair: [&image::Image; 2]| {
        let mut c = vec![0u8; w * h];
        for (im, x0) in pair.into_iter().zip([0, 32]) {
            for y in 0..im.height {
                for x in 0..im.width {
                    let p = im.pixels[y * im.width + x];
                    if p != 0 {
                        c[(h - im.height + y) * w + x0 + x] = p;
                    }
                }
            }
        }
        c
    };
    let (s, f) = (compose(skel), compose(floor));
    let px: Vec<u8> = s.iter().zip(&f).map(|(&a, &b)| if a != b { a } else { 0 }).collect();
    let on: Vec<usize> = (0..w * h).filter(|&i| px[i] != 0).collect();
    let Some((x0, x1, y0, y1)) = on.iter().fold(None, |b: Option<(usize, usize, usize, usize)>, &i| {
        let (x, y) = (i % w, i / w);
        Some(b.map_or((x, x, y, y), |(a, c, d, e)| (a.min(x), c.max(x), d.min(y), e.max(y))))
    }) else {
        return (0, 0, vec![]);
    };
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    (cw, ch, (0..cw * ch).map(|i| px[(y0 + i / cw) * w + x0 + i % cw]).collect())
}

/// Centre of the set pixels of an image (x from the left, y from the bottom, in px); the middle of the image if none is set.
fn centroid(w: usize, h: usize, px: &[u8]) -> [f32; 2] {
    let set: Vec<usize> = (0..w * h).filter(|&i| px[i] != 0).collect();
    if set.is_empty() {
        return [w as f32 / 2.0, h as f32 / 2.0];
    }
    let n = set.len() as f32;
    [set.iter().map(|&i| (i % w) as f32 + 0.5).sum::<f32>() / n, h as f32 - set.iter().map(|&i| (i / w) as f32 + 0.5).sum::<f32>() / n]
}

fn model(meshes: &mut Assets<Mesh>, w: usize, h: usize, px: &[u8], pal: &[[u8; 3]]) -> Model {
    let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
    let m = VoxelModel::from_image(w, h, px, false, rgb);
    let mesh = meshes.add(m.mesh(|p| {
        let [r, g, b] = rgb(p);
        Color::srgb_u8(r, g, b).to_linear().to_f32_array()
    }));
    Model { mesh, w, h }
}

impl Items {
    /// Decode and voxelise the item images from the game data folder; logs what is missing.
    /// `columns`: also build the voxel columns (`column_style` = voxel).
    /// `objects`: also build the voxel objects (`object_style` = voxel).
    pub fn load(dir: &str, columns: bool, objects: bool, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Items {
        let skeleton = ["VDUNGEON.DAT", "VPALACE.DAT"].map(|f| {
            let dat = Dat::open(format!("{dir}/{f}")).ok()?;
            let img = |id| dat.get(id).and_then(image::decode).filter(|i| i.bpp == 4);
            let pal = image::palette(dat.get(200)?)?;
            let (s0, s1, f0, f1) = (img(1230)?, img(1231)?, img(241)?, img(242)?);
            let (w, h, px) = bone_sprite([&s0, &s1], [&f0, &f1]);
            (w > 0).then(|| model(meshes, w, h, &px, &pal))
        });
        let prince = Dat::open(format!("{dir}/PRINCE.DAT")).ok();
        let pimg = |id| prince.as_ref().and_then(|d| d.get(id)).and_then(image::decode);
        let pal = prince.as_ref().and_then(|d| d.get(150)).and_then(image::palette).unwrap_or_default();
        // kind 1 small, kind 2 big
        let ims = [false, true].map(|palace| [1, 2].map(|k| pimg(bottle_image(k, palace))));
        let bottles = (!pal.is_empty() && ims.iter().flatten().all(Option::is_some)).then(|| ims.map(|set| set.map(|i| { let i = i.unwrap(); model(meshes, i.width, i.height, &i.pixels, &pal) })));
        // 1-bit frames: every set pixel is index 1, coloured by the material
        let bubble_imgs: Vec<_> = (166..=172).filter_map(pimg).collect();
        let bubble_at: Vec<[f32; 2]> = bubble_imgs.iter().map(|i| centroid(i.width, i.height, &i.pixels)).collect();
        let bubbles: Vec<Model> = bubble_imgs.iter().map(|i| model(meshes, i.width, i.height, &i.pixels, &[[0; 3], [255; 3]])).collect();
        // A910: chtab 1 image 10 + glint, 32 px wide, bottom 3 px above the tile's floor line, at the tile's left edge
        let sword: Vec<Model> = [160, 161].into_iter().filter_map(pimg).filter(|i| !pal.is_empty() && i.bpp == 4).map(|i| model(meshes, i.width, i.height, &i.pixels, &pal)).collect();
        for (what, ok) in [("floor sword (PRINCE.DAT 160/161)", sword.len() == 2), ("dungeon skeleton (VDUNGEON 1230/1231)", skeleton[0].is_some()), ("potion bottles (PRINCE.DAT 162..165)", bottles.is_some()), ("potion bubbles (PRINCE.DAT 166..172)", bubbles.len() == 7)] {
            if !ok {
                info!("items: {what} not found, procedural look used");
            }
        }
        let t0 = std::time::Instant::now();
        let mut faces = 0;
        let columns = ["VDUNGEON.DAT", "VPALACE.DAT"].map(|f| {
            let mut m = HashMap::new();
            let Some(dat) = Dat::open(format!("{dir}/{f}")).ok().filter(|_| columns) else { return m };
            let pal = dat.get(200).and_then(image::palette).unwrap_or_default();
            for id in COLUMN_IDS {
                let Some(im) = dat.get(id).and_then(image::decode).filter(|i| i.bpp == 4) else { continue };
                let v = column_model(&im);
                let mesh = v.mesh(|p| {
                    let [r, g, b] = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
                    Color::srgb_u8(r, g, b).to_linear().to_f32_array()
                });
                faces += mesh.count_vertices() / 4;
                m.insert(id, Column { mesh: meshes.add(mesh), h: v.h, az: v.axis_z() });
            }
            m
        });
        if columns.iter().any(|m| !m.is_empty()) {
            info!("items: {} voxel columns built in {:.1} ms, {faces} faces", columns[0].len() + columns[1].len(), t0.elapsed().as_secs_f32() * 1000.0);
        }
        let t0 = std::time::Instant::now();
        let mut faces = 0;
        let objects = ["VDUNGEON.DAT", "VPALACE.DAT"].map(|f| {
            let Some(dat) = Dat::open(format!("{dir}/{f}")).ok().filter(|_| objects) else { return Objects::default() };
            let (o, n) = load_objects(&dat, &dat.get(200).and_then(image::palette).unwrap_or_default(), meshes);
            faces += n;
            o
        });
        // flames: palette colours overbright on an unlit material, so they glow (and bloom in enhanced mode)
        let mut flame_px = vec![];
        let mut flame_at = vec![];
        let flames: Vec<Model> = if objects.iter().any(|o| !o.ids.is_empty()) {
            (151..=159)
                .filter_map(pimg)
                .map(|i| {
                    flame_px.push(i.pixels.iter().filter(|&&p| p != 0).count());
                    flame_at.push(centroid(i.width, i.height, &i.pixels));
                    let m = VoxelModel::revolve_runs(i.width, i.height, &i.pixels);
                    let mesh = m.mesh(|p| {
                        let [r, g, b] = pal.get(p as usize).copied().unwrap_or([255, 160, 0]);
                        (Color::srgb_u8(r, g, b).to_linear() * FLAME_GAIN).with_alpha(1.0).to_f32_array()
                    });
                    faces += mesh.count_vertices() / 4;
                    Model { mesh: meshes.add(mesh), w: i.width, h: i.height }
                })
                .collect()
        } else {
            vec![]
        };
        if objects.iter().any(|o| !o.ids.is_empty()) {
            let n: usize = objects.iter().map(|o| o.ids.len() + o.exit.iter().flatten().count() + o.panels.len() + o.stairs.is_some() as usize + o.debris.is_some() as usize).sum();
            info!("items: {} voxel objects and {} flame frames built in {:.1} ms, {faces} faces", n, flames.len(), t0.elapsed().as_secs_f32() * 1000.0);
            if flames.len() != 9 {
                info!("items: torch flames (PRINCE.DAT 151..159) not found, flame box used");
            }
        }
        let glow = GLOW.map(|[r, g, b]| {
            let c = Color::srgb_u8(r, g, b);
            mats.add(StandardMaterial { base_color: c, emissive: c.to_linear() * 3.0, perceptual_roughness: 0.4, ..default() })
        });
        Items { mat: mats.add(StandardMaterial { perceptual_roughness: 0.6, ..default() }), steel: mats.add(StandardMaterial { perceptual_roughness: 0.6, ..default() }), skeleton, bottles, bubble_at: if bubbles.len() == 7 { bubble_at } else { vec![] }, bubbles: if bubbles.len() == 7 { bubbles } else { vec![] }, sword: if sword.len() == 2 { sword } else { vec![] }, glow, columns, objects, flame_px: if flames.len() == 9 { flame_px } else { vec![] }, flame_at: if flames.len() == 9 { flame_at } else { vec![] }, flames: if flames.len() == 9 { flames } else { vec![] }, flame_mat: mats.add(StandardMaterial { unlit: true, ..default() }) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, px: &[u8]) -> image::Image {
        image::Image { width: w, height: h, bpp: 4, pixels: px.to_vec() }
    }

    /// Gate rise per modifier (closed 0 -> 1 px, open 188 and 238 / 255 -> 48 px) and the sprite: slices stacked on the tips,
    /// cut at 60 px over the floor line, no gap in the bars.
    #[test]
    fn gate_rise_and_stack() {
        assert_eq!([0, 3, 4, 187, 188, 238, 255].map(gate_rise), [1, 1, 2, 47, 48, 48, 48]);
        let back = img(16, 10, &[9; 160]);
        let front = img(24, 12, &[7; 288]);
        let slice = img(24, 8, &(0..8).flat_map(|r| [r as u8 + 1; 24]).collect::<Vec<_>>());
        for rise in [1, 12, 30, 48] {
            let (w, h, px) = gate_sprite(&back, &slice, &front, rise);
            assert_eq!((w, h), (24, 61 - rise), "bottom edge rise - 1, top edge 60");
            assert!((0..h).all(|y| px[y * w] != 0), "column 0 filled to the top and bottom");
            assert!((0..h).all(|y| px[y * w + 23] != 0), "last column filled to the top");
        }
    }

    /// Un-sheared real gate: rails lie on constant rows across the depth (every 8 px), the bar tips on one line (skips
    /// without Original_data).
    #[test]
    fn real_gate_rails_are_horizontal() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        for name in ["VDUNGEON.DAT", "VPALACE.DAT"] {
            let Ok(d) = Dat::open(format!("{dir}/{name}")) else { return };
            let get = |id| d.get(id).and_then(image::decode).unwrap();
            let (w, h, px) = gate_sprite(&get(251), &get(252), &get(250), 1);
            let bar = |x: usize| matches!(x, 2 | 3 | 8 | 9 | 14 | 15) || x >= 18;
            let mut rows = std::collections::BTreeSet::new();
            for y in 0..h - 12 {
                for x in (0..18).filter(|&x| !bar(x)) {
                    if px[y * w + x] != 0 { rows.insert(y % 8); }
                }
            }
            assert!(rows.len() <= 3, "{name}: rail pixels on rows {rows:?} (mod 8)");
            let tip = |x: usize| (0..h).rev().find(|&y| px[y * w + x] != 0);
            let t = tip(2);
            assert!(t >= Some(h - 2) && [tip(8), tip(14)] == [t; 2], "{name}: tips on one line (at the floor)");
        }
    }

    /// The original's partial slices 253..259 are the lower rows of 252 and the 250 tips are 251 plus the post (skips without
    /// Original_data).
    #[test]
    fn gate_images_match_the_stack_rule() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        for name in ["VDUNGEON.DAT", "VPALACE.DAT"] {
            let Ok(d) = Dat::open(format!("{dir}/{name}")) else { return };
            let get = |id| d.get(id).and_then(image::decode).unwrap();
            let s = get(252);
            assert_eq!((s.width, s.height), (24, 8));
            for n in 1..8usize {
                let p = get(260 - n as u16);
                assert_eq!((p.width, p.height), (24, n), "{name} {}", 260 - n);
                assert_eq!(p.pixels, s.pixels[(8 - n) * 24..], "{name} partial slice of {n} rows");
            }
            let (f, t) = (get(250), get(251));
            assert_eq!((f.height, t.height), (12, 10));
            for y in 0..10 {
                assert!((0..t.width).all(|x| t.pixels[y * t.width + x] == 0 || t.pixels[y * t.width + x] == f.pixels[y * f.width + x]), "{name} row {y}");
            }
        }
    }

    /// The table traced in PRINCE.EXE (docs/texture_inventory.md 16): kind -> (dungeon bottle, palace bottle, EGA bubble
    /// colour; 0 = no bubbles).
    #[test]
    fn potion_kinds() {
        const EGA: [u8; 3] = [12, 10, 9];
        let want = [(162, 164, 0), (162, 164, 12), (163, 165, 12), (163, 165, 10), (163, 165, 10), (162, 164, 9), (162, 164, 9), (162, 164, 12)];
        for (k, w) in want.into_iter().enumerate() {
            let k = k as u8;
            assert_eq!((bottle_image(k, false), bottle_image(k, true), potion_look(k).1.map_or(0, |c| EGA[c])), w, "kind {k}");
        }
        // GLOW = the standard EGA colours 12, 10, 9 (6-bit 63/21 -> 255/85)
        assert_eq!(GLOW, [[255, 85, 85], [85, 255, 85], [85, 85, 255]]);
    }

    /// Every potion in levels 1..14 has a valid look; the bubbles sit right on top of the bottle (bottle heights 11 / 15 =
    /// the original's 11 + 4 * big), bubble frames 5 px wide (skips without Original_data).
    #[test]
    fn level_potions_map_to_images() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        let (Ok(lv), Ok(p)) = (Dat::open(format!("{dir}/LEVELS.DAT")), Dat::open(format!("{dir}/PRINCE.DAT"))) else { return };
        let mut seen = [0usize; 8];
        for n in 1..=14usize {
            let l = pop_core::level::Level::parse(lv.get(2000 + n as u16).unwrap()).unwrap();
            for t in (1..=24).flat_map(|r| (0..30).map(move |i| (r, i))).map(|(r, i)| l.tile_at(r, i)).filter(|t| t.kind == pop_core::level::Tile::Potion) {
                let k = t.modifier >> 5;
                seen[k as usize] += 1;
                let im = image::decode(p.get(bottle_image(k, crate::kid::palace(n))).unwrap()).unwrap();
                assert_eq!((im.width, im.height), (10, 11 + 4 * potion_look(k).0 as usize), "level {n} kind {k}");
            }
        }
        // heal, life, feather, invert and hurt all occur (level 14's leftover pre-shifted bytes load as empty, kind 0)
        assert!(seen[1..=5].iter().all(|&c| c > 0) && seen[6..].iter().all(|&c| c == 0), "{seen:?}");
        for id in 166..=172 {
            assert_eq!(image::decode(p.get(id).unwrap()).unwrap().width, 5);
        }
    }

    #[test]
    fn bones_are_the_difference_to_the_plain_floor() {
        // floor: 1 everywhere; skeleton: a 9 in the base (taller image, bottom-aligned) and one in the right piece
        let floor = [img(32, 1, &[1; 32]), img(2, 1, &[1, 1])];
        let mut base = vec![0u8; 64];
        base[3] = 9; // row 0 of a 2-row image: above the floor row
        base[32..].copy_from_slice(&[1; 32]);
        let skel = [img(32, 2, &base), img(2, 1, &[1, 7])];
        assert_eq!(bone_sprite([&skel[0], &skel[1]], [&floor[0], &floor[1]]), (31, 2, {
            let mut v = vec![0u8; 62];
            v[0] = 9;
            v[61] = 7;
            v
        }));
        assert_eq!(bone_sprite([&floor[0], &floor[1]], [&floor[0], &floor[1]]).0, 0);
    }

    /// The item models' front projection is the original image (skips without Original_data).
    #[test]
    fn item_front_projection_equals_sprite() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        let (Ok(v), Ok(p)) = (Dat::open(format!("{dir}/VDUNGEON.DAT")), Dat::open(format!("{dir}/PRINCE.DAT"))) else { return };
        let i = |d: &Dat, id| image::decode(d.get(id).unwrap()).unwrap();
        let (w, h, px) = bone_sprite([&i(&v, 1230), &i(&v, 1231)], [&i(&v, 241), &i(&v, 242)]);
        assert!(w > 20 && h > 10, "bones {w}x{h}");
        assert_eq!(VoxelModel::from_image(w, h, &px, false, |c| [c.wrapping_mul(16); 3]).front(), px);
        for id in [162, 163, 166, 167, 168, 169, 170, 171, 172] {
            let im = i(&p, id);
            assert_eq!(VoxelModel::from_image(im.width, im.height, &im.pixels, false, |c| [c.wrapping_mul(16); 3]).front(), im.pixels, "PRINCE {id}");
        }
    }

    #[test]
    fn object_tables() {
        // exit panel: 7 slices on the bottom piece when closed, none when open (the bottom piece stays, behind the lintel)
        assert_eq!([0, 5, 6, 35, 40, 41, 43, 255].map(panel_slices), [7, 6, 6, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn spikes_settle_and_lose_the_fore_spikes() {
        // two spikes: one ending a row above the bottom, one on it; the lifted one drops, the other stays
        let px = [0, 7, 0, 0, 0, 7, 0, 7, 0, 0, 0, 8];
        assert_eq!(settle(4, 3, &px), vec![0, 0, 0, 0, 0, 7, 0, 7, 0, 7, 0, 8]);
        // a leaning spike is stood upright over its base
        assert_eq!(settle(3, 3, &[0, 0, 7, 0, 7, 0, 8, 0, 0]), vec![7, 0, 0, 7, 0, 0, 8, 0, 0]);
        let back = img(4, 3, &[7, 0, 0, 7, 7, 0, 0, 7, 8, 0, 0, 8]);
        let fore = img(1, 2, &[7, 8]); // the left spike's lower two rows, bottom-aligned
        assert_eq!(spike_back_only(&back, &fore), vec![7, 0, 0, 7, 0, 0, 0, 7, 0, 0, 0, 8]);
        let (w, h, s) = stack(&[(&img(2, 2, &[1, 1, 2, 2]), 1), (&img(3, 1, &[3, 3, 3]), 1)]);
        assert_eq!((w, h, s), (3, 2, vec![1, 1, 0, 3, 3, 3]));
    }

    #[test]
    fn rods_plates_and_shear_keep_the_front() {
        // a 3-px run is as deep as wide (round), a 1-px run one... two layers (plane pair)
        let px = [0, 5, 5, 5, 0, 0, 0, 6, 0, 0];
        let m = VoxelModel::revolve_runs(5, 2, &px);
        assert_eq!(m.front(), px);
        let depth = |m: &VoxelModel, x, y| (0..m.d as isize).filter(|&z| m.get(x, y, z) != 0).count();
        assert!(depth(&m, 2, 0) >= 3 && depth(&m, 1, 0) <= depth(&m, 2, 0) && depth(&m, 2, 1) <= 2);
        // plate: rim one voxel, inside 1 + 2 * cap; relief adds in front
        let p: Vec<u8> = vec![4; 25];
        let pl = VoxelModel::plate(5, 5, &p, 1.0, |_| 1);
        assert_eq!(pl.front(), p);
        assert_eq!((depth(&pl, 0, 0), depth(&pl, 2, 2)), (2, 4));
        // shear: the left column moves forward, the front view is unchanged
        let sh = pl.sheared(1.0, 4.0);
        assert_eq!(sh.front(), p);
        let zmax = |m: &VoxelModel, x| (0..m.d as isize).filter(|&z| m.get(x, 2, z) != 0).max().unwrap();
        assert_eq!(zmax(&sh, 0) - zmax(&sh, 4), 4);
    }

    /// The object models' front projection is the (prepared) original image, both sets: spikes (settled), chomper blades
    /// (floor rows cut) and teeth, torch holder, exit frames with lintels, door panels, stairs, debris heap (top view) and
    /// the flames; prints counts and faces (skips without Original_data).
    #[test]
    fn object_front_projection_equals_sprite() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        let Ok(prince) = Dat::open(format!("{dir}/PRINCE.DAT")) else { return };
        for id in 151..=159 {
            let im = image::decode(prince.get(id).unwrap()).unwrap();
            assert_eq!(VoxelModel::revolve_runs(im.width, im.height, &im.pixels).front(), im.pixels, "flame {id}");
        }
        for f in ["VDUNGEON.DAT", "VPALACE.DAT"] {
            let Ok(dat) = Dat::open(format!("{dir}/{f}")) else { return };
            let i = |id| image::decode(dat.get(id).unwrap()).unwrap();
            let (mut n, mut faces) = (0, 0);
            let mut check = |m: VoxelModel, want: &[u8], what: String| {
                assert_eq!(m.front(), want, "{f} {what}");
                faces += m.mesh(|_| [1.0; 4]).count_vertices() / 4;
                n += 1;
            };
            for k in 0..5 {
                let (b, r, fo) = (i(1328 + k), i(1334 + k), i(1339 + k));
                let s = settle(b.width, b.height, &spike_back_only(&b, &fo));
                assert!(s.iter().any(|&p| p != 0), "{f} {}: back frame has spikes of its own", 1328 + k);
                check(VoxelModel::revolve_runs(b.width, b.height, &s), &s, format!("spike {}", 1328 + k));
                for im in [r, fo] {
                    let s = settle(im.width, im.height, &im.pixels);
                    // settling keeps every pixel and puts something on the bottom row
                    assert_eq!(s.iter().filter(|&&p| p != 0).count(), im.pixels.iter().filter(|&&p| p != 0).count());
                    assert!(s[(im.height - 1) * im.width..].iter().any(|&p| p != 0));
                    check(VoxelModel::revolve_runs(im.width, im.height, &s), &s, "spike".into());
                }
            }
            for id in (1301..=1305).chain(1311..=1313) {
                let im = i(id);
                let h = im.height - if id <= 1305 { crate::tiles::floor_rows(crate::tiles::Role::JawBack, im.height) } else { 0 };
                let px = &im.pixels[..im.width * h];
                check(VoxelModel::plate(im.width, h, px, 1.0, |_| 0).sheared(JAW_SHEAR, JAW_XREF), px, format!("chomper {id}"));
            }
            let (h, px, axis, dropped) = column_sprite(&i(346));
            // the palace holder drops its lattice strip beside the rod (74 px; docs 13 called it doubtful)
            assert!(dropped < 100 && (14.0..=17.0).contains(&axis), "{f} holder axis {axis} dropped {dropped}");
            check(VoxelModel::revolve(32, h, &px, axis), &px, "holder".into());
            for (fr, li) in [(237, 238), (239, 240)] {
                let (w, h, px) = stack(&[(&i(li), i(li).height), (&i(fr), crate::tiles::EXIT_FRAME_H)]);
                assert!(h >= 60 && w >= 26);
                check(VoxelModel::plate(w, h, &px, 3.0, |p| p as usize % 3), &px, format!("exit {fr}"));
            }
            let (s, b) = (i(233), i(234));
            let (w, h, px) = stack(&[(&s, 6), (&s, 6), (&b, 6)]);
            check(VoxelModel::plate(w, h, &px, 1.0, |_| 1), &px, "panel".into());
            let st = i(344);
            check(VoxelModel::plate(st.width, st.height, &st.pixels, 2.0, |_| 0), &st.pixels, "stairs".into());
            let (w, h, px) = debris_sprite([&i(297), &i(298), &i(243)], [&i(241), &i(242), &i(243)]);
            assert!(px.iter().filter(|&&p| p != 0).count() > 40, "{f}: rubble pixels");
            check(VoxelModel::extrude(w, h, &px, 0.1, |_, dt| ((dt - 1.0).clamp(0.0, 3.0) as usize, 0)), &px, "debris".into());
            println!("{f}: {n} object models checked, {faces} faces");
        }
    }

    /// Columns: round rows are as deep as wide, the front projection is the image (floor rows and stray bits removed), the
    /// block's front is the image left of the split and its right side the part right of it.
    #[test]
    fn column_revolve_and_block_on_small_images() {
        // 7 wide: a 5-px shaft (x 1..5) under a 7-px capital; bottom row is floor (reaches x = 0) with a stray bit
        let mut px = vec![0u8; 7 * 6];
        px[..7].copy_from_slice(&[1; 7]);
        for y in 1..5 {
            px[y * 7 + 1..y * 7 + 6].copy_from_slice(&[2, 3, 4, 5, 6]);
        }
        px[2 * 7 + 6] = 9; // next to the shaft run: kept
        px[5 * 7..].copy_from_slice(&[7, 7, 7, 7, 7, 7, 7]);
        px[3 * 7] = 8; // stray pixel, its own run
        px[3 * 7 + 1] = 0;
        let (h, cut, axis, dropped) = column_sprite(&image::Image { width: 7, height: 6, bpp: 4, pixels: px.clone() });
        assert_eq!((h, axis), (5, 3.5), "floor row cut, axis = shaft centre");
        assert_eq!(dropped, 1);
        assert_eq!(cut[3 * 7], 0);
        let m = VoxelModel::revolve(7, h, &cut, axis);
        assert_eq!(m.front(), cut);
        let depth = |x: isize, y: isize| (0..m.d as isize).filter(|&z| m.get(x, y, z) != 0).count();
        assert_eq!(m.d, 8, "capital 7 px wide: 8 layers (radius 3.5, even around the axis)");
        assert!(depth(3, 2) >= 5 && depth(1, 2) <= 4 && depth(3, 0) > depth(3, 2), "round: centre {} edge {}", depth(3, 2), depth(1, 2));
        // block: front 3 px of colour 5, side 2 px of colour 3
        let b = VoxelModel::block(5, 2, &[5, 5, 5, 3, 4, 5, 5, 5, 3, 4], 3);
        assert_eq!((b.w, b.d), (3, 3));
        assert_eq!(b.front(), vec![5; 6]);
        // right side seen from the right, front to back: side colours 3, 4 then the corner
        assert_eq!((0..3).rev().map(|z| b.get(2, 0, z)).collect::<Vec<_>>(), [5, 3, 4]);
        assert_eq!(block_split(&image::Image { width: 5, height: 2, bpp: 4, pixels: vec![5, 5, 5, 3, 4, 5, 5, 5, 3, 3] }), 3);
    }

    /// Every column image of both sets: front projection = the (prepared) original image, few pixels dropped; prints the
    /// build time and mesh sizes (skips without Original_data).
    #[test]
    fn column_front_projection_equals_sprite() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        for f in ["VDUNGEON.DAT", "VPALACE.DAT"] {
            let Ok(dat) = Dat::open(format!("{dir}/{f}")) else { return };
            for id in COLUMN_IDS {
                let im = image::decode(dat.get(id).unwrap()).unwrap();
                let t = std::time::Instant::now();
                let m = column_model(&im);
                let faces = m.mesh(|_| [1.0; 4]).count_vertices() / 4;
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                if im.pixels[0] != 0 {
                    let s = block_split(&im);
                    assert_eq!((f, id, s), ("VDUNGEON.DAT", 295, 13), "only the dungeon pillar is a block");
                    let want: Vec<u8> = im.pixels.chunks(im.width).flat_map(|r| r[..s].to_vec()).collect();
                    assert_eq!(m.front(), want, "{f} {id}");
                    println!("{f} {id}: block {}x{}x{}, {faces} faces, {ms:.2} ms", m.w, m.h, m.d);
                } else {
                    let (h, px, axis, dropped) = column_sprite(&im);
                    assert_eq!(m.front(), px, "{f} {id}");
                    assert!(im.height - h <= 4 && dropped <= 4, "{f} {id}: cut {} rows, dropped {dropped}", im.height - h);
                    println!("{f} {id}: round {}x{}x{} axis {axis}, cut {} rows, dropped {dropped} px, {faces} faces, {ms:.2} ms", m.w, m.h, m.d, im.height - h);
                }
            }
        }
    }

    /// The floor sword images (PRINCE.DAT 160 / 161) are 32x10 and differ only by the glint star (skips without Original_data).
    #[test]
    fn floor_sword_images() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        let Ok(d) = Dat::open(format!("{dir}/PRINCE.DAT")) else { return };
        let (a, b) = (d.get(160).and_then(image::decode).unwrap(), d.get(161).and_then(image::decode).unwrap());
        assert_eq!((a.width, a.height, b.width, b.height), (32, 10, 32, 10));
        assert!(a.pixels.iter().zip(&b.pixels).any(|(x, y)| x != y), "the glint frame differs");
        let m = VoxelModel::from_image(a.width, a.height, &a.pixels, false, |_| [0; 3]);
        assert_eq!(m.front(), a.pixels);
    }
}

#[cfg(test)]
mod centroid_tests {
    use super::centroid;

    #[test]
    fn centroid_is_measured_from_the_bottom_left() {
        // 4 x 3 image: one set pixel in the top row, column 3 -> x 3.5, y 3 - 0.5 = 2.5 from the bottom
        let mut px = [0u8; 12];
        px[3] = 1;
        assert_eq!(centroid(4, 3, &px), [3.5, 2.5]);
        px[8] = 1; // bottom row, column 0: the mean of both
        assert_eq!(centroid(4, 3, &px), [2.0, 1.5]);
        assert_eq!(centroid(4, 3, &[0; 12]), [2.0, 1.5], "no pixel: the middle");
    }
}
