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
/// has. Walls get none (the base strip is part of their block texture, the palace bricks come from `palace_wall`); `_palace` is
/// kept for the callers. Only entries rated V or R high are used (docs 13 lists what was left out).
pub fn pieces(kind: Tile, modifier: u8, _palace: bool, present: impl Fn(u16) -> bool) -> Vec<(Role, u16)> {
    use Role::*;
    use Tile::*;
    let m = modifier & 0x7F;
    let mut v: Vec<(Role, u16)> = match kind {
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
        // record +0 of types 26..29 (A7BC, ds:25B0 + 12 * type) = 1, 3, 4, 5 -> 1201 capital, 1203 / 1204 / 1205 arch pieces, each
        // with the 7-row frieze 1209 (record +9 = 9, +11 = -53 px) on top of it: docs/texture_inventory.md 19
        LatticeSupport => vec![(BackPanel, 1201), (BackTop, 1209)],
        SmallLattice => vec![(BackPanel, 1203), (BackTop, 1209)],
        LatticeLeft => vec![(BackPanel, 1204), (BackTop, 1209)],
        LatticeRight => vec![(BackPanel, 1205), (BackTop, 1209)],
        _ => vec![],
    };
    v.retain(|&(_, id)| id != 0 && present(id));
    v
}

/// Rows of a tile-bottom-aligned image below the floor surface (the 2D floor rows; the 3D slab shows them).
const FLOOR_ROWS: i32 = 13;

/// The background piece the original draws in a cell from its left neighbour (draw_tile_right `0000:A332`, docs
/// texture_inventory.md 18): (image id, bottom of the image in px above the cell's floor surface; negative = below it).
/// Nothing when the cell itself is a wall. Left Empty, modifier 1..3: 324 / 325 / 326 (`ds:27AA`), bottom 20 / 20 / 0 px
/// above the tile's bottom line (`ds:27AE`); left Floor: 244 for modifier 0 / 1, 245 for 2 / 3 (above 3 counts as 0,
/// `ds:27B2`), 20 px up, skipped for modifier 0 in the dungeon and 1 in the palace. Everything else on the original's
/// background is palette colour 0 (black).
pub fn back_decor(left: Tile, left_mod: u8, curr: Tile, palace: bool) -> Option<(u16, i32)> {
    let (v, up) = match (left, curr) {
        (_, Tile::Wall) => return None,
        (Tile::Empty, _) if left_mod <= 3 => ([0, 124, 125, 126][left_mod as usize], [0, 20, 20, 0][left_mod as usize]),
        (Tile::Floor, _) => {
            let m = if left_mod > 3 { 0 } else { left_mod };
            if m == palace as u8 {
                return None;
            }
            ([44, 44, 45, 45][m as usize], 20)
        }
        // palace only: the tapestry (1278 / 1280 / 1282, the small tops 1281 / 1283) hanging in the cell right of a Doortop
        // (`ds:27A2[mod]` = 4E 50 52; modifiers 5 / 6 would give the 12-row tops 1281 / 1283, which lie wholly below the floor surface: not drawn), bottom on the row line; verified at 36 cells of levels 4, 5, 6, 10, 14
        (Tile::Doortop | Tile::DoortopWithFloor, _) if palace && left_mod <= 2 => ([1078, 1080, 1082][left_mod as usize], -2),
        _ => return None,
    };
    (v != 0).then_some((200 + v, up - FLOOR_ROWS))
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

/// The original's random(n) (`0C83:0412`): seed = seed * 214013 + 2531011, result = high word % (n + 1). The wall drawing
/// reseeds it per tile, so everything below is a pure function of (room, row, column).
pub struct Rnd(pub u32);

impl Rnd {
    pub fn next(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        (self.0 >> 16) % (n + 1)
    }
}

/// Seed of a wall tile (`C3A6`): `ds:2288[row]` (0, 10, 20) + drawn room + column, as a sign-extended word; one random(1) is
/// then thrown away (`C3B6`).
fn wall_rnd(room: usize, row: usize, col: usize) -> Rnd {
    let mut r = Rnd((([0u16, 10, 20][row.min(2)] + room as u16 + col as u16) as i16) as i32 as u32);
    r.next(1);
    r
}

/// Wall modifier of the original's runtime tile (`load_alter_mod` `B656`): bit 0 = the right neighbour is a wall, bit 1 = the
/// left neighbour is one (outside the level counts as a wall). The level's own modifier bit becomes 0x80 and is not read.
pub fn wall_modifier(left_wall: bool, right_wall: bool) -> u8 {
    right_wall as u8 | (left_wall as u8) << 1
}

/// Wall face by `modifier & 0x7F` (`ds:27BA`: 368, 370, 366, 364) and the 3-row base strip drawn below it (`ds:27B6`: 367, 369,
/// 365, 363): face 60 rows + strip 3 rows = the 63-px row. Modifiers above 3 do not occur (dungeon).
pub const WALL_FACE: [u16; 4] = [368, 370, 366, 364];
pub const WALL_STRIP: [u16; 4] = [367, 369, 365, 363];

/// A decoration the original draws on a dungeon wall (`C5BF..C81A`, `ABDA`, `AC56`): image `id` (chtab 7), left edge `x` px
/// from the tile's left, bottom edge `up` px above the row's bottom line (the block's bottom, = the floor line minus the slab).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub id: u16,
    pub x: i32,
    pub up: i32,
}

