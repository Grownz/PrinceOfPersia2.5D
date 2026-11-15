//! The kid in the app: input latching, per-tick control (pop-core), and drawing with the original sprites (KID.DAT).
//! Sequence viewer: `,` / `.` = previous/next sequence (kid no longer controlled), Space = restart it, F = flip,
//! C = back to the controlled kid at the level start.
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::kid::{Env, Keys, Kid};
use pop_core::sim::{mk_env, Data, Progress, World};
use std::sync::Arc;
use pop_core::{dat::Dat, exe::{Exe, SEQUENCES}, image, level::TILE_H_PX};
use std::collections::HashMap;

#[derive(Resource)]
pub struct KidAssets {
    pub data: Arc<Data>,
    dat: Dat,
    pal: Vec<[u8; 3]>,
    /// GUARD, FAT, SKEL, VIZIER, SHADOW (images 751 + i)
    gdats: Vec<Dat>,
    /// palettes: GUARD2 (dungeon), GUARD1 (palace), FAT, SKEL, VIZIER, SHADOW
    gpals: Vec<Vec<[u8; 3]>>,
    /// PRINCE.DAT resource 10: seven 16-colour guard palettes (48 bytes of 6-bit RGB each)
    colours: Vec<u8>,
    /// PRINCE.DAT: sword images 701 + i, palette 700
    pdat: Dat,
    sword_pal: Vec<[u8; 3]>,
    sword_table: Vec<(u8, i8, i8)>,
    /// (kind, image, shadow tint, palace) -> (material, width px, height px); kind 255 = kid images
    cache: HashMap<(u8, u8, bool, bool, u8), (Handle<StandardMaterial>, f32, f32)>,
}

#[derive(Component)]
pub struct KidSprite;

#[derive(Component)]
pub struct GuardSprite;

/// The sword of the kid (false) or of the guard (true)
#[derive(Component)]
pub struct SwordSprite(bool);

/// Levels with the palace tile set (they use the GUARD1 palette, ds:01CE)
fn palace(level: usize) -> bool {
    matches!(level, 4 | 5 | 6 | 10 | 11 | 14)
}

/// Keys seen since the last logic tick (a tap shorter than one tick must not be lost), and the `Game::gen` they belong to.
#[derive(Resource, Default)]
pub struct Latch(Keys, u32);

impl KidAssets {
    pub fn load(dir: &str) -> Self {
        let exe = Exe::open(format!("{dir}/PRINCE.EXE")).expect("cannot open PRINCE.EXE (set POP_DATA)");
        let dat = Dat::open(format!("{dir}/KID.DAT")).expect("cannot open KID.DAT");
        let pal = image::palette(dat.get(400).unwrap()).unwrap();
        let open = |f: &str| Dat::open(format!("{dir}/{f}")).unwrap_or_else(|_| panic!("cannot open {f}"));
        let gdats: Vec<Dat> = ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"].iter().map(|f| open(f)).collect();
        let gsizes = gdats.iter().map(|d| image::image_sizes(d, 751, 40)).collect();
        let pal_of = |f: &str| image::palette(open(f).get(750).unwrap()).unwrap();
        let gpals = ["GUARD2.DAT", "GUARD1.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"].iter().map(|f| pal_of(f)).collect();
        let data = Arc::new(Data { gframes: exe.guard_frames(), frames: exe.kid_frames(), sizes: image::image_sizes(&dat, 401, 220), gsizes, exe });
        let colours = open("PRINCE.DAT").get(10).map(|r| r.to_vec()).unwrap_or_default();
        let pdat = open("PRINCE.DAT");
        let sword_pal = image::palette(pdat.get(700).unwrap()).unwrap();
        let sword_table = data.exe.sword_table();
        KidAssets { data, dat, pal, gdats, gpals, colours, pdat, sword_pal, sword_table, cache: HashMap::new() }
    }
}

pub fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, KidSprite));
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, GuardSprite));
    for guard in [false, true] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, SwordSprite(guard)));
    }
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

