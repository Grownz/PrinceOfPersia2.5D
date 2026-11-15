//! Game flow (M6): title screen and attract loop, pause, cards (cutscene placeholders, time out, the end), HUD, save/load.
use crate::kid::{self, KidAssets};
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::sim::{Exit, Progress};
use pop_core::{dat::Dat, image};

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    #[default]
    Title,
    Playing,
    Paused,
    /// a full-screen text card (cutscene placeholder, time out, the end); then Playing or Title
    Card,
}

/// UNVERIFIED: the original attract loop (title, story text, demo) was not timed; a fixed delay stands in for it.
const ATTRACT_SECS: f32 = 15.0;
/// The cutscene before a level (tbl_cutscenes ds:0FB4, docs/edge_cases.md 4.1) by its music cue; a text card stands in for it.
fn cutscene_sound(level: usize) -> Option<u8> {
    match level {
        2 | 4 | 6 => Some(27),
        8 | 9 => Some(35),
        12 => Some(40),
        _ => None,
    }
}

#[derive(Resource, Default)]
pub struct Flow {
    idle: f32,
    card: String,
    card_left: f32,
    card_then_title: bool,
    /// music cue played while the card is shown
    pub card_sound: Option<u8>,
}

#[derive(Component, PartialEq)]
pub enum Overlay {
    Title,
    Card,
    Hud,
}

#[derive(Component)]
pub enum Label {
    Card,
    Centre,
}

/// (guard, index) of one hit-point marker
#[derive(Component)]
pub struct Heart(bool, u8);

/// Title picture: TITLE.DAT 51 (background) + 54 (logo) at (24, 107) + 55 (copyright) at (48, 184), palette 50.
/// The positions follow SDLPoP (UNVERIFIED against PRINCE.EXE). None if the file is missing or does not decode.
fn title_image(dir: &str) -> Option<Image> {
    let dat = Dat::open(format!("{dir}/TITLE.DAT")).ok()?;
    let pal = image::palette(dat.get(50)?)?;
    let bg = image::decode(dat.get(51)?)?;
    let (w, h) = (bg.width, bg.height);
    let mut px = bg.pixels;
    for (id, x0, y0) in [(54, 24, 107), (55, 48, 184)] {
        let im = image::decode(dat.get(id)?)?;
        for y in 0..im.height.min(h.saturating_sub(y0)) {
            for x in 0..im.width.min(w.saturating_sub(x0)) {
                let p = im.pixels[y * im.width + x];
                if p != 0 {
                    px[(y0 + y) * w + x0 + x] = p; // colour 0 is transparent
                }
            }
        }
    }
    let rgba = px.iter().flat_map(|&p| {
        let [r, g, b] = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
        [r, g, b, 255]
    });
    let mut img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, rgba.collect(), TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    Some(img)
}

fn full_screen(o: Overlay) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Px(16.0), ..default() },
        BackgroundColor(Color::BLACK),
        Visibility::Hidden,
        o,
    )
}

pub fn setup(mut cmds: Commands, mut imgs: ResMut<Assets<Image>>) {
    let font = |px: f32| TextFont::from_font_size(px);
    let gold = TextColor(Color::srgb(1.0, 0.85, 0.5));
    // title
    let pic = title_image(&crate::data_dir());
    cmds.spawn(full_screen(Overlay::Title)).with_children(|p| {
        match pic {
            Some(img) => p.spawn((ImageNode::new(imgs.add(img)), Node { height: Val::Percent(85.0), aspect_ratio: Some(320.0 / 200.0), ..default() })),
            None => p.spawn((Text::new("PRINCE OF PERSIA"), font(64.0), gold.clone())), // TITLE.DAT missing: text title
        };
        p.spawn((Text::new("Press any key  -  Ctrl+L load game"), font(22.0), gold.clone()));
    });
    // card
    cmds.spawn(full_screen(Overlay::Card)).with_children(|p| {
        p.spawn((Text::new(""), font(36.0), gold.clone(), TextLayout::justify(Justify::Center), Label::Card));
    });
    // HUD: kid hit points left, text in the middle, guard hit points right (the original's bottom line)
    let hearts = |p: &mut ChildSpawnerCommands, guard: bool| {
        p.spawn(Node { column_gap: Val::Px(4.0), ..default() }).with_children(|p| {
            for i in 0..10 {
                p.spawn((Node { width: Val::Px(12.0), height: Val::Px(16.0), border: UiRect::all(Val::Px(2.0)), ..default() }, BorderColor::all(Color::NONE), BackgroundColor(Color::NONE), Heart(guard, i)));
            }
        });
    };
    cmds.spawn((
        Node { position_type: PositionType::Absolute, bottom: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Px(28.0), padding: UiRect::horizontal(Val::Px(10.0)), justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() },
        BackgroundColor(Color::BLACK),
        Visibility::Hidden,
        Overlay::Hud,
    ))
    .with_children(|p| {
        hearts(p, false);
        p.spawn((Text::new(""), font(18.0), TextColor(Color::WHITE), Label::Centre));
        hearts(p, true);
    });
}

