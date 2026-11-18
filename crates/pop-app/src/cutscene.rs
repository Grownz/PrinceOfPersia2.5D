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
use pop_core::{dat::Dat, image, level::Tile};
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
    quad: Handle<Mesh>,
    /// title/story screens: torch animation without a scene
    idle: u32,
}

impl Cut {
    pub fn load(dir: &str) -> Cut {
        let open = |f: &str| Dat::open(format!("{dir}/{f}")).ok();
        Cut { pv: open("PV.DAT"), kid: open("KID.DAT"), prince: open("PRINCE.DAT"), ..default() }
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

/// Torch x (ds:0E08/0E0C) and flame bottom line.
const TORCH_X: [f32; 2] = [93.0, 211.0];
const TORCH_BOTTOM: f32 = 116.0;
/// The front columns of PV 951 (a 10 px shaft from y 107 to the screen bottom, x 64..73 and 245..254): shaft left edge
/// and axis depth. The left one stands in the hole PV 981 leaves for it
/// (bed transparent at x 64..73, the column shows through: axis in the bed plane z 0.52); the right one is in front of the
/// actors (952, foretable).
const FRONT_COLUMNS: [(f32, f32); 2] = [(64.0, 0.52), (245.0, 1.0)];
/// Bottom line of the bed picture (PV 981, 41 rows from y 142).
const BED_BOTTOM: f32 = 183.0;
/// Opaque shaft columns of PV 952 (first, last; 952 is drawn at x 240, so its shaft is 951's right column).
const FRONT_SHAFT: (usize, usize) = (5, 14);
const STARS: [(f32, f32); 6] = [(20.0, 97.0), (16.0, 104.0), (23.0, 110.0), (17.0, 116.0), (24.0, 120.0), (18.0, 128.0)];

/// Synthetic room for the set: a palace arcade (lattice screens above lattice pillars, as level 14 rooms 2 and 3).
fn chamber_level(base: &pop_core::level::Level) -> pop_core::level::Level {
    use Tile::*;
    let rows: [[Tile; 10]; 3] = [
        [LatticeSupport, SmallLattice, LatticeSupport, LatticeLeft, LatticeRight, LatticeSupport, SmallLattice, LatticeSupport, SmallLattice, LatticeSupport],
        [Wall, Empty, Empty, LatticePillar, Empty, Empty, Empty, Empty, LatticePillar, Wall],
        [Floor, Floor, Floor, LatticePillar, Floor, Floor, Floor, Floor, LatticePillar, Floor],
    ];
    let mut l = base.clone();
    for (r, row) in rows.iter().enumerate() {
        for (c, t) in row.iter().enumerate() {
            l.fg[r * 10 + c] = *t as u8;
            l.bg[r * 10 + c] = 0;
        }
    }
    l
}

pub fn setup(mut cmds: Commands, g: Res<Game>, p: Res<Palette>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut imgs: ResMut<Assets<Image>>, mut cut: ResMut<Cut>) {
    let level = chamber_level(&g.levels[g.levels.len() - 1]);
    room::spawn_chamber(&mut cmds, &level, 1, ORIGIN, &p);
    let o = ORIGIN;
    // the arched opening on the left with the night sky (stars at ds:0DD6)
    let sky = mats.add(StandardMaterial { base_color: Color::srgb(0.02, 0.03, 0.16), unlit: true, ..default() });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let boxed = |lo: Vec3, hi: Vec3| Transform::from_translation(o + (lo + hi) / 2.0).with_scale(hi - lo);
    cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(sky), boxed(Vec3::new(wx(11.0), wy(150.0), -0.89), Vec3::new(wx(34.0), wy(86.0), -0.88)), Chamber));
    let star_mats: Vec<Handle<StandardMaterial>> =
        [0.35, 0.7, 1.0].iter().map(|&v| mats.add(StandardMaterial { base_color: Color::srgb(v, v, v), emissive: LinearRgba::rgb(v * 3.0, v * 3.0, v * 3.0), unlit: true, ..default() })).collect();
    for (i, &(x, y)) in STARS.iter().enumerate() {
        let c = Vec3::new(wx(x), wy(y), -0.87);
        cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(star_mats[2].clone()), Transform::from_translation(o + c).with_scale(Vec3::splat(0.05)), StarDot(i), Chamber));
    }
    cmds.insert_resource(StarMats(star_mats));
    // the two front columns of PV 951 (PV 952 is the right one), built from PV 952: voxel (revolved like the level columns) or flat
    // (only its 10 px shaft, every row opaque: revolved about its centre, or a solid textured box)
    let front = cut.pv.as_ref().and_then(|d| Some((image::decode(d.get(952)?)?, image::palette(d.get(950)?)?))).map(|(im, pal)| {
        let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        let w = FRONT_SHAFT.1 + 1 - FRONT_SHAFT.0;
        let px: Vec<u8> = im.pixels.chunks(im.width).flat_map(|r| r[FRONT_SHAFT.0..=FRONT_SHAFT.1].to_vec()).collect();
        let v = crate::voxel::VoxelModel::revolve(w, im.height, &px, w as f32 / 2.0);
        let mesh = meshes.add(v.mesh(|p| {
            let [r, g, b] = rgb(p);
            Color::srgb_u8(r, g, b).to_linear().to_f32_array()
        }));
        let rgba: Vec<u8> = px.iter().flat_map(|&p| rgb(p).into_iter().chain([255])).collect();
        let mut img = Image::new(Extent3d { width: w as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        img.sampler = ImageSampler::nearest();
        let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), perceptual_roughness: 0.9, ..default() });
        (crate::items::Column { mesh, h: v.h, az: v.axis_z() }, crate::tiles::Tex { mat, w: w as f32, h: im.height as f32 })
    });
    // from the bed's bottom line (951 runs them to the screen bottom; the gap is open down to there) to the top of the row
    let (y0, y1) = (o.y + wy(BED_BOTTOM), o.y + TILE_H);
    for (x, z) in FRONT_COLUMNS {
        if !front.as_ref().is_some_and(|(v, t)| room::chamber_column(&mut cmds, &p, v, t, Vec3::new(o.x + wx(x), y0, o.z + z), y1)) {
            let r = 5.0 / 32.0;
            room::chamber_box(&mut cmds, &p, "stone", Vec3::new(o.x + wx(x), y0, o.z + z - r), Vec3::new(o.x + wx(x + 10.0), y1, o.z + z + r));
        }
    }
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    cut.quad = quad.clone();
    // torches: holder (the level's 346 with original tiles), flame (original sprite, or the level's voxel flame), the level light
    for (i, &x) in TORCH_X.iter().enumerate() {
        let cx = wx(x + 8.0);
        if !room::chamber_holder(&mut cmds, &p, o + Vec3::new(cx, wy(TORCH_BOTTOM), 0.0)) {
            room::chamber_box(&mut cmds, &p, "dark", o + Vec3::new(cx - 0.08, wy(TORCH_BOTTOM) - 0.25, -0.88), o + Vec3::new(cx + 0.08, wy(TORCH_BOTTOM) + 0.02, -0.75));
        }
        cmds.spawn((room::torch_light(), Transform::from_translation(o + Vec3::new(cx, wy(TORCH_BOTTOM) + 0.6, -0.2)), Chamber));
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, FlameSprite(i), bevy::light::NotShadowCaster));
    }
    for s in [Slot::Kid, Slot::Princess] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, ActorSprite(s)));
    }
    for stream in [false, true] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, GlassSprite(stream)));
    }
    // the bed on its carpet (PV 981, 240x41 at (0, 142)), a flat sprite standing just in front of the floor
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, Bed));
    cmds.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(10), BackgroundColor(Color::NONE), FadeOverlay));
}

