//! Original tile images (VDUNGEON.DAT / VPALACE.DAT) on the 3D room pieces. Which image goes where: docs/texture_inventory.md
//! sections 3, 5 and 13. Decoded at runtime from the user's own DAT files; anything missing falls back to the procedural material.
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::{dat::Dat, image, level::Tile};
use std::collections::HashMap;

/// Resource id of record value `v`: `1200 + v` if the set has that image (base-1200 slots overwrite the base-200 entry,
/// docs 8.0), else `200 + v`.
pub fn resolve(v: u8, present: &impl Fn(u16) -> bool) -> Option<u16> {
    [1200 + v as u16, 200 + v as u16].into_iter().find(|&id| present(id))
}

/// What an extra piece is for; room.rs places it on the 3D tile (docs/texture_inventory.md 8-12, decisions in 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// top strip of a dungeon wall (chtab 7 table ds:27B6), on the wall block's top face
    WallTop,
    /// chomper blade (1301 + k), its floor rows cut off; behind the figures
    JawBack,
    /// chomper front slice (1306 + k), floor rows cut off; in front of the figures
    JawFore,
    /// upper teeth of an open chomper (1311..1313); bottom this many px above the floor surface, top level with the blade
    JawTop(u8),
    /// spike frames: piece in the tile's column, piece in the next column, piece in front of the figures
    SpikeBack,
    SpikeRight,
    SpikeFore,
    /// exit door panel: stacked 6-px slices (233) with the bottom slice (234); ExitLeft only
    DoorSlice,
    DoorBottom,
    /// stairs behind an exit door that is not the level's start door (344)
    Stairs,
    /// torch holder below the flame (346)
    TorchStub,
    /// window / arch panel on the back wall, floor rows cut off (balcony 1210 / 1212, lattice support 1203)
    BackPanel,
    /// panel stacked on top of the BackPanel (balcony arches 1211 / 1213)
    BackTop,
}

/// Chomper frame k (images 1301 + k / 1306 + k) by modifier & 0x7F (capped at 6): table ds:2754, matches the runtime probe
/// (docs 10.1: mod 2 -> 1301, 3 -> 1302, 1 -> 1303, 0 / 5 / 6 -> 1304, 4 -> 1305).
const CHOMPER_FRAME: [u16; 7] = [3, 2, 0, 1, 4, 3, 3];
/// Bottom of the upper teeth 1311..1313 above the floor surface (A9D6 offsets 37 / 47 / 50 above the 13 floor rows).
const JAW_TOP: [u8; 3] = [24, 34, 37];
/// Spike pieces by frame 0..9 (modifier, >= 0x80 = 5): back = ds:2774, right and fore the matching series (docs 8.1, 12.1).
const SPIKE_BACK: [u16; 10] = [0, 1328, 1329, 1330, 1331, 1332, 1331, 1329, 1328, 0];
const SPIKE_RIGHT: [u16; 10] = [0, 1334, 1335, 1336, 1337, 1338, 1337, 1335, 1334, 0];
const SPIKE_FORE: [u16; 10] = [0, 1339, 1340, 1341, 1342, 1343, 1342, 1340, 1339, 0];

/// The extra pieces of a tile beyond the floor and the main piece of `tile_images`: (role, resource id), only ids the set
/// has. `palace`: the palace VGA path skips the chtab-7 wall back and fore pieces (docs 8.0), so palace walls get none here
/// (their bricks are built by `palace_wall`). Only entries rated V or R high are used (docs 13 lists what was left out).
pub fn pieces(kind: Tile, modifier: u8, palace: bool, present: impl Fn(u16) -> bool) -> Vec<(Role, u16)> {
    use Role::*;
    use Tile::*;
    let m = modifier & 0x7F;
    let mut v: Vec<(Role, u16)> = match kind {
        // ds:27B6: modifier 0 -> 367, 1 -> 369 (only 0 and 1 occur in the levels)
        Wall if !palace => vec![(WallTop, if m == 1 { 369 } else { 367 })],
        Chomper => {
            let k = CHOMPER_FRAME[m.min(6) as usize];
            let mut v = vec![(JawBack, 1301 + k), (JawFore, 1306 + k)];
            if k >= 2 {
                v.push((JawTop(JAW_TOP[k as usize - 2]), 1311 + k - 2));
            }
            v
        }
        Spikes => {
            let s = if modifier & 0x80 != 0 { 5 } else { modifier.min(9) as usize };
            vec![(SpikeBack, SPIKE_BACK[s]), (SpikeRight, SPIKE_RIGHT[s]), (SpikeFore, SPIKE_FORE[s])]
        }
        ExitLeft => vec![(DoorSlice, 233), (DoorBottom, 234), (Stairs, 344)],
        Torch | TorchDebris => vec![(TorchStub, 346)],
        BalconyLeft | BalconyRight => {
            let r = if kind == BalconyLeft { [10, 11] } else { [12, 13] };
            [(BackPanel, r[0]), (BackTop, r[1])].into_iter().filter_map(|(role, v)| Some((role, resolve(v, &present)?))).collect()
        }
        // 1203 is R high; 1204 / 1205 (R medium) are taken too so the arcade does not mix arches and plain boxes
        LatticeSupport => vec![(BackPanel, 1203)],
        SmallLattice => vec![(BackPanel, 1204)],
        LatticeLeft => vec![(BackPanel, 1205)],
        _ => vec![],
    };
    v.retain(|&(_, id)| id != 0 && present(id));
    v
}

