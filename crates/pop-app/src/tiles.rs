//! Original tile images (VDUNGEON.DAT / VPALACE.DAT) on the 3D room pieces. Which image goes where: docs/texture_inventory.md
//! sections 3 and 5. Decoded at runtime from the user's own DAT files; anything missing falls back to the procedural material.
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::{dat::Dat, image, level::Tile};
use std::collections::HashMap;

/// Resource id of record value `v`: `200 + v` if the set has that image, else `1200 + v` (docs 3, presence rule).
pub fn resolve(v: u8, present: &impl Fn(u16) -> bool) -> Option<u16> {
    [200 + v as u16, 1200 + v as u16].into_iter().find(|&id| present(id))
}

/// Record values (+0 base, +3 right piece, +8 front lip) of the floor of a tile type; None = no textured floor.
fn floor_record(kind: Tile) -> Option<[u8; 3]> {
    use Tile::*;
    Some(match kind {
        Spikes => [127, 133, 43],
        StuckButton => [41, 35, 36],
        DropButton => [41, 42, 96],
        RaiseButton => [147, 42, 149],
        Debris | TorchDebris => [97, 98, 43],
        Skeleton => [30, 31, 43],
        // LooseFloor has no record images and Unknown31 no record; BigPillarTop has no floor
        Empty | Wall | LooseFloor | BigPillarTop | Unknown31 => return None,
        // shortcut: types whose record lacks +0 / +3 or holds chtab-7 values (Mirror, Chomper, Exit, Doortop, Balcony, Lattice)
        // get the plain floor; trace their pieces if they look wrong
        _ => [41, 42, 43],
    })
}

/// Image ids of a tile's textured pieces: (floor [base, right, lip], extra piece). The extra piece is the pillar column,
/// the door post, the exit frame (its lintel is id + 1) or the wall face (chtab 7: 360 + T27BA[modifier], docs 3a).
/// A record whose images are not in the set falls back to the plain floor; a missing extra piece is None.
pub fn tile_images(kind: Tile, modifier: u8, present: impl Fn(u16) -> bool) -> (Option<[u16; 3]>, Option<u16>) {
    use Tile::*;
    let ids = |r: [u8; 3]| -> Option<[u16; 3]> { Some([resolve(r[0], &present)?, resolve(r[1], &present)?, resolve(r[2], &present)?]) };
    let floor = floor_record(kind).and_then(|r| ids(r).or_else(|| ids([41, 42, 43])));
    let extra = match kind {
        Pillar | LatticePillar => resolve(95, &present),
        BigPillarBottom => resolve(88, &present),
        BigPillarTop => resolve(91, &present),
        Gate | DoortopWithFloor | Doortop => resolve(49, &present),
        ExitLeft => resolve(37, &present),
        ExitRight => resolve(39, &present),
        // only modifiers 0 and 1 occur in the levels (T27BA[0] = 8, T27BA[1] = 10)
        Wall => Some(360 + if modifier & 0x7F == 1 { 10 } else { 8 }).filter(|&id| present(id)),
        _ => None,
    };
    (floor, extra)
}

/// The floor texture of one tile (palette indices, `base.width` wide). The original draws the top surface sheared: the base
/// image on the tile and its right piece in the next column, bottom-aligned. Each row is the opaque run of `base` continued
/// by the opaque run of the same row of `right` (minus its end face), cut or padded to the width; the front lip goes below.
pub fn floor_pixels(base: &image::Image, right: &image::Image, lip: &image::Image) -> (usize, usize, Vec<u8>) {
    let w = base.width;
    let dy = right.height.saturating_sub(base.height);
    let mut px = Vec::with_capacity(w * (base.height + lip.height));
    for y in 0..base.height {
        let row = &base.pixels[y * w..][..w];
        let mut run: Vec<u8> = row.iter().position(|&p| p != 0).map_or(vec![], |s| row[s..].to_vec());
        if let Some(r) = right.pixels.chunks(right.width).nth(y + dy) {
            // without its last colour run: that is the sloped end face of the next floor, not top surface
            let r = &r[..r.iter().rposition(|&p| p != 0).map_or(0, |e| e + 1)];
            let end = r.last().map_or(0, |&c| r.iter().rposition(|&p| p != c).map_or(0, |i| i + 1));
            run.extend(&r[..end]);
        }
        let last = run.last().copied().unwrap_or(0);
        run.resize(w, last);
        px.extend(run);
    }
    for r in lip.pixels.chunks(lip.width.max(1)) {
        let mut r = r.to_vec();
        let last = r.last().copied().unwrap_or(0);
        r.resize(w, last);
        px.extend(r);
    }
    (w, base.height + lip.height, px)
}

/// Rows of an exit frame image above its floor (237 is 60 rows, the last 13 are floor); the lintel goes on top of them.
pub const EXIT_FRAME_H: usize = 47;

/// One textured piece: its material and size in screen pixels.
#[derive(Clone)]
pub struct Tex {
    pub mat: Handle<StandardMaterial>,
    pub w: f32,
    pub h: f32,
}