/// The marks of a dungeon wall tile, in drawing order (docs/texture_inventory.md 19): the seeded random picks joints between
/// the bricks of the middle and bottom course (371 / 372), sometimes a plain top course (373) and a few cracks (374..377).
pub fn dungeon_wall_marks(room: usize, row: usize, col: usize, modifier: u8) -> Vec<Mark> {
    let m = modifier & 0x7F;
    if m > 3 {
        return vec![];
    }
    let mut r = wall_rnd(room, row, col);
    let (a1, a4, b1, b4) = (r.next(1) as i32, r.next(4) as i32, r.next(1) as i32, r.next(4) as i32);
    let mut v = vec![];
    let mid = Mark { id: 371 + a1 as u16, x: 8 + a4, up: 21 };
    let bot = Mark { id: 371 + b1 as u16, x: b4, up: 0 };
    let top = Mark { id: 373, x: 0, up: 42 };
    // AC56: id 376 + (i & 1); x = 24 for i < 2, else 8 + (a4 - 3); bottom 52 / 42 / 31 / 21 px up
    let edge = |i: u32| Mark { id: 376 + (i & 1) as u16, x: if i < 2 { 24 } else { 8 + a4 - 3 }, up: [52, 42, 31, 21][i as usize] };
    // ABDA: id 374 + (i & 1); x from the joint offsets; bottom 58 / 41 / 37 / 20 / 16 px up
    let crack = |i: u32| {
        let x = match i {
            0 | 1 => 0,
            2 | 3 => 8 + (a4 - a1) + 6,
            _ => (b4 - b1) + 6,
        };
        Mark { id: 374 + (i & 1) as u16, x, up: [58, 41, 37, 20, 16][i as usize] }
    };
    match m {
        0 => {
            if r.next(6) == 0 {
                v.push(crack(r.next(1)));
            }
        }
        1 => {
            if r.next(4) == 0 {
                v.push(top);
            }
            v.push(mid);
            if r.next(4) == 0 {
                v.push(edge(r.next(3)));
            }
            if r.next(4) == 0 {
                v.push(crack(r.next(3)));
            }
        }
        2 => {
            v.push(mid);
            v.push(bot);
            if r.next(4) == 0 {
                v.push(edge(r.next(1) + 2));
            }
            if r.next(4) == 0 {
                v.push(crack(r.next(4)));
            }
        }
        _ => {
            if r.next(4) == 0 {
                v.push(top);
            }
            v.push(mid);
            v.push(bot);
            if r.next(4) == 0 {
                v.push(edge(r.next(3)));
            }
            if r.next(4) == 0 {
                v.push(crack(r.next(4)));
            }
        }
    }
    v
}

/// Size of a palace wall tile texture: 60 rows of face + the 3-row base band (palette 360 indices, `MONO` = the speckle colour).
pub const PALACE_WALL: (usize, usize) = (32, 63);
/// Index after palette 360's 16 entries: colour 6 of palette 200, the colour of the mono-blitted speckle strips (blit `0x46`).
pub const MONO: u8 = 16;