/// Floor rows at the bottom of a piece that `role` cuts off (the 3D floor slab shows the floor): images drawn from the
/// tile's bottom line have 13 floor rows; the 62-row ones reach 2 px lower (as the exit frame, EXIT_FRAME_H).
pub fn floor_rows(role: Role, h: usize) -> usize {
    match role {
        Role::JawBack | Role::JawFore => if h > 60 { 15 } else { 13 },
        Role::BackPanel if h >= 60 => h - EXIT_FRAME_H,
        _ => 0,
    }
}

/// Size of a palace wall face (palette 360 indices).
pub const PALACE_WALL: (usize, usize) = (32, 60);

/// A palace wall face, `PALACE_WALL` pixels of palette 360. The VGA palace path fills brick rectangles with random colours
/// (as SDLPoP's generate_wall_pattern, from memory: courses alternate between colours 1..4 and 6..9 of palette 360, which are
/// exactly the near-identical tans of VPALACE 360) and draws the 1-bit chtab-7 strips 366..371 over them (docs 10.1, R high).
/// Reconstruction: 4 courses of 15 rows, mortar and joints in colour 14, a light top edge (11) per brick, one 1-bit strip
/// per course as grain in colour 14; brick joints and colours from `seed`.
pub fn palace_wall(grain: &[image::Image], seed: u32) -> Vec<u8> {
    let (w, h) = PALACE_WALL;
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(0x9E37);
    let mut rnd = |n: u32| {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s % n
    };
    let mut px = vec![0u8; w * h];
    for c in 0..4 {
        let (y0, base) = (c * 15, if c % 2 == 0 { 1 } else { 6 });
        let joint = 6 + rnd(20) as usize;
        let (left, right) = (base + rnd(4) as u8, base + rnd(4) as u8);
        let g = (!grain.is_empty()).then(|| &grain[rnd(grain.len() as u32) as usize]);
        for y in 0..15 {
            for x in 0..w {
                px[(y0 + y) * w + x] = if y == 0 || x == joint {
                    14
                } else if y == 1 {
                    11
                } else if g.is_some_and(|g| y >= 2 && y - 2 < g.height && x < g.width && g.pixels[(y - 2) * g.width + x] != 0) {
                    14
                } else if x < joint {
                    left
                } else {
                    right
                };
            }
        }
    }
    px
}

/// Wall variant for a cell (palace walls): a fixed hash of room and tile, so a room looks the same on every visit.
pub fn wall_variant(room: usize, idx: u8, n: usize) -> usize {
    (room.wrapping_mul(31).wrapping_add(idx as usize).wrapping_mul(2654435761) >> 7) % n.max(1)
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
    palace: bool,
    /// palace wall faces built by `palace_wall` (empty in the dungeon set or without the 1-bit strips)
    walls: Vec<Handle<StandardMaterial>>,
    /// exit doorway in px from the ExitLeft cell's left edge: (end of the left post in 237, start of the right post in 239 + 32)
    pub exit_gap: Option<(f32, f32)>,
}

/// First / last opaque column of an image row.
fn opaque_cols(im: &image::Image, y: usize) -> Option<(usize, usize)> {
    let row = &im.pixels[y.min(im.height.saturating_sub(1)) * im.width..][..im.width];
    Some((row.iter().position(|&p| p != 0)?, row.iter().rposition(|&p| p != 0)?))
}