/// Start a world from `p` and go to Playing (with the "Level N" banner and, when `entered` (a level change, new game or
/// loaded game, never a restart), the cutscene card).
fn begin(g: &mut Game, a: &KidAssets, f: &mut Flow, next: &mut NextState<Screen>, p: Progress, banner: bool, entered: bool) {
    kid::start(g, a, p);
    if let Some(text) = level_banner(g.level).filter(|_| banner) {
        g.banner = (24, text);
    }
    if let Some(id) = cutscene_sound(g.level).filter(|_| entered) {
        card(f, next, format!("LEVEL {}\n\n(cutscene not implemented)", g.level), false, Some(id));
    } else {
        next.set(Screen::Playing);
    }
}

/// "Level N" for 24 ticks (0000:C270, VERIFIED): levels 1..13 (level != 0 and < 14), level 13 shown as 12; the caller skips
/// it after the seamless 12 -> 13 ([59C8]).
fn level_banner(level: usize) -> Option<String> {
    (1..=13).contains(&level).then(|| format!("LEVEL {}", level.min(12)))
}

fn card(f: &mut Flow, next: &mut NextState<Screen>, text: String, then_title: bool, sound: Option<u8>) {
    (f.card, f.card_left, f.card_then_title, f.card_sound) = (text, 3.0, then_title, sound);
    next.set(Screen::Card);
}

fn save_path() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("PRINCE.SAV"))).unwrap_or_else(|| "PRINCE.SAV".into())
}

/// Write PRINCE.SAV atomically: a temporary file next to it, flushed, then renamed over the old save (a crash or a full
/// disk mid-write keeps the previous save).
fn write_save(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let path = save_path();
    let tmp = path.with_extension("SAV.tmp");
    let r = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    let r = r.and_then(|()| std::fs::rename(&tmp, &path));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp); // best effort: no stale temp file
    }
    r
}

/// Ctrl+L (key 0x0C, title/demo only, 0000:0507 -> 1E6F): load PRINCE.SAV.
fn load_game() -> Option<Progress> {
    Progress::from_save(&std::fs::read_to_string(save_path()).ok()?)
}

fn ctrl(k: &ButtonInput<KeyCode>) -> bool {
    k.pressed(KeyCode::ControlLeft) || k.pressed(KeyCode::ControlRight)
}

/// A key that starts the game (modifiers alone and the debug toggle do not).
fn any_key(k: &ButtonInput<KeyCode>) -> bool {
    use KeyCode::*;
    k.get_just_pressed().any(|c| !matches!(c, ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight | F3 | KeyL))
        || (k.just_pressed(KeyL) && !ctrl(k))
}

pub fn title(k: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, a: Res<KidAssets>, mut f: ResMut<Flow>, time: Res<Time>, mut next: ResMut<NextState<Screen>>) {
    f.idle += time.delta_secs();
    let loaded = (ctrl(&k) && k.just_pressed(KeyCode::KeyL)).then(load_game).flatten();
    if let Some(p) = loaded {
        begin(&mut g, &a, &mut f, &mut next, p, true, true);
    } else if any_key(&k) {
        begin(&mut g, &a, &mut f, &mut next, Progress::new_game(1), true, true); // new game (0000:4893)
    } else if f.idle > ATTRACT_SECS {
        begin(&mut g, &a, &mut f, &mut next, Progress::new_game(0), false, true); // the demo
    }
}

pub fn reset_idle(mut f: ResMut<Flow>) {
    f.idle = 0.0;
}

/// Esc / P toggle the pause (stepping stops because FixedUpdate only runs in Playing).
pub fn pause(k: Res<ButtonInput<KeyCode>>, state: Res<State<Screen>>, mut next: ResMut<NextState<Screen>>) {
    if k.just_pressed(KeyCode::Escape) || k.just_pressed(KeyCode::KeyP) {
        match state.get() {
            Screen::Playing => next.set(Screen::Paused),
            Screen::Paused => next.set(Screen::Playing),
            _ => {}
        }
    }
}

