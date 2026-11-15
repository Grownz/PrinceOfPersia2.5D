// release builds have no console window; errors go to pop.log next to the exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Prince of Persia remake: title -> game (procedural 3D rooms, original sprites) -> title.
//! Keys: arrows + Shift = kid, Esc/P = pause, Ctrl+A = restart, Ctrl+G = save, Ctrl+L = load (title/demo), M = mute,
//! - / = = volume, F11 = borderless fullscreen, F3 = debug text, controller supported (docs/controls.md)
//! (with it on: Ctrl+arrows = neighbouring room, Ctrl+PageUp/PageDown = level, sequence viewer keys).
//! POP_LEVEL / POP_ROOM skip the title; POP_SOUND=original uses the game's own sounds; POP_DATA = game data folder;
//! POP_PERF=1 logs the average frame time (vsync off) and room rebuilds every 300 frames.
mod audio;
mod flow;
mod kid;
mod room;
mod settings;
mod synth;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use pop_core::{dat::Dat, level::*, sim};
use settings::Settings;
use std::path::PathBuf;

#[derive(Resource)]
struct Game {
    world: sim::World,
    /// index = level number (0 = demo level)
    levels: Vec<Level>,
    level: usize,
    /// 1-based
    room: usize,
    /// kid position (world units) and room at the start of the last logic tick, for interpolation
    kid_prev: (Vec2, u8),
    guard_prev: (Vec2, u8),
    kid_respawn: bool,
    /// HUD message ("LEVEL N", "GAME SAVED"): (logic ticks left, text)
    banner: (u32, String),
    /// debug text and debug keys (F3)
    debug: bool,
    /// bumped on every World rebuild (audio resets on it)
    gen: u32,
}

static DATA: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The game data folder found at startup (`find_data`).
pub fn data_dir() -> String {
    DATA.get().cloned().unwrap_or_else(|| "Original_data".into())
}

/// Files that must be present for a folder to count as the game data.
/// (TITLE.DAT and the sound DATs are optional: the app falls back to a text title / synthesized sound.)
const REQUIRED: [&str; 11] = [
    "LEVELS.DAT", "PRINCE.EXE", "KID.DAT", "PRINCE.DAT", "GUARD.DAT", "GUARD1.DAT", "GUARD2.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT",
];

fn missing_files(d: &std::path::Path) -> Vec<&'static str> {
    REQUIRED.iter().copied().filter(|f| !d.join(f).is_file()).collect()
}

/// Where to look for the user's own game files, in order: POP_DATA, Original_data next to the exe, Original_data in the
/// current folder, settings.json "data_path".
fn data_candidates(set: &Settings) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::env::var("POP_DATA").ok().map(PathBuf::from).into_iter().collect();
    v.push(flow::exe_dir().join("Original_data"));
    v.push("Original_data".into());
    v.extend(set.data_path.as_ref().map(PathBuf::from));
    v
}

fn find_data(candidates: &[PathBuf]) -> Option<&PathBuf> {
    candidates.iter().find(|d| missing_files(d).is_empty())
}

