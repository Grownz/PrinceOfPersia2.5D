// release builds have no console window; errors go to pop.log next to the exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Prince of Persia remake: title -> game (procedural 3D rooms, original sprites) -> title.
//! Keys: arrows + Shift = kid, Esc = pause menu / title menu, P = pause, Ctrl+A = restart, Ctrl+G = save, Ctrl+L = load (title/demo), M = mute,
//! - / = = volume, F11 = borderless fullscreen, F3 = debug text, controller supported (docs/controls.md)
//! (with it on: Ctrl+arrows = neighbouring room, Ctrl+PageUp/PageDown = level, sequence viewer keys).
//! POP_LEVEL / POP_ROOM skip the title; POP_CUTSCENE=<level>|end|timeout|intro plays that cutscene at once (then quits
//! back into the normal flow); POP_SOUND=original uses the game's own sounds; POP_DATA = game data folder;
//! POP_PERF=1 logs the average frame time (vsync off) and room rebuilds every 300 frames. F9 / POP_FPS=1 frame-time overlay,
//! POP_BENCH=<frames> benchmark run (POP_RES, POP_FX: perf.rs, docs/enhanced_perf.md).
//! POP_RENDER / POP_CHARS / POP_TILES / POP_COLUMNS / POP_OBJECTS override settings.json "render_mode" / "character_style" / "tile_textures" / "column_style" / "object_style"; F5 toggles classic / enhanced rendering
//! (plus rt_experimental in a `--features solari` build started in that mode, docs/r3_solari.md),
//! F6 sprite / voxel characters,
//! F7 (debug text on, or POP_DEBUG_CAM=1) a closer, turned camera on the kid (POP_CAM_ANGLE="degrees[,distance]").
//! POP_CAMERA=smooth|flip overrides settings.json "camera_scroll" (camera.rs). POP_SET="start=ROOM:TILE,door=N,upside=1"
//! (with POP_LEVEL; "gate=N" sets every gate's opening, "exit=1" opens every exit door so "up" in front of it climbs the stairs) changes the first world for screenshots (`debug_set`). POP_MENU = main, settings (video, audio, graphics, downloads), controls, hof, name
//! or attract opens a menu page or a hall-of-fame screen at once (`flow::debug_menu`).
mod audio;
mod camera;
mod controls_art;
mod cutscene;
mod enhanced;
mod flow;
mod getdata;
mod hof;
mod items;
mod kid;
mod menu;
mod menu_ui;
mod mixer;
mod music;
mod particles;
mod perf;
mod room;
mod settings;
mod soundfont;
#[cfg(feature = "solari")]
mod solari;
mod synth;
mod tiles;
mod update;
mod voxel;

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
    /// kid / guard at the start of the last logic tick, for interpolation (kid.rs `lerps`)
    kid_prev: kid::Snap,
    guard_prev: kid::Snap,
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
    if set.external_terminal && !cfg!(debug_assertions) {
        flow::console(true);
    }
    // before any game data is touched: is there a newer release?
    if let Some(offer) = update::check(set.update_check) {
        return update::window(offer);
    }
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
    let world = debug_set(&assets, &levels[level], level);
    let room = env("POP_ROOM").filter(|r| (1..=ROOMS).contains(r)).unwrap_or(world.kid.room as usize);
    let skip_title = env("POP_LEVEL").is_some() || env("POP_ROOM").is_some();
    let playing = || in_state(flow::Screen::Playing);

    let perf = std::env::var("POP_PERF").is_ok();
    let bench = perf::bench_frames().is_some();
    let (render_mode, char_style) = set.render();
    // POP_RES=WxH (windowed) or "full" (borderless fullscreen at the monitor's size), for benchmarks
    let res = std::env::var("POP_RES").ok();
    let window = Window {
        title: "Prince of Persia x64".into(),
        resolution: res.as_deref().and_then(perf::parse_res).unwrap_or((set.width.max(320), set.height.max(200))).into(),
        mode: if res.as_deref() == Some("full") || (res.is_none() && set.borderless()) { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed },
        // POP_PERF / POP_BENCH measure the real frame cost, so they turn vsync off (Immediate where the surface has it)
        present_mode: if perf || bench { PresentMode::AutoNoVsync } else { PresentMode::AutoVsync },
        resizable: true,
        ..default()
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(window), ..default() }).set(bevy::log::LogPlugin { custom_layer: log_file_layer, ..default() }));
    // per-pass GPU times (wgpu timestamp queries; the device already has TIMESTAMP_QUERY when the adapter offers it)
    if perf::gpu_timing() {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    // R3 trial: Solari only when starting in rt_experimental (it moves every material to the deferred path)
    #[cfg(feature = "solari")]
    if render_mode == settings::RenderMode::Rt {
        app.add_plugins(solari::plugin);
    }
    app
        .insert_resource(set.key_map())
        .insert_resource(render_mode)
        .insert_resource(char_style)
        .insert_resource(set)
        .add_plugins((audio::plugin, enhanced::light_shadow_views_plugin))
        .insert_resource(ClearColor(BACKGROUND))
        .insert_resource(Time::<Fixed>::from_hz(sim::STEP_HZ))
        .insert_resource(Game { world, levels, level, room, kid_prev: kid::Snap::default(), guard_prev: kid::Snap::default(), kid_respawn: false, banner: (0, String::new()), debug: false, gen: 0 })
        .insert_resource(kid::Latch::default())
        .init_resource::<particles::Clash>()
        .init_resource::<particles::Swords>()
        .insert_resource(assets)
        .init_resource::<flow::Flow>()
        .init_resource::<menu_ui::MenuUi>()
        .insert_resource(hof::Hof::load())
        .init_resource::<camera::View>()
        .insert_state(if skip_title { flow::Screen::Playing } else { flow::Screen::Title })
        .insert_resource(cutscene::Cut::load(&dir))
        .add_systems(Startup, ((setup, cutscene::setup, debug_cutscene).chain(), kid::setup, (flow::setup, menu_ui::setup, flow::debug_menu).chain(), probe_gpu)) // first room is built by room::rebuild_on_change
        .add_systems(OnEnter(flow::Screen::Title), audio::enter_title)
        .add_systems(OnEnter(flow::Screen::Cutscene), audio::enter_cutscene)
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
        .add_systems(FixedUpdate, (audio::drain, cutscene::fixed.run_if(in_state(flow::Screen::Cutscene))).chain().run_if(|s: Res<State<flow::Screen>>| matches!(s.get(), flow::Screen::Title | flow::Screen::Cutscene)))
        .add_systems(
            Update,
            (
                kid::latch_keys.run_if(not(in_state(flow::Screen::Paused))), // menu keys must not reach the kid after resume
                (menu_ui::input, menu_ui::mouse, flow::menu_action).chain(),
                audio::keys.run_if(|f: Res<flow::Flow>| !f.typing()),
                (flow::title, flow::name_entry).chain().run_if(in_state(flow::Screen::Title)),
                (cutscene::latch, flow::cutscene_end).run_if(in_state(flow::Screen::Cutscene)),
                (navigate, kid::keys, flow::playing).chain().run_if(playing()),
                room::rebuild_on_change,
                room::animate,
                kid::render,
                flash,
                (camera::follow, cutscene::render, debug_camera, camera::mirror).chain(),
                (flow::overlay, menu_ui::draw, controls_art::draw, controls_art::load),
                debug_text,
                fullscreen,
                char_style_key,
                (kid::live_look, cutscene::live_look, (room::palette_live, cutscene::chamber_live).chain()), // settings that apply live
                (enhanced::mode_key, enhanced::watchdog, enhanced::apply, enhanced::sun_brightness, room::flicker, enhanced::torch_shadows, enhanced::bubble_lights, particles::clash_sparks, particles::sparks).chain(),
                perf_log.run_if(move || perf),
                (perf::overlay, perf::bench, perf::casters),
                screenshot_and_quit,
            )
                .chain(),
        )
        .run();
}