/// Material of one sprite image. `kind` 255 = KID.DAT, otherwise the guard-type file; `shadow` tints it dark (the prince's shadow).
fn material_for(a: &mut KidAssets, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, kind: u8, n: u8, shadow: bool, pal_palace: bool, colour: u8) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    let key = (kind, n, shadow, pal_palace, colour);
    if let Some(c) = a.cache.get(&key) {
        return Some(c.clone());
    }
    let coloured: Vec<[u8; 3]>;
    let (dat, pal) = if kind == 254 {
        (&a.pdat, &a.sword_pal)
    } else if kind == 255 {
        (&a.dat, &a.pal)
    } else {
        let pi = match kind { 0 => pal_palace as usize, t => t as usize + 1 };
        if kind == 0 && colour != 0 {
            // guard colour c: 16 entries from the slice 48 * (c - 1) of PRINCE.DAT resource 10
            let o = 48 * (colour as usize - 1);
            coloured = a.colours.get(o..o + 48).map_or_else(Vec::new, |v| v.chunks(3).map(|t| [0, 1, 2].map(|i| (t[i] as u32 * 255 / 63) as u8)).collect());
            (a.gdats.get(0)?, &coloured)
        } else {
            (a.gdats.get(kind as usize)?, a.gpals.get(pi)?)
        }
    };
    let im = image::decode(dat.get(match kind { 255 => 401, 254 => 701, _ => 751 } + n as u16)?)?;
    let mut rgba = Vec::with_capacity(im.pixels.len() * 4);
    for &p in &im.pixels {
        let mut c = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        if shadow {
            c = [c[0] / 5, c[1] / 5, c[2] / 3];
        }
        rgba.extend_from_slice(&[c[0], c[1], c[2], if p == 0 { 0 } else { 255 }]);
    }
    let mut img = Image::new(Extent3d { width: im.width as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: true, ..default() });
    let entry = (mat, im.width as f32, im.height as f32);
    a.cache.insert(key, entry.clone());
    Some(entry)
}

/// Drawing rule (docs/mechanics.md 7.3). KID.DAT images face LEFT as stored; the engine mirrors them when the kid faces right.
/// Returns (sx / 32, bottom) in world units: sx = screen x of the image's left edge (facing left) or right edge (facing right),
/// bottom = the image's bottom line (row 2 floor = 0).
fn world_pos(k: &Kid, env: &Env) -> Vec2 {
    let f = env.frame_of(k.charid, k.frame);
    let dir_byte: u8 = if k.facing_left { 0xFF } else { 0 };
    let dxs = if k.facing_left { -(f.dx as i32) } else { f.dx as i32 };
    let parity = (((f.flags ^ dir_byte) & 0x80) == 0) as i32;
    let obj_x = 2 * (k.x as i32 + dxs) - 116 + parity;
    let sx = obj_x * 320 / 280;
    let bottom = k.y.wrapping_add(f.dy as u8) as f32;
    // Logic column c spans x = 65 + 14c .. 79 + 14c (collision and floor checks use col_m7 = x - 7), i.e. screen px 32c + 16.
    // The tile geometry is built at world x = c .. c + 1, so the sprite is shifted left by half a tile (16 px).
    Vec2::new(sx as f32 / 32.0 - 0.5, (181.0 - bottom) * crate::room::TILE_H / TILE_H_PX as f32)
}

/// Build the world for `p` (restart, next level, new game, the demo level 0). Cutscenes: see `flow` (text cards only).
pub fn start(g: &mut Game, a: &KidAssets, p: Progress) {
    let p = if p.level < g.levels.len() { p } else { Progress::new_game(0) }; // after the last level: back to the title
    g.level = p.level;
    g.world = World::from_progress(a.data.clone(), g.levels[g.level].clone(), p);
    g.gen = g.gen.wrapping_add(1);
    g.room = g.world.kid.room as usize;
    let env = mk_env(&a.data, &g.world.level, g.world.guard_type);
    g.kid_prev = (world_pos(&g.world.kid, &env), g.world.kid.room);
    g.guard_prev = g.world.guard.as_ref().map_or((Vec2::ZERO, 0), |k| (world_pos(k, &env), k.room));
}

