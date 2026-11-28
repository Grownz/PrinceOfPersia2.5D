//! Cutscenes (docs/cutscenes.md) in the 2.5D style of the levels: pop-core's `Cutscene` player drives the original sprites
//! (PV.DAT princess / Jaffar, KID.DAT kid and mouse, PRINCE.DAT torch flames, PV.DAT hourglass and bed) inside a princess
//! chamber built with the room builder (room.rs tiles, torch lights). The set stays at `ORIGIN`, away from the level rooms;
//! the camera moves there for the cutscenes and the title/story screens.
//! Per element: room, arcade, front pillar, window = tile geometry; actors, flames, hourglass, sand, bed = decoded original
//! sprites as textured quads (the bed is a flat painted carpet in PV 981, so it stays a sprite). With tile_textures original
//! the set uses the palace texture mapping of the levels (walls, floors, arches, arcade columns 295, holders 346 and, with
//! voxel objects, the voxel flames; docs/texture_inventory.md 17) and the two front columns from the cutscene's own PV 952;
//! the sky opening and procedural tiles keep the plain boxes. With character_style voxel
//! the actors (princess, Jaffar, kid, mouse) are voxel meshes built like the level characters (voxel.rs), anchored the same
//! way; flames, hourglass, sand and the bed stay sprites (set dressing, not characters).
use crate::room::{self, Chamber, Palette, TILE_H};
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::cutscene::{Actor, Cutscene, End, Scene, Slot, SpriteFile, STAR_COLOURS};
use pop_core::{dat::Dat, image};
use std::collections::HashMap;

/// Where the chamber set is built (far below the level rooms, so neither lights nor geometry mix).
pub const ORIGIN: Vec3 = Vec3::new(0.0, -80.0, 0.0);
/// The cutscene floor line (Char.y 166) is the floor of the set's bottom row.
const FLOOR_Y: f32 = 166.0;

/// What happens after a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Then {
    #[default]
    Play,
    Title,
    /// the ending pages (44, music 56)
    EndPages,
    /// back into the attract loop (the intro scene)
    Attract,
}

#[derive(Resource, Default)]
pub struct Cut {
    pub cs: Option<Cutscene>,
    pub scene: Option<Scene>,
    pub then: Then,
    /// set when the scene is over; flow.rs takes it
    pub ended: Option<End>,
    /// 60 Hz steps since the last cutscene frame
    sub: u32,
    /// a key since the last frame
    pub key: bool,
    pv: Option<Dat>,
    kid: Option<Dat>,
    prince: Option<Dat>,
    /// (file 0 PV / 1 KID / 2 PRINCE, resource) -> (material, width px, height px)
    mats: HashMap<(u8, u16), (Handle<StandardMaterial>, f32, f32)>,
    /// voxel actors: (file, resource, mirrored) -> mesh (None = did not decode); their shared lit material and the sprite quad
    vox: HashMap<(u8, u16, bool), Option<Handle<Mesh>>>,
    voxel_mat: Handle<StandardMaterial>,
    /// enhanced mode: the bed's lit copy (made on first use), so the actors' shadows fall on the carpet
    bed_lit: Handle<StandardMaterial>,
    quad: Handle<Mesh>,
    /// title/story screens: torch animation without a scene
    idle: u32,
}

impl Cut {
    pub fn load(dir: &str) -> Cut {
        let open = |f: &str| Dat::open(format!("{dir}/{f}")).ok();
        Cut { pv: open("PV.DAT"), kid: open("KID.DAT"), prince: open("PRINCE.DAT"), ..default() }
    }

    /// Frame (0..9) of torch flame `i`: the scene's while one runs (`in_scene`), else the title/story screens' idle animation.
    pub fn flame_frame(&self, i: usize, in_scene: bool) -> u8 {
        self.cs.as_ref().filter(|_| in_scene && self.ended.is_none()).map_or(((self.idle / 6 + i as u32 * 5) % 9) as u8, |cs| cs.torch[i])
    }

    /// Start `scene` (None if there is none or PV.DAT is missing: the caller goes straight on).
    pub fn start(&mut self, data: std::sync::Arc<pop_core::sim::Data>, scene: Scene, rem_min: i32, then: Then) -> bool {
        let Some(script) = scene.script(rem_min).filter(|_| self.pv.is_some()) else { return false };
        self.cs = Some(Cutscene::new(data, script));
        (self.scene, self.then, self.ended, self.key) = (Some(scene), then, None, false);
        self.sub = u32::MAX; // the first frame runs on the first step
        true
    }
}

#[derive(Component)]
pub struct ActorSprite(Slot);
#[derive(Component)]
pub struct GlassSprite(bool);
#[derive(Component)]
pub struct FlameSprite(usize);
#[derive(Component)]
pub struct StarDot(usize);
/// Full-screen fade / flash layer above everything.
#[derive(Component)]
pub struct FadeOverlay;

/// screen px (320 wide) -> world x, as kid.rs (half a tile to the left)
fn wx(px: f32) -> f32 {
    px / 32.0 - 0.5
}

/// screen line -> height above the set's floor
fn wy(line: f32) -> f32 {
    (FLOOR_Y - line) * TILE_H / pop_core::level::TILE_H_PX as f32
}

/// Where a point of the original's picture (screen px, screen line) lands on the arcade plane `ARCADE_Z`: the camera (`camera_on_set`)
/// sees that plane `(7.8 + 2.6) / 7.8` times farther away, so x and y are spread from the camera axis by that factor to
/// appear at the same screen position as the painted picture. (x, y above the set's floor, relative to `ORIGIN`.)
fn far(px: f32, line: f32) -> (f32, f32) {
    far_at(-room::ARCADE_Z, px, line)
}

/// The same for a plane `depth` units behind the picture plane (the far wall: `room::CHAMBER_DEPTH`).
fn far_at(depth: f32, px: f32, line: f32) -> (f32, f32) {
    let k = far_scale(depth);
    (CAM_X + (wx(px) - CAM_X) * k, CAM_Y - (CAM_Y - wy(line)) * k)
}

/// How much larger a picture pixel is on a plane `depth` behind the picture plane.
fn far_scale(depth: f32) -> f32 {
    (CAM_Z + depth) / CAM_Z
}
/// The camera of the set: level (no pitch), centred on the picture, at the distance where the 63 px rows of the 3 tile rows of 2
/// units fill a 45 degree view: the picture plane (z 0) then maps one to one onto the screen (320 x 200 px = 10 x 6.35 units).
const CAM_X: f32 = 4.6;
const CAM_Y: f32 = 2.1;
const CAM_Z: f32 = 7.66;