/// No game data: a window with the instructions, and the offer to download the files from the Internet Archive (`getdata`).
fn missing_data(msg: String) {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "Prince of Persia x64 - game data missing".into(), ..default() }), ..default() }).set(bevy::log::LogPlugin { custom_layer: log_file_layer, ..default() }))
        .insert_resource(DataMsg(msg))
        .add_systems(Startup, |mut cmds: Commands| {
            cmds.spawn(Camera2d);
            cmds.spawn((Text::new(""), TextFont::from_font_size(18.0), Node { margin: UiRect::all(Val::Px(24.0)), ..default() }));
        })
        .add_systems(Update, (data_ui, screenshot_and_quit))
        .run();
}

#[derive(Resource)]
struct DataMsg(String);

/// The missing-data window: D asks, Enter confirms the licence sentence and downloads, Esc goes back / cancels / quits.
/// When the files are in place the app starts itself again.
fn data_ui(k: Res<ButtonInput<KeyCode>>, msg: Res<DataMsg>, mut text: Single<&mut Text>, mut exit: MessageWriter<AppExit>) {
    use getdata::Status;
    let st = getdata::status();
    if k.just_pressed(KeyCode::Escape) {
        match st {
            Status::Confirm => getdata::back(),
            Status::Busy { .. } => getdata::cancel(),
            _ => {
                exit.write(AppExit::Success);
            }
        }
    }
    if k.just_pressed(KeyCode::KeyD) {
        getdata::ask();
    }
    if k.just_pressed(KeyCode::Enter) {
        getdata::start(flow::exe_dir().join("Original_data"));
    }
    if st == Status::Done {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        exit.write(AppExit::Success);
    }
    text.0 = match st {
        Status::Idle | Status::Done => format!("{}

Press D to download the game files from the Internet Archive ({}), Esc to quit.", msg.0, getdata::PAGE),
        Status::Confirm => format!("{}

Enter = confirm and download, Esc = back.", getdata::LICENCE),
        Status::Busy { phase, done } => format!("{phase}... {} KB

Esc = cancel.", done / 1000),
        Status::Failed(e) => format!("Download failed: {e}

{}

Press D to try again, Esc to quit.", msg.0),
    };
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

/// F6: sprite <-> voxel characters, remembered in settings.json.
fn char_style_key(k: Res<ButtonInput<KeyCode>>, mut style: ResMut<settings::CharacterStyle>, mut set: ResMut<Settings>) {
    use settings::CharacterStyle::*;
    if !k.just_pressed(KeyCode::F6) {
        return;
    }
    *style = if *style == Sprite { Voxel } else { Sprite };
    set.character_style = if *style == Voxel { "voxel" } else { "sprite" }.into();
    set.save();
    info!("character style: {}", set.character_style);
}

/// R1 capability probe: log the adapter and whether it could do ray queries. Nothing is requested from the device here
/// (asking for an unsupported feature would fail startup); the classic renderer runs whatever the answer.
fn probe_gpu(info: Option<Res<bevy::render::renderer::RenderAdapterInfo>>, adapter: Option<Res<bevy::render::renderer::RenderAdapter>>, mode: Res<settings::RenderMode>) {
    let Some(info) = info else {
        warn!("no render adapter info; renderer {:?}", *mode);
        return;
    };
    let rq = adapter.is_some_and(|a| a.features().contains(bevy::render::settings::WgpuFeatures::EXPERIMENTAL_RAY_QUERY));
    info!("GPU: {} ({:?}, {:?}, driver {} {}), ray queries: {}; renderer {:?}", info.name, info.backend, info.device_type, info.driver, info.driver_info, if rq { "exposed by the adapter (not enabled)" } else { "no" }, *mode);
}

/// F7 with the debug text on (or POP_DEBUG_CAM=1): a closer camera, turned 35 degrees around the kid, to inspect voxel depth.
fn debug_camera(k: Res<ButtonInput<KeyCode>>, g: Res<Game>, state: Res<State<flow::Screen>>, mut on: Local<Option<bool>>, kid: Query<&Transform, (With<kid::KidSprite>, Without<Camera3d>)>, mut cam: Query<&mut Transform, With<Camera3d>>) {
    let on = on.get_or_insert_with(|| std::env::var("POP_DEBUG_CAM").is_ok());
    *on ^= g.debug && k.just_pressed(KeyCode::F7);
    let (true, Ok(kt), flow::Screen::Playing | flow::Screen::Paused) = (*on, kid.single(), state.get()) else { return };
    let target = kt.translation.with_z(0.6) + Vec3::Y * 0.6;
    // POP_CAM_ANGLE="degrees[,distance]" (default 34 degrees, 4 units): preset views for screenshots
    let (deg, dist) = std::env::var("POP_CAM_ANGLE").ok().map_or((34.4, 4.0), |v| {
        let mut it = v.split(',').map(|x| x.trim().parse::<f32>().ok());
        (it.next().flatten().unwrap_or(34.4), it.next().flatten().unwrap_or(4.0))
    });
    for mut t in &mut cam {
        *t = Transform::from_translation(target + Quat::from_rotation_y(deg.to_radians()) * Vec3::new(0.0, dist / 4.0, dist)).looking_at(target, Vec3::Y);
    }
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

fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut imgs: ResMut<Assets<Image>>, set: Res<Settings>) {
    let dir = set.tile_textures().then(data_dir);
    cmds.insert_resource(room::Palette::new(&mut meshes, &mut mats, &mut imgs, dir.as_deref(), set.voxel_columns(), set.voxel_objects()));
    let centre = Vec3::new(5.0, 3.0, 0.0);
    cmds.spawn((
        Camera3d::default(),
        game_camera(16.0 / 9.0),
        enhanced::classic_ambient(),
    ));
    cmds.spawn((DirectionalLight { illuminance: enhanced::CLASSIC_SUN, ..default() }, Transform::from_xyz(-4.0, 9.0, 10.0).looking_at(centre, Vec3::Y)));
    cmds.spawn((
        Text::new(""),
        Node { position_type: PositionType::Absolute, left: Val::Px(8.0), top: Val::Px(8.0), ..default() },
        Visibility::Hidden,
        DebugText,
    ));
    // full-window flash tint, below the HUD and texts
    cmds.spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(-1), BackgroundColor(Color::NONE), FlashOverlay));
}

/// The level view: the room is 10 wide and 3 * TILE_H tall; look at its centre with a slight downward tilt for depth.
/// The room (10 columns x 3 rows of TILE_H) fills about 80% of the window along its limiting axis, so the neighbouring rooms
/// show in the margins. Default perspective fov is 45 degrees; the slight view from above (1.2 over 13.5) is kept.
pub fn game_camera(aspect: f32) -> Transform {
    const FILL: f32 = 0.8;
    let visible_h = (3.0 * room::TILE_H / FILL).max(10.0 / (FILL * aspect.max(0.1)));
    let d = visible_h / (2.0 * (std::f32::consts::FRAC_PI_8).tan());
    Transform::from_xyz(5.0, 3.0 + 1.2 * d / 13.5, d).looking_at(Vec3::new(5.0, 3.0, 0.0), Vec3::Y)
}

/// POP_CUTSCENE=<level>|end|timeout|intro (debugging): start that scene right away (POP_REM_MIN sets the minutes left for the
/// hourglass and the level-12 branch, default 60). After it the normal flow goes on.
fn debug_cutscene(mut cut: ResMut<cutscene::Cut>, a: Res<kid::KidAssets>, mut next: ResMut<NextState<flow::Screen>>) {
    use pop_core::cutscene::Scene;
    let Ok(v) = std::env::var("POP_CUTSCENE") else { return };
    let rem = std::env::var("POP_REM_MIN").ok().and_then(|m| m.parse().ok()).unwrap_or(60);
    let (scene, then) = match v.as_str() {
        "end" => (Scene::Ending, cutscene::Then::EndPages),
        "timeout" => (Scene::TimeOut, cutscene::Then::Title),
        "intro" => (Scene::Intro, cutscene::Then::Title),
        l => (Scene::Level(l.parse().unwrap_or(2)), cutscene::Then::Play),
    };
    if cut.start(a.data.clone(), scene, rem, then) {
        next.set(flow::Screen::Cutscene);
    }
}

/// The first world; test hook POP_SET="start=ROOM:TILE,door=N,upside=1" (screenshots of states that take long to reach):
/// another start position, leveldoor_open = N (level 8: the mouse comes at 150 while the kid is in room 16), upside-down on.
fn debug_set(a: &kid::KidAssets, level: &Level, n: usize) -> sim::World {
    let set = std::env::var("POP_SET").unwrap_or_default();
    let val = |k: &str| set.split(',').find_map(|kv| kv.trim().strip_prefix(k)?.strip_prefix('=').map(str::to_owned));
    let mut l = level.clone();
    if let Some((r, t)) = val("start").as_deref().and_then(|v| v.split_once(':')).and_then(|(r, t)| Some((r.parse().ok()?, t.parse().ok()?))) {
        (l.start_room, l.start_tile) = (r, t);
    }
    let mut w = sim::World::new(a.data.clone(), l, n);
    w.leveldoor_open = val("door").and_then(|v| v.parse().ok()).unwrap_or(w.leveldoor_open);
    // exit=1: every exit door stands open and the level counts as started elsewhere, so "up" in front of it climbs the stairs
    if val("exit").as_deref() == Some("1") {
        w.leveldoor_open = 1;
        w.level.start_room = 0;
        for (room, idx) in (1..=ROOMS).flat_map(|r| (0..30).map(move |i| (r, i))) {
            if w.level.tile_at(room, idx).kind == pop_core::level::Tile::ExitLeft {
                w.level.set_modifier(room, idx, 43);
            }
        }
    }
    w.upside |= val("upside").as_deref() == Some("1");
    // gate=N: every gate stands at opening N (0 closed .. 188 open), for screenshots of the half-open states
    if let Some(m) = val("gate").and_then(|v| v.parse::<u8>().ok()) {
        for (room, idx) in (1..=ROOMS).flat_map(|r| (0..30).map(move |i| (r, i))) {
            if w.level.tile_at(room, idx).kind == pop_core::level::Tile::Gate {
                w.level.set_modifier(room, idx, m);
            }
        }
    }
    w
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
            "level {}  room {}  (L{} R{} U{} D{})  start room {} tile {}\ntick {}  kid: seq {} frame {} action {} x {} y {} room {} row {} fall {} hp {}/{} sword {}{}\narrows+Shift kid  Ctrl+arrows room  Ctrl+PgUp/PgDn level  , . sequence viewer  Space restart  F flip  C control  F3 text  F5 classic/enhanced  F6 sprite/voxel  F7 camera",
            g.level, g.room, left, right, up, down, l.start_room, l.start_tile, g.world.tick, g.world.kid.seq, g.world.kid.frame, g.world.kid.action, g.world.kid.x, g.world.kid.y, g.world.kid.room, g.world.kid.row, g.world.kid.fall_y, g.world.kid.hp, g.world.kid.hp_max, g.world.kid.sword,
            g.world.guard.as_ref().map_or(String::new(), |gd| format!("   guard: seq {} frame {} x {} hp {}/{} sword {} skill {} see {}", gd.seq, gd.frame, gd.x, gd.hp, gd.hp_max, gd.sword, gd.skill, g.world.shared.can_see))
        );
        let w = &g.world;
        t.0 += &format!("
time {}:{:03}  death {}  flash {:?}  text timer {}", w.progress.rem_min, w.progress.rem_tick, w.death_count, w.flash, w.text_timer);
    }
}

/// Test hook (shaders compile asynchronously, so wait a few hundred frames): with `POP_SHOT=file.png` (and optionally `POP_ROOM`, `POP_LEVEL`, `POP_SHOT_FRAME`) save a screenshot after a few frames and quit.
/// `POP_SHOT_TICK=n` instead waits for logic tick n (the world ticks in real time, so frames do not map to ticks; use with POP_SCRIPT).
fn screenshot_and_quit(mut cmds: Commands, mut st: Local<(u32, Option<u32>)>, mut exit: MessageWriter<AppExit>, g: Option<Res<Game>>) {
    // shortcut: fixed frame count, depends on how fast shaders compile; upgrade to waiting for pipelines if flaky
    let Ok(path) = std::env::var("POP_SHOT") else { return };
    st.0 += 1;
    let frame = st.0;
    let due = match (std::env::var("POP_SHOT_TICK").ok().and_then(|v| v.parse::<u64>().ok()), g) {
        (Some(t), Some(g)) => g.world.tick >= t,
        _ => frame == std::env::var("POP_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(400),
    };
    match st.1 {
        None if due => {
            st.1 = Some(frame);
            cmds.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
        }
        Some(at) if frame == at + 60 => {
            exit.write(AppExit::Success);
        }
        _ => {}
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
