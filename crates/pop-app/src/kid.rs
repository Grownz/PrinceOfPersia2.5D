//! The kid in the app: input latching, per-tick control (pop-core), and drawing with the original sprites (KID.DAT).
//! Sequence viewer: `,` / `.` = previous/next sequence (kid no longer controlled), Space = restart it, F = flip,
//! C = back to the controlled kid at the level start.
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::kid::{Env, Keys, Kid};
use pop_core::{dat::Dat, exe::{Exe, Frame, SEQUENCES}, image, level::TILE_H_PX};
use std::collections::HashMap;

#[derive(Resource)]
pub struct KidAssets {
    pub exe: Exe,
    pub frames: Vec<Frame>,
    pub sizes: Vec<(u16, u16)>,
    dat: Dat,
    pal: Vec<[u8; 3]>,
    /// image number -> (material, width px, height px)
    cache: HashMap<u8, (Handle<StandardMaterial>, f32, f32)>,
}

#[derive(Component)]
pub struct KidSprite;

/// Keys seen since the last logic tick (a tap shorter than one tick must not be lost).
#[derive(Resource, Default)]
pub struct Latch(Keys);

impl KidAssets {
    pub fn load(dir: &str) -> Self {
        let exe = Exe::open(format!("{dir}/PRINCE.EXE")).expect("cannot open PRINCE.EXE (set POP_DATA)");
        let dat = Dat::open(format!("{dir}/KID.DAT")).expect("cannot open KID.DAT");
        let pal = image::palette(dat.get(400).unwrap()).unwrap();
        KidAssets { frames: exe.kid_frames(), sizes: image::image_sizes(&dat, 401, 220), exe, dat, pal, cache: HashMap::new() }
    }
}

pub fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    cmds.spawn((Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, KidSprite));
}

fn ctrl_held(k: &ButtonInput<KeyCode>) -> bool {
    k.pressed(KeyCode::ControlLeft) || k.pressed(KeyCode::ControlRight) // Ctrl+arrows navigate rooms instead
}

fn pressed(k: &ButtonInput<KeyCode>) -> Keys {
    if ctrl_held(k) {
        return Keys::default();
    }
    Keys {
        left: k.pressed(KeyCode::ArrowLeft),
        right: k.pressed(KeyCode::ArrowRight),
        up: k.pressed(KeyCode::ArrowUp),
        down: k.pressed(KeyCode::ArrowDown),
        shift: k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight),
    }
}

/// Runs every frame: remember keys that were down at any point since the last logic tick.
pub fn latch_keys(k: Res<ButtonInput<KeyCode>>, mut l: ResMut<Latch>) {
    let p = pressed(&k);
    l.0 = Keys { left: l.0.left | p.left, right: l.0.right | p.right, up: l.0.up | p.up, down: l.0.down | p.down, shift: l.0.shift | p.shift };
}

fn material_for(a: &mut KidAssets, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, n: u8) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    if let Some(c) = a.cache.get(&n) {
        return Some(c.clone());
    }
    let im = image::decode(a.dat.get(401 + n as u16)?)?;
    let mut rgba = Vec::with_capacity(im.pixels.len() * 4);
    for &p in &im.pixels {
        let c = a.pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        rgba.extend_from_slice(&[c[0], c[1], c[2], if p == 0 { 0 } else { 255 }]);
    }
    let mut img = Image::new(Extent3d { width: im.width as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: true, ..default() });
    let entry = (mat, im.width as f32, im.height as f32);
    a.cache.insert(n, entry.clone());
    Some(entry)
}

/// Drawing rule (docs/mechanics.md 7.3). KID.DAT images face LEFT as stored; the engine mirrors them when the kid faces right.
/// Returns (sx / 32, bottom) in world units: sx = screen x of the image's left edge (facing left) or right edge (facing right),
/// bottom = the image's bottom line (row 2 floor = 0).
fn world_pos(k: &Kid, frames: &[Frame]) -> Vec2 {
    let f = frames.get(k.frame as usize).copied().unwrap_or(Frame { image: 255, sword: 0, dx: 0, dy: 0, flags: 0 });
    let dir_byte: u8 = if k.facing_left { 0xFF } else { 0 };
    let dxs = if k.facing_left { -(f.dx as i32) } else { f.dx as i32 };
    let parity = (((f.flags ^ dir_byte) & 0x80) == 0) as i32;
    let obj_x = 2 * (k.x as i32 + dxs) - 116 + parity;
    let sx = obj_x * 320 / 280;
    let bottom = k.y.wrapping_add(f.dy as u8) as f32;
    Vec2::new(sx as f32 / 32.0, (181.0 - bottom) * crate::room::TILE_H / TILE_H_PX as f32)
}

