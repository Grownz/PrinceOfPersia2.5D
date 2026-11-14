//! Room viewer: draws one room of a level as procedural 3D geometry (placeholder art).
//! Keys: arrows = neighbouring room, PageUp/PageDown = level, Tab = debug text.
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
    kid: pop_core::kid::Kid,
    kid_prev: (i32, i32),
}

#[derive(Component)]
struct DebugText;

fn main() {
    let dir = std::env::var("POP_DATA").unwrap_or_else(|_| "Original_data".into());
    let dat = Dat::open(format!("{dir}/LEVELS.DAT")).expect("cannot open LEVELS.DAT (set POP_DATA)");
    let levels: Vec<Level> = (2000..=2014).map(|id| Level::parse(dat.get(id).expect("level")).expect("bad level")).collect();
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
    let level = env("POP_LEVEL").filter(|&l| l < levels.len()).unwrap_or(1);
    let room = env("POP_ROOM").filter(|r| (1..=ROOMS).contains(r)).unwrap_or(levels[level].start_room as usize);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Prince of Persia x64".into(), ..default() }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.04)))
        .insert_resource(Time::<Fixed>::from_hz(sim::STEP_HZ))
        .insert_resource(Game { world: sim::World::default(), levels, level, room, kid: default(), kid_prev: (0, 0) })
        .insert_resource(kid::KidAssets::load(&dir))
        .add_systems(Startup, (setup, kid::setup)) // first room is built by room::rebuild_on_change
        .add_systems(FixedUpdate, |mut g: ResMut<Game>, a: Option<Res<kid::KidAssets>>| {
            if g.world.step() {
                if let Some(a) = a {
                    kid::tick(&mut g, &a);
                }
            }
        })
        .add_systems(Update, (navigate, kid::keys, room::rebuild_on_change, kid::render, debug_text, screenshot_and_quit).chain())
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
    for (key, dir) in [(KeyCode::ArrowLeft, 0), (KeyCode::ArrowRight, 1), (KeyCode::ArrowUp, 2), (KeyCode::ArrowDown, 3)] {
        if keys.just_pressed(key) {
            let to = g.levels[g.level].links[g.room - 1][dir] as usize;
            if (1..=ROOMS).contains(&to) {
                g.room = to;
            }
        }
    }
    let step = keys.just_pressed(KeyCode::PageUp) as i32 - keys.just_pressed(KeyCode::PageDown) as i32;
    if step != 0 {
        g.level = (g.level as i32 + step).rem_euclid(g.levels.len() as i32) as usize;
        g.room = g.levels[g.level].start_room as usize;
    }
    if keys.just_pressed(KeyCode::Tab) {
        for mut v in &mut text {
            *v = if *v == Visibility::Hidden { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

fn debug_text(g: Res<Game>, mut q: Query<&mut Text, With<DebugText>>) {
    let l = &g.levels[g.level];
    let [left, right, up, down] = l.links[g.room - 1];
    for mut t in &mut q {
        t.0 = format!(
            "level {}  room {}  (L{} R{} U{} D{})  start room {} tile {}\ntick {}  kid: seq {} frame {} action {} x {} y {}\narrows room  PgUp/PgDn level  , . sequence  Space restart  F flip  Tab text",
            g.level, g.room, left, right, up, down, l.start_room, l.start_tile, g.world.tick, g.kid.seq, g.kid.frame, g.kid.action, g.kid.x, g.kid.y
        );
    }
}

/// Test hook (shaders compile asynchronously, so wait a few hundred frames): with `POP_SHOT=file.png` (and optionally `POP_ROOM`, `POP_LEVEL`) save a screenshot after a few frames and quit.
fn screenshot_and_quit(mut cmds: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    // shortcut: fixed frame count, depends on how fast shaders compile; upgrade to waiting for pipelines if flaky
    let Ok(path) = std::env::var("POP_SHOT") else { return };
    *frame += 1;
    if *frame == 400 {
        cmds.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    } else if *frame == 460 {
        exit.write(AppExit::Success);
    }
}