/// Depth (z) of the floor point the camera sees on picture line `line` (below the horizon, line 100): where the painted room's things
/// stand (thin columns on line 146, the front columns on 200).
fn floor_z(line: f32) -> f32 {
    CAM_Z - CAM_Y * CAM_Z / (CAM_Y - wy(line))
}

/// Torch x (ds:0E08/0E0C) and flame bottom line.
const TORCH_X: [f32; 2] = [93.0, 211.0];
const TORCH_BOTTOM: f32 = 116.0;
/// The front columns of PV 951 (a 10 px shaft from y 107 to the screen bottom, x 64..73 and 245..254): shaft left edge
/// and axis depth. The left one stands in the hole PV 981 leaves for it
/// (bed transparent at x 64..73, the column shows through: axis in the bed plane z 0.52); the right one is in front of the
/// actors (952, foretable).
#[allow(dead_code)]
const FRONT_COLUMNS: [(f32, f32); 2] = [(64.0, 0.52), (245.0, 1.0)];
/// Opaque shaft columns of PV 952 (first, last; 952 is drawn at x 240, so its shaft is 951's right column).
#[allow(dead_code)]
const FRONT_SHAFT: (usize, usize) = (5, 14);
/// Screen x of the thin arcade columns in PV 951.
const ARCADE_COLUMNS: [f32; 4] = [49.0, 112.0, 206.0, 269.0];
const STARS: [(f32, f32); 6] = [(20.0, 97.0), (16.0, 104.0), (23.0, 110.0), (17.0, 116.0), (24.0, 120.0), (18.0, 128.0)];

/// The set's room has no tiles: the architecture is the floor, the thin and front columns and the picture layers around them.
fn chamber_level(base: &pop_core::level::Level) -> pop_core::level::Level {
    let mut l = base.clone();
    l.fg = [0; 720];
    l.bg = [0; 720];
    l
}

/// The carpet of the bed as a mesh on the floor (y 0.003): one strip per picture line, each at the floor depth the camera sees that
/// line at, so the picture appears exactly as painted; u = x / 240 px, v = row / 41.
fn bed_mesh() -> Mesh {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let (rows, w) = (41usize, 240.0f32);
    let (mut pos, mut uv, mut idx) = (vec![], vec![], vec![]);
    for r in 0..=rows {
        let line = 142.0 + r as f32;
        let z = floor_z(line);
        let k = far_scale(-z);
        for (i, px) in [-(CARPET_EXT as f32), w].into_iter().enumerate() {
            pos.push([CAM_X + (wx(px) - CAM_X) * k, 0.003, z]);
            uv.push([i as f32, r as f32 / rows as f32]);
        }
        if r > 0 {
            let a = (r as u32 - 1) * 2;
            // counter-clockwise seen from above: the face (and with it the up normal of the double-sided lit enhanced material) points up
            idx.extend([a, a + 3, a + 1, a, a + 2, a + 3]);
        }
    }
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

/// A column built from a picture of its rows (`px` = palette indices, 0 transparent, one opaque run per row): the voxel column
/// (revolved about image x `axis`, like the levels' columns) and the flat textured box variant (cut out where 0).
#[allow(clippy::too_many_arguments)]
fn column_assets(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>, pal: &[[u8; 3]], w: usize, h: usize, px: &[u8], axis: f32) -> (crate::items::Column, crate::tiles::Tex) {
    let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
    let v = crate::voxel::VoxelModel::revolve(w, h, px, axis);
    let mesh = meshes.add(v.mesh(|p| {
        let [r, g, b] = rgb(p);
        Color::srgb_u8(r, g, b).to_linear().to_f32_array()
    }));
    let rgba: Vec<u8> = px.iter().flat_map(|&p| { let c = rgb(p); [c[0], c[1], c[2], if p == 0 { 0 } else { 255 }] }).collect();
    let mut img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), perceptual_roughness: 0.9, alpha_mode: AlphaMode::Mask(0.5), ..default() });
    (crate::items::Column { mesh, h: v.h, az: v.axis_z() }, crate::tiles::Tex { mat, w: w as f32, h: h as f32 })
}

/// One thin arcade column of PV 951 cut out of the picture: x 41..57 around the shaft axis 49, rows 90..147 (capital, shaft with
/// its rings, base). Only the contiguous run of column colours (palette 1..7) around the axis of each row is kept; the floor
/// (8..) and the arch pieces beside the capital are not.
const THIN: (usize, usize, usize, usize) = (41, 57, 90, 147);
const THIN_AXIS: usize = 49;

fn thin_column(im: &image::Image) -> (usize, usize, Vec<u8>) {
    let (x0, x1, y0, y1) = THIN;
    let w = x1 - x0;
    let mut px = vec![0u8; w * (y1 - y0)];
    for y in y0..y1 {
        let row = &im.pixels[y * im.width..][..im.width];
        let col = |x: usize| (1..=7).contains(&row[x]);
        if !col(THIN_AXIS) {
            continue;
        }
        let (mut a, mut b) = (THIN_AXIS, THIN_AXIS);
        while a > x0 && col(a - 1) {
            a -= 1;
        }
        while b + 1 < x1 && col(b + 1) {
            b += 1;
        }
        px[(y - y0) * w + a - x0..=(y - y0) * w + b - x0].copy_from_slice(&row[a..=b]);
    }
    (w, y1 - y0, px)
}

/// The dotted band across the far wall: PV 951 lines 119..131 from x 52 (colours 8, 9, 10 on black).
const BAND: (usize, usize, usize) = (52, 119, 132);

/// The front piers of PV 951 above the front columns (x0 = 56 and 241): the body (lattice-framed pier, lines 0..77), the big capital
/// (78..96) and the lower capital (97..107), as rectangles of the picture.
const PIERS: [usize; 2] = [56, 241];

fn in_pier(x0: usize, x: usize, y: usize) -> bool {
    let inside = |a: usize, b: usize| x + 5 >= x0 + a && x + 5 < x0 + b;
    match y {
        0..=77 => inside(5, 31),
        78..=96 => inside(0, 36),
        97..=107 => inside(7, 29),
        _ => false,
    }
}

/// The front columns of PV 951: x 57..81 / 238..262 around the shaft axes 69 / 250, lines 107..199 (capital, shaft with its rings).
const FRONT_COL: [(usize, usize); 2] = [(57, 69), (238, 250)];
const FRONT_TOP: usize = 107;

