//! The kid in the app: input latching, per-tick control (pop-core), and drawing with the original sprites (KID.DAT).
//! Sequence viewer: `,` / `.` = previous/next sequence (kid no longer controlled), Space = restart it, F = flip,
//! C = back to the controlled kid at the level start.
use crate::settings::{CharacterStyle, KeyMap, PadLayout};
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
    /// eased voxel depth of the kid and the guard (see `ease_z`)
    voxel_zs: [f32; 2],
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
    vcache: HashMap<(u8, u8, bool, bool, u8, bool, Hang), Option<Handle<Mesh>>>,
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
pub struct Latch(Keys, u32, u64); // the u64: tick of the last early tick (quick_input)

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
        KidAssets { data, dat, pal, gdats, gpals, colours, pdat, sword_pal, sword_table, cache: HashMap::new(), vcache: HashMap::new(), quad: default(), voxel_mat: default(), voxel_flash: default(), smooth: true, voxel_zs: [VOXEL_Z; 2] }
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

/// Applies `voxel_shading` and `voxel_normals` when the settings change: the shared materials are rewritten in place and the
/// mesh cache is dropped, so `show` builds the figures' meshes again with the new normals on the next frame.
pub fn live_look(set: Res<crate::settings::Settings>, mut a: ResMut<KidAssets>, mut mats: ResMut<Assets<StandardMaterial>>) {
    if !set.is_changed() || set.is_added() {
        return;
    }
    for (h, flash) in [(a.voxel_mat.clone(), false), (a.voxel_flash.clone(), true)] {
        if let Some(mut m) = mats.get_mut(&h) {
            *m = crate::voxel::character_material(set.unlit_voxels(), flash);
        }
    }
    if a.smooth != set.smooth_normals() {
        a.smooth = set.smooth_normals();
        a.vcache.clear();
    }
}

fn ctrl_held(k: &ButtonInput<KeyCode>) -> bool {
    k.pressed(KeyCode::ControlLeft) || k.pressed(KeyCode::ControlRight) // Ctrl+arrows navigate rooms instead
}

/// The kid's keys held now: the bound keyboard keys (settings.json) or any controller (d-pad / left stick, A = Shift).
pub fn pressed(k: &ButtonInput<KeyCode>, m: &KeyMap, pads: &Query<&Gamepad>) -> Keys {
    let mut keys = if ctrl_held(k) { Keys::default() } else { Keys { left: k.any_pressed(m.left.iter().copied()), right: k.any_pressed(m.right.iter().copied()), up: k.any_pressed(m.up.iter().copied()), down: k.any_pressed(m.down.iter().copied()), shift: k.any_pressed(m.shift.iter().copied()) } };
    if !ctrl_held(k) {
        for p in k.get_pressed().map(|&c| keypad(c)) {
            keys = Keys { left: keys.left | p.left, right: keys.right | p.right, up: keys.up | p.up, down: keys.down | p.down, shift: keys.shift };
        }
    }
    for p in pads {
        let c = pad_keys(m.pad, |b| p.pressed(b), p.left_stick());
        keys = Keys { left: keys.left | c.left, right: keys.right | c.right, up: keys.up | c.up, down: keys.down | c.down, shift: keys.shift | c.shift };
    }
    keys
}