/// stderr plus, in release builds (no console), pop.log next to the exe.
fn report(msg: &str) {
    eprintln!("{msg}");
    if !cfg!(debug_assertions) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(flow::exe_dir().join("pop.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

/// Release builds: Bevy's log (warnings, errors, info) also goes to pop.log, truncated at each start.
fn log_file_layer(_: &mut App) -> Option<bevy::log::BoxedLayer> {
    if cfg!(debug_assertions) {
        return None;
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(flow::exe_dir().join("pop.log")).ok()?;
    Some(Box::new(bevy::log::tracing_subscriber::fmt::layer().with_writer(std::sync::Mutex::new(file)).with_ansi(false)))
}

#[derive(Component)]
struct DebugText;

fn main() {
    if !cfg!(debug_assertions) {
        let _ = std::fs::write(flow::exe_dir().join("pop.log"), "");
        std::panic::set_hook(Box::new(|info| report(&format!("panic: {info}"))));
    }
    let set = Settings::load();
    let candidates = data_candidates(&set);
    let Some(dir) = find_data(&candidates) else {
        let tried: Vec<String> = candidates
            .iter()
            .map(|p| match (p.is_dir(), missing_files(p)) {
                (true, m) => format!("  {} (missing files: {})", p.display(), m.join(", ")),
                _ => format!("  {} (not a folder)", p.display()),
            })
            .collect();
        let msg = format!(
            "Game data not found. This remake needs your own copy of Prince of Persia (DOS): a folder with {}.\nPut it in a folder named Original_data next to the exe, set POP_DATA, or set \"data_path\" in {}.\nSearched:\n{}",
            REQUIRED.join(", "), Settings::path().display(), tried.join("\n")
        );
        report(&msg);
        return missing_data(msg);
    };
    let dir = DATA.get_or_init(|| dir.to_string_lossy().into_owned()).clone();
    let dat = Dat::open(format!("{dir}/LEVELS.DAT")).expect("cannot open LEVELS.DAT (set POP_DATA)");
    let levels: Vec<Level> = (2000..=2014).map(|id| Level::parse(dat.get(id).expect("level")).expect("bad level")).collect();
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
    let level = env("POP_LEVEL").filter(|&l| l < levels.len()).unwrap_or(1);
    let assets = kid::KidAssets::load(&dir);
    let world = sim::World::new(assets.data.clone(), levels[level].clone(), level);
    let room = env("POP_ROOM").filter(|r| (1..=ROOMS).contains(r)).unwrap_or(world.kid.room as usize);
    let skip_title = env("POP_LEVEL").is_some() || env("POP_ROOM").is_some();
    let playing = || in_state(flow::Screen::Playing);

    let perf = std::env::var("POP_PERF").is_ok();
    let window = Window {
        title: "Prince of Persia x64".into(),
        resolution: (set.width.max(320), set.height.max(200)).into(),
        mode: if set.borderless() { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed },
        // POP_PERF measures the real frame cost, so it turns vsync off
        present_mode: if perf { PresentMode::AutoNoVsync } else { PresentMode::AutoVsync },
        resizable: true,
        ..default()
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(window), ..default() }).set(bevy::log::LogPlugin { custom_layer: log_file_layer, ..default() }))
        .insert_resource(set.key_map())
        .insert_resource(set)
        .add_plugins(audio::plugin)
        .insert_resource(ClearColor(BACKGROUND))
        .insert_resource(Time::<Fixed>::from_hz(sim::STEP_HZ))
        .insert_resource(Game { world, levels, level, room, kid_prev: (Vec2::ZERO, 0), guard_prev: (Vec2::ZERO, 0), kid_respawn: false, banner: (0, String::new()), debug: false, gen: 0 })
        .insert_resource(kid::Latch::default())
        .insert_resource(assets)
        .init_resource::<flow::Flow>()
        .insert_state(if skip_title { flow::Screen::Playing } else { flow::Screen::Title })
        .add_systems(Startup, (setup, kid::setup, flow::setup)) // first room is built by room::rebuild_on_change
        .add_systems(OnEnter(flow::Screen::Title), (flow::reset_idle, audio::enter_title))
        .add_systems(OnEnter(flow::Screen::Card), audio::enter_card)
        .add_systems(OnEnter(flow::Screen::Paused), audio::pause)
        .add_systems(OnExit(flow::Screen::Paused), audio::resume)
        .add_systems(
            FixedUpdate,
            (audio::before_step, |mut g: ResMut<Game>, a: Res<kid::KidAssets>, mut latch: ResMut<kid::Latch>, keys: Res<ButtonInput<KeyCode>>, map: Res<settings::KeyMap>, pads: Query<&Gamepad>| {
                kid::fixed_step(&mut g, &a, &mut latch, kid::pressed(&keys, &map, &pads));
            }, audio::fixed)
                .chain()
                .run_if(playing()),
        )
        .add_systems(
            Update,
            (
                kid::latch_keys,
                flow::pause,
                audio::keys,
                flow::title.run_if(in_state(flow::Screen::Title)),
                flow::card_step.run_if(in_state(flow::Screen::Card)),
                (navigate, kid::keys, flow::playing).chain().run_if(playing()),
                room::rebuild_on_change,
                kid::render,
                flash,
                flow::overlay,
                debug_text,
                fullscreen,
                perf_log.run_if(move || perf),
                screenshot_and_quit,
            )
                .chain(),
        )
        .run();
}

/// No game data: a window with the instructions instead of a panic.
fn missing_data(msg: String) {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "Prince of Persia x64 - game data missing".into(), ..default() }), ..default() }).set(bevy::log::LogPlugin { custom_layer: log_file_layer, ..default() }))
        .add_systems(Startup, move |mut cmds: Commands| {
            cmds.spawn(Camera2d);
            cmds.spawn((Text::new(msg.clone()), TextFont::from_font_size(18.0), Node { margin: UiRect::all(Val::Px(24.0)), ..default() }));
        })
        .add_systems(Update, screenshot_and_quit)
        .run();
}