/// The rows of PV 951 around an axis as one opaque run of column colours (1..7) per row, like `thin_column`.
fn run_column(im: &image::Image, x0: usize, x1: usize, axis: usize, y0: usize, y1: usize) -> (usize, usize, Vec<u8>) {
    let w = x1 - x0;
    let mut px = vec![0u8; w * (y1 - y0)];
    for y in y0..y1 {
        let row = &im.pixels[y * im.width..][..im.width];
        let col = |x: usize| (1..=7).contains(&row[x]);
        if !col(axis) {
            continue;
        }
        let (mut a, mut b) = (axis, axis);
        while a > x0 && col(a - 1) {
            a -= 1;
        }
        while b + 1 < x1 && col(b + 1) {
            b += 1;
        }
        px[(y - y0) * w + a - x0..=(y - y0) * w + b - x0].copy_from_slice(&row[a..=b]);
    }
    (w, y1 - y0, px)
}

/// An image layer: the rectangle `x0..x1` x `y0..y1` of PV 951 (index 0 and the pixels `keep` rejects transparent) as an unlit cut-out
/// sheet on the plane `depth` units behind the picture plane (negative: in front), covering the picture pixels `x0 + shift ..`;
/// `flip` mirrors the image (the copies beside the picture, so the arcade goes on to the sides of a wide view).
#[allow(clippy::too_many_arguments)]
fn layer(cmds: &mut Commands, quad: &Handle<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>, im: &image::Image, pal: &[[u8; 3]], rect: (usize, usize, usize, usize), keep: &dyn Fn(usize, usize) -> bool, depth: f32, shift: f32, flip: bool) {
    let (x0, y0, x1, y1) = rect;
    let (w, h) = (x1 - x0, y1 - y0);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in y0..y1 {
        for i in 0..w {
            let x = if flip { x1 - 1 - i } else { x0 + i };
            let p = im.pixels[y * im.width + x];
            let c = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
            rgba.extend_from_slice(&[c[0], c[1], c[2], if p != 0 && keep(x, y) { 255 } else { 0 }]);
        }
    }
    let mut img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() });
    let (xa, xb) = (far_at(depth, x0 as f32 + shift, 0.0).0, far_at(depth, x1 as f32 + shift, 0.0).0);
    let (yt, yb) = (far_at(depth, 0.0, y0 as f32).1, far_at(depth, 0.0, y1 as f32).1);
    // a single sheet (a box would show its side faces at the edges of a wide view)
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d(mat), Transform::from_translation(ORIGIN + Vec3::new((xa + xb) / 2.0, (yt + yb) / 2.0, -depth)).with_scale(Vec3::new(xb - xa, yt - yb, 1.0)), Chamber));
}

