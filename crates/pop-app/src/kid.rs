//! The kid in the app: input latching, per-tick control (pop-core), and drawing with the original sprites (KID.DAT).
//! Sequence viewer: `,` / `.` = previous/next sequence (kid no longer controlled), Space = restart it, F = flip,
//! C = back to the controlled kid at the level start.
use crate::settings::{CharacterStyle, KeyMap};
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
    /// voxel meshes: the material key plus mirrored -> mesh (None = image did not decode)
    vcache: HashMap<(u8, u8, bool, bool, u8, bool), Option<Handle<Mesh>>>,
    /// the unit quad of the sprites, the shared material of the voxel meshes (vertex colours) and its hit flash variant,
    /// and whether the voxel meshes get smooth normals; set in `setup`
    quad: Handle<Mesh>,
    voxel_mat: Handle<StandardMaterial>,
    voxel_flash: Handle<StandardMaterial>,
    smooth: bool,
}

#[derive(Component)]
pub struct KidSprite;

#[derive(Component)]
pub struct GuardSprite;

/// The sword of the kid (false) or of the guard (true)
#[derive(Component)]
pub struct SwordSprite(bool);

/// Levels with the palace tile set (they use the GUARD1 palette, ds:01CE)
pub(crate) fn palace(level: usize) -> bool {
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
        KidAssets { data, dat, pal, gdats, gpals, colours, pdat, sword_pal, sword_table, cache: HashMap::new(), vcache: HashMap::new(), quad: default(), voxel_mat: default(), voxel_flash: default(), smooth: true }
    }
}

pub fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut a: ResMut<KidAssets>, set: Res<crate::settings::Settings>) {
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    a.quad = quad.clone();
    let unlit = set.unlit_voxels();
    a.voxel_mat = mats.add(crate::voxel::character_material(unlit, false));
    a.voxel_flash = mats.add(crate::voxel::character_material(unlit, true));
    a.smooth = set.smooth_normals();
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, KidSprite));
    cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, GuardSprite));
    for guard in [false, true] {
        cmds.spawn((Mesh3d(quad.clone()), MeshMaterial3d::<StandardMaterial>::default(), Transform::default(), Visibility::Hidden, SwordSprite(guard)));
    }
}

fn ctrl_held(k: &ButtonInput<KeyCode>) -> bool {
    k.pressed(KeyCode::ControlLeft) || k.pressed(KeyCode::ControlRight) // Ctrl+arrows navigate rooms instead
}

/// The kid's keys held now: the bound keyboard keys (settings.json) or any controller (d-pad / left stick, A = Shift).
pub fn pressed(k: &ButtonInput<KeyCode>, m: &KeyMap, pads: &Query<&Gamepad>) -> Keys {
    use GamepadButton::*;
    let mut keys = if ctrl_held(k) { Keys::default() } else { Keys { left: k.any_pressed(m.left.iter().copied()), right: k.any_pressed(m.right.iter().copied()), up: k.any_pressed(m.up.iter().copied()), down: k.any_pressed(m.down.iter().copied()), shift: k.any_pressed(m.shift.iter().copied()) } };
    if !ctrl_held(k) {
        for p in k.get_pressed().map(|&c| keypad(c)) {
            keys = Keys { left: keys.left | p.left, right: keys.right | p.right, up: keys.up | p.up, down: keys.down | p.down, shift: keys.shift };
        }
    }
    for p in pads {
        let s = p.left_stick();
        keys.left |= p.pressed(DPadLeft) || s.x < -0.5;
        keys.right |= p.pressed(DPadRight) || s.x > 0.5;
        keys.up |= p.pressed(DPadUp) || s.y > 0.5;
        keys.down |= p.pressed(DPadDown) || s.y < -0.5;
        keys.shift |= p.pressed(South) || p.pressed(West);
    }
    keys
}

/// Numeric keypad, fixed like the original: its key table is indexed by scancode and the E0 prefix is dropped (0C83:00DE),
/// so keypad 7/Home (0x47) = up+left, 8 (0x48) = up, 9/PgUp (0x49) = up+right, 4 (0x4B) = left, 6 (0x4D) = right,
/// 5 (0x4C) and 2 (0x50) = down (read at 0000:1533). Keypad 1/End, 3/PgDn and 0/Ins are not read. Bevy reports keypad keys
/// as `NumpadN` whatever the NumLock state; the separate Home / PageUp keys share the scancode, so they act the same.
pub fn keypad(c: KeyCode) -> Keys {
    use KeyCode::*;
    let (left, right, up, down) = match c {
        Numpad7 | Home => (true, false, true, false),
        Numpad8 => (false, false, true, false),
        Numpad9 | PageUp => (false, true, true, false),
        Numpad4 => (true, false, false, false),
        Numpad6 => (false, true, false, false),
        Numpad5 | Numpad2 => (false, false, false, true),
        _ => (false, false, false, false),
    };
    Keys { left, right, up, down, shift: false }
}