/// The textures of one tile set (dungeon or palace).
pub struct TileSet {
    floors: HashMap<[u16; 3], Handle<StandardMaterial>>,
    imgs: HashMap<u16, Tex>,
    /// image ids of the set that decode as 4-bit images (the 1-bit palace wall overlays are left out)
    present: std::collections::HashSet<u16>,
}

impl TileSet {
    /// Decode the pieces `tile_images` names. Returns the set, the bytes of RGBA uploaded and log lines for what is missing.
    pub fn load(dat: &Dat, name: &str, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>) -> (TileSet, usize, Vec<String>) {
        let decoded: HashMap<u16, image::Image> = dat.ids().filter_map(|id| Some((id, image::decode(dat.get(id)?).filter(|im| im.bpp == 4)?))).collect();
        let pal = |id: u16| dat.get(id).and_then(image::palette).unwrap_or_default();
        // chtab 6 uses palette 200, chtab 7 (resources 361..377) palette 360 (docs 4)
        let (p200, p360) = (pal(200), pal(360));
        let mut set = TileSet { floors: HashMap::new(), imgs: HashMap::new(), present: decoded.keys().copied().filter(|&id| id != 200 && id != 360).collect() };
        let (mut bytes, mut missing) = (0, vec![]);
        let mut texture = |w: usize, h: usize, px: &[u8], pal: &[[u8; 3]], opaque: bool| {
            let mut rgba = Vec::with_capacity(px.len() * 4);
            for &p in px {
                let c = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
                rgba.extend_from_slice(&[c[0], c[1], c[2], if p == 0 && !opaque { 0 } else { 255 }]);
            }
            bytes += rgba.len();
            let cut = px.contains(&0) && !opaque;
            let mut img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
            img.sampler = ImageSampler::nearest();
            mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), perceptual_roughness: 0.9, alpha_mode: if cut { AlphaMode::Mask(0.5) } else { AlphaMode::Opaque }, ..default() })
        };
        for k in 0..32u8 {
            let kind = Tile::from_u8(k);
            if let Some(r) = floor_record(kind) {
                if r.iter().any(|&v| resolve(v, &|id| set.present.contains(&id)).is_none()) {
                    missing.push(format!("{name}: {kind:?} floor (record {r:?}) not in the set, plain floor used"));
                }
            }
            for m in [0, 1] {
                let (floor, extra) = tile_images(kind, m, |id| set.present.contains(&id));
                if let Some(f) = floor.filter(|f| !set.floors.contains_key(f)) {
                    let (w, h, px) = floor_pixels(&decoded[&f[0]], &decoded[&f[1]], &decoded[&f[2]]);
                    let mat = texture(w, h, &px, &p200, true); // black where the original shows the black background
                    set.floors.insert(f, mat);
                }
                // exit frames: the image without its floor rows, plus the lintel above it
                let exit = matches!(kind, Tile::ExitLeft | Tile::ExitRight);
                for id in extra.into_iter().chain(extra.filter(|_| exit).map(|id| id + 1)) {
                    let Some(im) = decoded.get(&id).filter(|_| !set.imgs.contains_key(&id)) else { continue };
                    let h = if exit && Some(id) == extra { im.height.min(EXIT_FRAME_H) } else { im.height };
                    let mat = texture(im.width, h, &im.pixels[..im.width * h], if (361..=377).contains(&id) { &p360 } else { &p200 }, false);
                    set.imgs.insert(id, Tex { mat, w: im.width as f32, h: h as f32 });
                }
                let wanted = matches!(kind, Tile::Pillar | Tile::LatticePillar | Tile::BigPillarBottom | Tile::BigPillarTop | Tile::Gate | Tile::DoortopWithFloor | Tile::Doortop | Tile::ExitLeft | Tile::ExitRight | Tile::Wall);
                if wanted && extra.is_none() && m == 0 {
                    missing.push(format!("{name}: no 4-bit image for the {kind:?} piece, procedural used"));
                }
            }
        }
        (set, bytes, missing)
    }

    /// Floor material of a tile (None: keep the procedural one).
    pub fn floor(&self, kind: Tile) -> Option<Handle<StandardMaterial>> {
        tile_images(kind, 0, |id| self.present.contains(&id)).0.and_then(|f| self.floors.get(&f).cloned())
    }

    /// The tile's extra piece (pillar, door post, exit frame, wall face).
    pub fn extra(&self, kind: Tile, modifier: u8) -> Option<&Tex> {
        tile_images(kind, modifier, |id| self.present.contains(&id)).1.and_then(|id| self.imgs.get(&id))
    }

    /// Any loaded image by resource id (exit lintels).
    pub fn img(&self, id: u16) -> Option<&Tex> {
        self.imgs.get(&id)
    }
}

