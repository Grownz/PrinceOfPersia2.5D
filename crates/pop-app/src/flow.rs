//! Game flow (M6): title and attract loop (docs/cutscenes.md 2), cutscenes (cutscene.rs), pause, HUD, save/load.
use crate::audio::{AudioState, Sfx, Sounds};
use crate::cutscene::{Cut, Then};
use crate::kid::{self, KidAssets};
use crate::Game;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use pop_core::cutscene::{End, Scene};
use pop_core::sim::{Exit, Progress};
use pop_core::{dat::Dat, image};
use std::collections::HashMap;

#[derive(States, Clone, Copy, Default, Eq, PartialEq, Hash, Debug)]
pub enum Screen {
    /// the title and attract loop, and the ending pages
    #[default]
    Title,
    Playing,
    Paused,
    /// a cutscene in the princess chamber (cutscene.rs); then Playing, Title or the ending pages
    Cutscene,
}

/// One picture of the title/story screens (320x200, TITLE.DAT).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pic {
    Black,
    /// 51 plain, 1: + 52 "presents", 2: + 53 "a game by", 3: + 54 logo + 55 copyright
    Title(u8),
    /// story frame 41 + text page 42 / 43 / 44 / 45
    Page(u16),
    /// the intro cutscene (pv_scene) runs here
    Intro,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fx {
    Cut,
    FadeIn,
    FadeOut,
    /// transition_ltr: the new picture replaces the old one in columns from the left
    Wipe,
}

/// Phase length in 1/60 s units, or until no sound plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Len {
    Units(u32),
    Quiet,
}

type Phase = (Pic, Fx, Len, Option<u8>);

/// fade_in_2 / fade_out_2: 64 steps of 2 units (INFERRED, docs/cutscenes.md 1.5)
const FADE2: u32 = 128;
/// transition_ltr: machine speed in the original (UNVERIFIED); 160 steps over 0.5 s as the doc suggests
const WIPE: u32 = 30;

/// show_title (0000:181D, docs/cutscenes.md 2): (picture, effect, length, sound started with the phase). There is no idle
/// timer: the loop ends in the demo, and the demo ends back on the title. The hall of fame is skipped (the remake keeps none).
const ATTRACT: [Phase; 17] = [
    (Pic::Title(0), Fx::FadeIn, Len::Units(FADE2), None),
    (Pic::Title(0), Fx::Cut, Len::Units(130), Some(54)),
    (Pic::Title(1), Fx::Cut, Len::Units(205), None),
    (Pic::Title(0), Fx::Cut, Len::Units(65), None),
    (Pic::Title(2), Fx::Cut, Len::Units(270), None),
    (Pic::Title(0), Fx::Cut, Len::Units(235), None),
    (Pic::Title(3), Fx::Cut, Len::Quiet, None),
    (Pic::Page(42), Fx::Wipe, Len::Units(600), Some(55)),
    (Pic::Page(42), Fx::FadeOut, Len::Units(FADE2), None),
    (Pic::Black, Fx::Cut, Len::Quiet, None), // load_intro waits until sound 55 has ended
    (Pic::Intro, Fx::Cut, Len::Units(0), None),
    (Pic::Page(43), Fx::FadeIn, Len::Units(FADE2), None),
    (Pic::Page(43), Fx::Cut, Len::Quiet, None), // sound 52 from the intro scene
    (Pic::Title(3), Fx::Wipe, Len::Units(120), None),
    (Pic::Page(45), Fx::Wipe, Len::Units(360), None),
    (Pic::Page(45), Fx::Cut, Len::Quiet, None),
    (Pic::Page(45), Fx::FadeOut, Len::Units(FADE2), None),
];

/// end_sequence after the animation (0000:2F2F): music 56, page 44 for 900 units, the title, wait for the music (or a key).
const ENDING: [Phase; 5] = [
    (Pic::Page(44), Fx::FadeIn, Len::Units(FADE2), Some(56)),
    (Pic::Page(44), Fx::Cut, Len::Units(900), None),
    (Pic::Title(0), Fx::Wipe, Len::Units(240), None),
    (Pic::Title(0), Fx::Cut, Len::Quiet, None),
    (Pic::Title(0), Fx::FadeOut, Len::Units(FADE2), None),
];

#[derive(Resource, Default)]
pub struct Flow {
    ending: bool,
    phase: usize,
    /// units into the phase
    t: f32,
    entered: bool,
    /// the picture before this phase (wipes)
    prev: Option<Pic>,
    pics: HashMap<Pic, Vec<u8>>,
    screen: Handle<Image>,
    drawn: Option<(Option<Pic>, Pic, u32)>,
}

