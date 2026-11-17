//! Voxel items on the floor tiles: the skeleton pile (VDUNGEON 1230 / 1231) and the potions (PRINCE.DAT chtab 1: bottles
//! 162 small / 163 big, bubble frames 166..172 drawn in the potion's colour). Built at runtime with the character voxel
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
    /// skeleton pile per set [dungeon, palace] (the palace set has no skeleton images)
    pub skeleton: [Option<Model>; 2],
    /// potion bottles [small, big]
    pub bottles: Option<[Model; 2]>,
    /// bubble frames 1..7 (potion_fram_bubb), one mesh each
    pub bubbles: Vec<Model>,
    /// emissive liquid materials [red, green, blue]
    pub glow: [Handle<StandardMaterial>; 3],
    /// voxel columns per set [dungeon, palace] by image id (COLUMN_IDS); empty with `column_style` = flat
    pub columns: [HashMap<u16, Column>; 2],
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
/// draw_tile_anim / draw_tile_fore as in the DOS game (SDLPoP seg008): big bottle for kinds 2..4; 1 heal and 2 life red,
/// 3 feather and 4 invert green, 5 hurt and 6 open blue. [I] from memory of SDLPoP, not traced in PRINCE.EXE.
pub fn potion_look(kind: u8) -> (bool, Option<usize>) {
    ((2..5).contains(&kind), match kind { 0 => None, 3 | 4 => Some(1), 5 | 6 => Some(2), _ => Some(0) })
}

/// Bubble colours: EGA 12 / 10 / 9 (light red, green, blue), the colours of the mono blit.
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
    pub fn load(dir: &str, columns: bool, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Items {
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
        let bottles = match (pimg(162), pimg(163)) {
            (Some(s), Some(b)) if !pal.is_empty() => Some([s, b].map(|i| model(meshes, i.width, i.height, &i.pixels, &pal))),
            _ => None,
        };
        // 1-bit frames: every set pixel is index 1, coloured by the material
        let bubbles: Vec<Model> = (166..=172).filter_map(pimg).map(|i| model(meshes, i.width, i.height, &i.pixels, &[[0; 3], [255; 3]])).collect();
        for (what, ok) in [("dungeon skeleton (VDUNGEON 1230/1231)", skeleton[0].is_some()), ("potion bottles (PRINCE.DAT 162/163)", bottles.is_some()), ("potion bubbles (PRINCE.DAT 166..172)", bubbles.len() == 7)] {
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
        let glow = GLOW.map(|[r, g, b]| {
            let c = Color::srgb_u8(r, g, b);
            mats.add(StandardMaterial { base_color: c, emissive: c.to_linear() * 3.0, perceptual_roughness: 0.4, ..default() })
        });
        Items { mat: mats.add(StandardMaterial { perceptual_roughness: 0.6, ..default() }), skeleton, bottles, bubbles: if bubbles.len() == 7 { bubbles } else { vec![] }, glow, columns }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, px: &[u8]) -> image::Image {
        image::Image { width: w, height: h, bpp: 4, pixels: px.to_vec() }
    }

    #[test]
    fn potion_kinds() {
        assert_eq!(potion_look(0), (false, None));
        assert_eq!(potion_look(1), (false, Some(0)));
        assert_eq!(potion_look(2), (true, Some(0)));
        assert_eq!(potion_look(3), (true, Some(1)));
        assert_eq!(potion_look(4), (true, Some(1)));
        assert_eq!(potion_look(5), (false, Some(2)));
        assert_eq!(potion_look(6), (false, Some(2)));
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
}