/// The kid's keys from one controller (buttons, left stick) in a layout. Modern: A and Y = up (jump, parry), B = down (no down on the
/// stick or D-pad), X / RB / RT = Shift. Combat: down also on stick and D-pad, parry on LB / LT instead of Y. Classic: A or X = Shift.
pub fn pad_keys(layout: PadLayout, held: impl Fn(GamepadButton) -> bool, s: Vec2) -> Keys {
    use GamepadButton::*;
    let any = |b: &[GamepadButton]| b.iter().any(|&x| held(x));
    let (left, right) = (held(DPadLeft) || s.x < -0.5, held(DPadRight) || s.x > 0.5);
    let (up, down) = (held(DPadUp) || s.y > 0.5, held(DPadDown) || s.y < -0.5);
    match layout {
        PadLayout::Classic => Keys { left, right, up, down, shift: any(&[South, West]) },
        PadLayout::Combat => Keys { left, right, up: up || any(&[South, LeftTrigger, LeftTrigger2]), down: down || held(East), shift: any(&[West, RightTrigger, RightTrigger2]) },
        PadLayout::Modern => Keys { left, right, up: up || any(&[South, North]), down: held(East), shift: any(&[West, RightTrigger, RightTrigger2]) },
    }
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
fn voxel_for(a: &mut KidAssets, meshes: &mut Assets<Mesh>, kind: u8, n: u8, shadow: bool, pal_palace: bool, colour: u8, mirrored: bool, hang: Hang) -> Option<Handle<Mesh>> {
    let key = (kind, n, shadow, pal_palace, colour, mirrored, hang);
    if let Some(m) = a.vcache.get(&key) {
        return m.clone();
    }
    let m = decoded(a, kind, n, pal_palace, colour).map(|(im, pal)| {
        // the profile's colour regions come from the untinted palette (the shadow's dark tint would merge them)
        let model = posed_model(&im, &pal, mirrored, hang);
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

/// The voxel model of image `im`, with the hands swept or the climb's ramp applied (`hang`).
fn posed_model(im: &image::Image, pal: &[[u8; 3]], mirrored: bool, hang: Hang) -> crate::voxel::VoxelModel {
    let model = crate::voxel::VoxelModel::character(im.width, im.height, &im.pixels, mirrored, |p| rgb(pal, p, false));
    match hang {
        (0, _) => model,
        (rows, false) => model.rows_swept(rows, hand_back, true),
        (rows, true) => model.rows_swept(rows, ramp_back(rows), false),
    }
}

/// Blood on a sword picture (cinematic fights): level 1 puts flecks on the grey blade pixels, level 2 covers the blade. Two reds
/// are appended to the palette; the golden hilt stays.
fn bloody(im: &mut image::Image, pal: &mut Vec<[u8; 3]>, level: u8) {
    let Ok(base) = u8::try_from(pal.len()) else { return };
    pal.extend([[150, 12, 16], [96, 6, 10]]);
    let w = im.width as usize;
    for (i, p) in im.pixels.iter_mut().enumerate() {
        let Some(c) = pal.get(*p as usize) else { continue };
        let (hi, lo) = (c.iter().max().copied().unwrap_or(0), c.iter().min().copied().unwrap_or(0));
        let blade = *p != 0 && hi - lo < 48 && hi > 80;
        let fleck = (i % w) * 5 + (i / w) * 7 + (i % w) * (i / w);
        if blade && (level >= 2 || fleck % 3 == 0) {
            *p = base + (i % 2) as u8;
        }
    }
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
    let mut im = image::decode(dat.get(match kind { 255 => 401, 254 => 701, _ => 751 } + n as u16)?)?;
    let mut pal = pal.clone();
    if kind == 254 && colour > 0 {
        bloody(&mut im, &mut pal, colour);
    }
    Some((im, pal))
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
    Vec2::new((sx as f32 + hang_dx(k.charid, k.frame, k.facing_left)) / 32.0 - 0.5, (181.0 - bottom) * crate::room::TILE_H / TILE_H_PX as f32)
}

/// The ledge poses (hang 78..=99, climb 135..=149): without a shift the hands lie 5..7 px (facing left, ledge col c - 1, kid x
/// 118 / 62) or 2..4 px (facing right, kid x 123; the sim grabs 5 logic units differently per mirroring) inside the slab's
/// end, measured on frame 91 (see `real_hang_hands_grip_the_slab_end_in_both_mirrorings`). The hands should grip the top just
/// inside its end (0..2 px), so the figure hangs AWAY from the ledge by these px: right when facing left, left when facing right.
const LEDGE_AWAY: [f32; 2] = [5.0, 2.0]; // [facing left, facing right]
fn hang_dx(charid: u8, frame: u8, facing_left: bool) -> f32 {
    // the grab (78..=80) slides out, the drop (81..=85) back; the climb's last frames slide onto the top
    let k = match (charid < 2, frame) {
        (true, 78..=80) => (frame - 77) as f32 / 4.0,
        (true, 81..=85) => (86 - frame) as f32 / 5.0 * 0.75,
        (true, 87..=99 | 135..=140) => 1.0,
        (true, 141..=144) => (145 - frame) as f32 / 5.0,
        _ => 0.0,
    };
    if facing_left { LEDGE_AWAY[0] * k } else { -LEDGE_AWAY[1] * k }
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
    g.kid_prev = Snap::of(&g.world.kid, &env);
    g.guard_prev = g.world.guard.as_ref().map_or_else(Snap::default, |k| Snap::of(k, &env));
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
pub fn fixed_step(g: &mut Game, a: &KidAssets, latch: &mut Latch, held: Keys, quick: bool) {
    // test hook: POP_WARMUP=n holds the world for n steps (60 Hz), so POP_SHOT_TICK screenshots wait out the shader compile
    static WARM: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if WARM.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < std::env::var("POP_WARMUP").ok().and_then(|v| v.parse().ok()).unwrap_or(0) {
        return;
    }
    if std::env::var("POP_SHOT_TICK").ok().and_then(|v| v.parse::<u64>().ok()).is_some_and(|t| g.world.tick >= t) {
        return; // test hook: freeze at the screenshot tick
    }
    if g.world.exit.is_some() {
        return; // an exit is pending: Update consumes it before any further tick mutates the carried state
    }
    if latch.1 != g.gen {
        // a new world: taps latched before it (the key that skipped a card, started the game or ended the demo) must not reach
        // its first tick; keys still held count
        (latch.0, latch.1) = (held, g.gen);
    }
    // quick_input: a direction key arriving while the kid stands idle starts the tick now instead of up to 83 ms later; the cooldown
    // (3 ticks) keeps a key that does nothing from speeding the game up
    if quick && (latch.0.left || latch.0.right || latch.0.up || latch.0.down) && g.world.tick >= latch.2 + 3 && g.world.tick_now() {
        latch.2 = g.world.tick;
    }
    if g.world.next_is_tick() {
        let env = mk_env(&a.data, &g.world.level, g.world.guard_type);
        g.kid_prev = Snap::of(&g.world.kid, &env);
        g.guard_prev = g.world.guard.as_ref().map_or_else(Snap::default, |k| Snap::of(k, &env));
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
            if g.world.kid.room != g.kid_prev.room && (1..=pop_core::level::ROOMS as u8).contains(&g.world.kid.room) {
                g.room = g.world.kid.room as usize; // room 0 = outside the level: keep showing the last room
            }
        } else if g.world.kid.x < 20 || g.world.kid.x > 200 {
            g.world.kid.x = 100; // sequence viewer: keep the kid in view
            let env = mk_env(&a.data, &g.world.level, g.world.guard_type);
            g.kid_prev = Snap::of(&g.world.kid, &env);
        }
    }
}

/// A character at the start of a logic tick: the DRAWN centre of its image (world units: world_pos anchor, `hang_dx` and the
/// image offset included), room, mirroring, frame. Room 0 = none. Interpolating the centre keeps the picture continuous when
/// the frame (and with it the image and its anchor) changes, as in the climb's Dy(-63).
#[derive(Clone, Copy, Default, Debug)]
pub struct Snap {
    pub pos: Vec2,
    pub room: u8,
    pub left: bool,
    pub frame: u8,
}

impl Snap {
    fn of(k: &Kid, env: &Env) -> Snap {
        let (h, w) = env.size_of(&env.frame_of(k.charid, k.frame));
        let (a, sw, sh) = (world_pos(k, env), w as f32 / 32.0, h as f32 * crate::room::TILE_H / TILE_H_PX as f32);
        Snap { pos: Vec2::new(if k.facing_left { a.x + sw / 2.0 } else { a.x - sw / 2.0 }, a.y + sh / 2.0), room: k.room, left: k.facing_left, frame: k.frame }
    }
}

/// Largest per-axis step (screen px) of the drawn centre still drawn as motion; see `interpolation_measurement`.
pub const MAX_LERP_PX: f32 = 40.0;

/// Whether the figure is drawn between `prev` and `cur` (interpolation) or at `cur`: not across a room change, a mirror flip
/// or a step of the drawn centre over `MAX_LERP_PX` on an axis (a teleport, not motion). The ledge poses are interpolated too.
pub fn lerps(prev: &Snap, cur: &Snap) -> bool {
    let (dx, dy) = ((cur.pos.x - prev.pos.x) * 32.0, (cur.pos.y - prev.pos.y) * TILE_H_PX as f32 / crate::room::TILE_H);
    prev.room == cur.room && prev.left == cur.left && dx.abs() <= MAX_LERP_PX && dy.abs() <= MAX_LERP_PX
}

/// Place a character sprite: `prev` is the position at the start of the last logic tick (interpolation).
#[allow(clippy::too_many_arguments)]
fn draw_char(
    k: &Kid,
    prev: Snap,
    alpha: f32,
    smooth: bool,
    a: &mut KidAssets,
    level: &pop_core::level::Level,
    gtype: i8,
    level_no: usize,
    (slot, dt): (usize, f32),
    centre: &mut Option<Vec2>,
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
    let Some((m, w, h)) = (f.image != 255 && !stairs_gone(k.frame)).then(|| material_for(a, imgs, mats, key.0, key.1, key.2, key.3, key.4)).flatten() else {
        *vis = Visibility::Hidden;
        *centre = None;
        return None;
    };
    *vis = Visibility::Inherited;
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    let cur = Snap::of(k, &env);
    let c = if smooth && lerps(&prev, &cur) { prev.pos.lerp(cur.pos, alpha) } else { cur.pos };
    let p = Vec2::new(if k.facing_left { c.x - sw / 2.0 } else { c.x + sw / 2.0 }, c.y - sh / 2.0); // the anchor
    // facing left: p.x is the left edge, image as stored; facing right: p.x is the right edge and the image is mirrored
    let (cx, flip) = if k.facing_left { (p.x + sw / 2.0, 1.0) } else { (p.x - sw / 2.0, -1.0) };
    let vz = settle_z(&mut a.voxel_zs[slot], k.frame, k.action, dt);
    let hang = hang_of(k.frame, h as i32, f.dy as i32);
    show(a, vx, key, !k.facing_left, m, Vec3::new(cx, p.y + sh / 2.0, if on_stairs(k.frame) { STAIRS_Z } else { 0.6 }), Vec2::new(flip * sw, sh), vz, hang, t, mat, mesh);
    // hit flash (voxel only): hp_delta stays negative until the next logic tick clears it
    if k.hp_delta < 0 && mat.0 == a.voxel_mat {
        mat.0 = a.voxel_flash.clone();
    }
    *centre = Some(Vec2::new(cx, p.y));
    Some(p)
}

/// Depth (world z) of the voxel figures' centre. Tile blocks span z -0.5..0.5 (room.rs), so the figure stands inside the
/// room's depth instead of in front of the blocks. Not exactly the middle (0.0): pillars and bars are centred there and
/// would cut through the body; 0.25 keeps the figure (about 0.08 behind, 0.14 in front) clear of them and behind the block front faces.
/// The flat sprite quads stay at z 0.6 (in front).
const VOXEL_Z: f32 = 0.25;

/// Hanging and climbing poses (jump-up grab, hang, climb up/down, drop; actions 2 and 6, frames 78..=99 and 135..=149) are
/// authored for the 2D painter order: hands and body overlap the ledge's end face and the wall beside it. In 3D those
/// blocks reach z 0.5 (their front face), so at VOXEL_Z the arms and shoulders sink into the slab and the climber into the
/// wall. The whole figure goes in front of the faces instead: its rearmost layer (about 0.08 behind the plane) just clears 0.5.
const HANG_Z: f32 = 0.62;

/// Where the hands lie: the ledge slab is z -0.95..0.5 (room.rs), a hanging figure's arms reach over its top face. The rows
/// above the ledge line are swept back into the top face (`hand_back`, joined to the arm);
/// so the body (below the line, in front of the front face) and the hands (above the top face, inside the footprint) never touch the slab.
/// Voxels the row `d` (>= 1) rows above the ledge line moves back. The original draws the ledge's top face as 13 rows
/// (docs/texture_inventory.md 20) for the slab's whole depth, so a hand `d` rows above the lip lies `d / 13` of the depth
/// into the top face (cabinet projection, the same scale as everywhere else): from the front lip (z SLAB_FRONT) back, joined to the arm.
fn hand_back(d: usize) -> usize {
    let (lip, per_row) = ((HANG_Z - crate::room::SLAB_FRONT) * 32.0, (crate::room::SLAB_FRONT - crate::room::SLAB_BACK) * 32.0 / 13.0);
    (lip + per_row * d as f32 + 0.5) as usize
}

/// The climb (frames 135..=149): the rows above the slab top lean back from HANG_Z at the line to VOXEL_Z (the slab's
/// footprint) within RAMP_BACK rows, 1 voxel per row (fewer rows: the same total over the rows there are), while the rows below stay in front of the front face.
const RAMP_BACK: usize = ((HANG_Z - VOXEL_Z) * 32.0 + 0.5) as usize;

/// Voxels the row `d` (>= 1) above the line moves back in a figure with `rows` rows above it.
fn ramp_back(rows: usize) -> impl Fn(usize) -> usize {
    let r = rows.min(RAMP_BACK);
    move |d| (RAMP_BACK * d.min(r) + r / 2) / r
}

/// Image rows (from the top) above the ledge top of a hanging frame's image `h` px tall with offset `dy`: the ledge is one
/// tile (TILE_H_PX) above the logic y the sprite bottom hangs from (k.y + dy), whatever the room.
fn hang_rows(h: i32, dy: i32) -> usize {
    (h - dy - TILE_H_PX as i32).clamp(0, h) as usize
}

/// The mesh variant of a ledge frame: (rows above the slab top, true = leaning ramp / false = swept hands); (0, _) = plain.
/// The climb's Dy(-63) comes after frame 140: from 141 the logic y is the slab top itself, the legs still dangle below it.
type Hang = (usize, bool);
fn hang_of(frame: u8, h: i32, dy: i32) -> Hang {
    let rows = ledge_rows(frame, h, dy);
    match frame {
        78..=99 => (rows, false),
        135..=149 if rows > 0 && (rows as i32) < h => (rows, true),
        _ => (0, false),
    }
}

/// `hang_rows` of a ledge frame; from 141 on the logic y is the slab top itself (the climb's Dy(-63), see `hang_of`).
fn ledge_rows(frame: u8, h: i32, dy: i32) -> usize {
    hang_rows(h, if frame >= 141 { dy - TILE_H_PX as i32 } else { dy })
}

/// Moves the drawn depth `z` towards `target` (frame-rate independent exponential ease, time constant 40 ms), so the
/// figure slides in front of the blocks when it grabs a ledge and back when it stands up instead of jumping.
fn ease_z(z: &mut f32, target: f32, dt: f32) -> f32 {
    *z += (target - *z) * (1.0 - (-dt / 0.04).exp());
    *z
}

/// The depth of this frame. The climb frames (135..=149) set it at once: their mesh already leans from HANG_Z to VOXEL_Z, an ease would slide the whole figure through the slab.
fn settle_z(z: &mut f32, frame: u8, action: u8, dt: f32) -> f32 {
    if matches!(frame, 135..=149 | 217..=228) {
        *z = voxel_z(frame, action);
        return *z;
    }
    ease_z(z, voxel_z(frame, action), dt)
}

/// The exit stairs climb (frames 217..=228, docs/tiles.md 8.1): the prince walks into the doorway and has to vanish behind it.
/// The exit door's frame reaches about z -0.17 at its front, its panel and stairs end at -0.35 / -0.42, so the figure goes
/// between them (voxel figure about 0.08 behind and 0.14 in front of its plane, flat quad exactly at it).
const STAIRS_Z: f32 = -0.32;

/// The last steps of the climb lead past the doorway's right post; the original hides them behind the door's mask, which the
/// 3D door has no counterpart for, so the figure is gone from here on.
fn stairs_gone(frame: u8) -> bool {
    matches!(frame, 225..=228)
}

fn on_stairs(frame: u8) -> bool {
    matches!(frame, 217..=228)
}

fn voxel_z(frame: u8, action: u8) -> f32 {
    if on_stairs(frame) {
        STAIRS_Z
    } else if matches!(action, 2 | 6) || matches!(frame, 78..=99 | 135..=144) { HANG_Z } else { VOXEL_Z }
}

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
fn show(a: &mut KidAssets, vx: &mut Voxels, key: (u8, u8, bool, bool, u8), mirrored: bool, sprite: Handle<StandardMaterial>, centre: Vec3, size: Vec2, vz: f32, hang: Hang, t: &mut Transform, mat: &mut MeshMaterial3d<StandardMaterial>, mesh: &mut Mesh3d) {
    let voxel = if vx.on { voxel_for(a, vx.meshes, key.0, key.1, key.2, key.3, key.4, mirrored, hang) } else { None };
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
    blood: u8,
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
) -> Option<(Vec2, Vec2)> {
    let data = a.data.clone();
    let env = mk_env(&data, level, gtype);
    let f = env.frame_of(k.charid, k.frame);
    let entry = a.sword_table.get((f.sword & 0x3F) as usize).copied();
    let (Some(p), Some((img, dx, dy)), true) = (body, entry, sword_shown(k)) else {
        *vis = Visibility::Hidden;
        return None;
    };
    let Some((m, w, h)) = (img != 255).then(|| material_for(a, imgs, mats, 254, img, false, false, blood)).flatten() else {
        *vis = Visibility::Hidden;
        return None;
    };
    *vis = Visibility::Inherited;
    let dxs = if k.facing_left { -(dx as f32) } else { dx as f32 };
    let (sw, sh) = (w / 32.0, h * crate::room::TILE_H / TILE_H_PX as f32);
    let x = p.x + dxs / 32.0;
    let y = p.y - dy as f32 * crate::room::TILE_H / TILE_H_PX as f32;
    let (cx, flip) = if k.facing_left { (x + sw / 2.0, 1.0) } else { (x - sw / 2.0, -1.0) };
    // voxel sword just in front of the body's thickest layers (the original draws it over the body)
    show(a, vx, (254, img, false, false, blood), !k.facing_left, m, Vec3::new(cx, y + sh / 2.0, 0.65), Vec2::new(flip * sw, sh), VOXEL_Z + 5.0 / 32.0, (0, false), t, mat, mesh);
    Some((Vec2::new(cx, y + sh / 2.0), Vec2::new(sw, sh)))
}

pub fn render(
    g: Res<Game>,
    time: Res<Time>,
    a: ResMut<KidAssets>,
    mut imgs: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    style: Res<CharacterStyle>,
    set: Res<crate::settings::Settings>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (With<KidSprite>, Without<GuardSprite>)>,
    mut qg: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (With<GuardSprite>, Without<KidSprite>)>,
    mut swords: ResMut<crate::particles::Swords>,
    mut gore: ResMut<crate::blood::Gore>,
    mut qs: Query<(&SwordSprite, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d), (Without<KidSprite>, Without<GuardSprite>)>,
) {
    let mut a = a;
    let dt = time.delta_secs();
    let smooth = set.smooth_motion();
    // test hook: POP_SHOT_ALPHA=0.5 draws the frozen POP_SHOT_TICK world at that point of its last logic frame
    let alpha = std::env::var("POP_SHOT_ALPHA").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| g.world.alpha());
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
            kid_body = draw_char(&w.kid, g.kid_prev, alpha, smooth, &mut a, &w.level, w.guard_type, w.level_no, (0, dt), &mut gore.bodies[0], &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh);
        }
    }
    if let Ok((mut t, mut vis, mut mat, mut mesh)) = qg.single_mut() {
        match &w.guard {
            Some(gd) if gd.room as usize == g.room => guard_body = draw_char(gd, g.guard_prev, alpha, smooth, &mut a, &w.level, w.guard_type, w.level_no, (1, dt), &mut gore.bodies[1], &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh),
            _ => {
                *vis = Visibility::Hidden;
                gore.bodies[1] = None;
            }
        }
    }
    for (SwordSprite(is_guard), mut t, mut vis, mut mat, mut mesh) in &mut qs {
        let (k, body) = if *is_guard { (w.guard.as_ref(), guard_body) } else { (Some(&w.kid), kid_body) };
        match k {
            Some(k) => swords.0[*is_guard as usize] = draw_sword(k, body, gore.sword[*is_guard as usize], &mut a, &w.level, w.guard_type, &mut imgs, &mut mats, vx, &mut t, &mut vis, &mut mat, &mut mesh),
            None => {
                *vis = Visibility::Hidden;
                swords.0[*is_guard as usize] = None;
            }
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
    #[test]
    fn blood_flecks_then_covers_the_blade_but_not_the_hilt() {
        // 12 grey blade pixels (palette 1) and a gold hilt pixel (palette 2)
        let img = |p: &[u8]| pop_core::image::Image { width: 13, height: 1, bpp: 8, pixels: p.to_vec() };
        let src: Vec<u8> = [vec![1u8; 12], vec![2]].concat();
        let pal = vec![[0, 0, 0], [200, 200, 205], [220, 180, 40]];
        let red = |im: &pop_core::image::Image| im.pixels.iter().filter(|&&p| p >= 3).count();
        let (mut one, mut two) = (img(&src), img(&src));
        super::bloody(&mut one, &mut pal.clone(), 1);
        super::bloody(&mut two, &mut pal.clone(), 2);
        assert!(red(&one) > 0 && red(&one) < 12, "flecks: {}", red(&one));
        assert_eq!(red(&two), 12);
        assert_eq!((one.pixels[12], two.pixels[12]), (2, 2), "the hilt stays");
    }

    use super::*;

    /// Replays scripted runs on the shipped levels (random key holds: running, turning, jumping, climbing, falling, the demo
    /// and guard fights) and counts per logic tick whether `lerps` draws the figure between the two snapshots; `--nocapture`
    /// lists the counts. The interpolated steps never exceed the bound, turns and ledge poses are never interpolated.
    #[test]
    fn interpolation_measurement() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        if Exe::open(format!("{dir}/PRINCE.EXE")).is_err() {
            return;
        }
        let a = KidAssets::load(&dir);
        let lv = Dat::open(format!("{dir}/LEVELS.DAT")).unwrap();
        let mut seed = 12345u32;
        let mut rnd = move || { seed = seed.wrapping_mul(1664525).wrapping_add(1013904223); seed >> 16 };
        let px = |p: Vec2, q: Vec2| ((q.x - p.x).abs() * 32.0, (q.y - p.y).abs() * TILE_H_PX as f32 / crate::room::TILE_H);
        // [lerped, room, turn, jump]; the same for the ledge poses (67..=99, 135..=149) alone
        let (mut n, mut climb, mut max_lerped, mut max_turn) = ([0usize; 4], [0usize; 4], 0f32, 0f32);
        let mut jumps = std::collections::BTreeMap::new();
        for level in 0..=13usize {
            let mut w = World::new(a.data.clone(), pop_core::level::Level::parse(lv.get(2000 + level as u16).unwrap()).unwrap(), level);
            let mut keys = Keys::default();
            for t in 0..3000 {
                if t % 12 == 0 && w.kid.controlled && level > 0 {
                    let r = rnd();
                    keys = Keys { left: r & 3 == 0, right: r & 3 == 1 || r & 12 == 8, up: r & 16 != 0, down: r & 96 == 96, shift: r & 128 != 0 };
                }
                let env = mk_env(&a.data, &w.level, w.guard_type);
                let before = [Some(Snap::of(&w.kid, &env)), w.guard.as_ref().map(|k| Snap::of(k, &env))];
                while !w.step(keys) {}
                let env = mk_env(&a.data, &w.level, w.guard_type);
                for (b, k) in before.into_iter().zip([Some(&w.kid), w.guard.as_ref()]) {
                    let (Some(b), Some(k)) = (b, k) else { continue };
                    let c = Snap::of(k, &env);
                    let (dx, dy) = px(b.pos, c.pos);
                    let i = if lerps(&b, &c) { 0 } else if b.room != c.room { 1 } else if b.left != c.left { 2 } else { 3 };
                    n[i] += 1;
                    if matches!(b.frame, 67..=99 | 135..=149) || matches!(c.frame, 67..=99 | 135..=149) {
                        climb[i] += 1;
                    }
                    match i {
                        0 => {
                            max_lerped = max_lerped.max(dx).max(dy);
                            assert!(dx <= MAX_LERP_PX && dy <= MAX_LERP_PX);
                        }
                        2 => max_turn = max_turn.max(dx).max(dy),
                        3 => *jumps.entry((b.frame, c.frame)).or_insert(0) += 1,
                        _ => {}
                    }
                }
            }
        }
        let total: usize = n.iter().sum();
        let mut top: Vec<_> = jumps.into_iter().collect();
        top.sort_by_key(|j| std::cmp::Reverse(j.1));
        println!("ticks {total}: lerped {} skipped: room {} turn {} jump>{MAX_LERP_PX}px {}; ledge poses: lerped {} skipped {:?}", n[0], n[1], n[2], n[3], climb[0], &climb[1..]);
        println!("max lerped step {max_lerped:.1} px, max drawn step on a turn {max_turn:.1} px, jumps (frame pair, count) {:?}", &top[..top.len().min(8)]);
        assert!(n[0] * 100 > total * 95, "nearly all ticks stay smooth");
        assert!(climb[0] * 100 > climb.iter().sum::<usize>() * 95, "the climb is smooth");
        assert!(max_lerped <= MAX_LERP_PX);
    }

    /// The decision on the drawn centre: motion up to the bound (running, hang / climb steps) is interpolated, turns, rooms
    /// and teleports are not.
    #[test]
    fn lerp_decision() {
        let s = |x: f32, y: f32, left: bool, room: u8| Snap { pos: Vec2::new(x / 32.0, y * crate::room::TILE_H / TILE_H_PX as f32), room, left, frame: 0 };
        assert!(lerps(&s(0.0, 0.0, false, 1), &s(16.0, 0.0, false, 1)), "running");
        assert!(lerps(&s(0.0, 0.0, true, 1), &s(0.0, 0.0, true, 1)), "standing");
        assert!(lerps(&s(0.0, 0.0, false, 1), &s(3.0, -33.0, false, 1)), "climb / free fall (33 px per tick)");
        assert!(!lerps(&s(0.0, 0.0, true, 1), &s(2.0, 0.0, false, 1)), "turn");
        assert!(!lerps(&s(0.0, 0.0, false, 1), &s(41.0, 0.0, false, 1)) && !lerps(&s(0.0, 0.0, false, 1), &s(0.0, 41.0, false, 1)), "teleport");
        assert!(!lerps(&s(0.0, 0.0, false, 1), &s(1.0, 0.0, false, 2)), "room change");
    }

    /// Every frame from the grab (78; 67..77 is the plain jump-up rise) on in the grab / hang / climb sequences (8, 9, 10, 16, 24, 25, 68, 73) is drawn in front of the blocks' front face (z 0.5).
    #[test]
    fn hang_and_climb_frames_stand_in_front_of_the_block_faces() {
        let Ok(exe) = Exe::open(format!("{}/../../Original_data/PRINCE.EXE", env!("CARGO_MANIFEST_DIR"))) else { return };
        for seq in [8, 9, 16, 24, 25, 73, 68] {
            for op in exe.sequence(seq).unwrap() {
                if let pop_core::exe::Op::Frame(f @ 78..=140) = op {
                    assert!(voxel_z(f, 0) > 0.5 + 0.08, "seq {seq} frame {f}");
                }
            }
        }
        for f in 135..=140 {
            assert!(voxel_z(f, 1) > 0.58, "climb frame {f}");
        }
        for f in 141..=144 {
            assert!(voxel_z(f, 1) > 0.58, "legs still over the edge, frame {f}");
        }
        for f in 145..=149 {
            assert_eq!(voxel_z(f, 1), VOXEL_Z, "on the slab, frame {f}");
        }
        assert_eq!((voxel_z(15, 1), voxel_z(0, 0)), (VOXEL_Z, VOXEL_Z), "standing is unchanged");
    }

    /// The ledge line of the hang frames (h px tall, bottom offset dy; measured in the running game: 91 -> 4 rows, 138 -> 14, the
    /// grab frame 78 -> none) and the swept-back hands: no voxel of the body below the line is inside the slab (z < front face,
    /// y in the slab's thickness), no hand voxel above it is in front of the front face or below the top.
    #[test]
    fn hands_lie_on_the_ledge_top_and_the_body_clears_the_slab() {
        use crate::room::{SLAB, SLAB_BACK, SLAB_FRONT};
        assert_eq!((hang_rows(57, -10), hang_rows(53, -12), hang_rows(45, -32), hang_rows(47, -3)), (4, 2, 14, 0));
        let (w, h, rows) = (6, 40, 4);
        let model = crate::voxel::VoxelModel::character(w, h, &vec![1; w * h], false, |_| [200, 150, 100]).rows_swept(rows, hand_back, true);
        let (slab_rows, px) = ((SLAB * 63.0 / 2.0).ceil() as usize, 1.0 / 32.0);
        for y in 0..h {
            for z in 0..model.d {
                for x in 0..w {
                    if model.get(x as isize, y as isize, z as isize) == 0 {
                        continue;
                    }
                    let (back, front) = (HANG_Z + (z as f32 - 0.5 - model.plane as f32) * px, HANG_Z + (z as f32 + 0.5 - model.plane as f32) * px);
                    if y >= rows && y < rows + slab_rows {
                        assert!(back >= SLAB_FRONT, "body row {y} voxel at z {back} is inside the slab");
                    }
                    if y < rows {
                        assert!(back >= SLAB_BACK, "hand row {y} reaches back past the slab: {back}");
                        // joined to the arm: unbroken from here forward to the body's plane layer
                        assert!((z..=model.plane).all(|k| model.get(x as isize, y as isize, k as isize) != 0), "hand gap at {x},{y},{z}");
                    }
                }
            }
        }
        assert!(hand_back(1) < hand_back(4) && hand_back(4) < 24, "deeper into the top face with each row");
    }

    /// Hang frame 91 (image 102, 14 px wide, hands in stored columns 7..9; dx 3, flags 0x80) on the same ledge edge, logic x
    /// from pop-core: facing left on level 8 room 19 (ledge col 3, kid x 118), facing right on level 2 room 1 (ledge col 4,
    /// kid x 123). Both edges are logic x 121 = screen px 144 (128 after the -16 px tile shift). Facing left the image is
    /// 131..145 (hands 138..140, 4..6 px over the ledge), facing right 141..155 mirrored (hands 145..147, 1..3 px over it):
    /// the 2 px difference is the original's own rounding (core mirror pairs x <-> 283 - x, the 8/7 truncation), not an anchor error.
    #[test]
    fn depth_eases_without_a_jump_and_independent_of_the_frame_rate() {
        let (mut a, mut b) = (VOXEL_Z, VOXEL_Z);
        let first = ease_z(&mut a, HANG_Z, 1.0 / 60.0);
        assert!(first > VOXEL_Z && first < HANG_Z - 0.2, "no jump: {first}");
        for _ in 0..30 { ease_z(&mut a, HANG_Z, 1.0 / 60.0); }
        for _ in 0..15 { ease_z(&mut b, HANG_Z, 1.0 / 30.0); }
        assert!((a - b).abs() < 1e-4 && HANG_Z - a < 0.01, "{a} {b}");
    }

    /// The leaning ramp: 1 voxel per row from the line up to RAMP_BACK (HANG_Z -> VOXEL_Z), fewer rows reach it sooner.
    #[test]
    fn ramp_goes_from_the_front_face_to_the_footprint() {
        let f = ramp_back(14);
        assert_eq!((f(1), f(6), f(12), f(14)), (1, 6, RAMP_BACK, RAMP_BACK));
        assert!((HANG_Z - RAMP_BACK as f32 / 32.0 - VOXEL_Z).abs() < 0.02);
        let g = ramp_back(2);
        assert_eq!((g(1), g(2)), (RAMP_BACK / 2, RAMP_BACK));
        let mut z = HANG_Z;
        assert_eq!(settle_z(&mut z, 145, 1, 1.0 / 60.0), VOXEL_Z, "the climb sets its depth at once");
        assert_eq!(settle_z(&mut z, 15, 1, 1.0 / 60.0), VOXEL_Z);
    }

    /// The real sprite meshes of the ledge poses (hang 78..=99, climb 135..=149, both directions: the climb down plays the
    /// same frames), in room coordinates: no voxel of the figure below the slab top lies inside the slab / wall box (its
    /// back behind the front face z 0.5), above the top the ramp has brought every row past the plateau into the footprint.
    #[test]
    fn real_ledge_poses_stay_out_of_the_slab() {
        use crate::room::{SLAB_BACK, SLAB_FRONT};
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        if Exe::open(format!("{dir}/PRINCE.EXE")).is_err() {
            return;
        }
        let a = KidAssets::load(&dir);
        let px = 1.0 / 32.0;
        for frame in (78u8..=99).chain(135..=149) {
            let fr = a.data.frames[frame as usize];
            let Some((im, pal)) = decoded(&a, 255, fr.image, false, 0) else { continue };
            let (h, dy) = (im.height as i32, fr.dy as i32);
            let (line, hang) = (ledge_rows(frame, h, dy), hang_of(frame, h, dy));
            let vz = voxel_z(frame, if frame < 100 { 2 } else { 1 });
            for mirrored in [false, true] {
                let m = posed_model(&im, &pal, mirrored, hang);
                for y in 0..m.h {
                    for z in 0..m.d {
                        for x in 0..m.w {
                            if m.get(x as isize, y as isize, z as isize) == 0 {
                                continue;
                            }
                            let (back, front) = (vz + (z as f32 - 0.5 - m.plane as f32) * px, vz + (z as f32 + 0.5 - m.plane as f32) * px);
                            let what = format!("frame {frame} row {y} (line {line}) x {x}: z {back:.3}..{front:.3}");
                            if y >= line {
                                assert!(back >= SLAB_FRONT - 1e-4, "inside the slab, {what}");
                            } else if hang.1 {
                                assert!(back >= SLAB_BACK, "behind the slab, {what}");
                                if line - y >= RAMP_BACK.min(line) {
                                    assert!(front <= SLAB_FRONT + 1e-4, "above the top but in front of the face, {what}");
                                }
                            }
                        }
                    }
                }
                if line >= h as usize && frame >= 145 {
                    assert!(vz + (m.d as f32 - 0.5 - m.plane as f32) * px <= SLAB_FRONT, "frame {frame} stands on the slab");
                }
            }
        }
    }

    /// Hang frame 91 with the real image: the topmost pixel row is the hands. In room coordinates (sx + shift - 16 px, the tile
    /// shift) the ledge end is world x 4 (drawn px 128) for both mirrorings, and px 0 for the level-1 room 2 ledge (kid x 62):
    /// the hands lie 0..2 px inside it in every case (facing left: ledge col 3, kid x 118; facing right: ledge col 4, kid x
    /// 123; frames[91]: image 103, 12 px, dx 2). The climb slides back to 0 at the standing frames.
    #[test]
    fn real_hang_hands_grip_the_slab_end_in_both_mirrorings() {
        let dir = format!("{}/../../Original_data", env!("CARGO_MANIFEST_DIR"));
        if Exe::open(format!("{dir}/PRINCE.EXE")).is_err() {
            return;
        }
        let a = KidAssets::load(&dir);
        let fr = a.data.frames[91];
        let Some((im, _)) = decoded(&a, 255, fr.image, false, 0) else { return };
        let top = im.pixels.iter().position(|&p| p != 0).unwrap() / im.width;
        let cols: Vec<i32> = (0..im.width).filter(|&x| im.pixels[top * im.width + x] != 0).map(|x| x as i32).collect();
        let (lo, hi) = (cols[0], *cols.last().unwrap());
        for (x, end) in [(118, 128), (62, 0)] {
            let left = screen_x(x, fr.dx, fr.flags, true) + hang_dx(0, 91, true) as i32 - 16;
            assert_eq!((end - (left + hi), end - (left + lo)), (0, 2), "facing left, kid x {x}");
        }
        let right = screen_x(123, fr.dx, fr.flags, false) + hang_dx(0, 91, false) as i32 - 16;
        assert_eq!((right - 1 - hi - 128, right - 1 - lo - 128), (0, 2), "facing right");
        for l in [true, false] {
            assert!((87..=99).chain(135..=140).all(|f| hang_dx(0, f, l) != 0.0));
            assert!((141..=144).all(|f| hang_dx(0, f, l).abs() < hang_dx(0, 140, l).abs()));
            assert!(hang_dx(0, 145, l) == 0.0 && hang_dx(0, 15, l) == 0.0 && hang_dx(2, 91, l) == 0.0);
        }
    }

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
    fn pad_layouts_map_the_buttons() {
        use GamepadButton::*;
        let k = |l, b: &[GamepadButton], s: Vec2| { let k = pad_keys(l, |x| b.contains(&x), s); (k.left, k.right, k.up, k.down, k.shift) };
        let (m, c, cl) = (PadLayout::Modern, PadLayout::Combat, PadLayout::Classic);
        // modern: A and Y jump / parry, B crouches, no down on stick or D-pad, X / RB / RT = Shift
        assert_eq!(k(m, &[South], Vec2::ZERO), (false, false, true, false, false));
        assert_eq!(k(m, &[North], Vec2::ZERO), (false, false, true, false, false));
        assert_eq!(k(m, &[East], Vec2::ZERO), (false, false, false, true, false));
        assert_eq!(k(m, &[DPadDown], Vec2::new(0.0, -1.0)), (false, false, false, false, false));
        assert_eq!(k(m, &[DPadUp, DPadLeft], Vec2::ZERO), (true, false, true, false, false));
        assert_eq!(k(m, &[West], Vec2::ZERO).4 && k(m, &[RightTrigger], Vec2::ZERO).4 && k(m, &[RightTrigger2], Vec2::ZERO).4, true);
        assert_eq!(k(m, &[LeftTrigger], Vec2::ZERO), (false, false, false, false, false));
        // combat: down also on stick / D-pad, parry on LB / LT, Y unused
        assert_eq!(k(c, &[], Vec2::new(0.0, -1.0)), (false, false, false, true, false));
        assert_eq!(k(c, &[LeftTrigger2], Vec2::ZERO), (false, false, true, false, false));
        assert_eq!(k(c, &[North], Vec2::ZERO), (false, false, false, false, false));
        // classic: the first layout
        assert_eq!(k(cl, &[South], Vec2::ZERO), (false, false, false, false, true));
        assert_eq!(k(cl, &[East], Vec2::new(0.8, -0.8)), (false, true, false, true, false));
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
