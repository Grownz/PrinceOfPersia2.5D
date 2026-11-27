//! The level camera on top of `game_camera`: the pan from the old room to the new one (setting camera_scroll, POP_CAMERA)
//! and the upside-down view of the invert potion (`World::upside`).
//!
//! The rooms are always built around the shown room (`room::rebuild_on_change`, the shown room at origin 0), so a room
//! change re-origins the whole set. The pan keeps the camera where it was by starting from the old room's place in the new
//! frame (one room width/height away) and easing back to 0; a change in the middle of a pan starts from wherever the
//! camera is at that moment. Not a neighbour (level change, restart, teleport) or camera_scroll = flip: a hard cut, as the
//! original redraws the new room at once.
//!
//! Upside-down (VERIFIED, docs/edge_cases.md 5.1): the original flips the 320x192 playfield buffer (rect ds:0F9A) about
//! its horizontal middle around every blit (0C83:19BD before/after, rect y -> 192 - y in 0000:15A0); the status line
//! below it is not part of that buffer, so the hit points and texts stay upright. Here the 3D view is mirrored the same
//! way (the rendered picture shown flipped, `mirror`) and the UI is drawn upright over it.
use crate::room::TILE_H;
use crate::Game;
use bevy::prelude::*;
use pop_core::level::{COLS, ROOMS, ROWS};

/// Length of the pan in seconds (wall clock, frame rate independent).
pub const PAN_SECS: f32 = 0.3;

#[derive(Resource, Default)]
pub struct View {
    /// camera offset from the shown room's centre at the start of the pan (world units, in the shown room's frame)
    from: Vec2,
    /// seconds since the pan started
    t: f32,
    /// (level, world generation, shown room) the last time `follow` ran
    seen: (usize, u32, usize),
    upside: bool,
    /// cinematic fights: the smoothed camera shift from the room's centre and the zoom beyond 1 (both 0 = the plain camera)
    focus: Vec2,
    zoom: f32,
    /// cinematic fights on and the kid's sword is drawn against a living guard in the shown room (the minimap fades out meanwhile)
    pub fighting: bool,
}

/// Cinematic fights: the zoom never goes beyond this, and the fighters keep this margin (world units) to the picture's edge.
const MAX_ZOOM: f32 = 1.8;
const MARGIN_X: f32 = 1.2;
const HALF_H: f32 = 1.3;
/// Per second: how fast the camera approaches (or leaves) the fight.
const FIGHT_RATE: f32 = 3.0;
/// The look-at point of `game_camera` (room centre).
const LOOK: Vec2 = Vec2::new(5.0, 3.0);

/// The camera aim for a fight between two figures standing at `a` and `b` (image centre x, bottom y): the shift of the look-at
/// point (to the middle of the two) and the zoom beyond 1. The zoom is as strong as both stay in the picture with the margins
/// (the sword's reach, the figures' height), up to MAX_ZOOM; never below the plain camera.
pub fn fight_aim(a: Vec2, b: Vec2, aspect: f32) -> (Vec2, f32) {
    let d = crate::game_camera(aspect).translation.z;
    let half_h = d * (std::f32::consts::FRAC_PI_8).tan();
    let half_w = half_h * aspect;
    let mid = (a + b) / 2.0 + Vec2::new(0.0, 0.8);
    let need_w = (a.x - b.x).abs() / 2.0 + MARGIN_X;
    let z = (half_w / need_w).min(half_h / (HALF_H + (a.y - b.y).abs() / 2.0)).clamp(1.0, MAX_ZOOM);
    (mid - LOOK, z - 1.0)
}