/// Playing: save (Ctrl+G), the demo's keys, and the world's exit (restart, next level, title, the end).
pub fn playing(k: Res<ButtonInput<KeyCode>>, mut g: ResMut<Game>, a: Res<KidAssets>, mut f: ResMut<Flow>, mut next: ResMut<NextState<Screen>>, audio: Res<crate::audio::AudioState>) {
    if ctrl(&k) && k.just_pressed(KeyCode::KeyG) && g.world.exit.is_none() {
        // Ctrl+G (key 7, 0000:05EA): only on levels 3..13; messages from ds:0AA4 / ds:0AB0 (display time UNVERIFIED)
        if pop_core::sim::SAVE_LEVELS.contains(&g.level) {
            let ok = write_save(&g.world.progress.save_text(g.world.kid.hp_max)).is_ok();
            g.banner = (24, if ok { "GAME SAVED" } else { "UNABLE TO SAVE GAME" }.into());
        }
    }
    if g.world.demo {
        // any key during the demo starts the game, Ctrl+L loads (edge_cases.md 4.4); the world itself only sees the arrows/Shift
        if let Some(p) = (ctrl(&k) && k.just_pressed(KeyCode::KeyL)).then(load_game).flatten() {
            return begin(&mut g, &a, &mut f, &mut next, p, true, true);
        }
        if any_key(&k) && !k.just_pressed(KeyCode::Escape) && !k.just_pressed(KeyCode::KeyP) {
            g.world.exit.get_or_insert(Exit::NewGame);
        }
    }
    let Some(e) = g.world.exit else { return };
    // play_level_2 (0000:4DA2): after the stairs the level ends only when no sound plays (the end-level music); the level-6
    // fall and the seamless 12 -> 13 stop the sounds first, so they do not wait
    if e == (Exit::NextLevel { seamless: false }) && g.level != 6 && audio.busy > 0 {
        return;
    }
    let expired = g.world.progress.rem_min == 0 && g.level != 0;
    let p = g.world.progress_after(e);
    match e {
        Exit::Title if expired => {
            kid::start(&mut g, &a, p);
            card(&mut f, &mut next, "TIME HAS EXPIRED".into(), true, Some(36)); // the original plays cutscene 0000:2A1B
        }
        Exit::Title => {
            kid::start(&mut g, &a, p);
            next.set(Screen::Title);
        }
        Exit::End => {
            kid::start(&mut g, &a, p);
            card(&mut f, &mut next, "THE END\n\nThe princess is saved.".into(), true, Some(26)); // placeholder for the ending 0000:2F2F
        }
        // "Level N" also on a restart (edge_cases.md 3.3), but never the cutscene
        Exit::Restart => begin(&mut g, &a, &mut f, &mut next, p, true, false),
        Exit::NextLevel { seamless } => begin(&mut g, &a, &mut f, &mut next, p, !seamless, true),
        Exit::NewGame => begin(&mut g, &a, &mut f, &mut next, p, true, true),
    }
}

pub fn card_step(k: Res<ButtonInput<KeyCode>>, mut f: ResMut<Flow>, time: Res<Time>, mut next: ResMut<NextState<Screen>>) {
    f.card_left -= time.delta_secs();
    if f.card_left <= 0.0 || (f.card_left < 2.5 && any_key(&k)) {
        next.set(if f.card_then_title { Screen::Title } else { Screen::Playing });
    }
}

/// Shows the overlay of the current screen and fills the HUD.
pub fn overlay(
    state: Res<State<Screen>>,
    g: Res<Game>,
    f: Res<Flow>,
    mut q: Query<(&Overlay, &mut Visibility)>,
    mut labels: Query<(&Label, &mut Text)>,
    mut hearts: Query<(&Heart, &mut Node, &mut BackgroundColor, &mut BorderColor)>,
) {
    let s = *state.get();
    for (o, mut v) in &mut q {
        let show = match o {
            Overlay::Title => s == Screen::Title,
            Overlay::Card => s == Screen::Card,
            Overlay::Hud => matches!(s, Screen::Playing | Screen::Paused),
        };
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    let w = &g.world;
    for (l, mut t) in &mut labels {
        let text = match l {
            Label::Card => f.card.clone(),
            Label::Centre if s == Screen::Paused => "PAUSED  (Esc / P)".into(),
            Label::Centre if w.continue_text() => "Press Button to Continue".into(),
            Label::Centre if g.banner.0 > 0 => g.banner.1.clone(),
            Label::Centre if w.demo => "DEMO  -  press any key".into(),
            // shortcut: the original shows "N MINUTES LEFT" only at times (show_time 0000:C170); here it is always on
            Label::Centre if w.progress.rem_min > 0 && w.level_no < 14 => format!("{} MINUTES LEFT", w.progress.rem_min),
            Label::Centre => String::new(),
        };
        if t.0 != text {
            t.0 = text;
        }
    }
    // the guard's hit points only while he is in the shown room (the mouse has none)
    let guard = w.guard.as_ref().filter(|gd| gd.room as usize == g.room && gd.charid != 24).map(|gd| (gd.hp, gd.hp_max));
    for (Heart(is_guard, i), mut n, mut bg, mut border) in &mut hearts {
        let (hp, max, c) = if *is_guard { guard.map_or((0, 0, Color::NONE), |(hp, m)| (hp, m, Color::srgb(0.3, 0.6, 1.0))) } else { (w.kid.hp, w.kid.hp_max, Color::srgb(0.95, 0.15, 0.1)) };
        let i = *i as i32;
        let display = if i < max { Display::Flex } else { Display::None };
        if n.display != display {
            n.display = display;
        }
        bg.set_if_neq(BackgroundColor(if i < hp { c } else { Color::NONE }));
        border.set_if_neq(BorderColor::all(c));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn level_banner_c270() {
        let b = |l| super::level_banner(l);
        assert_eq!((b(0), b(14), b(15)), (None, None, None));
        assert_eq!((b(1).unwrap(), b(12).unwrap()), ("LEVEL 1".to_string(), "LEVEL 12".to_string()));
        assert_eq!(b(13).unwrap(), "LEVEL 12", "level 13 is shown as 12");
    }
}