/// Restart keys; with the debug text on (F3) also the sequence viewer and respawn keys. Exits are handled in `flow::playing`.
pub fn keys(k: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, a: Res<KidAssets>) {
    // restart keys (0000:054D): Shift held or Enter after death, Ctrl+A any time
    let enter = k.just_pressed(KeyCode::Enter) || k.just_pressed(KeyCode::NumpadEnter);
    let shift = k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight);
    g.world.restart_input(shift || enter, ctrl_held(&k) && k.just_pressed(KeyCode::KeyA));
    if !g.debug || g.world.exit.is_some() {
        return;
    }
    if g.kid_respawn || k.just_pressed(KeyCode::KeyC) {
        g.kid_respawn = false;
        let l = g.level;
        start(&mut g, &a, Progress::new_game(l));
        return;
    }
    let step = k.just_pressed(KeyCode::Period) as i32 - k.just_pressed(KeyCode::Comma) as i32;
    if step != 0 {
        g.world.kid.seq = ((g.world.kid.seq as i32 - 1 + step).rem_euclid(SEQUENCES as i32) + 1) as usize;
    }
    if k.just_pressed(KeyCode::KeyF) {
        g.world.kid.facing_left = !g.world.kid.facing_left;
    }
    if step != 0 || k.just_pressed(KeyCode::Space) {
        let seq = g.world.kid.seq.max(1);
        g.world.kid_start_seq(seq);
    }
}

/// 60 Hz step of the world; on logic ticks the latched keys are consumed (or the test script is played).
pub fn fixed_step(g: &mut Game, a: &KidAssets, latch: &mut Latch, held: &ButtonInput<KeyCode>) {
    if g.world.exit.is_some() {
        return; // an exit is pending: Update consumes it before any further tick mutates the carried state
    }
    if latch.1 != g.gen {
        // a new world: taps latched before it (the key that skipped a card, started the game or ended the demo) must not reach
        // its first tick; keys still held count
        (latch.0, latch.1) = (pressed(held), g.gen);
    }
    if g.world.next_is_tick() {
        let env = mk_env(&a.data, &g.world.level, g.world.guard_type);
        g.kid_prev = (world_pos(&g.world.kid, &env), g.world.kid.room);
        g.guard_prev = g.world.guard.as_ref().map_or((Vec2::ZERO, 0), |k| (world_pos(k, &env), k.room));
    }
    let mut keys = latch.0;
    if let Some(sc) = script() {
        keys = sc.get(g.world.tick as usize).copied().unwrap_or_default();
    }
    if g.world.step(keys) {
        latch.0 = pressed(held);
        g.banner.0 = g.banner.0.saturating_sub(1);
        if g.world.kid.controlled {
            if (1..=pop_core::level::ROOMS as u8).contains(&g.world.kid.room) {
                g.room = g.world.kid.room as usize; // room 0 = outside the level: keep showing the last room
            }
        } else if g.world.kid.x < 20 || g.world.kid.x > 200 {
            g.world.kid.x = 100; // sequence viewer: keep the kid in view
            let env = mk_env(&a.data, &g.world.level, g.world.guard_type);
            g.kid_prev.0.x = world_pos(&g.world.kid, &env).x;
        }
    }
}

/// Place a character sprite: `prev` is the position at the start of the last logic tick (interpolation).
#[allow(clippy::too_many_arguments)]
fn draw_char(
    k: &Kid,
    prev: (Vec2, u8),
    alpha: f32,
    a: &mut KidAssets,
    level: &pop_core::level::Level,
    gtype: i8,
    level_no: usize,
    imgs: &mut Assets<Image>,
    mats: &mut Assets<StandardMaterial>,
    t: &mut Transform,
    vis: &mut Visibility,
    mat: &mut MeshMaterial3d<StandardMaterial>,
) -> Option<Vec2> {
    let data = a.data.clone();
    let env = mk_env(&data, level, gtype);
    let f = env.frame_of(k.charid, k.frame);
    // chtab = 2 + (sword >> 6): 5 = guard-type images, otherwise the kid images (the shadow draws those dark)
    let guard_img = f.sword >> 6 == 3;
    let kind = if guard_img { gtype.max(0) as u8 } else { 255 };
    let Some((m, w, h)) = (f.image != 255).then(|| material_for(a, imgs, mats, kind, f.image, k.charid == 1 && !guard_img, palace(level_no), k.colour)).flatten() else {
        *vis = Visibility::Hidden;
        return None;
    };
    *vis = Visibility::Inherited;
    mat.0 = m;
    let cur = world_pos(k, &env);
    let p = if prev.1 == k.room { prev.0.lerp(cur, alpha) } else { cur };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    // facing left: p.x is the left edge, image as stored; facing right: p.x is the right edge and the image is mirrored
    let (cx, flip) = if k.facing_left { (p.x + sw / 2.0, 1.0) } else { (p.x - sw / 2.0, -1.0) };
    t.translation = Vec3::new(cx, p.y + sh / 2.0, 0.6);
    t.scale = Vec3::new(flip * sw, sh, 1.0);
    Some(p)
}

