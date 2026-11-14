use bevy::prelude::*;
use pop_core::{dat::Dat, level::*, sim};

#[derive(Resource)]
struct Game {
    world: sim::World,
    level: Level,
}

fn main() {
    let dir = std::env::var("POP_DATA").unwrap_or_else(|_| "Original_data".into());
    let dat = Dat::open(format!("{dir}/LEVELS.DAT")).expect("cannot open LEVELS.DAT (set POP_DATA)");
    let level = Level::parse(dat.get(2001).expect("level 1")).expect("bad level");

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Prince of Persia x64".into(), ..default() }),
            ..default()
        }))
        .insert_resource(Time::<Fixed>::from_hz(sim::TICKS_PER_SEC))
        .insert_resource(Game { world: sim::World::default(), level })
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, |mut g: ResMut<Game>| g.world.step())
        .run();
}

/// Draw the start room as 3D blocks (placeholder for real tile models).
fn setup(mut cmds: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, g: Res<Game>) {
    let room = g.level.start_room as usize;
    let cube = meshes.add(Cuboid::new(1.0, 0.2, 1.0));
    let wall = meshes.add(Cuboid::new(1.0, 3.0, 1.0));
    let floor_mat = mats.add(Color::srgb(0.7, 0.6, 0.4));
    let wall_mat = mats.add(Color::srgb(0.4, 0.35, 0.3));
    for row in 0..ROWS {
        for col in 0..COLS {
            let (t, _) = g.level.tile(room, col, row);
            let (x, y) = (col as f32, -(row as f32) * 3.0);
            match t {
                0 => {}                                                                            // empty
                20 => { cmds.spawn((Mesh3d(wall.clone()), MeshMaterial3d(wall_mat.clone()), Transform::from_xyz(x, y + 1.5, 0.0))); } // wall
                _ => { cmds.spawn((Mesh3d(cube.clone()), MeshMaterial3d(floor_mat.clone()), Transform::from_xyz(x, y, 0.0))); }       // floor-like
            }
        }
    }
    cmds.spawn((Camera3d::default(), Transform::from_xyz(4.5, -3.0, 14.0).looking_at(Vec3::new(4.5, -3.0, 0.0), Vec3::Y)));
    cmds.spawn((DirectionalLight::default(), Transform::from_xyz(3.0, 5.0, 8.0).looking_at(Vec3::new(4.5, -3.0, 0.0), Vec3::Y)));
}
