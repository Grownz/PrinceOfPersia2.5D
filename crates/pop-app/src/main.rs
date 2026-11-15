//! Room viewer: draws one room of a level as procedural 3D geometry (placeholder art).
//! Keys: arrows + Shift = kid, Ctrl+arrows = neighbouring room, PageUp/PageDown = level, Tab = debug text.
mod kid;
mod room;

use bevy::prelude::*;
use pop_core::{dat::Dat, level::*, sim};

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
    /// "Level N" banner: (logic ticks left, number shown)
    banner: (u32, usize),
}

#[derive(Component)]
struct DebugText;

fn main() {
    let dir = std::env::var("POP_DATA").unwrap_or_else(|_| "Original_data".into());
    let dat = Dat::open(format!("{dir}/LEVELS.DAT")).expect("cannot open LEVELS.DAT (set POP_DATA)");
    let levels: Vec<Level> = (2000..=2014).map(|id| Level::parse(dat.get(id).expect("level")).expect("bad level")).collect();
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
    let level = env("POP_LEVEL").filter(|&l| l < levels.len()).unwrap_or(1);
    let assets = kid::KidAssets::load(&dir);
    let world = sim::World::new(assets.data.clone(), levels[level].clone(), level);
    let room = env("POP_ROOM").filter(|r| (1..=ROOMS).contains(r)).unwrap_or(world.kid.room as usize);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Prince of Persia x64".into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(BACKGROUND))
        .insert_resource(Time::<Fixed>::from_hz(sim::STEP_HZ))
        .insert_resource(Game { world, levels, level, room, kid_prev: (Vec2::ZERO, 0), guard_prev: (Vec2::ZERO, 0), kid_respawn: false, banner: (0, 0) })
        .insert_resource(kid::Latch::default())
        .insert_resource(assets)
        .add_systems(Startup, (setup, kid::setup)) // first room is built by room::rebuild_on_change
        .add_systems(FixedUpdate, |mut g: ResMut<Game>, a: Res<kid::KidAssets>, mut latch: ResMut<kid::Latch>, keys: Res<ButtonInput<KeyCode>>| {
            kid::fixed_step(&mut g, &a, &mut latch, &keys);
        })
        .add_systems(Update, (kid::latch_keys, navigate, kid::keys, room::rebuild_on_change, kid::render, flash, debug_text, screenshot_and_quit).chain())
        .run();
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
        DebugText,
    ));
}

fn navigate(keys: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, mut text: Query<&mut Visibility, With<DebugText>>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    for (key, dir) in [(KeyCode::ArrowLeft, 0), (KeyCode::ArrowRight, 1), (KeyCode::ArrowUp, 2), (KeyCode::ArrowDown, 3)] {
        if ctrl && keys.just_pressed(key) {
            let to = g.levels[g.level].links[g.room - 1][dir] as usize;
            if (1..=ROOMS).contains(&to) {
                g.room = to;
            }
        }
    }
    let step = keys.just_pressed(KeyCode::PageUp) as i32 - keys.just_pressed(KeyCode::PageDown) as i32;
    if step != 0 {
        g.level = (g.level as i32 + step).rem_euclid(g.levels.len() as i32) as usize;
        g.kid_respawn = true;
    }
    if keys.just_pressed(KeyCode::Tab) {
        for mut v in &mut text {
            *v = if *v == Visibility::Hidden { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

const BACKGROUND: Color = Color::srgb(0.02, 0.02, 0.04);
/// EGA colours (ds:2BBA, 6-bit RGB); flash() sets VGA palette entry 0 (the black background) to one of them.
/// 2, 4, 12, 14 and 15 are the ones used (docs/edge_cases.md 3.6); the rest is the standard EGA palette (UNVERIFIED against ds:2BBA).
const EGA: [[u8; 3]; 16] = [
    [0, 0, 0], [0, 0, 42], [0, 42, 0], [0, 42, 42], [42, 0, 0], [42, 0, 42], [42, 21, 0], [42, 42, 42],
    [21, 21, 21], [21, 21, 63], [21, 63, 21], [21, 63, 63], [63, 21, 21], [63, 21, 63], [63, 63, 21], [63, 63, 63],
];

/// Screen flash (flash_if_hurt / remove_flash_if_hurt): only the background changes colour, for `flash.1` logic frames.
fn flash(g: Res<Game>, mut clear: ResMut<ClearColor>) {
    let (c, frames) = g.world.flash;
    let want = if frames > 0 {
        let [r, gr, b] = EGA[c as usize & 15].map(|v| (v as u32 * 255 / 63) as u8);
        Color::srgb_u8(r, gr, b)
    } else {
        BACKGROUND
    };
    if clear.0 != want {
        clear.0 = want;
    }
}

fn debug_text(g: Res<Game>, mut q: Query<&mut Text, With<DebugText>>) {
    let l = &g.world.level;
    let [left, right, up, down] = l.links[g.room - 1];
    for mut t in &mut q {
        t.0 = format!(
            "level {}  room {}  (L{} R{} U{} D{})  start room {} tile {}\ntick {}  kid: seq {} frame {} action {} x {} y {} room {} row {} fall {} hp {}/{} sword {}{}\narrows+Shift kid  Ctrl+arrows room  PgUp/PgDn level  , . sequence viewer  Space restart  F flip  C control  Tab text",
            g.level, g.room, left, right, up, down, l.start_room, l.start_tile, g.world.tick, g.world.kid.seq, g.world.kid.frame, g.world.kid.action, g.world.kid.x, g.world.kid.y, g.world.kid.room, g.world.kid.row, g.world.kid.fall_y, g.world.kid.hp, g.world.kid.hp_max, g.world.kid.sword,
            g.world.guard.as_ref().map_or(String::new(), |gd| format!("   guard: seq {} frame {} x {} hp {}/{} sword {} skill {} see {}", gd.seq, gd.frame, gd.x, gd.hp, gd.hp_max, gd.sword, gd.skill, g.world.shared.can_see))
        );
        let w = &g.world;
        t.0 += &format!(
            "
time {}:{:03}  death {}  flash {:?}{}{}{}",
            w.progress.rem_min, w.progress.rem_tick, w.death_count, w.flash,
            if w.demo { "  DEMO (any key starts the game)" } else { "" },
            if g.banner.0 > 0 { format!("

          LEVEL {}", g.banner.1) } else { String::new() },
            if w.continue_text() { "

          Press Button to Continue  (Shift/Enter; Ctrl+A restarts any time)" } else { "" },
        );
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