/// Runs every frame: remember keys that were down at any point since the last logic tick.
pub fn latch_keys(k: Res<ButtonInput<KeyCode>>, m: Res<KeyMap>, pads: Query<&Gamepad>, mut l: ResMut<Latch>) {
    let p = pressed(&k, &m, &pads);
    l.0 = Keys { left: l.0.left | p.left, right: l.0.right | p.right, up: l.0.up | p.up, down: l.0.down | p.down, shift: l.0.shift | p.shift };
}

/// Material of one sprite image. `kind` 255 = KID.DAT, otherwise the guard-type file; `shadow` tints it dark (the prince's shadow).
fn material_for(a: &mut KidAssets, imgs: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>, kind: u8, n: u8, shadow: bool, pal_palace: bool, colour: u8) -> Option<(Handle<StandardMaterial>, f32, f32)> {
    let key = (kind, n, shadow, pal_palace, colour);
    if let Some(c) = a.cache.get(&key) {
        return Some(c.clone());
    }
    let (im, pal) = decoded(a, kind, n, pal_palace, colour)?;
    let mut rgba = Vec::with_capacity(im.pixels.len() * 4);
    for &p in &im.pixels {
        let c = rgb(&pal, p, shadow);
        rgba.extend_from_slice(&[c[0], c[1], c[2], if p == 0 { 0 } else { 255 }]);
    }
    let mut img = Image::new(Extent3d { width: im.width as u32, height: im.height as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    let mat = mats.add(StandardMaterial { base_color_texture: Some(imgs.add(img)), unlit: true, alpha_mode: AlphaMode::Mask(0.5), cull_mode: None, double_sided: true, ..default() });
    let entry = (mat, im.width as f32, im.height as f32);
    a.cache.insert(key, entry.clone());
    Some(entry)
}

/// Colour of palette index `p` (magenta if the palette is short), darkened for the prince's shadow.
fn rgb(pal: &[[u8; 3]], p: u8, shadow: bool) -> [u8; 3] {
    let c = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
    if shadow { [c[0] / 5, c[1] / 5, c[2] / 3] } else { c }
}

/// Voxel mesh of the same image as `material_for` (key plus `mirrored`), built once from the original pixels.
#[allow(clippy::too_many_arguments)]
fn voxel_for(a: &mut KidAssets, meshes: &mut Assets<Mesh>, kind: u8, n: u8, shadow: bool, pal_palace: bool, colour: u8, mirrored: bool) -> Option<Handle<Mesh>> {
    let key = (kind, n, shadow, pal_palace, colour, mirrored);
    if let Some(m) = a.vcache.get(&key) {
        return m.clone();
    }
    let m = decoded(a, kind, n, pal_palace, colour).map(|(im, pal)| {
        // the profile's colour regions come from the untinted palette (the shadow's dark tint would merge them)
        let model = crate::voxel::VoxelModel::character(im.width, im.height, &im.pixels, mirrored, |p| rgb(&pal, p, false));
        let smooth = a.smooth;
        meshes.add(model.mesh_with(
            |p| {
                let [r, g, b] = rgb(&pal, p, shadow);
                Color::srgb_u8(r, g, b).to_linear().to_f32_array()
            },
            smooth,
        ))
    });
    a.vcache.insert(key, m.clone());
    m
}

/// The decoded image and its palette (see `material_for`).
fn decoded(a: &KidAssets, kind: u8, n: u8, pal_palace: bool, colour: u8) -> Option<(image::Image, Vec<[u8; 3]>)> {
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
    Some((im, pal.clone()))
}

/// Drawing rule (docs/mechanics.md 7.3). KID.DAT images face LEFT as stored; the engine mirrors them when the kid faces right.
/// Returns (sx / 32, bottom) in world units: sx = screen x of the image's left edge (facing left) or right edge (facing right),
/// bottom = the image's bottom line (row 2 floor = 0).
fn world_pos(k: &Kid, env: &Env) -> Vec2 {
    let f = env.frame_of(k.charid, k.frame);
    let sx = screen_x(k.x, f.dx, f.flags, k.facing_left);
    let bottom = k.y.wrapping_add(f.dy as u8) as f32;
    // Logic column c spans x = 65 + 14c .. 79 + 14c (collision and floor checks use col_m7 = x - 7), i.e. screen px 32c + 16.
    // The tile geometry is built at world x = c .. c + 1, so the sprite is shifted left by half a tile (16 px).
    Vec2::new(sx as f32 / 32.0 - 0.5, (181.0 - bottom) * crate::room::TILE_H / TILE_H_PX as f32)
}

/// 7.3: screen x (320 px wide) of the image's left edge (facing left) or exclusive right edge (facing right).
pub(crate) fn screen_x(x: u8, dx: i8, flags: u8, facing_left: bool) -> i32 {
    let dir_byte: u8 = if facing_left { 0xFF } else { 0 };
    let dxs = if facing_left { -(dx as i32) } else { dx as i32 };
    let parity = (((flags ^ dir_byte) & 0x80) == 0) as i32;
    (2 * (x as i32 + dxs) - 116 + parity) * 320 / 280
}

/// Build the world for `p` (restart, next level, new game, the demo level 0). Cutscenes: see `flow` and `cutscene`.
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
pub fn keys(k: Res<ButtonInput<KeyCode>>, m: Res<KeyMap>, pads: Query<&Gamepad>, mut g: ResMut<Game>, a: Res<KidAssets>) {
    // restart keys (0000:054D): Shift held or Enter after death, Ctrl+A (controller: Back/Select) any time
    let enter = k.just_pressed(KeyCode::Enter) || k.just_pressed(KeyCode::NumpadEnter);
    let shift = pressed(&k, &m, &pads).shift;
    let back = pads.iter().any(|p| p.just_pressed(GamepadButton::Select));
    g.world.restart_input(shift || enter, (ctrl_held(&k) && k.just_pressed(KeyCode::KeyA)) || back);
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
pub fn fixed_step(g: &mut Game, a: &KidAssets, latch: &mut Latch, held: Keys) {
    if g.world.exit.is_some() {
        return; // an exit is pending: Update consumes it before any further tick mutates the carried state
    }
    if latch.1 != g.gen {
        // a new world: taps latched before it (the key that skipped a card, started the game or ended the demo) must not reach
        // its first tick; keys still held count
        (latch.0, latch.1) = (held, g.gen);
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
        latch.0 = held;
        g.banner.0 = g.banner.0.saturating_sub(1);
        if g.world.kid.controlled {
            // follow the kid when he changes rooms (a room picked with POP_ROOM or Ctrl+arrows stays until then)
            if g.world.kid.room != g.kid_prev.1 && (1..=pop_core::level::ROOMS as u8).contains(&g.world.kid.room) {
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
    vx: &mut Voxels,
    t: &mut Transform,
    vis: &mut Visibility,
    mat: &mut MeshMaterial3d<StandardMaterial>,
    mesh: &mut Mesh3d,
) -> Option<Vec2> {
    let data = a.data.clone();
    let env = mk_env(&data, level, gtype);
    let f = env.frame_of(k.charid, k.frame);
    // chtab = 2 + (sword >> 6): 5 = guard-type images, otherwise the kid images (the shadow draws those dark)
    let guard_img = f.sword >> 6 == 3;
    let kind = if guard_img { gtype.max(0) as u8 } else { 255 };
    let key = (kind, f.image, k.charid == 1 && !guard_img, palace(level_no), k.colour);
    let Some((m, w, h)) = (f.image != 255).then(|| material_for(a, imgs, mats, key.0, key.1, key.2, key.3, key.4)).flatten() else {
        *vis = Visibility::Hidden;
        return None;
    };
    *vis = Visibility::Inherited;
    let cur = world_pos(k, &env);
    let p = if prev.1 == k.room { prev.0.lerp(cur, alpha) } else { cur };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    // facing left: p.x is the left edge, image as stored; facing right: p.x is the right edge and the image is mirrored
    let (cx, flip) = if k.facing_left { (p.x + sw / 2.0, 1.0) } else { (p.x - sw / 2.0, -1.0) };
    show(a, vx, key, !k.facing_left, m, Vec3::new(cx, p.y + sh / 2.0, 0.6), Vec2::new(flip * sw, sh), VOXEL_Z, t, mat, mesh);
    // hit flash (voxel only): hp_delta stays negative until the next logic tick clears it
    if k.hp_delta < 0 && mat.0 == a.voxel_mat {
        mat.0 = a.voxel_flash.clone();
    }
    Some(p)
}

/// Depth (world z) of the voxel figures' centre. Tile blocks span z -0.5..0.5 (room.rs), so the figure stands inside the
/// room's depth instead of in front of the blocks. Not exactly the middle (0.0): pillars and bars are centred there and
/// would cut through the body; 0.25 keeps the figure (about 0.08 behind, 0.14 in front) clear of them and behind the block front faces.
/// The flat sprite quads stay at z 0.6 (in front).
const VOXEL_Z: f32 = 0.25;

/// The voxel side of `render`: the style in use and the mesh store.
pub struct Voxels<'a> {
    pub on: bool,
    pub meshes: &'a mut Assets<Mesh>,
}

/// Show image `key` as the sprite quad (centre, size with a negative x = mirrored), or in voxel style as its voxel mesh on
/// the same rectangle: mesh origin = the image's bottom-left corner, one voxel = one sprite pixel, depth centred on `vz`.
/// The mirrored image is a separate mesh (no negative scale, so face winding and normals stay right). An image without a
/// voxel mesh falls back to the sprite.
#[allow(clippy::too_many_arguments)]
fn show(a: &mut KidAssets, vx: &mut Voxels, key: (u8, u8, bool, bool, u8), mirrored: bool, sprite: Handle<StandardMaterial>, centre: Vec3, size: Vec2, vz: f32, t: &mut Transform, mat: &mut MeshMaterial3d<StandardMaterial>, mesh: &mut Mesh3d) {
    let voxel = if vx.on { voxel_for(a, vx.meshes, key.0, key.1, key.2, key.3, key.4, mirrored) } else { None };
    if let Some(v) = voxel {
        let px = Vec3::new(1.0 / 32.0, crate::room::TILE_H / TILE_H_PX as f32, 1.0 / 32.0);
        mesh.0 = v;
        mat.0 = a.voxel_mat.clone();
        t.translation = Vec3::new(centre.x - size.x.abs() / 2.0, centre.y - size.y / 2.0, vz);
        t.scale = px;
        return;
    }
    if mesh.0 != a.quad {
        mesh.0 = a.quad.clone();
    }
    mat.0 = sprite;
    t.translation = centre;
    t.scale = Vec3::new(size.x, size.y, 1.0);
}

/// draw_sword (0000:84EC) draws the frame's sword entry when Char.sword != 0, on the sword pickup frames 229..237 (the kid
/// raises the found sword with sword still 0), or for a living guard (charid 2, alive < 0).
fn sword_shown(k: &Kid) -> bool {
    k.sword != 0 || (229..=237).contains(&k.frame) || (k.charid == 2 && k.alive)
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
    vx: &mut Voxels,
    t: &mut Transform,
    vis: &mut Visibility,
    mat: &mut MeshMaterial3d<StandardMaterial>,
    mesh: &mut Mesh3d,
) {
    let data = a.data.clone();
    let env = mk_env(&data, level, gtype);
    let f = env.frame_of(k.charid, k.frame);
    let entry = a.sword_table.get((f.sword & 0x3F) as usize).copied();
    let (Some(p), Some((img, dx, dy)), true) = (body, entry, sword_shown(k)) else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some((m, w, h)) = (img != 255).then(|| material_for(a, imgs, mats, 254, img, false, false, 0)).flatten() else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let dxs = if k.facing_left { -(dx as f32) } else { dx as f32 };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    let x = p.x + dxs / 32.0;
    let y = p.y - dy as f32 * crate::room::TILE_H / TILE_H_PX as f32;
    let (cx, flip) = if k.facing_left { (x + sw / 2.0, 1.0) } else { (x - sw / 2.0, -1.0) };
    // voxel sword just in front of the body's thickest layers (the original draws it over the body)
    show(a, vx, (254, img, false, false, 0), !k.facing_left, m, Vec3::new(cx, y + sh / 2.0, 0.65), Vec2::new(flip * sw, sh), VOXEL_Z + 5.0 / 32.0, t, mat, mesh);
}

pub fn render(
    g: Res<Game>,
    a: ResMut<KidAssets>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    style: Res<CharacterStyle>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (With<KidSprite>, Without<GuardSprite>)>,
    mut qg: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (With<GuardSprite>, Without<KidSprite>)>,
    mut qs: Query<(&SwordSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (Without<KidSprite>, Without<GuardSprite>)>,
) {
    let mut a = a;
    let w = &g.world;
    let mut kid_body = None;
    let mut guard_body = None;
    let vx = &mut Voxels { on: *style == CharacterStyle::Voxel, meshes: &mut meshes };
    // the shown room can differ from the kid's (POP_ROOM, Ctrl+arrows); room 0 = outside the level: keep drawing him
    let kid_here = w.kid.room as usize == g.room || !(1..=pop_core::level::ROOMS as u8).contains(&w.kid.room);
    if let Ok((mut t, mut vis, mut mat, mut mesh)) = q.single_mut() {
        if !kid_here {
            *vis = Visibility::Hidden;
        } else {
            kid_body = draw_char(&w.kid, g.kid_prev, w.alpha(), &mut a, &w.level, w.guard_type, w.level_no, &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh);
        }
    }
    if let Ok((mut t, mut vis, mut mat, mut mesh)) = qg.single_mut() {
        match &w.guard {
            Some(gd) if gd.room as usize == g.room => guard_body = draw_char(gd, g.guard_prev, w.alpha(), &mut a, &w.level, w.guard_type, w.level_no, &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh),
            _ => *vis = Visibility::Hidden,
        }
    }
    for (SwordSprite(is_guard), mut t, mut vis, mut mat, mut mesh) in &mut qs {
        let (k, body) = if *is_guard { (w.guard.as_ref(), guard_body) } else { (Some(&w.kid), kid_body) };
        match k {
            Some(k) => draw_sword(k, body, &mut a, &w.level, w.guard_type, &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Hang frame 91 (image 102, 14 px wide, hands in stored columns 7..9; dx 3, flags 0x80) on the same ledge edge, logic x
    /// from pop-core: facing left on level 8 room 19 (ledge col 3, kid x 118), facing right on level 2 room 1 (ledge col 4,
    /// kid x 123). Both edges are logic x 121 = screen px 144 (128 after the -16 px tile shift). Facing left the image is
    /// 131..145 (hands 138..140, 4..6 px over the ledge), facing right 141..155 mirrored (hands 145..147, 1..3 px over it):
    /// the 2 px difference is the original's own rounding (core mirror pairs x <-> 283 - x, the 8/7 truncation), not an anchor error.
    #[test]
    fn hang_anchor_matches_the_original_formula() {
        let (left, right) = (screen_x(118, 3, 0x80, true), screen_x(123, 3, 0x80, false));
        assert_eq!((left, right), (131, 155));
        let (lh, rh) = ((left + 7, left + 9), (right - 1 - 9, right - 1 - 7));
        assert_eq!((lh, rh), ((138, 140), (145, 147)));
        assert!(lh.1 < 144 && rh.0 >= 144, "hands on the ledge side of the edge in both directions");
        assert!((144 - lh.0) - (rh.1 - 143) <= 2, "within 2 px of mirror symmetric");
    }

    #[test]
    fn sword_drawn_on_pickup_frames_and_when_drawn() {
        let k = |frame, sword, charid, alive| { let mut k = Kid::default(); (k.frame, k.sword, k.charid, k.alive) = (frame, sword, charid, alive); k };
        assert!(sword_shown(&k(230, 0, 0, true)) && sword_shown(&k(237, 0, 0, true)), "pickup frames, sword still 0");
        assert!(!sword_shown(&k(238, 0, 0, true)) && !sword_shown(&k(15, 0, 0, true)), "sheathed frames");
        assert!(sword_shown(&k(158, 2, 0, true)));
        assert!(sword_shown(&k(166, 0, 2, true)) && !sword_shown(&k(166, 0, 2, false)), "living guard (charid 2)");
    }

    #[test]
    fn keypad_matches_the_original_scancodes() {
        let dirs = |c| { let k = keypad(c); (k.left, k.right, k.up, k.down, k.shift) };
        assert_eq!(dirs(KeyCode::Numpad7), (true, false, true, false, false));
        assert_eq!(dirs(KeyCode::Home), dirs(KeyCode::Numpad7));
        assert_eq!(dirs(KeyCode::Numpad8), (false, false, true, false, false));
        assert_eq!(dirs(KeyCode::Numpad9), (false, true, true, false, false));
        assert_eq!(dirs(KeyCode::PageUp), dirs(KeyCode::Numpad9));
        assert_eq!(dirs(KeyCode::Numpad4), (true, false, false, false, false));
        assert_eq!(dirs(KeyCode::Numpad6), (false, true, false, false, false));
        assert_eq!(dirs(KeyCode::Numpad5), (false, false, false, true, false));
        assert_eq!(dirs(KeyCode::Numpad2), (false, false, false, true, false));
        for c in [KeyCode::Numpad1, KeyCode::Numpad3, KeyCode::Numpad0, KeyCode::End, KeyCode::PageDown, KeyCode::Insert, KeyCode::KeyA] {
            assert_eq!(dirs(c), (false, false, false, false, false), "{c:?}");
        }
    }
}