impl Flow {
    fn slides(&self) -> &'static [Phase] {
        if self.ending { &ENDING } else { &ATTRACT }
    }

    fn cur(&self) -> Phase {
        self.slides()[self.phase.min(self.slides().len() - 1)]
    }

    /// Brightness of the title/story screens (0 = black), for cutscene::render's fade layer.
    pub fn fade(&self) -> f32 {
        let (pic, fx, ..) = self.cur();
        match fx {
            _ if pic == Pic::Black || pic == Pic::Intro => 0.0,
            Fx::FadeIn => (self.t / FADE2 as f32).min(1.0),
            Fx::FadeOut => 1.0 - (self.t / FADE2 as f32).min(1.0),
            _ => 1.0,
        }
    }

    /// At the start of the attract loop or the ending pages (nothing shown yet).
    pub fn fresh(&self) -> bool {
        self.phase == 0 && !self.entered
    }

    /// Start the attract loop (false) or the ending pages from the beginning.
    fn restart(&mut self, ending: bool) {
        (self.ending, self.phase, self.t, self.entered, self.prev, self.drawn) = (ending, 0, 0.0, false, None, None);
    }

    /// The next phase; true when the slides are over.
    fn advance(&mut self) -> bool {
        self.prev = Some(self.cur().0).filter(|&p| p != Pic::Intro).or(self.prev);
        (self.phase, self.t, self.entered) = (self.phase + 1, 0.0, false);
        self.phase >= self.slides().len()
    }
}

fn to_title(f: &mut Flow, next: &mut NextState<Screen>) {
    f.restart(false);
    next.set(Screen::Title);
}

#[derive(Component, PartialEq)]
pub enum Overlay {
    Title,
    Hud,
}

#[derive(Component)]
pub enum Label {
    Hint,
    Centre,
}

/// (guard, index) of one hit-point marker
#[derive(Component)]
pub struct Heart(bool, u8);

/// The title and story pictures, 320x200 RGBA. Title pictures (palette 50) are opaque; pages (frame 41 + a 1-bpp text page
/// in colour 15, palette 40) keep colour 0 transparent so the chamber set shows through. Positions VERIFIED (docs 2).
fn title_pics(dir: &str) -> Option<HashMap<Pic, Vec<u8>>> {
    let dat = Dat::open(format!("{dir}/TITLE.DAT")).ok()?;
    let compose = |pal_id: u16, layers: &[(u16, usize, usize)], opaque: bool| -> Option<Vec<u8>> {
        let pal = image::palette(dat.get(pal_id)?)?;
        let mut px = vec![0u8; 320 * 200];
        for &(id, x0, y0) in layers {
            let im = image::decode(dat.get(id)?)?;
            for y in 0..im.height.min(200usize.saturating_sub(y0)) {
                for x in 0..im.width.min(320usize.saturating_sub(x0)) {
                    let p = im.pixels[y * im.width + x];
                    if p != 0 {
                        px[(y0 + y) * 320 + x0 + x] = if im.bpp == 1 { 15 } else { p }; // colour 0 is transparent
                    }
                }
            }
        }
        Some(px.iter().flat_map(|&p| {
            let [r, g, b] = pal.get(p as usize).copied().unwrap_or([255, 0, 255]);
            [r, g, b, if p == 0 && !opaque { 0 } else { 255 }]
        }).collect())
    };
    let mut m = HashMap::new();
    m.insert(Pic::Title(0), compose(50, &[(51, 0, 0)], true)?);
    m.insert(Pic::Title(1), compose(50, &[(51, 0, 0), (52, 96, 106)], true)?);
    m.insert(Pic::Title(2), compose(50, &[(51, 0, 0), (53, 96, 122)], true)?);
    m.insert(Pic::Title(3), compose(50, &[(51, 0, 0), (54, 24, 107), (55, 48, 184)], true)?);
    for (page, y) in [(42, 25), (43, 25), (44, 25), (45, 26)] {
        m.insert(Pic::Page(page), compose(40, &[(41, 0, 0), (page, 24, y)], false)?);
    }
    Some(m)
}

fn full_screen(o: Overlay) -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Px(16.0), ..default() },
        BackgroundColor(Color::BLACK),
        Visibility::Hidden,
        o,
    )
}