/// Everything of the princess chamber that depends on the `Palette` (tiles, columns, torch holders), plus the sky and the
/// stars. Respawned by `chamber_live` when the tile settings change.
fn chamber_parts(cmds: &mut Commands, g: &Game, p: &Palette, cut: &Cut, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, imgs: &mut Assets<Image>) {
    let level = chamber_level(&g.levels[g.levels.len() - 1]);
    room::spawn_chamber(cmds, &level, 1, ORIGIN, p);
    let o = ORIGIN;
    let pv951 = cut.pv.as_ref().and_then(|d| Some((image::decode(d.get(951)?)?, image::palette(d.get(950)?)?)));
    // depths from the picture: the lines the things stand on (floor_z): the front columns on 200, the thin ones on 146, the far
    // floor edge on 144
    let zf = floor_z(200.0);
    let z = room::ARCADE_Z;
    room::chamber_floor(cmds, p, o, zf + 1.0);
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let sheet = meshes.add(Rectangle::new(1.0, 1.0));
    let boxed = |lo: Vec3, hi: Vec3| Transform::from_translation(o + (lo + hi) / 2.0).with_scale(hi - lo);
    let sky = mats.add(StandardMaterial { base_color: Color::srgb(0.02, 0.03, 0.16), unlit: true, ..default() });
    let black = mats.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() });
    let (top, d) = (3.0 * TILE_H, room::CHAMBER_DEPTH);
    // the far wall: black below the arcade's arches (line 76), the night sky above them (it shows through the arches' transparent
    // dither); the wall and the window of the left wall are one plane
    cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(sky), boxed(Vec3::new(-12.0, far_at(d, 0.0, 76.0).1, -d), Vec3::new(22.0, top * 2.0, -d + 0.01)), Chamber));
    cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(black), boxed(Vec3::new(-12.0, -room::SLAB, -d), Vec3::new(22.0, far_at(d, 0.0, 76.0).1, -d + 0.01)), Chamber));
    let star_mats: Vec<Handle<StandardMaterial>> =
        [0.35, 0.7, 1.0].iter().map(|&v| mats.add(StandardMaterial { base_color: Color::srgb(v, v, v), emissive: LinearRgba::rgb(v * 3.0, v * 3.0, v * 3.0), unlit: true, ..default() })).collect();
    for (i, &(x, y)) in STARS.iter().enumerate() {
        let (sx, sy) = far_at(d - 0.05, x, y);
        cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(star_mats[2].clone()), Transform::from_translation(o + Vec3::new(sx, sy, -d + 0.05)).with_scale(Vec3::splat(0.07)), StarDot(i), Chamber));
    }
    cmds.insert_resource(StarMats(star_mats));
    if let Some((im, pal)) = pv951.as_ref() {
        let none: &dyn Fn(usize, usize) -> bool = &|_, _| true;
        // the arcade: lattice ceiling, the big arch and the small ones, the capitals on the thin columns (lines 0..89), on the plane of
        // the thin columns; without the front piers; a mirrored copy on each side for a wide view
        let arcade: &dyn Fn(usize, usize) -> bool = &|x, y| !PIERS.iter().any(|&x0| in_pier(x0, x, y));
        // (columns 1..318: the picture's outermost columns are a dark edge line, which the mirror would double)
        layer(cmds, &sheet, mats, imgs, im, pal, (1, 0, 319, 90), arcade, -z, 0.0, false);
        layer(cmds, &sheet, mats, imgs, im, pal, (1, 0, 319, 90), arcade, -z, 318.0, true);
        layer(cmds, &sheet, mats, imgs, im, pal, (1, 0, 319, 90), arcade, -z, -318.0, true);
        // the left wall with its arched door (frame lines 88..143, x 0..41) on the far wall's plane, the door's inside transparent
        layer(cmds, &sheet, mats, imgs, im, pal, (0, 88, 41, 144), none, d - 0.03, 0.0, false);
        // the dotted band across the far wall (lines 119..131 from x 52), repeated sideways
        let band: &dyn Fn(usize, usize) -> bool = &|x, y| x >= BAND.0 && y >= BAND.1 && y < BAND.2 && (8..=10).contains(&im.pixels[y * im.width + x]);
        for i in -1..=1 {
            layer(cmds, &sheet, mats, imgs, im, pal, (BAND.0, BAND.1, 320, BAND.2), band, d - 0.01, (320 - BAND.0) as f32 * i as f32, false);
        }
        // the front piers on the plane of the front columns, in front of everything
        for x0 in PIERS {
            let keep: &dyn Fn(usize, usize) -> bool = &move |x, y| in_pier(x0, x, y);
            layer(cmds, &sheet, mats, imgs, im, pal, (x0 - 5, 0, x0 + 31, 108), keep, -zf, 0.0, false);
        }
    }
    // the four thin arcade columns of PV 951 (screen px 49, 112, 206, 269; base line 147, capital top 90), cut out of the picture:
    // voxel (revolved) or flat like the front columns, `k` times larger because their plane is farther than the picture's
    let thin = pv951.as_ref().map(|(im, pal)| {
        let (w, h, px) = thin_column(im);
        column_assets(meshes, mats, imgs, pal, w, h, &px, (THIN_AXIS - THIN.0) as f32)
    });
    let k = far_scale(-z);
    let tan = mats.add(StandardMaterial { base_color: Color::srgb(0.93, 0.76, 0.48), perceptual_roughness: 0.8, ..default() });
    let shaft = meshes.add(Cylinder::new(0.07, 1.0));
    for px in ARCADE_COLUMNS {
        let (left, y0) = far(px - (THIN_AXIS - THIN.0) as f32, THIN.3 as f32);
        let y1 = far(px, THIN.2 as f32).1;
        if !thin.as_ref().is_some_and(|(v, t)| room::chamber_column(cmds, p, v, t, Vec3::new(o.x + left, o.y + y0, o.z + z), o.y + y1, k)) {
            let (x, _) = far(px, 0.0);
            cmds.spawn((Mesh3d(shaft.clone()), MeshMaterial3d(tan.clone()), Transform::from_translation(o + Vec3::new(x, (y0 + y1) / 2.0, z)).with_scale(Vec3::new(1.0, y1 - y0, 1.0)), Chamber));
        }
    }
    // the two front columns of PV 951 (capital and shaft, lines 107..199, cut out of the picture like the thin ones), standing on
    // the floor at the depth of line 200, `kf` times smaller than the picture's pixels
    let kf = far_scale(-zf);
    for (x0, axis) in FRONT_COL {
        let front = pv951.as_ref().map(|(im, pal)| {
            let (w, h, px) = run_column(im, x0, x0 + 24, axis, FRONT_TOP, 200);
            column_assets(meshes, mats, imgs, pal, w, h, &px, (axis - x0) as f32)
        });
        let (left, y1) = far_at(-zf, x0 as f32, FRONT_TOP as f32);
        if !front.as_ref().is_some_and(|(v, t)| room::chamber_column(cmds, p, v, t, Vec3::new(o.x + left, o.y, o.z + zf), o.y + y1, kf)) {
            let r = 5.0 * kf / 32.0;
            let x = far_at(-zf, axis as f32, 0.0).0;
            room::chamber_box(cmds, p, "stone", Vec3::new(o.x + x - r, o.y, o.z + zf - r), Vec3::new(o.x + x + r, o.y + y1, o.z + zf + r));
        }
    }
    // torches: holder (the level's 346 with original tiles) and the level light (a `TorchLight` like the levels': enhanced mode
    // puts it at the flame's pixels, `room::flicker`; classic keeps the old place)
    let flames = room::chamber_flames(p);
    for (i, &x) in TORCH_X.iter().enumerate() {
        let (cx, cy) = far(x + 8.0, TORCH_BOTTOM);
        if !room::chamber_holder(cmds, p, o + Vec3::new(cx, cy, 0.0), k) {
            room::chamber_box(cmds, p, "dark", o + Vec3::new(cx - 0.08, cy - 0.25, room::ARCADE_Z - 0.1), o + Vec3::new(cx + 0.08, cy + 0.02, room::ARCADE_Z + 0.05));
        }
        // the voxel flame's lower left corner (as `render` places it); the light stands 0.17 (a level's offset) in front of the
        // flame's plane, scaled like the flame; sprite flames: about the middle of the sprite
        let (lo, rest) = match flames {
            Some((_, _, rel)) => {
                let lo = o.truncate() + Vec2::new(cx + rel.x * k, cy + rel.y * k);
                (lo, lo.extend(o.z + rel.z + 0.17 * k))
            }
            None => (Vec2::ZERO, Vec3::new(o.x + cx, o.y + far(x + 8.0, TORCH_BOTTOM - 8.0).1, o.z + room::ARCADE_Z + 0.4)),
        };
        let classic = o + Vec3::new(cx, cy + 0.6, room::ARCADE_Z + 0.6);
        cmds.spawn((room::torch_light(), room::TorchLight(0, i as u8, 1.0, lo), room::ChamberTorch { i, classic, rest, k, voxel: flames.is_some() }, Transform::from_translation(classic), Chamber));
    }
    // the air (enhanced + atmosphere, fx.rs): dust motes round the torches' glows and embers over the flames, as in a palace room.
    // No light shafts: the chamber has no window in its back wall, the arcade opens onto the night sky, not onto a sun. The
    // set is open, so the dust has no solid tiles (mask 0).
    let spots: Vec<Vec2> = TORCH_X.iter().map(|&x| { let (cx, cy) = far(x + 8.0, TORCH_BOTTOM); Vec2::new(cx, cy + 0.05) + o.truncate() }).collect();
    if let Some((c, size)) = crate::fx::dust_rect(&spots) {
        let mut a = [Vec2::ZERO; 4];
        a[..spots.len()].copy_from_slice(&spots);
        let z = o.z + room::ARCADE_Z + 0.4;
        cmds.spawn((crate::fx::FxSpawn::Dust(a, spots.len(), 0, o.truncate()), Transform::from_translation(c.extend(z)).with_scale(size.extend(1.0)), Chamber));
        for s in &spots {
            let (c, size) = crate::fx::ember_rect(s.x, s.y - 0.05);
            let c = Vec2::new(c.x, s.y - 0.05 + (c.y - (s.y - 0.05)) * k); // the rect rises `k` times higher on this plane
            cmds.spawn((crate::fx::FxSpawn::Embers, Transform::from_translation(c.extend(z)).with_scale((size * k).extend(1.0)), Chamber));
        }
    }
}

/// The chamber follows the tile / column / object settings: it is respawned when the `Palette` is rebuilt.
pub fn chamber_live(mut cmds: Commands, g: Res<Game>, p: Res<Palette>, cut: Res<Cut>, old: Query<Entity, With<Chamber>>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut imgs: ResMut<Assets<Image>>) {
    if !p.is_changed() || p.is_added() {
        return;
    }
    for e in &old {
        cmds.entity(e).despawn();
    }
    chamber_parts(&mut cmds, &g, &p, &cut, &mut meshes, &mut mats, &mut imgs);
}

