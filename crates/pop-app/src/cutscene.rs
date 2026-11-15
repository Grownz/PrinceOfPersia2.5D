//! Cutscenes (docs/cutscenes.md) in the 2.5D style of the levels: pop-core's `Cutscene` player drives the original sprites
//! (PV.DAT princess / Jaffar, KID.DAT kid and mouse, PRINCE.DAT torch flames, PV.DAT hourglass and bed) inside a princess
//! chamber built with the room builder (room.rs tiles, torch lights). The set stays at `ORIGIN`, away from the level rooms;
//! the camera moves there for the cutscenes and the title/story screens.
//! Per element: room, arcade, front pillar, window = tile geometry; actors, flames, hourglass, sand, bed = decoded original
//! sprites as textured quads (the bed is a flat painted carpet in PV 981, so it stays a sprite). With character_style voxel
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

pub fn setup(mut cmds: Commands, g: Res<Game>, p: Res<Palette>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut cut: ResMut<Cut>) {
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
    // the column the bed picture leaves a gap for (PV 981 is transparent at px 68..77)
    room::chamber_box(&mut cmds, &p, "stone", o + Vec3::new(wx(68.0), 0.0, 0.25), o + Vec3::new(wx(78.0), TILE_H, 0.5));
    // the front pillar piece (PV 952 at x 240, drawn in front of the actors) as a stone column
    room::chamber_box(&mut cmds, &p, "stone", o + Vec3::new(wx(240.0), 0.0, 0.8), o + Vec3::new(wx(256.0), TILE_H, 1.2));
    // torches: holder, original flame sprite, the level torch light
    for (i, &x) in TORCH_X.iter().enumerate() {
        let cx = wx(x + 8.0);
        room::chamber_box(&mut cmds, &p, "dark", o + Vec3::new(cx - 0.08, wy(TORCH_BOTTOM) - 0.25, -0.88), o + Vec3::new(cx + 0.08, wy(TORCH_BOTTOM) + 0.02, -0.75));
        cmds.spawn((room::torch_light(), Transform::from_translation(o + Vec3::new(cx, wy(TORCH_BOTTOM) + 0.6, -0.2)), Chamber));
        cmds.spawn((Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, FlameSprite(i), bevy::light::NotShadowCaster));
    }
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    cut.quad = quad.clone();
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
    mut q: ParamSet<(
        Query<(&ActorSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
        Query<(&GlassSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>)>,
        Query<(&FlameSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>)>,
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
    // torch flames: PRINCE.DAT 151 + frame (palette 150)
    let idle = c.idle;
    for (FlameSprite(i), mut t, mut vis, mut mat) in &mut q.p2() {
        let frame = cs.map_or(((idle / 6 + *i as u32 * 5) % 9) as u8, |cs| cs.torch[*i]);
        let shown = on_set.then(|| material(c, &mut imgs, &mut mats, 2, 151 + frame as u16, 150)).flatten().map(|(m, w, h)| {
            place(&mut t, wx(TORCH_X[*i]), wy(TORCH_BOTTOM), -0.7, w, h, false);
            m
        });
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
            place(&mut t, wx(0.0), wy(183.0), 0.52, w, h, false);
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
}