/// F11: windowed <-> borderless fullscreen, remembered in settings.json.
fn fullscreen(k: Res<ButtonInput<KeyCode>>, mut win: Single<&mut Window, With<PrimaryWindow>>, mut set: ResMut<Settings>) {
    if !k.just_pressed(KeyCode::F11) {
        return;
    }
    let to_full = !matches!(win.mode, WindowMode::BorderlessFullscreen(_));
    win.mode = if to_full { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
    set.window = if to_full { "borderless" } else { "windowed" }.into();
    set.save();
}

/// POP_PERF=1: average frame time and room rebuilds per 300 frames.
fn perf_log(time: Res<Time<Real>>, mut acc: Local<(u32, f32, u32)>) {
    acc.0 += 1;
    acc.1 += time.delta_secs();
    if acc.0 == 300 {
        let rebuilds = room::REBUILDS.load(std::sync::atomic::Ordering::Relaxed);
        info!("perf: {:.3} ms/frame avg over 300 frames, {} room rebuilds", acc.1 / 300.0 * 1000.0, rebuilds - acc.2);
        *acc = (0, 0.0, rebuilds);
    }
}

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    cmds.insert_resource(room::Palette::new(&mut meshes, &mut mats));
    // The room is 10 wide and 3 * TILE_H tall; look at its centre with a slight downward tilt for depth.
    let centre = Vec3::new(5.0, 3.0, 0.0);
    cmds.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 4.2, 13.5).looking_at(centre, Vec3::Y),
        AmbientLight { color: Color::srgb(0.6, 0.65, 0.9), brightness: 150.0, ..default() },
    ));
    cmds.spawn((DirectionalLight { illuminance: 4000.0, ..default() }, Transform::from_xyz(-4.0, 9.0, 10.0).looking_at(centre, Vec3::Y)));
    cmds.spawn((
        Text::new(""),
        Node { position_type: PositionType::Absolute, left: Val::Px(8.0), top: Val::Px(8.0), ..default() },
        Visibility::Hidden,
        DebugText,
    ));
    // full-window flash tint, below the HUD and texts
    cmds.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(-1), BackgroundColor(Color::NONE), FlashOverlay));
}

#[derive(Component)]
struct FlashOverlay;