/// A room's brick colour table (`1763`, seeded by the room number): 3 tile rows x 4 courses x 11 bricks, screen colours 102..105
/// for the courses 0 and 2 and 97..100 for 1 and 3 (palette 360 index = colour - 96); neighbours in a course differ.
pub fn palace_pattern(room: usize) -> [[[u8; 11]; 4]; 3] {
    let mut r = Rnd(room as u32);
    r.next(1);
    let mut t = [[[0u8; 11]; 4]; 3];
    for row in t.iter_mut() {
        for (course, bricks) in row.iter_mut().enumerate() {
            let (base, mut last) = (if course % 2 == 0 { 0x66 } else { 0x61 }, 255);
            for b in bricks.iter_mut() {
                let mut c = base + r.next(3) as u8;
                while c == last {
                    c = base + r.next(3) as u8;
                }
                (*b, last) = (c, c);
            }
        }
    }
    t
}

/// The wall of palace tile (`room`, `row`, `col`) as the VGA palace path draws it (`C3E5..C5BC`): brick rectangles from the room's
/// colour table (course 0: one 32-px brick, course 1: 16 + 16, course 2: 8 + 24, so the joints of a course run on into the next
/// column; 20 / 21 / 19 rows), the 3-row base band, and five mono speckle strips picked by the tile's seeded random (ids 363..365 at
/// x 24, 366..368, 369..371, 372..374 and, on the band, 375..377). `strip(id)` = the strip's image (set pixels = nonzero).
pub fn palace_wall<'a>(room: usize, row: usize, col: usize, pat: &[[[u8; 11]; 4]; 3], strip: impl Fn(u16) -> Option<&'a image::Image>) -> Vec<u8> {
    let (w, h) = PALACE_WALL;
    let (c, col) = (&pat[row.min(2)], col.min(9));
    let mut px = vec![0u8; w * h];
    let mut fill = |y0: usize, y1: usize, x0: usize, x1: usize, colour: u8| {
        for y in y0..y1 {
            px[y * w + x0..y * w + x1].fill(colour - 96);
        }
    };
    fill(0, 20, 0, 32, c[0][col]);
    fill(20, 41, 0, 16, c[1][col]);
    fill(20, 41, 16, 32, c[1][col + 1]);
    fill(41, 60, 0, 8, c[2][col]);
    fill(41, 60, 8, 32, c[2][col + 1]);
    fill(60, 63, 0, 32, c[3][col]);
    let mut r = wall_rnd(room, row, col);
    // (first id, x, bottom row): the speckle strips are drawn over the bricks, the last one over the band
    for (base, x, bottom) in [(363u16, 24i32, 6i32), (366, 0, 25), (369, 0, 46), (372, 0, 59), (375, 0, 62)] {
        let Some(im) = strip(base + r.next(2) as u16) else { continue };
        for y in 0..im.height {
            for xx in 0..im.width {
                let (dx, dy) = (x + xx as i32, bottom - im.height as i32 + 1 + y as i32);
                if im.pixels[y * im.width + xx] != 0 && (0..w as i32).contains(&dx) && (0..h as i32).contains(&dy) {
                    px[dy as usize * w + dx as usize] = MONO;
                }
            }
        }
    }
    px
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
        // the palace lattices' front lip is 1202 (record +8 = 2; found at 30 of the 45 hits, docs/level_render_check.md)
        LatticeSupport | SmallLattice | LatticeLeft | LatticeRight => [41, 42, 2],
        // LooseFloor has no record images and Unknown31 no record; BigPillarTop has no floor
        Empty | Wall | LooseFloor | BigPillarTop | Unknown31 => return None,
        // shortcut: types whose record lacks +0 / +3 or holds chtab-7 values (Mirror, Chomper, Exit, Doortop, Balcony, Lattice)
        // get the plain floor; trace their pieces if they look wrong
        _ => [41, 42, 43],
    })
}