impl TileSet {
    /// Decode the pieces `tile_images` names. Returns the set, the bytes of RGBA uploaded and log lines for what is missing.
    pub fn load(dat: &Dat, name: &str, palace: bool, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>) -> (TileSet, usize, Vec<String>) {
        let mut all: HashMap<u16, image::Image> = dat.ids().filter_map(|id| Some((id, image::decode(dat.get(id)?)?))).collect();
        // palace: the 1-bit wall strips 366..371 (the grain of the brick faces)
        let grain: Vec<image::Image> = if palace { (366..=371).filter_map(|id| all.remove(&id).filter(|im| im.bpp == 1)).collect() } else { vec![] };
        let decoded: HashMap<u16, image::Image> = all.into_iter().filter(|(_, im)| im.bpp == 4).collect();
        let pal = |id: u16| dat.get(id).and_then(image::palette).unwrap_or_default();
        // chtab 6 uses palette 200, chtab 7 (resources 361..377) palette 360 (docs 4)
        let (p200, p360) = (pal(200), pal(360));
        let exit_gap = match (decoded.get(&237), decoded.get(&239)) {
            (Some(l), Some(r)) => Some(((opaque_cols(l, 20).map_or(0, |c| c.0) + 1..l.width).find(|&x| l.pixels[20 * l.width + x] == 0).unwrap_or(0) as f32, 32.0 + opaque_cols(r, 20).map_or(r.width, |c| c.0) as f32)),
            _ => None,
        };
        let mut set = TileSet { floors: HashMap::new(), imgs: HashMap::new(), present: decoded.keys().copied().filter(|&id| id != 200 && id != 360).collect(), palace, walls: vec![], exit_gap };
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
                let wanted = matches!(kind, Tile::Pillar | Tile::LatticePillar | Tile::BigPillarBottom | Tile::BigPillarTop | Tile::Gate | Tile::DoortopWithFloor | Tile::Doortop | Tile::ExitLeft | Tile::ExitRight) || (kind == Tile::Wall && !palace);
                if wanted && extra.is_none() && m == 0 {
                    missing.push(format!("{name}: no 4-bit image for the {kind:?} piece, procedural used"));
                }
            }
            // the other pieces of every modifier state (chomper and spike frames)
            for m in 0..=16u8 {
                for (role, id) in pieces(kind, if m == 16 { 0x80 } else { m }, palace, |id| set.present.contains(&id)) {
                    let Some(im) = decoded.get(&id).filter(|_| !set.imgs.contains_key(&id)) else { continue };
                    let h = im.height - floor_rows(role, im.height).min(im.height);
                    let mat = texture(im.width, h, &im.pixels[..im.width * h], if (361..=377).contains(&id) { &p360 } else { &p200 }, false);
                    set.imgs.insert(id, Tex { mat, w: im.width as f32, h: h as f32 });
                }
            }
        }
        if palace {
            if grain.is_empty() {
                missing.push(format!("{name}: no 1-bit wall strips 366..371, procedural palace walls"));
            } else {
                let (w, h) = PALACE_WALL;
                set.walls = (0..8).map(|v| texture(w, h, &palace_wall(&grain, v), &p360, true)).collect();
            }
        }
        (set, bytes, missing)
    }

    /// Material of a palace wall face for tile `idx` of `room` (None: dungeon set or no strips).
    pub fn palace_wall(&self, room: usize, idx: u8) -> Option<Handle<StandardMaterial>> {
        self.walls.get(wall_variant(room, idx, self.walls.len())).cloned()
    }

    /// The tile's extra pieces with their textures (see `pieces`).
    pub fn pieces(&self, kind: Tile, modifier: u8) -> Vec<(Role, Tex)> {
        pieces(kind, modifier, self.palace, |id| self.present.contains(&id)).into_iter().filter_map(|(r, id)| Some((r, self.imgs.get(&id)?.clone()))).collect()
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
            let (set, bytes, missing) = TileSet::load(&dat, f, f == "VPALACE.DAT", imgs, mats);
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
        (232..=274).contains(&id) || (292..=300).contains(&id) || [285, 324, 325, 326, 344].contains(&id) || (346..=351).contains(&id) || (361..=377).contains(&id) || (1230..=1231).contains(&id) || (1286..=1291).contains(&id) || (1301..=1323).contains(&id) || (1327..=1343).contains(&id)
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

    /// VPALACE ids that matter here (section 2.2): no skeleton, 1201..1213 present, wall strips 366..374 are 1-bit (absent).
    fn palace(id: u16) -> bool {
        (dungeon(id) && !(1230..=1231).contains(&id) && !(366..=374).contains(&id)) || (1201..=1213).contains(&id)
    }

    #[test]
    fn piece_table_ids() {
        use Role::*;
        let ids = |k, m, p: bool| pieces(k, m, p, if p { palace } else { dungeon }).into_iter().map(|(_, id)| id).collect::<Vec<_>>();
        // chomper by modifier (docs 10.1 runtime vectors)
        assert_eq!(pieces(Chomper, 2, false, dungeon), vec![(JawBack, 1301), (JawFore, 1306)]);
        assert_eq!(ids(Chomper, 3, false), [1302, 1307]);
        assert_eq!(pieces(Chomper, 1, false, dungeon), vec![(JawBack, 1303), (JawFore, 1308), (JawTop(24), 1311)]);
        assert_eq!(ids(Chomper, 0x81, false), [1303, 1308, 1311]);
        for m in [0, 5, 6, 15] {
            assert_eq!(ids(Chomper, m, true), [1304, 1309, 1312], "mod {m}");
        }
        assert_eq!(ids(Chomper, 4, false), [1305, 1310, 1313]);
        // spikes: frame = modifier, held out (>= 0x80) = 5; 0 and 9 draw nothing (ds:2774)
        assert_eq!(ids(Spikes, 0, false), Vec::<u16>::new());
        assert_eq!(pieces(Spikes, 1, false, dungeon), vec![(SpikeBack, 1328), (SpikeRight, 1334), (SpikeFore, 1339)]);
        assert_eq!(ids(Spikes, 0x8F, false), [1332, 1338, 1343]);
        assert_eq!(ids(Spikes, 255, true), [1332, 1338, 1343]);
        assert_eq!(ids(Spikes, 6, false), [1331, 1337, 1342]);
        assert_eq!(ids(Spikes, 9, false), Vec::<u16>::new());
        // walls: dungeon top strip by modifier (ds:27B6), none on the palace VGA path
        assert_eq!(pieces(Wall, 0, false, dungeon), vec![(WallTop, 367)]);
        assert_eq!(ids(Wall, 1, false), [369]);
        assert_eq!(ids(Wall, 0, true), Vec::<u16>::new());
        // exit door slices and stairs on ExitLeft only, torch stub, balcony and lattice palace only
        assert_eq!(pieces(ExitLeft, 0, false, dungeon), vec![(DoorSlice, 233), (DoorBottom, 234), (Stairs, 344)]);
        assert_eq!(ids(ExitRight, 0, false), Vec::<u16>::new());
        assert_eq!((ids(Torch, 0, false), ids(TorchDebris, 0, true)), (vec![346], vec![346]));
        assert_eq!(pieces(BalconyLeft, 0, true, palace), vec![(BackPanel, 1210), (BackTop, 1211)]);
        assert_eq!(ids(BalconyRight, 0, true), [1212, 1213]);
        assert_eq!(ids(LatticeSupport, 0, true), [1203]);
        assert_eq!((ids(SmallLattice, 0, true), ids(LatticeLeft, 0, true), ids(LatticeRight, 0, true)), (vec![1204], vec![1205], vec![]));
        assert_eq!(ids(Floor, 0, true), Vec::<u16>::new());
    }

    #[test]
    fn absent_pieces_are_dropped() {
        // the dungeon set has no balcony or lattice images: those tiles keep their procedural look
        assert!(pieces(BalconyLeft, 0, false, dungeon).is_empty());
        assert!(pieces(LatticeSupport, 0, false, dungeon).is_empty());
        assert!(pieces(Chomper, 1, false, |id| id != 1311).iter().all(|&(_, id)| id != 1311));
        assert!(pieces(Spikes, 3, false, |_| false).is_empty());
        // base 1200 wins when both exist (docs 8.0)
        assert_eq!(resolve(30, &|id| id == 230 || id == 1230), Some(1230));
        assert_eq!(resolve(41, &|id| id == 241), Some(241));
        assert_eq!(resolve(10, &dungeon), None);
    }

    #[test]
    fn floor_rows_cut() {
        assert_eq!(floor_rows(Role::JawBack, 60), 13);
        assert_eq!(floor_rows(Role::JawFore, 30), 13);
        assert_eq!(floor_rows(Role::BackPanel, 62), 15);
        assert_eq!(floor_rows(Role::BackPanel, 43), 0);
        assert_eq!(floor_rows(Role::TorchStub, 15), 0);
    }

    #[test]
    fn palace_wall_bricks() {
        let g = || img(4, 2, &[1, 0, 0, 1, 0, 1, 0, 0]);
        let a = palace_wall(&[g()], 3);
        assert_eq!(a.len(), PALACE_WALL.0 * PALACE_WALL.1);
        assert!(a.iter().all(|&c| [1, 2, 3, 4, 6, 7, 8, 9, 11, 14].contains(&c)));
        // mortar row on top of each course, brick top edge below it, grain at row 2
        assert!(a[..32].iter().all(|&c| c == 14) && a[15 * 32..16 * 32].iter().all(|&c| c == 14));
        assert_eq!(a[2 * 32], 14);
        assert_eq!(a, palace_wall(&[g()], 3));
        assert_ne!(a, palace_wall(&[g()], 4));
        assert!((0..30u8).all(|i| wall_variant(7, i, 8) < 8));
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