fn navigate(keys: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, mut text: Query<&mut Visibility, With<DebugText>>) {
    if keys.just_pressed(KeyCode::F3) {
        g.debug = !g.debug;
        for mut v in &mut text {
            *v = if g.debug { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
    if !g.debug {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    for (key, dir) in [(KeyCode::ArrowLeft, 0), (KeyCode::ArrowRight, 1), (KeyCode::ArrowUp, 2), (KeyCode::ArrowDown, 3)] {
        if ctrl && keys.just_pressed(key) {
            let to = g.levels[g.level].links[g.room - 1][dir] as usize;
            if (1..=ROOMS).contains(&to) {
                g.room = to;
            }
        }
    }
    // Ctrl+PageUp/PageDown: plain PageUp is the kid's up+right (it shares keypad 9's scancode), Ctrl mutes the kid's keys
    let step = ctrl as i32 * (keys.just_pressed(KeyCode::PageUp) as i32 - keys.just_pressed(KeyCode::PageDown) as i32);
    if step != 0 {
        g.level = (g.level as i32 + step).rem_euclid(g.levels.len() as i32) as usize;
        g.kid_respawn = true;
    }
}

const BACKGROUND: Color = Color::srgb(0.02, 0.02, 0.04);
/// EGA colours (ds:2BBA, 6-bit RGB, read by 0C83:198F); flash() sets VGA palette entry 0 (the black background) to one of them.
/// 2, 4, 12, 14 and 15 are the ones used (docs/edge_cases.md 3.6). VERIFIED against the exe (test below).
const EGA: [[u8; 3]; 16] = [
    [0, 0, 0], [0, 0, 42], [0, 42, 0], [0, 42, 42], [42, 0, 0], [42, 0, 42], [42, 21, 0], [42, 42, 42],
    [21, 21, 21], [21, 21, 63], [21, 63, 21], [21, 63, 63], [63, 21, 21], [63, 21, 63], [63, 63, 21], [63, 63, 63],
];

/// Screen flash (flash_if_hurt / remove_flash_if_hurt) for `flash.1` logic frames. The original recolours palette entry 0 (the
/// black that shows through open areas); the opaque 3D room hides most of that, so the clear colour changes and a translucent
/// overlay of the same colour (alpha FLASH_ALPHA) covers the whole view.
fn flash(g: Res<Game>, mut clear: ResMut<ClearColor>, mut overlay: Query<&mut BackgroundColor, With<FlashOverlay>>) {
    let (c, frames) = g.world.flash;
    let (want, tint) = if frames > 0 {
        let [r, gr, b] = EGA[c as usize & 15].map(|v| (v as u32 * 255 / 63) as u8);
        (Color::srgb_u8(r, gr, b), Color::srgba_u8(r, gr, b, (FLASH_ALPHA * 255.0) as u8))
    } else {
        (BACKGROUND, Color::NONE)
    };
    if clear.0 != want {
        clear.0 = want;
    }
    for mut bg in &mut overlay {
        if bg.0 != tint {
            bg.0 = tint;
        }
    }
}

const FLASH_ALPHA: f32 = 0.4;

fn debug_text(g: Res<Game>, mut q: Query<&mut Text, With<DebugText>>) {
    if !g.debug {
        return;
    }
    let l = &g.world.level;
    let [left, right, up, down] = l.links[g.room - 1];
    for mut t in &mut q {
        t.0 = format!(
            "level {}  room {}  (L{} R{} U{} D{})  start room {} tile {}\ntick {}  kid: seq {} frame {} action {} x {} y {} room {} row {} fall {} hp {}/{} sword {}{}\narrows+Shift kid  Ctrl+arrows room  Ctrl+PgUp/PgDn level  , . sequence viewer  Space restart  F flip  C control  F3 text",
            g.level, g.room, left, right, up, down, l.start_room, l.start_tile, g.world.tick, g.world.kid.seq, g.world.kid.frame, g.world.kid.action, g.world.kid.x, g.world.kid.y, g.world.kid.room, g.world.kid.row, g.world.kid.fall_y, g.world.kid.hp, g.world.kid.hp_max, g.world.kid.sword,
            g.world.guard.as_ref().map_or(String::new(), |gd| format!("   guard: seq {} frame {} x {} hp {}/{} sword {} skill {} see {}", gd.seq, gd.frame, gd.x, gd.hp, gd.hp_max, gd.sword, gd.skill, g.world.shared.can_see))
        );
        let w = &g.world;
        t.0 += &format!("
time {}:{:03}  death {}  flash {:?}  text timer {}", w.progress.rem_min, w.progress.rem_tick, w.death_count, w.flash, w.text_timer);
    }
}

/// Test hook (shaders compile asynchronously, so wait a few hundred frames): with `POP_SHOT=file.png` (and optionally `POP_ROOM`, `POP_LEVEL`, `POP_SHOT_FRAME`) save a screenshot after a few frames and quit.
fn screenshot_and_quit(mut cmds: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    // shortcut: fixed frame count, depends on how fast shaders compile; upgrade to waiting for pipelines if flaky
    let Ok(path) = std::env::var("POP_SHOT") else { return };
    *frame += 1;
    let at = std::env::var("POP_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(400);
    if *frame == at {
        cmds.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    } else if *frame == at + 60 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    /// The first candidate holding the required files wins; incomplete folders are skipped.
    #[test]
    fn find_data_picks_first_complete_folder() {
        let base = std::env::temp_dir().join(format!("pop_find_data_{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("LEVELS.DAT"), "").unwrap(); // incomplete
        for f in super::REQUIRED {
            std::fs::write(b.join(f), "").unwrap();
        }
        let c = [base.join("missing"), a.clone(), b.clone()];
        assert_eq!(super::find_data(&c), Some(&b));
        assert_eq!(super::find_data(&c[..2]), None);
        // partial install: only the files the app opens at startup are required, and the missing ones are named
        let m = super::missing_files(&a);
        assert!(m.contains(&"KID.DAT") && m.contains(&"GUARD2.DAT") && !m.contains(&"LEVELS.DAT"));
        assert!(!super::REQUIRED.contains(&"TITLE.DAT") && super::missing_files(&b).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// ds:2BBA = file offset 0x1E47A (ds + 112832): 16 RGB triples, equal to the table above.
    #[test]
    fn ega_table_matches_ds_2bba() {
        let Ok(exe) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PRINCE.EXE")) else { return };
        let t: Vec<[u8; 3]> = exe[0x2BBA + 112832..][..48].chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        assert_eq!(t, super::EGA);
    }
}