pub fn setup(mut cmds: Commands, g: Res<Game>, p: Res<Palette>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut imgs: ResMut<Assets<Image>>, mut cut: ResMut<Cut>) {
    chamber_parts(&mut cmds, &g, &p, &cut, &mut meshes, &mut mats, &mut imgs);
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    cut.quad = quad.clone();
    // torch flames: the original sprite, or the level's voxel flame
    for i in 0..TORCH_X.len() {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, FlameSprite(i), bevy::light::NotShadowCaster));
    }
    for s in [Slot::Kid, Slot::Princess] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, ActorSprite(s)));
    }
    for stream in [false, true] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, GlassSprite(stream)));
    }
    // the bed on its carpet (PV 981, 240x41 at (0, 142)): lying on the floor, its rows on the floor lines the camera sees them on
    cmds.spawn((Mesh3d(meshes.add(bed_mesh())), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, Bed));
    cmds.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(10), BackgroundColor(Color::NONE), FadeOverlay));
}

#[derive(Resource)]
pub struct StarMats(Vec<Handle<StandardMaterial>>);
#[derive(Component)]
pub struct Bed;

/// Camera on the set: level and centred, so the picture plane is the screen (see `CAM_Z`).
pub fn camera_on_set() -> Transform {
    Transform::from_translation(ORIGIN + Vec3::new(CAM_X, CAM_Y, CAM_Z)).looking_at(ORIGIN + Vec3::new(CAM_X, CAM_Y, 0.0), Vec3::Y)
}

/// The bed picture (PV 981, 240 x 41) runs off the picture's left edge; a wide view shows its cut. The carpet's left end is completed
/// the way the picture shows its beginning (the tan margin already slants down to the left there, 2.5 px per row): below line 18
/// the margin goes on down to the bottom-left corner, its outer rows are the right end's profile mirrored (tan, outline, the blue and
/// red border with its dashes), the corner between it and the bed is the border's red, the bottom border rows go on with their
/// 7-px dash pattern and the bottom margin runs out to the corner. `CARPET_EXT` px are added to the left.
const CARPET_EXT: usize = 56;

fn extend_carpet(im: &image::Image) -> image::Image {
    let (w, h, ext) = (im.width, im.height, CARPET_EXT);
    let nw = w + ext;
    let mut px = vec![0u8; nw * h];
    for y in 0..h {
        px[y * nw + ext..(y + 1) * nw].copy_from_slice(&im.pixels[y * w..(y + 1) * w]);
    }
    if w >= 233 && h >= 41 {
        let at = |x: usize, y: usize| im.pixels[y * w + x];
        let (red, tan) = (at(0, 35), at(0, 40));
        for r in 19..41usize {
            let xe = ((15.0 - 2.5 * (r as f32 - 12.0)).floor() as i32 + ext as i32).max(0) as usize;
            if r >= 37 {
                px[r * nw + xe..r * nw + ext].fill(tan);
                continue;
            }
            let xt = (0..w).rev().find(|&x| at(x, r) != 0).unwrap_or(w - 1).max(14);
            for i in 0..14 {
                if xe + i < ext {
                    px[r * nw + xe + i] = at(xt - i, r);
                }
            }
            for x in xe + 14..ext {
                px[r * nw + x] = if r >= 34 { at(150 + (x as i32 - ext as i32 - 150).rem_euclid(7) as usize, r) } else { red };
            }
        }
    }
    image::Image { width: nw, height: h, bpp: im.bpp, pixels: px }
}

/// Material of an original image: colour 0 transparent, unlit like the level sprites.
fn material(c: &mut Cut, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, file: u8, id: u16, pal_id: u16) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    if let Some(m) = c.mats.get(&(file, id)) {
        return Some(m.clone());
    }
    let dat = match file { 0 => c.pv.as_ref(), 1 => c.kid.as_ref(), _ => c.prince.as_ref() }?;
    let pal = image::palette(dat.get(pal_id)?)?;
    let im = image::decode(dat.get(id)?)?;
    let im = if (file, id) == (0, 981) { extend_carpet(&im) } else { im };
    let rgba: Vec<u8> = im.pixels.iter().flat_map(|&p| {
        let [r, g, b] = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        [r, g, b, if p == 0 { 0 } else { 255 }]
    }).collect();
    let mut img = Image::new(Extent3d { width: im.width as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let m = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: true, ..default() });
    let e = (m, im.width as f32, im.height as f32);
    c.mats.insert((file, id), e.clone());
    Some(e)
}

/// Voxel mesh of an actor image (see `material`), mirrored when the actor faces right.
fn voxel(c: &mut Cut, meshes: &mut Assets<Mesh>, file: u8, id: u16, pal_id: u16, mirrored: bool, smooth: bool) -> Option<Handle<Mesh>> {
    if let Some(m) = c.vox.get(&(file, id, mirrored)) {
        return m.clone();
    }
    let dat = match file { 0 => c.pv.as_ref(), 1 => c.kid.as_ref(), _ => c.prince.as_ref() };
    let m = dat.and_then(|dat| Some((image::palette(dat.get(pal_id)?)?, image::decode(dat.get(id)?)?))).map(|(pal, im)| {
        let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        let model = crate::voxel::VoxelModel::character(im.width, im.height, &im.pixels, mirrored, rgb);
        meshes.add(model.mesh_with(
            |p| {
                let [r, g, b] = rgb(p);
                Color::srgb_u8(r, g, b).to_linear().to_f32_array()
            },
            smooth,
        ))
    });
    c.vox.insert((file, id, mirrored), m.clone());
    m
}

/// Voxel actors stand a little behind the sprite depth would put their centre: their back clears the bed carpet (z 0.52)
/// and the figure stays behind the hourglass.
const VOXEL_DZ: f32 = 0.04;

/// Place a sprite on the plane `z`: left edge `x` and bottom `y` are those of the picture plane (z 0, `wx` / `wy`); the plane's
/// perspective scale moves and sizes it so it lands on the same screen position. Mirrored when `flip`.
fn place(t: &mut Transform, x: f32, y: f32, z: f32, w: f32, h: f32, flip: bool) {
    let k = far_scale(-z);
    let (x, y) = (CAM_X + (x - CAM_X) * k, CAM_Y - (CAM_Y - y) * k);
    let (sw, sh) = (w / 32.0 * k, h * TILE_H / pop_core::level::TILE_H_PX as f32 * k);
    t.translation = ORIGIN + Vec3::new(x + sw / 2.0, y + sh / 2.0, z);
    t.scale = Vec3::new(if flip { -sw } else { sw }, sh, 1.0);
}