fn respawn(g: &mut Game, a: &KidAssets) {
    let env = Env { exe: &a.exe, frames: &a.frames, level: &g.levels[g.level], sizes: &a.sizes };
    g.kid = Kid::spawn(&env, g.level);
    g.room = g.kid.room as usize;
    g.kid_prev = (world_pos(&g.kid, &a.frames), g.kid.room);
}

/// Sequence viewer and respawn keys.
pub fn keys(k: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, a: Res<KidAssets>) {
    if g.kid_respawn || k.just_pressed(KeyCode::KeyC) {
        g.kid_respawn = false;
        respawn(&mut g, &a);
        return;
    }
    let step = k.just_pressed(KeyCode::Period) as i32 - k.just_pressed(KeyCode::Comma) as i32;
    if step != 0 {
        g.kid.seq = ((g.kid.seq as i32 - 1 + step).rem_euclid(SEQUENCES as i32) + 1) as usize;
    }
    if k.just_pressed(KeyCode::KeyF) {
        g.kid.facing_left = !g.kid.facing_left;
    }
    if step != 0 || k.just_pressed(KeyCode::Space) {
        let seq = g.kid.seq.max(1);
        let Game { kid, levels, level, .. } = &mut *g;
        let env = Env { exe: &a.exe, frames: &a.frames, level: &levels[*level], sizes: &a.sizes };
        kid.controlled = false;
        kid.start_seq(&env, seq);
    }
}

/// Logic tick for the kid (called from the fixed-step system when a logic tick ran).
pub fn tick(g: &mut Game, a: &KidAssets, latch: &mut Latch, held: &ButtonInput<KeyCode>) {
    let mut keys = latch.0;
    latch.0 = pressed(held);
    if let Some(sc) = script() {
        keys = sc.get(g.world.tick as usize).copied().unwrap_or_default();
    }
    g.kid_prev = (world_pos(&g.kid, &a.frames), g.kid.room);
    let Game { kid, levels, level, .. } = &mut *g;
    let env = Env { exe: &a.exe, frames: &a.frames, level: &levels[*level], sizes: &a.sizes };
    kid.tick(&env, keys);
    if g.kid.controlled {
        if (1..=pop_core::level::ROOMS as u8).contains(&g.kid.room) {
            g.room = g.kid.room as usize; // room 0 = outside the level: keep showing the last room
        }
    } else if g.kid.x < 20 || g.kid.x > 200 {
        g.kid.x = 100; // sequence viewer: keep the kid in view
        g.kid_prev.0.x = world_pos(&g.kid, &a.frames).x;
    }
}

pub fn render(
    g: Res<Game>,
    a: ResMut<KidAssets>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<KidSprite>>,
) {
    let mut a = a;
    let Ok((mut t, mut vis, mut mat)) = q.single_mut() else { return };
    let img_no = a.frames.get(g.kid.frame as usize).map_or(255, |f| f.image);
    let Some((m, w, h)) = (img_no != 255).then(|| material_for(&mut a, &mut imgs, &mut mats, img_no)).flatten() else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    mat.0 = m;
    let cur = world_pos(&g.kid, &a.frames);
    let p = if g.kid_prev.1 == g.kid.room { g.kid_prev.0.lerp(cur, g.world.alpha()) } else { cur };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    // facing left: p.x is the left edge, image as stored; facing right: p.x is the right edge and the image is mirrored
    let (cx, flip) = if g.kid.facing_left { (p.x + sw / 2.0, 1.0) } else { (p.x - sw / 2.0, -1.0) };
    t.translation = Vec3::new(cx, p.y + sh / 2.0, 0.6);
    t.scale = Vec3::new(flip * sw, sh, 1.0);
}

/// Test hook: `POP_SCRIPT="r:25,ru:1,-:30"` plays keys per logic tick (letters l r u d s, `-` = none), instead of the keyboard.
fn script() -> Option<&'static Vec<Keys>> {
    static S: std::sync::OnceLock<Option<Vec<Keys>>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        let v = std::env::var("POP_SCRIPT").ok()?;
        let mut out = vec![];
        for part in v.split(',') {
            let (k, n) = part.split_once(':')?;
            let keys = Keys { left: k.contains('l'), right: k.contains('r'), up: k.contains('u'), down: k.contains('d'), shift: k.contains('s') };
            out.extend(std::iter::repeat(keys).take(n.parse().ok()?));
        }
        Some(out)
    })
    .as_ref()
}