pub fn setup(mut cmds: Commands, mut imgs: ResMut<Assets<Image>>, mut f: ResMut<Flow>) {
    let font = |px: f32| TextFont::from_font_size(px);
    let gold = TextColor(Color::srgb(1.0, 0.85, 0.5));
    let pics = title_pics(&crate::data_dir());
    let mut img = Image::new(Extent3d { width: 320, height: 200, depth_or_array_layers: 1 }, TextureDimension::D2, vec![0; 320 * 200 * 4], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    img.sampler = ImageSampler::nearest();
    f.screen = imgs.add(img);
    let screen = f.screen.clone();
    cmds.spawn(full_screen(Overlay::Title)).with_children(|p| {
        if pics.is_some() {
            p.spawn((ImageNode::new(screen), Node { height: Val::Percent(85.0), aspect_ratio: Some(320.0 / 200.0), ..default() }));
        } else {
            p.spawn((Text::new("PRINCE OF PERSIA"), font(64.0), gold.clone())); // TITLE.DAT missing: text title
        }
        p.spawn((Text::new(""), font(22.0), gold.clone(), Label::Hint));
    });
    f.pics = pics.unwrap_or_default();
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

/// Start a world from `p` with the "Level N" banner; when `entered` (a level change, new game or loaded game, never a
/// restart) the level's cutscene plays first (tbl_cutscenes, levels 2, 4, 6, 8, 9, 12).
fn begin(g: &mut Game, a: &KidAssets, cut: &mut Cut, next: &mut NextState<Screen>, p: Progress, banner: bool, entered: bool) {
    kid::start(g, a, p);
    if let Some(text) = level_banner(g.level).filter(|_| banner) {
        g.banner = (24, text);
    }
    let scene = entered && cut.start(a.data.clone(), Scene::Level(g.level), g.world.progress.rem_min, Then::Play);
    next.set(if scene { Screen::Cutscene } else { Screen::Playing });
}

/// "Level N" for 24 ticks (0000:C270, VERIFIED): levels 1..13 (level != 0 and < 14), level 13 shown as 12; the caller skips
/// it after the seamless 12 -> 13 ([59C8]).
fn level_banner(level: usize) -> Option<String> {
    (1..=13).contains(&level).then(|| format!("LEVEL {}", level.min(12)))
}

/// The folder of the running exe (PRINCE.SAV, settings.json, pop.log live there); the cwd if it is unknown.
pub fn exe_dir() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| ".".into())
}

fn save_path() -> std::path::PathBuf {
    exe_dir().join("PRINCE.SAV")
}

fn write_save(text: &str) -> std::io::Result<()> {
    write_atomic(&save_path(), text)
}

/// Write a file atomically: a temporary file next to it, flushed, then renamed over the old one (a crash or a full disk
/// mid-write keeps the previous file).
pub fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let r = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    let r = r.and_then(|()| std::fs::rename(&tmp, path));
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
pub fn any_key(k: &ButtonInput<KeyCode>) -> bool {
    use KeyCode::*;
    k.get_just_pressed().any(|c| !matches!(c, ControlLeft | ControlRight | AltLeft | AltRight | SuperLeft | SuperRight | F3 | KeyL))
        || (k.just_pressed(KeyL) && !ctrl(k))
}

/// Any controller button pressed this frame (starts the game / skips a cutscene like any key).
pub fn pad_any(pads: &Query<&Gamepad>) -> bool {
    pads.iter().any(|p| p.get_just_pressed().next().is_some())
}

/// Longest frame delta the attract/title timers accept: a stall (window drag, alt-tab) must not skip phases.
const MAX_DT: f32 = 0.1;

fn clamp_dt(dt: f32) -> f32 {
    dt.clamp(0.0, MAX_DT)
}