/// An actor like a level character (kid.rs drawing rule): images face left, mirrored when facing right. A bottom line
/// below the floor line (y 167..169: crouching princess, mouse) is drawn on the floor but nearer the camera.
fn actor(cs: &Cutscene, a: &Actor, w: f32, h: f32, z: f32, t: &mut Transform) {
    let f = cs.frame_rec(a);
    let sx = crate::kid::screen_x(a.x, f.dx, f.flags, a.facing_left) as f32;
    let bottom = a.y.wrapping_add(f.dy as u8) as f32;
    let depth = (bottom - FLOOR_Y).max(0.0) * 0.1;
    let left = if a.facing_left { wx(sx) } else { wx(sx) - w / 32.0 };
    place(t, left, wy(bottom.min(FLOOR_Y)), z + depth, w, h, !a.facing_left);
}

/// FixedUpdate in Cutscene: one cutscene frame every `frame_time` steps; started sounds go to the audio path.
pub fn fixed(mut c: ResMut<Cut>, mut cmds: Commands, s: Res<crate::audio::Sounds>, mut a: ResMut<crate::audio::AudioState>, playing: Query<(Entity, &crate::audio::Sfx)>) {
    let c = &mut *c;
    let Some(cs) = c.cs.as_mut() else { return };
    if c.ended.is_some() {
        return;
    }
    c.sub = c.sub.saturating_add(1);
    if c.sub < cs.frame_time as u32 {
        return;
    }
    c.sub = 0;
    let r = cs.frame(std::mem::take(&mut c.key), a.busy > 0);
    for &id in &cs.started {
        crate::audio::play(&mut cmds, &s, &mut a, id, &playing);
    }
    if r == Some(End::Skipped) {
        crate::audio::stop_all(&mut cmds, &mut a, &playing);
    }
    c.ended = r;
}

/// Update in Cutscene: remember a key for the next frame (any key or Shift, like idle() in the original).
/// Applies `voxel_shading` and `voxel_normals` to the cutscene actors when the settings change (see `kid::live_look`).
pub fn live_look(set: Res<crate::settings::Settings>, mut c: ResMut<Cut>, mut mats: ResMut<Assets<StandardMaterial>>) {
    if !set.is_changed() || set.is_added() {
        return;
    }
    c.vox.clear();
    if let Some(mut m) = mats.get_mut(&c.voxel_mat) {
        *m = crate::voxel::character_material(set.unlit_voxels(), false);
    }
}

pub fn latch(k: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut c: ResMut<Cut>) {
    if crate::flow::skip_key(&k, &pads) {
        c.key = true;
    }
}