#[derive(Resource)]
pub struct StarMats(Vec<Handle<StandardMaterial>>);
#[derive(Component)]
pub struct Bed;

/// Camera on the set: the whole bottom row and the arcade above it, slightly from above like the level view.
pub fn camera_on_set() -> Transform {
    Transform::from_translation(ORIGIN + Vec3::new(4.6, 2.5, 7.8)).looking_at(ORIGIN + Vec3::new(4.6, 1.4, 0.0), Vec3::Y)
}

/// Material of an original image: colour 0 transparent, unlit like the level sprites.
fn material(c: &mut Cut, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, file: u8, id: u16, pal_id: u16) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    if let Some(m) = c.mats.get(&(file, id)) {
        return Some(m.clone());
    }
    let dat = match file { 0 => c.pv.as_ref(), 1 => c.kid.as_ref(), _ => c.prince.as_ref() }?;
    let pal = image::palette(dat.get(pal_id)?)?;
    let im = image::decode(dat.get(id)?)?;
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
fn voxel(c: &mut Cut, meshes: &mut Assets<Mesh>, file: u8, id: u16, pal_id: u16, mirrored: bool) -> Option<Handle<Mesh>> {
    if let Some(m) = c.vox.get(&(file, id, mirrored)) {
        return m.clone();
    }
    let dat = match file { 0 => c.pv.as_ref(), 1 => c.kid.as_ref(), _ => c.prince.as_ref() };
    let m = dat.and_then(|dat| Some((image::palette(dat.get(pal_id)?)?, image::decode(dat.get(id)?)?))).map(|(pal, im)| {
        let rgb = |p: u8| pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        let model = crate::voxel::VoxelModel::from_image(im.width, im.height, &im.pixels, mirrored, rgb);
        meshes.add(model.mesh(|p| {
            let [r, g, b] = rgb(p);
            Color::srgb_u8(r, g, b).to_linear().to_f32_array()
        }))
    });
    c.vox.insert((file, id, mirrored), m.clone());
    m
}