/// Title / attract loop and ending pages: timers in 1/60 s units, sounds at phase starts, "until no sound" waits on the
/// deterministic `AudioState::busy`. Any key (or a controller button) during the attract loop starts the game at level 1,
/// Ctrl+L loads (process_key 0000:04E2); during the ending pages a key skips to the final fade.
#[allow(clippy::too_many_arguments)]
pub fn title(
    k: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut g: ResMut<Game>,
    a: Res<KidAssets>,
    mut f: ResMut<Flow>,
    mut cut: ResMut<Cut>,
    time: Res<Time>,
    mut next: ResMut<NextState<Screen>>,
    mut cmds: Commands,
    s: Res<Sounds>,
    mut audio: ResMut<AudioState>,
    playing: Query<(Entity, &Sfx)>,
    mut imgs: ResMut<Assets<Image>>,
) {
    let key = any_key(&k) || pad_any(&pads);
    if !f.ending {
        let loaded = (ctrl(&k) && k.just_pressed(KeyCode::KeyL)).then(load_game).flatten();
        if let Some(p) = loaded {
            return begin(&mut g, &a, &mut cut, &mut next, p, true, true);
        } else if key {
            return begin(&mut g, &a, &mut cut, &mut next, Progress::new_game(1), true, true); // new game (0000:4893)
        }
    } else if key && f.phase + 1 < ENDING.len() {
        (f.phase, f.t, f.entered) = (ENDING.len() - 1, 0.0, true);
        crate::audio::stop_all(&mut cmds, &mut audio, &playing);
    }
    if !f.entered {
        f.entered = true;
        let (pic, _, _, sound) = f.cur();
        if let Some(id) = sound {
            crate::audio::play(&mut cmds, &s, &mut audio, id, &playing);
        }
        if pic == Pic::Intro {
            // load_intro(0, pv_scene, 0); without PV.DAT the scene is skipped
            if cut.start(a.data.clone(), Scene::Intro, 60, Then::Attract) {
                return next.set(Screen::Cutscene);
            }
            f.advance();
            return;
        }
    }
    f.t += clamp_dt(time.delta_secs()) * 60.0;
    let done = match f.cur().2 {
        Len::Units(n) => f.t >= n as f32,
        Len::Quiet => audio.busy == 0,
    };
    if done && f.advance() {
        if f.ending {
            to_title(&mut f, &mut next);
        } else {
            begin(&mut g, &a, &mut cut, &mut next, Progress::new_game(0), false, true); // init_game(0): the demo
        }
    }
    draw_pic(&mut f, &mut imgs);
}

/// Compose the shown picture (the wipe reveals the new one in columns from the left) into the screen image.
fn draw_pic(f: &mut Flow, imgs: &mut Assets<Image>) -> Option<()> {
    let (pic, fx, ..) = f.cur();
    let cols = if fx == Fx::Wipe { ((f.t / WIPE as f32).min(1.0) * 320.0) as u32 } else { 320 };
    let key = (f.prev.filter(|_| cols < 320), pic, cols);
    if f.drawn == Some(key) {
        return Some(());
    }
    f.drawn = Some(key);
    let black = vec![0u8; 320 * 200 * 4];
    let new = f.pics.get(&pic).unwrap_or(&black);
    let old = f.prev.and_then(|p| f.pics.get(&p)).unwrap_or(&black);
    let mut out = new.clone();
    for y in 0..200 {
        let row = y * 320 * 4;
        out[row + cols as usize * 4..row + 320 * 4].copy_from_slice(&old[row + cols as usize * 4..row + 320 * 4]);
    }
    imgs.get_mut(&f.screen)?.data = Some(out);
    Some(())
}

/// Esc / P toggle the pause (stepping stops because FixedUpdate only runs in Playing).
pub fn pause(k: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, state: Res<State<Screen>>, mut next: ResMut<NextState<Screen>>) {
    if k.just_pressed(KeyCode::Escape) || k.just_pressed(KeyCode::KeyP) || pads.iter().any(|p| p.just_pressed(GamepadButton::Start)) {
        match state.get() {
            Screen::Playing => next.set(Screen::Paused),
            Screen::Paused => next.set(Screen::Playing),
            _ => {}
        }
    }
}

/// Playing: save (Ctrl+G), the demo's keys, and the world's exit (restart, next level, title, the end).
#[allow(clippy::too_many_arguments)]
pub fn playing(k: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut g: ResMut<Game>, a: Res<KidAssets>, mut f: ResMut<Flow>, mut cut: ResMut<Cut>, mut next: ResMut<NextState<Screen>>, audio: Res<AudioState>) {
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
            return begin(&mut g, &a, &mut cut, &mut next, p, true, true);
        }
        if (any_key(&k) && !k.just_pressed(KeyCode::Escape) && !k.just_pressed(KeyCode::KeyP)) || (pad_any(&pads) && !pads.iter().any(|p| p.just_pressed(GamepadButton::Start))) {
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
    let rem_min = g.world.progress.rem_min;
    let p = g.world.progress_after(e);
    match e {
        // expired (0000:31F4): the time-out scene 2A1B, then the title
        Exit::Title if expired => {
            kid::start(&mut g, &a, p);
            if cut.start(a.data.clone(), Scene::TimeOut, 0, Then::Title) {
                next.set(Screen::Cutscene);
            } else {
                to_title(&mut f, &mut next);
            }
        }
        Exit::Title => {
            kid::start(&mut g, &a, p);
            to_title(&mut f, &mut next);
        }
        // end_sequence (0000:2F2F): the ending scene 2960, then the ending pages
        Exit::End => {
            kid::start(&mut g, &a, p);
            if cut.start(a.data.clone(), Scene::Ending, rem_min, Then::EndPages) {
                next.set(Screen::Cutscene);
            } else {
                f.restart(true);
                next.set(Screen::Title);
            }
        }
        // "Level N" also on a restart (edge_cases.md 3.3), but never the cutscene
        Exit::Restart => begin(&mut g, &a, &mut cut, &mut next, p, true, false),
        Exit::NextLevel { seamless } => begin(&mut g, &a, &mut cut, &mut next, p, !seamless, true),
        Exit::NewGame => begin(&mut g, &a, &mut cut, &mut next, p, true, true),
    }
}