/// Draw the set: actors, hourglass, flames, stars, the bed; fade and flash. `fade`: the title/story screens' own fade
/// (0 = black) while no scene runs.
#[allow(clippy::too_many_arguments)]
pub fn render(
    state: Res<State<crate::flow::Screen>>,
    mut c: ResMut<Cut>,
    f: Res<crate::flow::Flow>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    star_mats: Res<StarMats>,
    win: Single<&Window, With<bevy::window::PrimaryWindow>>,
    style: Res<crate::settings::CharacterStyle>,
    mut meshes: ResMut<Assets<Mesh>>,
    pal: Res<Palette>,
    mode: Res<crate::settings::RenderMode>,
    set: Res<crate::settings::Settings>,
    view: Res<crate::camera::View>,
    mut q: ParamSet<(
        Query<(&ActorSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
        Query<(&GlassSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>)>,
        Query<(&FlameSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
        Query<(&StarDot, &mut MeshMaterial3d<StandardMaterial>)>,
        Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<Bed>>,
        Query<&mut BackgroundColor, With<FadeOverlay>>,
        Query<&mut Transform, (With<Camera3d>, Without<crate::mirror::MirrorCam>)>,
    )>,
) {
    use crate::flow::Screen;
    let s = *state.get();
    let in_scene = s == Screen::Cutscene && c.cs.is_some() && c.ended.is_none(); // ended (e.g. skipped): no stale frame
    let on_set = in_scene || s == Screen::Title;
    // camera: the set for cutscenes and the title/story screens, the level otherwise
    for mut t in &mut q.p6() {
        let want = if on_set { camera_on_set() } else { crate::camera::transform(win.width() / win.height().max(1.0), &view) };
        if *t != want {
            *t = want;
        }
    }
    c.idle = c.idle.wrapping_add(1);
    let c = &mut *c;
    let scene = c.cs.take(); // put back below (material() borrows the cache)
    let cs = scene.as_ref().filter(|_| in_scene);
    // actors: kid slot behind the princess (INFERRED draw order, docs/cutscenes.md 1.4)
    let voxels = *style == crate::settings::CharacterStyle::Voxel;
    if voxels && c.voxel_mat == Handle::default() {
        c.voxel_mat = mats.add(crate::voxel::character_material(set.unlit_voxels(), false)); // made on first use: sprite style keeps its assets
    }
    for (ActorSprite(slot), mut t, mut vis, mut mat, mut mesh) in &mut q.p0() {
        let a = cs.and_then(|cs| if *slot == Slot::Kid { cs.kid.as_ref() } else { cs.princess.as_ref() }).filter(|_| !cs.unwrap().blank);
        let shown = a.and_then(|a| {
            let cs = cs.unwrap();
            let (file, id, pal) = cs.sprite(a)?;
            let file = if file == SpriteFile::Pv { 0 } else { 1 };
            let (m, w, h) = material(c, &mut imgs, &mut mats, file, id, pal)?;
            actor(cs, a, w, h, if *slot == Slot::Kid { 0.0 } else { 0.02 }, &mut t);
            // voxel style: the same rectangle, mesh origin at its bottom-left corner, one voxel = one pixel (kid.rs `show`)
            if let Some(v) = voxels.then(|| voxel(c, &mut meshes, file, id, pal, t.scale.x < 0.0, set.smooth_normals())).flatten() {
                let half = Vec3::new(t.scale.x.abs(), t.scale.y, 0.0) / 2.0;
                let k = t.scale.x.abs() * 32.0 / w; // the plane's scale (place)
                t.translation = t.translation - half + Vec3::Z * VOXEL_DZ;
                t.scale = Vec3::new(1.0 / 32.0, TILE_H / pop_core::level::TILE_H_PX as f32, 1.0 / 32.0) * k;
                mesh.set_if_neq(Mesh3d(v));
                return Some(c.voxel_mat.clone());
            }
            if mesh.0 != c.quad {
                mesh.0 = c.quad.clone();
            }
            Some(m)
        });
        set_vis(&mut vis, &mut mat, shown);
    }
    // hourglass (midtable, behind the actors) and the sand stream (foretable, in front)
    let (glass, stream) = cs.map_or((None, None), |cs| cs.hourglass_images());
    for (GlassSprite(is_stream), mut t, mut vis, mut mat) in &mut q.p1() {
        let id = if *is_stream { stream } else { glass };
        let shown = id.and_then(|id| {
            let (m, w, h) = material(c, &mut imgs, &mut mats, 0, id, 950)?;
            if *is_stream {
                place(&mut t, wx(160.0), wy(164.0), floor_z(168.0) + 0.05, w, h, false);
            } else {
                place(&mut t, wx(152.0), wy(168.0), floor_z(168.0), w, h, false); // ybottom 168: on the floor, where the camera sees that line
            }
            Some(m)
        });
        set_vis(&mut vis, &mut mat, shown);
    }
    // torch flames: PRINCE.DAT 151 + frame (palette 150); voxel objects: the level's voxel flame of that frame on its holder
    let idle = c.idle;
    let vflames = room::chamber_flames(&pal);
    for (FlameSprite(i), mut t, mut vis, mut mat, mut mesh) in &mut q.p2() {
        let frame = cs.map_or(((idle / 6 + *i as u32 * 5) % 9) as u8, |cs| cs.torch[*i]);        let shown = match vflames {
            Some((frames, fmat, rel)) => on_set.then(|| {
                let (fx, fy) = far(TORCH_X[*i] + 8.0, TORCH_BOTTOM);
                let k = far_scale(-room::ARCADE_Z);
                *t = Transform::from_translation(ORIGIN + Vec3::new(fx + rel.x * k, fy + rel.y * k, rel.z)).with_scale(room::voxel_px() * k);
                mesh.set_if_neq(Mesh3d(frames[frame as usize % frames.len()].mesh.clone()));
                fmat.clone()
            }),
            None => on_set.then(|| material(c, &mut imgs, &mut mats, 2, 151 + frame as u16, 150)).flatten().map(|(m, w, h)| {
                place(&mut t, wx(TORCH_X[*i]), wy(TORCH_BOTTOM), room::ARCADE_Z + 0.2, w, h, false);
                m
            }),
        };
        set_vis(&mut vis, &mut mat, shown);
    }
    if let Some(cs) = cs {
        for (StarDot(i), mut mat) in &mut q.p3() {
            let k = match STAR_COLOURS[cs.stars[*i].2 as usize] { 8 => 0, 7 => 1, _ => 2 };
            if mat.0 != star_mats.0[k] {
                mat.0 = star_mats.0[k].clone();
            }
        }
    }
    for (mut t, mut vis, mut mat) in &mut q.p4() {
        let shown = on_set.then(|| material(c, &mut imgs, &mut mats, 0, 981, 980)).flatten().map(|(m, w, h)| {
            let _ = (w, h);
            *t = Transform::from_translation(ORIGIN);
            if *mode != crate::settings::RenderMode::Enhanced {
                return m;
            }
            // an unlit material receives no shadows: enhanced draws the carpet lit (the sun's shadows of the actors in front)
            if c.bed_lit == Handle::default() {
                let lit = mats.get(&m).map(lit_carpet);
                c.bed_lit = lit.map_or(m.clone(), |l| mats.add(l));
            }
            c.bed_lit.clone()
        });
        set_vis(&mut vis, &mut mat, shown);
    }
    // fade to black (1 - fade) or the white flash (the original flickers; shown as a visible flash like SDLPoP)
    let tint = match cs {
        Some(cs) if cs.flash_now.is_some() => Color::srgba(1.0, 1.0, 1.0, 0.7),
        Some(cs) => Color::srgba(0.0, 0.0, 0.0, 1.0 - cs.fade),
        None if s == Screen::Title => Color::srgba(0.0, 0.0, 0.0, 1.0 - f.fade()),
        None if s == Screen::Cutscene => Color::BLACK,
        None => Color::NONE,
    };
    for mut bg in &mut q.p5() {
        bg.set_if_neq(BackgroundColor(tint));
    }
    c.cs = scene;
}

/// The lit version of a sprite material (the bed carpet in enhanced mode): same texture and alpha mask, matte like the
/// tile textures (roughness 0.9, low reflectance, so no sheen washes out the painted colours).
fn lit_carpet(unlit: &StandardMaterial) -> StandardMaterial {
    StandardMaterial { unlit: false, perceptual_roughness: 0.9, reflectance: 0.2, ..unlit.clone() }
}

fn set_vis(vis: &mut Visibility, mat: &mut MeshMaterial3d<StandardMaterial>, m: Option<Handle<StandardMaterial>>) {
    match m {
        Some(m) => {
            if *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
            if mat.0 != m {
                mat.0 = m;
            }
        }
        None => {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Princess x 120, frame 11 (dx 0, flags 0x80) facing left. docs/cutscenes.md 5 says parity 0 -> px 141; the in-game rule
    /// (kid.rs, mechanics.md 7.3) gives parity 1 -> px 142, and the cutscene uses that rule unchanged (1 px, UNVERIFIED which).
    #[test]
    fn princess_screen_x() {
        assert_eq!(crate::kid::screen_x(120, 0, 0x80, true), 142);
        assert!((super::wx(141.0) - (141.0 / 32.0 - 0.5)).abs() < 1e-6);
        assert_eq!(super::wy(166.0), 0.0);
    }

    /// Enhanced carpet: lit (an unlit material gets no shadows), same texture and alpha mask as the classic sprite.
    #[test]
    fn lit_carpet_receives_shadows() {
        use bevy::prelude::*;
        let u = StandardMaterial { base_color_texture: Some(Handle::default()), unlit: true, alpha_mode: AlphaMode::Mask(0.5), double_sided: true, cull_mode: None, ..default() };
        let l = super::lit_carpet(&u);
        assert!(!l.unlit && l.base_color_texture == u.base_color_texture && l.alpha_mode == u.alpha_mode && l.double_sided && l.cull_mode.is_none());
        assert_eq!(l.base_color, u.base_color);
    }

    /// The carpet mesh declares +y normals; every triangle must also face up (counter-clockwise from above), or the double-sided
    /// lit enhanced material flips the shading normal away from the sun and the carpet turns black.
    #[test]
    fn bed_mesh_faces_up() {
        use bevy::mesh::{Indices, VertexAttributeValues};
        let m = super::bed_mesh();
        let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(bevy::prelude::Mesh::ATTRIBUTE_POSITION) else { panic!("positions") };
        let Some(Indices::U32(idx)) = m.indices() else { panic!("indices") };
        assert!(!idx.is_empty());
        for t in idx.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| p[i as usize]);
            let (u, v) = ([b[0] - a[0], b[2] - a[2]], [c[0] - a[0], c[2] - a[2]]);
            // y component of u x v = u.z * v.x - u.x * v.z
            assert!(u[1] * v[0] - u[0] * v[1] > 0.0, "triangle {t:?} faces down");
        }
    }

    /// The set's pieces through the palace mapping (docs/texture_inventory.md 17): every lattice screen
    /// gets its arch piece and frieze, the columns (set and added pillars) image 295, the torches holder 346; walls are built palace faces.
    #[test]
    fn chamber_pieces_use_the_palace_mapping() {
        use crate::tiles::{pieces, tile_images, Role};
        use pop_core::level::{Level, Tile};
        // VPALACE (texture_inventory 2.2): the base-200 ids plus the palace slots 1201..1213
        let palace = |id: u16| id < 1200 || (1201..=1213).contains(&id);
        // the set's room has no tiles of its own: floor, columns and picture layers are built around it
        let l = super::chamber_level(&Level::parse(&[0u8; 4096]).expect("zero level"));
        assert!((0..30).all(|i| l.tile_at(1, i).kind == Tile::Empty));
        assert_eq!(tile_images(Tile::LatticePillar, 0, palace).1, Some(295));
        assert_eq!(tile_images(Tile::Pillar, 0, palace).1, Some(295));
        assert_eq!(pieces(Tile::Torch, 0, true, palace), vec![(Role::TorchStub, 346)]);
        assert!(tile_images(Tile::Floor, 0, palace).0.is_some());
    }

    /// The camera sees the painted room's things where the picture puts them: the picture plane maps one to one, the thin columns stand on
    /// line 146 at `ARCADE_Z`, the front columns on line 200 at the depth where the floor ends on the screen, and a point on a deeper
    /// plane lands on its own screen position (so layers and columns made for the picture line up).
    #[test]
    fn the_set_is_laid_out_from_the_picture() {
        assert!(super::floor_z(166.0).abs() < 1e-4, "the actors' line is the picture plane");
        assert!((super::floor_z(146.0) - crate::room::ARCADE_Z).abs() < 0.1, "{}", super::floor_z(146.0));
        assert!((super::floor_z(200.0) - 2.6).abs() < 0.1, "{}", super::floor_z(200.0));
        assert!(super::floor_z(144.0) < -3.5 && super::floor_z(144.0) > -4.2, "the far floor edge");
        let (x, y) = super::far_at(0.0, 100.0, 50.0);
        assert!((x - super::wx(100.0)).abs() < 1e-5 && (y - super::wy(50.0)).abs() < 1e-5);
        // a point on a plane behind it, seen from the camera, is on the screen position of the picture's point
        let (px, py) = super::far_at(3.3, 40.0, 120.0);
        let k = super::far_scale(3.3);
        assert!(((px - super::CAM_X) / k - (super::wx(40.0) - super::CAM_X)).abs() < 1e-4 && ((py - super::CAM_Y) / k - (super::wy(120.0) - super::CAM_Y)).abs() < 1e-4);
    }

    /// The carpet's completed left end keeps the picture and adds the slanted end below line 18 (checked on a made-up 240 x 41 picture).
    #[test]
    fn the_carpet_gets_its_left_end() {
        let (w, h) = (240usize, 41usize);
        let mut pixels = vec![0u8; w * h];
        for y in 12..41 {
            for x in 0..233 {
                pixels[y * w + x] = 1 + ((x + y) % 7) as u8;
            }
        }
        let im = pop_core::image::Image { width: w, height: h, bpp: 4, pixels: pixels.clone() };
        let e = super::extend_carpet(&im);
        let ext = super::CARPET_EXT;
        assert_eq!((e.width, e.height), (w + ext, h));
        assert!((0..h).all(|y| e.pixels[y * e.width + ext..(y + 1) * e.width] == pixels[y * w..(y + 1) * w]), "the picture is kept");
        let row = |y: usize| &e.pixels[y * e.width..y * e.width + ext];
        assert!(row(12).iter().all(|&p| p == 0) && row(18).iter().all(|&p| p == 0), "above line 19 nothing is added");
        assert!(row(40).iter().filter(|&&p| p != 0).count() > 50, "the bottom margin runs out to the corner");
        // the end slants: the first opaque pixel moves left by 2.5 px per row
        let first = |y: usize| row(y).iter().position(|&p| p != 0).unwrap();
        assert!(first(22) > first(30) && first(30) > first(36), "{} {} {}", first(22), first(30), first(36));
    }

    /// The gap left of the bed carpet: PV 981 is fully transparent exactly where PV 951 has its left front column (x 64..73,
    /// the same 10 px shaft as the right one at 245..254, which PV 952 redraws at x 240); FRONT_COLUMNS puts 952's shaft on
    /// both (skips without Original_data).
    #[test]
    fn bed_gap_is_the_left_front_column() {
        use pop_core::{dat::Dat, image};
        let Ok(pv) = Dat::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PV.DAT")) else { return };
        let im = |id| image::decode(pv.get(id).unwrap()).unwrap();
        let (room, bed, piece) = (im(951), im(981), im(952));
        let shaft = &piece.pixels[30 * 16..][super::FRONT_SHAFT.0..=super::FRONT_SHAFT.1];
        assert_eq!(shaft, [1, 2, 3, 4, 5, 6, 7, 7, 6, 4]);
        let lefts: Vec<usize> = super::FRONT_COLUMNS.iter().map(|c| c.0 as usize).collect();
        assert_eq!(lefts[1], 240 + super::FRONT_SHAFT.0);
        assert!((0..48).all(|y| piece.pixels[y * 16..][super::FRONT_SHAFT.0..=super::FRONT_SHAFT.1].iter().all(|&p| p != 0)), "952 shaft opaque");
        // below the ring bands (rows 121..128) the plain shaft runs to the screen bottom
        for y in 129..200 {
            let row = &room.pixels[y * 320..][..320];
            let runs: Vec<usize> = (0..311).filter(|&x| row[x..x + 10] == *shaft).collect();
            assert_eq!(runs, lefts, "951 row {y}");
        }
        // fully transparent bed columns (233..239 is the right edge of the 240 px image)
        let clear: Vec<usize> = (0..233).filter(|&x| (0..bed.height).all(|y| bed.pixels[y * bed.width + x] == 0)).collect();
        assert_eq!(clear, (64..=73).collect::<Vec<_>>());
    }
}