/// Voxel actors stand a little behind the sprite depth would put their centre: their back clears the bed carpet (z 0.52)
/// and the figure stays behind the hourglass, which moves to `GLASS_Z_VOXEL`.
const VOXEL_DZ: f32 = 0.04;
const GLASS_Z_VOXEL: f32 = 0.84;

/// Place a sprite: left edge `x` (world), bottom `y`, mirrored when `flip`.
fn place(t: &mut Transform, x: f32, y: f32, z: f32, w: f32, h: f32, flip: bool) {
    let (sw, sh) = (w / 32.0, h * TILE_H / pop_core::level::TILE_H_PX as f32);
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
pub fn latch(k: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut c: ResMut<Cut>) {
    if crate::flow::any_key(&k) || k.just_pressed(KeyCode::ShiftLeft) || k.just_pressed(KeyCode::ShiftRight) || crate::flow::pad_any(&pads) {
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
    mut q: ParamSet<(
        Query<(&ActorSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
        Query<(&GlassSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>)>,
        Query<(&FlameSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
        Query<(&StarDot, &mut MeshMaterial3d<StandardMaterial>)>,
        Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<Bed>>,
        Query<&mut BackgroundColor, With<FadeOverlay>>,
        Query<&mut Transform, With<Camera3d>>,
    )>,
) {
    use crate::flow::Screen;
    let s = *state.get();
    let in_scene = s == Screen::Cutscene && c.cs.is_some() && c.ended.is_none(); // ended (e.g. skipped): no stale frame
    let on_set = in_scene || s == Screen::Title;
    // camera: the set for cutscenes and the title/story screens, the level otherwise
    for mut t in &mut q.p6() {
        let want = if on_set { camera_on_set() } else { crate::game_camera(win.width() / win.height().max(1.0)) };
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
        c.voxel_mat = mats.add(StandardMaterial { perceptual_roughness: 0.9, ..default() }); // made on first use: sprite style keeps its assets
    }
    for (ActorSprite(slot), mut t, mut vis, mut mat, mut mesh) in &mut q.p0() {
        let a = cs.and_then(|cs| if *slot == Slot::Kid { cs.kid.as_ref() } else { cs.princess.as_ref() }).filter(|_| !cs.unwrap().blank);
        let shown = a.and_then(|a| {
            let cs = cs.unwrap();
            let (file, id, pal) = cs.sprite(a)?;
            let file = if file == SpriteFile::Pv { 0 } else { 1 };
            let (m, w, h) = material(c, &mut imgs, &mut mats, file, id, pal)?;
            actor(cs, a, w, h, if *slot == Slot::Kid { 0.6 } else { 0.62 }, &mut t);
            // voxel style: the same rectangle, mesh origin at its bottom-left corner, one voxel = one pixel (kid.rs `show`)
            if let Some(v) = voxels.then(|| voxel(c, &mut meshes, file, id, pal, t.scale.x < 0.0)).flatten() {
                let half = Vec3::new(t.scale.x.abs(), t.scale.y, 0.0) / 2.0;
                t.translation = t.translation - half + Vec3::Z * VOXEL_DZ;
                t.scale = Vec3::new(1.0 / 32.0, TILE_H / pop_core::level::TILE_H_PX as f32, 1.0 / 32.0);
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
                place(&mut t, wx(160.0), wy(164.0), 0.9, w, h, false);
            } else {
                place(&mut t, wx(152.0), 0.0, if voxels { GLASS_Z_VOXEL } else { 0.58 + 0.2 }, w, h, false); // ybottom 168: on the floor, a little nearer
            }
            Some(m)
        });
        set_vis(&mut vis, &mut mat, shown);
    }
    // torch flames: PRINCE.DAT 151 + frame (palette 150); voxel objects: the level's voxel flame of that frame on its holder
    let idle = c.idle;
    let vflames = room::chamber_flames(&pal);
    for (FlameSprite(i), mut t, mut vis, mut mat, mut mesh) in &mut q.p2() {
        let frame = cs.map_or(((idle / 6 + *i as u32 * 5) % 9) as u8, |cs| cs.torch[*i]);
        let shown = match vflames {
            Some((frames, fmat, rel)) => on_set.then(|| {
                *t = Transform::from_translation(ORIGIN + Vec3::new(wx(TORCH_X[*i] + 8.0), wy(TORCH_BOTTOM), 0.0) + rel).with_scale(room::voxel_px());
                mesh.set_if_neq(Mesh3d(frames[frame as usize % frames.len()].mesh.clone()));
                fmat.clone()
            }),
            None => on_set.then(|| material(c, &mut imgs, &mut mats, 2, 151 + frame as u16, 150)).flatten().map(|(m, w, h)| {
                place(&mut t, wx(TORCH_X[*i]), wy(TORCH_BOTTOM), -0.7, w, h, false);
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
            place(&mut t, wx(0.0), wy(BED_BOTTOM), 0.52, w, h, false);
            m
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

    /// The set's pieces through the palace mapping (docs/texture_inventory.md 17): every lattice screen but LatticeRight
    /// gets its arch, the columns (set and added pillars) image 295, the torches holder 346; walls are built palace faces.
    #[test]
    fn chamber_pieces_use_the_palace_mapping() {
        use crate::tiles::{pieces, tile_images, Role};
        use pop_core::level::{Level, Tile};
        // VPALACE (texture_inventory 2.2): the base-200 ids plus the palace slots 1201..1213
        let palace = |id: u16| id < 1200 || (1201..=1213).contains(&id);
        let l = super::chamber_level(&Level::parse(&[0u8; 4096]).expect("zero level"));
        for idx in 0..10 {
            let t = l.tile_at(1, idx).kind;
            let arch = pieces(t, 0, true, palace).iter().any(|(r, _)| *r == Role::BackPanel);
            assert_eq!(arch, t != Tile::LatticeRight, "{t:?}");
        }
        assert_eq!(l.tile_at(1, 13).kind, Tile::LatticePillar);
        assert_eq!(tile_images(Tile::LatticePillar, 0, palace).1, Some(295));
        assert_eq!(tile_images(Tile::Pillar, 0, palace).1, Some(295));
        assert_eq!(pieces(Tile::Torch, 0, true, palace), vec![(Role::TorchStub, 346)]);
        assert!(tile_images(Tile::Floor, 0, palace).0.is_some());
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