/// Cutscene over: on to the level, the title, the ending pages or back into the attract loop (a key in the intro starts
/// the game, as process_key does in the title).
pub fn cutscene_end(mut g: ResMut<Game>, a: Res<KidAssets>, mut f: ResMut<Flow>, mut cut: ResMut<Cut>, mut next: ResMut<NextState<Screen>>) {
    let Some(e) = cut.ended.take() else { return };
    debug!("cutscene {:?} {e:?} after {} frames", cut.scene, cut.cs.as_ref().map_or(0, |c| c.frames));
    cut.cs = None;
    match (cut.then, e) {
        (Then::Play, _) => next.set(Screen::Playing),
        (Then::Title, _) => to_title(&mut f, &mut next),
        (Then::EndPages, _) => {
            f.restart(true);
            next.set(Screen::Title);
        }
        (Then::Attract, End::Skipped) => begin(&mut g, &a, &mut cut, &mut next, Progress::new_game(1), true, true),
        (Then::Attract, End::Done) => {
            f.advance();
            next.set(Screen::Title);
        }
    }
}

/// Shows the overlay of the current screen and fills the HUD.
pub fn overlay(
    state: Res<State<Screen>>,
    g: Res<Game>,
    f: Res<Flow>,
    mut q: Query<(&Overlay, &mut Visibility, &mut BackgroundColor), Without<Heart>>,
    mut labels: Query<(&Label, &mut Text)>,
    mut hearts: Query<(&Heart, &mut Node, &mut BackgroundColor, &mut BorderColor)>,
) {
    let s = *state.get();
    // title pictures are opaque; the story pages are drawn over the chamber set, dimmed
    let dim = if matches!(f.cur().0, Pic::Page(_)) { Color::srgba(0.0, 0.0, 0.0, 0.45) } else { Color::BLACK };
    for (o, mut v, mut bg) in &mut q {
        if *o == Overlay::Title {
            bg.set_if_neq(BackgroundColor(dim));
        }
        let show = match o {
            Overlay::Title => s == Screen::Title && f.cur().0 != Pic::Intro,
            Overlay::Hud => matches!(s, Screen::Playing | Screen::Paused),
        };
        v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
    }
    let w = &g.world;
    for (l, mut t) in &mut labels {
        let text = match l {
            Label::Hint if f.cur().0 == Pic::Title(3) && !f.ending => "Press any key  -  Ctrl+L load game".into(),
            Label::Hint => String::new(),
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

    /// show_title timers (docs/cutscenes.md 2), counted from the start of music 54: "presents" at 130..335, "a game by" at
    /// 400..670, the logo from 905 until the music ends; one intro scene; no idle timer anywhere.
    #[test]
    fn attract_timers_follow_show_title() {
        use super::*;
        let mut t = 0;
        let mut shown = vec![];
        for &(pic, _, len, sound) in &ATTRACT[1..7] {
            if sound == Some(54) {
                t = 0;
            }
            shown.push((pic, t));
            if let Len::Units(n) = len {
                t += n;
            }
        }
        assert_eq!(shown, [(Pic::Title(0), 0), (Pic::Title(1), 130), (Pic::Title(0), 335), (Pic::Title(2), 400), (Pic::Title(0), 670), (Pic::Title(3), 905)]);
        assert_eq!(ATTRACT.iter().filter(|p| p.0 == Pic::Intro).count(), 1);
        assert_eq!((clamp_dt(5.0), clamp_dt(0.016), clamp_dt(-1.0)), (MAX_DT, 0.016, 0.0));
        assert_eq!(ATTRACT.iter().filter(|p| p.2 == Len::Quiet).count(), 4, "logo, before the intro, page 43, before the demo");
        assert_eq!(ENDING[0].3, Some(56));
    }
}