/// The sword of a fighter with his sword drawn: position from the sword table (screen pixels relative to the body).
#[allow(clippy::too_many_arguments)]
fn draw_sword(
    k: &Kid,
    body: Option<Vec2>,
    a: &mut KidAssets,
    level: &pop_core::level::Level,
    gtype: i8,
    imgs: &mut Assets<Image>,
    mats: &mut Assets<StandardMaterial>,
    t: &mut Transform,
    vis: &mut Visibility,
    mat: &mut MeshMaterial3d<StandardMaterial>,
) {
    let data = a.data.clone();
    let env = mk_env(&data, level, gtype);
    let f = env.frame_of(k.charid, k.frame);
    let entry = a.sword_table.get((f.sword & 0x3F) as usize).copied();
    let (Some(p), Some((img, dx, dy)), true) = (body, entry, k.sword == 2) else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some((m, w, h)) = (img != 255).then(|| material_for(a, imgs, mats, 254, img, false, false, 0)).flatten() else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    mat.0 = m;
    let dxs = if k.facing_left { -(dx as f32) } else { dx as f32 };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    let x = p.x + dxs / 32.0;
    let y = p.y - dy as f32 * crate::room::TILE_H / TILE_H_PX as f32;
    let (cx, flip) = if k.facing_left { (x + sw / 2.0, 1.0) } else { (x - sw / 2.0, -1.0) };
    t.translation = Vec3::new(cx, y + sh / 2.0, 0.65);
    t.scale = Vec3::new(flip * sw, sh, 1.0);
}

pub fn render(
    g: Res<Game>,
    a: ResMut<KidAssets>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (With<KidSprite>, Without<GuardSprite>)>,
    mut qg: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (With<GuardSprite>, Without<KidSprite>)>,
    mut qs: Query<(&SwordSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), (Without<KidSprite>, Without<GuardSprite>)>,
) {
    let mut a = a;
    let w = &g.world;
    let mut kid_body = None;
    let mut guard_body = None;
    if let Ok((mut t, mut vis, mut mat)) = q.single_mut() {
        kid_body = draw_char(&w.kid, g.kid_prev, w.alpha(), &mut a, &w.level, w.guard_type, w.level_no, &mut imgs, &mut mats, &mut t, &mut vis, &mut mat);
    }
    if let Ok((mut t, mut vis, mut mat)) = qg.single_mut() {
        match &w.guard {
            Some(gd) if gd.room as usize == g.room => guard_body = draw_char(gd, g.guard_prev, w.alpha(), &mut a, &w.level, w.guard_type, w.level_no, &mut imgs, &mut mats, &mut t, &mut vis, &mut mat),
            _ => *vis = Visibility::Hidden,
        }
    }
    for (SwordSprite(is_guard), mut t, mut vis, mut mat) in &mut qs {
        let (k, body) = if *is_guard { (w.guard.as_ref(), guard_body) } else { (Some(&w.kid), kid_body) };
        match k {
            Some(k) => draw_sword(k, body, &mut a, &w.level, w.guard_type, &mut imgs, &mut mats, &mut t, &mut vis, &mut mat),
            None => *vis = Visibility::Hidden,
        }
    }
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