/// Smoothstep: zero speed at both ends, so the pan neither jerks off nor snaps in.
pub fn ease(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

/// Camera offset `t` seconds into a pan that started at `from`: `from` at 0, zero from PAN_SECS on.
pub fn pan_offset(from: Vec2, t: f32) -> Vec2 {
    from * (1.0 - ease(t / PAN_SECS))
}

/// Where room `to` lies seen from the room with these links ([left, right, up, down]), in room units; None if it is not
/// a direct neighbour.
pub fn room_step(links: [u8; 4], to: usize) -> Option<Vec2> {
    let dirs = [Vec2::NEG_X, Vec2::X, Vec2::Y, Vec2::NEG_Y];
    links.iter().position(|&r| r as usize == to).map(|i| dirs[i])
}

/// Size of a room in world units.
fn room_size() -> Vec2 {
    Vec2::new(COLS as f32, ROWS as f32 * TILE_H)
}

/// Every frame, before the camera is placed: start a pan when the shown room changed, advance it, take the upside flag.
#[allow(clippy::too_many_arguments)]
pub fn follow(
    time: Res<Time>,
    g: Res<Game>,
    set: Res<crate::settings::Settings>,
    mode: Res<crate::settings::RenderMode>,
    state: Res<State<crate::flow::Screen>>,
    gore: Res<crate::blood::Gore>,
    win: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut v: ResMut<View>,
) {
    let now = (g.level, g.gen, g.room);
    if v.seen != now {
        let old = v.seen.2;
        let same_world = (v.seen.0, v.seen.1) == (now.0, now.1) && (1..=ROOMS).contains(&old);
        let step = same_world.then(|| room_step(g.world.level.links[old - 1], g.room)).flatten().filter(|_| set.smooth_scroll());
        v.from = step.map_or(Vec2::ZERO, |s| pan_offset(v.from, v.t) - s * room_size());
        v.t = 0.0;
        v.seen = now;
    }
    v.t += time.delta_secs();
    v.upside = g.world.upside;
    v.fighting = set.cinematic_fights && *state.get() == crate::flow::Screen::Playing && g.world.kid.sword == 2 && g.world.kid.alive && g.world.kid.room as usize == g.room && g.world.guard.as_ref().is_some_and(|k| k.alive && k.room as usize == g.room);
    // cinematic fights: the kid has drawn his sword against a living guard in the shown room
    let fight = (set.cinematic_fights && *mode != crate::settings::RenderMode::Classic && *state.get() == crate::flow::Screen::Playing && !g.world.upside && g.world.kid.sword == 2 && g.world.kid.alive && g.world.kid.room as usize == g.room)
        .then_some(())
        .and_then(|_| g.world.guard.as_ref().filter(|k| k.alive && k.room as usize == g.room))
        .and_then(|_| gore.bodies[0].zip(gore.bodies[1]));
    let (focus, zoom) = fight.map_or((Vec2::ZERO, 0.0), |(a, b)| fight_aim(a, b, win.width() / win.height().max(1.0)));
    let k = 1.0 - (-FIGHT_RATE * time.delta_secs()).exp();
    v.focus = v.focus.lerp(focus, k);
    v.zoom += (zoom - v.zoom) * k;
}

/// The level camera for this window aspect: `game_camera` moved by the pan.
pub fn transform(aspect: f32, v: &View) -> Transform {
    let mut t = crate::game_camera(aspect);
    // the fight shifts the aim point and moves the camera closer along its line of sight (same direction, so no new rotation)
    let look = LOOK.extend(0.0);
    t.translation = look + v.focus.extend(0.0) + (t.translation - look) / (1.0 + v.zoom);
    t.translation += pan_offset(v.from, v.t).extend(0.0);
    t
}

/// Upside-down applies on the level screens only (not on the cutscene set or the title).
pub fn mirrored(upside: bool, screen: crate::flow::Screen) -> bool {
    use crate::flow::Screen;
    upside && matches!(screen, Screen::Playing | Screen::Paused)
}

/// The pieces of the mirrored picture while it is on: the image the 3D camera renders into, the UI camera and the
/// full-window UI image that shows it flipped.
#[derive(Default)]
pub struct Mirror(Option<(Handle<Image>, Entity, Entity)>);

/// Upside-down: the 3D camera renders into an image and a full-window UI image under every other UI node shows it with
/// flip_y, like the original's buffer flip; the HUD stays upright on top. (A mirrored projection or a camera with scale
/// y = -1 lit the tiles wrongly in Bevy 0.20 even with invert_culling, so the picture itself is flipped.) Off: nothing of
/// this exists and the camera draws to the window as always.
pub fn mirror(
    mut cmds: Commands,
    v: Res<View>,
    state: Res<State<crate::flow::Screen>>,
    win: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    cam: Single<Entity, With<Camera3d>>,
    mut m: Local<Mirror>,
) {
    use bevy::camera::RenderTarget;
    use bevy::render::render_resource::{Extent3d, TextureFormat};
    let size = win.physical_size().max(UVec2::ONE);
    match (mirrored(v.upside, *state.get()), &m.0) {
        (true, None) => {
            let img = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None));
            cmds.entity(*cam).insert(RenderTarget::Image(img.clone().into()));
            let ui = cmds.spawn((Camera2d, Camera { order: 1, ..default() })).id();
            let node = cmds
                .spawn((ImageNode { flip_y: true, ..ImageNode::new(img.clone()) }, Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(i32::MIN)))
                .id();
            m.0 = Some((img, ui, node));
        }
        (true, Some((img, ..))) => {
            // window resized: the image follows (get first, so an unchanged size does not mark the asset changed)
            if images.get(img).is_some_and(|i| i.size() != size) {
                if let Some(mut i) = images.get_mut(img) {
                    i.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
                }
            }
        }
        (false, Some((img, ui, node))) => {
            cmds.entity(*cam).insert(RenderTarget::Window(bevy::window::WindowRef::Primary));
            cmds.entity(*ui).despawn();
            cmds.entity(*node).despawn();
            images.remove(img);
            m.0 = None;
        }
        (false, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_eases_from_the_old_room_to_the_new() {
        let from = Vec2::new(-10.0, 0.0);
        assert_eq!(pan_offset(from, 0.0), from);
        assert_eq!(pan_offset(from, PAN_SECS), Vec2::ZERO);
        assert_eq!(pan_offset(from, 5.0), Vec2::ZERO);
        // monotonic, symmetric about the middle, slow at both ends
        let xs: Vec<f32> = (0..=30).map(|i| pan_offset(from, PAN_SECS * i as f32 / 30.0).x).collect();
        assert!(xs.windows(2).all(|w| w[1] >= w[0]));
        assert!((pan_offset(from, PAN_SECS / 2.0).x + 5.0).abs() < 1e-5);
        assert!((xs[1] - xs[0]) < (xs[16] - xs[15]) / 3.0 && (xs[30] - xs[29]) < (xs[16] - xs[15]) / 3.0);
        // frame rate independent: the offset depends on the elapsed time only
        let at = |fps: f32| (0..(fps * 0.2) as usize).fold(0.0, |t, _| t + 1.0 / fps);
        assert!((pan_offset(from, at(30.0)) - pan_offset(from, at(240.0))).length() < 1e-3);
    }

    #[test]
    fn a_fight_zooms_but_keeps_both_fighters_in_the_picture() {
        let aspect = 16.0 / 9.0;
        let half_h = crate::game_camera(aspect).translation.z * (std::f32::consts::FRAC_PI_8).tan();
        let half_w = half_h * aspect;
        // close together: the full zoom, aimed at the middle
        let (f, z) = fight_aim(Vec2::new(4.0, 1.0), Vec2::new(5.0, 1.0), aspect);
        assert!((z - (MAX_ZOOM - 1.0)).abs() < 1e-5 && (f - (Vec2::new(4.5, 1.8) - LOOK)).length() < 1e-5);
        // far apart: less zoom, and at that zoom both still lie inside the picture with the margin
        let (_, z) = fight_aim(Vec2::new(1.0, 1.0), Vec2::new(7.5, 1.0), aspect);
        assert!(z < MAX_ZOOM - 1.0 && 3.25 + MARGIN_X <= half_w / (1.0 + z) + 1e-4);
        // wider than the picture: no zoom at all, never a zoom out
        assert_eq!(fight_aim(Vec2::new(-30.0, 1.0), Vec2::new(40.0, 1.0), aspect).1, 0.0);
    }

    #[test]
    fn room_steps_follow_the_links() {
        let links = [3, 7, 0, 12]; // left 3, right 7, no room above, 12 below
        assert_eq!(room_step(links, 3), Some(Vec2::NEG_X));
        assert_eq!(room_step(links, 7), Some(Vec2::X));
        assert_eq!(room_step(links, 12), Some(Vec2::NEG_Y));
        assert_eq!(room_step(links, 5), None);
        assert_eq!(room_step([0, 0, 9, 0], 9), Some(Vec2::Y));
    }

    /// Moving right: the old room sits one room to the left in the new frame, so the pan starts there (no jump) and a
    /// second change mid-pan starts from the current point.
    #[test]
    fn re_origin_keeps_the_camera_in_place() {
        let first = pan_offset(Vec2::ZERO, 1.0) - Vec2::X * room_size();
        assert_eq!(first, Vec2::new(-10.0, 0.0));
        let mid = pan_offset(first, PAN_SECS / 2.0);
        let second = mid - Vec2::NEG_Y * room_size(); // then down: the room above is +6 in y
        assert_eq!(second, Vec2::new(-5.0, 6.0));
        assert_eq!(pan_offset(second, 0.0), second);
    }

    #[test]
    fn upside_only_on_the_level_screens() {
        use crate::flow::Screen;
        assert!(mirrored(true, Screen::Playing) && mirrored(true, Screen::Paused));
        assert!(!mirrored(true, Screen::Title) && !mirrored(true, Screen::Cutscene) && !mirrored(false, Screen::Playing));
        // the pan only moves the camera; no pan = the plain game camera
        assert_eq!(transform(16.0 / 9.0, &View::default()), crate::game_camera(16.0 / 9.0));
    }
}