/// Image ids of a tile's textured pieces: (floor [base, right, lip], extra piece). The extra piece is the pillar column,
/// the door post, the exit frame (its lintel is id + 1) or the wall face (chtab 7: 360 + T27BA[modifier], docs 3a; the modifier
/// is the runtime one, from the wall's neighbours: `wall_modifier`).
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
        Wall => WALL_FACE.get((modifier & 0x7F) as usize).copied().filter(|&id| present(id)),
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
    /// palace wall tiles built by `palace_wall`, index room * 30 + tile (rooms 0..=24; empty in the dungeon set)
    walls: Vec<Handle<StandardMaterial>>,
    /// floor texture -> (surface rows / all rows), for the slab's UVs
    splits: Vec<(AssetId<StandardMaterial>, f32)>,
    /// dungeon wall blocks by wall modifier 0..=3: face + 3-row base strip, 32 x 63 px
    faces: [Option<Handle<StandardMaterial>>; 4],
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
        // palace: the speckle strips 363..377 drawn over the brick rectangles of the walls
        let strips: HashMap<u16, image::Image> = if palace { (363..=377).filter_map(|id| Some((id, all.remove(&id)?))).collect() } else { HashMap::new() };
        let decoded: HashMap<u16, image::Image> = all.into_iter().filter(|(_, im)| im.bpp == 4).collect();
        let pal = |id: u16| dat.get(id).and_then(image::palette).unwrap_or_default();
        // chtab 6 uses palette 200, chtab 7 (resources 361..377) palette 360 (docs 4)
        let (p200, p360) = (pal(200), pal(360));
        let exit_gap = match (decoded.get(&237), decoded.get(&239)) {
            (Some(l), Some(r)) => Some(((opaque_cols(l, 20).map_or(0, |c| c.0) + 1..l.width).find(|&x| l.pixels[20 * l.width + x] == 0).unwrap_or(0) as f32, 32.0 + opaque_cols(r, 20).map_or(r.width, |c| c.0) as f32)),
            _ => None,
        };
        let mut set = TileSet { floors: HashMap::new(), imgs: HashMap::new(), present: decoded.keys().copied().filter(|&id| id != 200 && id != 360).collect(), palace, walls: vec![], faces: [None, None, None, None], splits: vec![], exit_gap };
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
                if r.iter().any(|&v| resolve(v, &|id| set.present.contains(&id)).is_none()) && (palace || !matches!(kind, Tile::LatticeSupport | Tile::SmallLattice | Tile::LatticeLeft | Tile::LatticeRight)) {
                    missing.push(format!("{name}: {kind:?} floor (record {r:?}) not in the set, plain floor used"));
                }
            }
            for m in 0..4 {
                let (floor, extra) = tile_images(kind, m, |id| set.present.contains(&id));
                if let Some(f) = floor.filter(|f| !set.floors.contains_key(f)) {
                    let (w, h, px) = floor_pixels(&decoded[&f[0]], &decoded[&f[1]], &decoded[&f[2]]);
                    let mat = texture(w, h, &px, &p200, true); // black where the original shows the black background
                    set.splits.push((mat.id(), decoded[&f[0]].height as f32 / h as f32));
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
        // background pieces, cut at the floor surface (326 reaches 13 rows below it)
        for (kind, m) in [Tile::Empty, Tile::Floor, Tile::Doortop].into_iter().flat_map(|k| (0..4).map(move |m| (k, m))) {
            let Some((id, up)) = back_decor(kind, m, Tile::Empty, palace) else { continue };
            let Some(im) = decoded.get(&id).filter(|_| !set.imgs.contains_key(&id)) else { continue };
            let h = im.height - ((-up).max(0) as usize).min(im.height);
            if h == 0 {
                continue;
            }
            let mat = texture(im.width, h, &im.pixels[..im.width * h], &p200, false);
            set.imgs.insert(id, Tex { mat, w: im.width as f32, h: h as f32 });
        }
        // dungeon walls: the face of each modifier with its base strip below, as one 32 x 63 block texture
        if !palace {
            for m in 0..4 {
                let (Some(f), Some(st)) = (decoded.get(&WALL_FACE[m]), decoded.get(&WALL_STRIP[m])) else { continue };
                if f.width == 32 && f.height == 60 && st.width == 32 && st.height == 3 {
                    set.faces[m] = Some(texture(32, 63, &[f.pixels.clone(), st.pixels.clone()].concat(), &p360, true));
                }
            }
            // the marks (joints 371 / 372, the plain top course 373, cracks 374..377) as cut-outs
            for id in 371..=377u16 {
                let Some(im) = decoded.get(&id).filter(|_| !set.imgs.contains_key(&id)) else { continue };
                let mat = texture(im.width, im.height, &im.pixels, &p360, false);
                set.imgs.insert(id, Tex { mat, w: im.width as f32, h: im.height as f32 });
            }
        } else {
            // the speckle strips are a dark tan on the original's screen (192, 152, 80 in 21 palace rooms measured against the
            // DOSBox pictures, docs/level_render_check.md), not palette 200's colour 6 (light cream); 6-bit 48 / 38 / 20 scaled
            let mono = [p360.clone(), vec![[194, 154, 81]]].concat();
            let (w, h) = PALACE_WALL;
            let pats: Vec<_> = (0..=24).map(palace_pattern).collect();
            for room in 0..=24usize {
                for idx in 0..30usize {
                    let px = palace_wall(room, idx / 10, idx % 10, &pats[room], |id| strips.get(&id));
                    set.walls.push(texture(w, h, &px, &mono, true));
                }
            }
            if strips.is_empty() {
                missing.push(format!("{name}: no speckle strips 363..377, palace walls without them"));
            }
        }
        (set, bytes, missing)
    }

    /// Material of the palace wall at tile `idx` of `room` (None: dungeon set).
    pub fn palace_wall(&self, room: usize, idx: u8) -> Option<Handle<StandardMaterial>> {
        self.walls.get(room.min(24) * 30 + idx as usize).cloned()
    }

    /// Material of a dungeon wall block for the runtime wall modifier (None: palace set or images missing).
    pub fn wall_face(&self, modifier: u8) -> Option<Handle<StandardMaterial>> {
        self.faces.get((modifier & 0x7F) as usize)?.clone()
    }

    /// Every floor texture with the share of its rows that are the top surface (the rest is the front lip).
    pub fn floor_splits(&self) -> impl Iterator<Item = (AssetId<StandardMaterial>, f32)> + '_ {
        self.splits.iter().copied()
    }

    /// A wall mark's image (`dungeon_wall_marks`).
    pub fn mark(&self, id: u16) -> Option<&Tex> {
        self.imgs.get(&id)
    }

    /// The tile's extra pieces with their textures (see `pieces`).
    pub fn pieces(&self, kind: Tile, modifier: u8) -> Vec<(Role, Tex)> {
        pieces(kind, modifier, self.palace, |id| self.present.contains(&id)).into_iter().filter_map(|(r, id)| Some((r, self.imgs.get(&id)?.clone()))).collect()
    }

    /// The tile's extra pieces as image ids (the voxel objects are looked up by them).
    pub fn piece_ids(&self, kind: Tile, modifier: u8) -> Vec<(Role, u16)> {
        pieces(kind, modifier, self.palace, |id| self.present.contains(&id))
    }

    /// Floor material of a tile (None: keep the procedural one).
    pub fn floor(&self, kind: Tile) -> Option<Handle<StandardMaterial>> {
        tile_images(kind, 0, |id| self.present.contains(&id)).0.and_then(|f| self.floors.get(&f).cloned())
    }

    /// The tile's extra piece (pillar, door post, exit frame, wall face).
    pub fn extra(&self, kind: Tile, modifier: u8) -> Option<&Tex> {
        self.extra_id(kind, modifier).and_then(|id| self.imgs.get(&id))
    }

    /// Resource id of the tile's extra piece (the voxel columns are looked up by it).
    pub fn extra_id(&self, kind: Tile, modifier: u8) -> Option<u16> {
        tile_images(kind, modifier, |id| self.present.contains(&id)).1
    }

    /// The background piece of a cell from its left neighbour (`back_decor`): texture and px of its bottom above the floor.
    pub fn decor(&self, left: Tile, left_mod: u8, curr: Tile) -> Option<(Tex, f32)> {
        let (id, up) = back_decor(left, left_mod, curr, self.palace)?;
        Some((self.imgs.get(&id)?.clone(), up.max(0) as f32))
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
        // wall face by the runtime modifier (neighbours): none, right, left, both
        assert_eq!(tile_images(Wall, 0, dungeon), (None, Some(368)));
        assert_eq!([1, 2, 3, 0x81, 4].map(|m| tile_images(Wall, m, dungeon).1), [Some(370), Some(366), Some(364), Some(370), None]);
        assert_eq!(tile_images(LooseFloor, 0, dungeon), (None, None));
    }

    #[test]
    fn missing_images_fall_back() {
        // palace: no skeleton images (1230/1231) -> plain floor; no 4-bit wall faces -> procedural wall
        let palace = |id: u16| dungeon(id) && !(1230..=1231).contains(&id) && !(366..=374).contains(&id);
        assert_eq!(tile_images(Skeleton, 0, palace).0, Some([241, 242, 243]));
        assert_eq!(tile_images(LatticeSupport, 0, |id| palace(id) || id == 1202).0, Some([241, 242, 1202]));
        assert_eq!(tile_images(LatticeSupport, 0, palace).0, Some([241, 242, 243]));
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
        // walls: the base strip is part of the block texture (WALL_STRIP), no extra piece
        assert_eq!(ids(Wall, 1, false), Vec::<u16>::new());
        // exit door slices and stairs on ExitLeft only, torch stub, balcony and lattice palace only
        assert_eq!(pieces(ExitLeft, 0, false, dungeon), vec![(DoorSlice, 233), (DoorBottom, 234), (Stairs, 344)]);
        assert_eq!(ids(ExitRight, 0, false), Vec::<u16>::new());
        assert_eq!((ids(Torch, 0, false), ids(TorchDebris, 0, true)), (vec![346], vec![346]));
        assert_eq!(pieces(BalconyLeft, 0, true, palace), vec![(BackPanel, 1210), (BackTop, 1211)]);
        assert_eq!(ids(BalconyRight, 0, true), [1212, 1213]);
        // arcade (record +0 of types 26..29 = 1, 3, 4, 5) with the frieze 1209 on each
        assert_eq!(pieces(LatticeSupport, 0, true, palace), vec![(BackPanel, 1201), (BackTop, 1209)]);
        assert_eq!([SmallLattice, LatticeLeft, LatticeRight].map(|k| ids(k, 0, true)), [[1203, 1209], [1204, 1209], [1205, 1209]]);
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
    fn background_pieces_follow_draw_tile_right() {
        // left Empty by modifier (ds:27AA / 27AE), none above 3
        assert_eq!(back_decor(Empty, 0, Floor, false), None);
        assert_eq!(back_decor(Empty, 1, Empty, false), Some((324, 7)));
        assert_eq!(back_decor(Empty, 2, Floor, true), Some((325, 7)));
        assert_eq!(back_decor(Empty, 3, Empty, true), Some((326, -13)));
        assert_eq!(back_decor(Empty, 255, Empty, false), None);
        // palace tapestries right of a Doortop by its modifier (2 = 1282 on the plain background), none in the dungeon or before a wall
        assert_eq!(back_decor(Doortop, 2, Empty, true), Some((1282, -15)));
        assert_eq!(back_decor(DoortopWithFloor, 1, Floor, true), Some((1280, -15)));
        assert_eq!((back_decor(Doortop, 0, Empty, true), back_decor(Doortop, 3, Empty, true)), (Some((1278, -15)), None));
        assert_eq!((back_decor(Doortop, 2, Empty, false), back_decor(Doortop, 2, Wall, true)), (None, None));
        // left Floor (ds:27B2): dungeon skips 0, palace skips 1; above 3 counts as 0
        assert_eq!(back_decor(Floor, 0, Empty, false), None);
        assert_eq!(back_decor(Floor, 1, Empty, false), Some((244, 7)));
        assert_eq!(back_decor(Floor, 2, Floor, false), Some((245, 7)));
        assert_eq!(back_decor(Floor, 255, Empty, true), Some((244, 7)));
        assert_eq!(back_decor(Floor, 1, Empty, true), None);
        assert_eq!(back_decor(Floor, 255, Empty, false), None);
        // a wall cell hides it; other left kinds draw none here
        assert_eq!(back_decor(Empty, 1, Wall, false), None);
        assert_eq!(back_decor(Torch, 1, Empty, false), None);
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
    fn random_and_wall_modifier() {
        // seed 0: 2531011 >> 16 = 38; the tile seed adds ds:2288[row] + room + column
        assert_eq!(Rnd(0).next(99), 38);
        let mut r = Rnd(18);
        r.next(1);
        assert_eq!(wall_rnd(5, 1, 3).0, r.0);
        // runtime modifier: bit 0 = right neighbour a wall, bit 1 = left neighbour a wall
        assert_eq!([(false, false), (false, true), (true, false), (true, true)].map(|(l, r)| wall_modifier(l, r)), [0, 1, 2, 3]);
        assert_eq!(WALL_FACE.map(|f| f - 360), [8, 10, 6, 4]);
    }

    #[test]
    fn dungeon_wall_marks_follow_the_c5bf_path() {
        let all: Vec<_> = (1..=24).flat_map(|room| (0..30).map(move |i| (room, i / 10, i % 10))).collect();
        // deterministic, nothing for an impossible modifier, positions inside the 32 x 63 block
        for &(room, row, col) in &all {
            for m in 0..4 {
                let v = dungeon_wall_marks(room, row, col, m);
                assert_eq!(v, dungeon_wall_marks(room, row, col, m | 0x80));
                for k in &v {
                    assert!((371..=377).contains(&k.id) && (0..=63).contains(&k.up) && (-3..=40).contains(&k.x), "{k:?}");
                }
                // both bottom joints (371 / 372 at the bottom line) exactly when a wall touches the left one (modifier 2, 3)
                assert_eq!(v.iter().any(|k| k.up == 0 && k.id <= 372), m >= 2);
                // the middle joint (21 px up) whenever a neighbour is a wall
                assert_eq!(v.iter().any(|k| k.up == 21 && k.id <= 372), m >= 1);
            }
            assert!(dungeon_wall_marks(room, row, col, 4).is_empty());
        }
        // modifier 0: a crack in about 1 of 6 tiles, 374 / 375 at 58 or 41 px
        let n = all.iter().filter(|&&(r, w, c)| !dungeon_wall_marks(r, w, c, 0).is_empty()).count();
        assert!(n > 720 / 12 && n < 720 / 3, "{n}");
        // the plain top course (373) shows in about 1 of 4 walls with a wall on the right
        let t = all.iter().filter(|&&(r, w, c)| dungeon_wall_marks(r, w, c, 3).iter().any(|k| k.id == 373)).count();
        assert!(t > 720 / 8 && t < 720 / 2, "{t}");
    }

    #[test]
    fn palace_walls_follow_the_vga_path() {
        let pat = palace_pattern(7);
        assert_eq!(pat, palace_pattern(7));
        assert_ne!(pat, palace_pattern(8));
        for (row, courses) in pat.iter().enumerate() {
            for (c, bricks) in courses.iter().enumerate() {
                let base = if c % 2 == 0 { 0x66 } else { 0x61 };
                assert!(bricks.iter().all(|b| (base..base + 4).contains(b)), "row {row} course {c}");
                assert!(bricks.windows(2).all(|w| w[0] != w[1]), "neighbours differ");
            }
        }
        let strip = img(8, 7, &[1; 56]);
        let w = palace_wall(7, 1, 4, &pat, |_| Some(&strip));
        let (pw, ph) = PALACE_WALL;
        assert_eq!(w.len(), pw * ph);
        let c = &pat[1];
        let at = |x: usize, y: usize| w[y * pw + x];
        // courses: 20 rows (one brick), 21 rows (16 + 16), 19 rows (8 + 24), 3-row band; neighbours share the joint bricks
        assert_eq!((at(0, 10), at(31, 19)), (c[0][4] - 96, c[0][4] - 96));
        assert_eq!((at(0, 30), at(15, 40), at(16, 30), at(31, 40)), (c[1][4] - 96, c[1][4] - 96, c[1][5] - 96, c[1][5] - 96));
        assert_eq!((at(0, 50), at(7, 52), at(8, 50), at(31, 45)), (c[2][4] - 96, c[2][4] - 96, c[2][5] - 96, c[2][5] - 96));
        assert_eq!((at(10, 60), at(31, 61)), (c[3][4] - 96, c[3][4] - 96));
        // the speckle strips (all set here): top right corner (x 24, rows 0..6) and the band
        assert!(w[..7 * pw].iter().filter(|&&p| p == MONO).count() >= 56 && at(24, 0) == MONO && at(23, 0) != MONO);
        assert_eq!(at(0, 62), MONO);
        // without strips only the rectangles
        assert!(palace_wall(7, 1, 4, &pat, |_| None).iter().all(|&p| p != MONO));
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