/// Both tile sets, [dungeon, palace]; None where the DAT is missing or `tile_textures` is "procedural".
pub fn load_sets(dir: &str, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>) -> [Option<TileSet>; 2] {
    let t0 = std::time::Instant::now();
    let mut total = 0;
    let sets = ["VDUNGEON.DAT", "VPALACE.DAT"].map(|f| match Dat::open(format!("{dir}/{f}")) {
        Ok(dat) => {
            let (set, bytes, missing) = TileSet::load(&dat, f, imgs, mats);
            total += bytes;
            for m in missing {
                info!("tile textures: {m}");
            }
            Some(set)
        }
        Err(e) => {
            warn!("tile textures: cannot open {f} ({e}); procedural tiles for that set");
            None
        }
    });
    info!("tile textures: loaded in {:.1} ms, {} KB RGBA", t0.elapsed().as_secs_f32() * 1000.0, total / 1024);
    sets
}

/// "original" (default) or "procedural" from settings.json, POP_TILES overrides; unknown values mean original.
pub fn use_textures(setting: &str, env: Option<&str>) -> bool {
    env.unwrap_or(setting) != "procedural"
}

#[cfg(test)]
mod tests {
    use super::*;
    use Tile::*;

    fn img(w: usize, h: usize, px: &[u8]) -> image::Image {
        image::Image { width: w, height: h, bpp: 4, pixels: px.to_vec() }
    }

    /// Ids of section 3 (VDUNGEON) as present in the set.
    fn dungeon(id: u16) -> bool {
        (232..=274).contains(&id) || (292..=300).contains(&id) || [285, 324, 325, 326, 344].contains(&id) || (346..=351).contains(&id) || (361..=377).contains(&id) || (1230..=1231).contains(&id) || (1286..=1343).contains(&id)
    }

    #[test]
    fn table_ids() {
        assert_eq!(tile_images(Floor, 0, dungeon), (Some([241, 242, 243]), None));
        assert_eq!(tile_images(Spikes, 0, dungeon).0, Some([1327, 1333, 243]));
        assert_eq!(tile_images(StuckButton, 0, dungeon).0, Some([241, 235, 236]));
        assert_eq!(tile_images(DropButton, 0, dungeon).0, Some([241, 242, 296]));
        assert_eq!(tile_images(RaiseButton, 0, dungeon).0, Some([347, 242, 349]));
        assert_eq!(tile_images(Debris, 0, dungeon).0, Some([297, 298, 243]));
        assert_eq!(tile_images(Skeleton, 0, dungeon).0, Some([1230, 1231, 243]));
        assert_eq!(tile_images(Pillar, 0, dungeon), (Some([241, 242, 243]), Some(295)));
        assert_eq!(tile_images(BigPillarBottom, 0, dungeon).1, Some(1288));
        assert_eq!(tile_images(BigPillarTop, 0, dungeon), (None, Some(1291)));
        assert_eq!(tile_images(Gate, 188, dungeon).1, Some(249));
        assert_eq!((tile_images(ExitLeft, 0, dungeon).1, tile_images(ExitRight, 0, dungeon).1), (Some(237), Some(239)));
        assert_eq!((tile_images(Wall, 0, dungeon), tile_images(Wall, 1, dungeon).1), ((None, Some(368)), Some(370)));
        assert_eq!(tile_images(LooseFloor, 0, dungeon), (None, None));
    }

    #[test]
    fn missing_images_fall_back() {
        // palace: no skeleton images (1230/1231) -> plain floor; no 4-bit wall faces -> procedural wall
        let palace = |id: u16| dungeon(id) && !(1230..=1231).contains(&id) && !(366..=374).contains(&id);
        assert_eq!(tile_images(Skeleton, 0, palace).0, Some([241, 242, 243]));
        assert_eq!(tile_images(Wall, 0, palace).1, None);
        assert_eq!(resolve(43, &palace), Some(243));
        assert_eq!(resolve(127, &palace), Some(1327)); // 327 absent -> base 1200
        assert_eq!(tile_images(Floor, 0, |_| false), (None, None));
    }

    #[test]
    fn floor_unshears_the_top_and_adds_the_lip() {
        let base = img(4, 2, &[0, 0, 1, 1, 0, 1, 1, 1]);
        // bottom-aligned: base row 0 = right row 1; the trailing run of 6 is the end face and is dropped
        let right = img(3, 3, &[9, 9, 9, 2, 6, 0, 2, 6, 6]);
        let lip = img(4, 1, &[3, 3, 3, 3]);
        assert_eq!(floor_pixels(&base, &right, &lip), (4, 3, vec![1, 1, 2, 2, 1, 1, 1, 2, 3, 3, 3, 3]));
        // too short runs are padded with their last pixel, too long ones cut
        let (_, _, px) = floor_pixels(&img(4, 1, &[0, 0, 0, 5]), &img(1, 1, &[0]), &img(2, 1, &[7, 6]));
        assert_eq!(px, vec![5, 5, 5, 5, 7, 6, 6, 6]);
    }

    #[test]
    fn setting_and_env() {
        assert!(use_textures("original", None));
        assert!(!use_textures("procedural", None));
        assert!(use_textures("procedural", Some("original")));
        assert!(!use_textures("original", Some("procedural")));
    }
}
