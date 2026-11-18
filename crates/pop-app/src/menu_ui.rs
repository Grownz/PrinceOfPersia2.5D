//! The menus on screen (M6): reads the keys and the controller for menu.rs, applies live settings, draws the panel.
//! Esc / Start opens the title menu (the attract loop holds while it is open) and pauses the game (the pause menu);
//! P still pauses and resumes directly. Actions that change the game go to `flow::menu_action`.
use crate::audio::{AudioState, Sfx};
use crate::flow::Screen;
use crate::hof::Hof;
use crate::menu::{Action, Ctx, Menu, Nav, Page, OPTS};
use crate::settings::{CharacterStyle, RenderMode, Settings};
use crate::Game;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

#[derive(Resource, Default)]
pub struct MenuUi {
    pub menu: Option<Menu>,
    /// for flow::menu_action
    pub action: Option<Action>,
    /// the menu used this frame's keys: the title must not also take them as "start the game"
    pub consumed: bool,
    /// "GAME SAVED", "NO SAVED GAME"
    pub note: String,
}

#[derive(Component)]
pub struct MenuPanel;

#[derive(Component)]
pub struct MenuLine(usize);

const LINES: usize = 20;
const GOLD: Color = Color::srgb(1.0, 0.85, 0.5);
const GREY: Color = Color::srgb(0.8, 0.8, 0.85);
const HINT: &str = "Arrows / d-pad: choose   Enter / A: select, change   Esc / B: back";

pub fn setup(mut cmds: Commands) {
    cmds.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        GlobalZIndex(20), // above the fade layer (10), below the frame-time overlay (30)
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)), // dims the busy title pictures and the frozen game
        Visibility::Hidden,
        MenuPanel,
    ))
    .with_children(|p| {
        p.spawn((
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::axes(Val::VMin(4.0), Val::VMin(2.5)), row_gap: Val::VMin(0.6), max_width: Val::Percent(96.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
        ))
        .with_children(|p| {
            for i in 0..LINES {
                // sizes relative to the window (legible at any size)
                let size = match i {
                    0 => FontSize::VMin(5.0),
                    _ => FontSize::VMin(3.3),
                };
                p.spawn((Text::new(""), TextFont::from_font_size(size), TextColor(Color::WHITE), TextLayout::justify(Justify::Center), MenuLine(i)));
            }
        });
    });
}

fn read_nav(k: &ButtonInput<KeyCode>, pads: &Query<&Gamepad>) -> Option<Nav> {
    use GamepadButton as B;
    use KeyCode::*;
    let key = |c: &[KeyCode]| k.any_just_pressed(c.iter().copied());
    let pad = |b: &[GamepadButton]| pads.iter().any(|p| b.iter().any(|&x| p.just_pressed(x)));
    if key(&[ArrowUp, Numpad8]) || pad(&[B::DPadUp]) {
        Some(Nav::Up)
    } else if key(&[ArrowDown, Numpad2]) || pad(&[B::DPadDown]) {
        Some(Nav::Down)
    } else if key(&[ArrowLeft, Numpad4]) || pad(&[B::DPadLeft]) {
        Some(Nav::Left)
    } else if key(&[ArrowRight, Numpad6]) || pad(&[B::DPadRight]) {
        Some(Nav::Right)
    } else if key(&[Enter, NumpadEnter, Space, Numpad5]) || pad(&[B::South]) {
        Some(Nav::Enter)
    } else if key(&[Escape, Backspace]) || pad(&[B::East, B::Start]) {
        Some(Nav::Back)
    } else {
        None
    }
}

/// Opens, drives and closes the menus; pauses and resumes the game.
#[allow(clippy::too_many_arguments)]
pub fn input(
    k: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    state: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    mut ui: ResMut<MenuUi>,
    f: Res<crate::flow::Flow>,
    g: Res<Game>,
    mut set: ResMut<Settings>,
    mut live: (ResMut<RenderMode>, ResMut<CharacterStyle>, Query<&mut Window, With<PrimaryWindow>>),
    mut audio: ResMut<AudioState>,
    mut sinks: Query<&mut AudioSink, With<Sfx>>,
    mut pending: Query<&mut PlaybackSettings, (With<Sfx>, Without<AudioSink>)>,
) {
    ui.consumed = false;
    let open = k.just_pressed(KeyCode::Escape) || pads.iter().any(|p| p.just_pressed(GamepadButton::Start));
    match state.get() {
        Screen::Playing => {
            if open || k.just_pressed(KeyCode::KeyP) {
                // the fixed step only runs in Playing, so the world stays frozen while paused
                let can_save = pop_core::sim::SAVE_LEVELS.contains(&g.level) && g.world.exit.is_none() && !g.world.demo;
                (ui.menu, ui.note) = (Some(Menu::new(Ctx::Pause, can_save, false)), String::new());
                next.set(Screen::Paused);
            }
            return;
        }
        Screen::Paused if k.just_pressed(KeyCode::KeyP) => {
            ui.menu = None;
            return next.set(Screen::Playing);
        }
        Screen::Paused => {}
        Screen::Title if f.menu_allowed() => {
            if ui.menu.is_none() {
                if open {
                    (ui.menu, ui.note, ui.consumed) = (Some(Menu::new(Ctx::Title, false, crate::flow::save_path().is_file())), String::new(), true);
                }
                return;
            }
            ui.consumed = true;
        }
        _ => return,
    }
    let Some(n) = read_nav(&k, &pads) else { return };
    let Some(m) = ui.menu.as_mut() else { return };
    let act = m.nav(n, &mut set);
    let ctx = m.ctx;
    ui.note.clear();
    match act {
        Some(Action::Close) => {
            ui.menu = None;
            if ctx == Ctx::Pause {
                next.set(Screen::Playing);
            }
        }
        Some(Action::Changed(i)) => {
            apply(OPTS[i].key, &set, &mut live, &mut audio, &mut sinks, &mut pending);
            set.save();
        }
        a => ui.action = a,
    }
}

/// Settings that take effect at once (the same as F5, F6, F11, M and - / =); the others are read at startup.
fn apply(
    key: &str,
    set: &Settings,
    (mode, style, win): &mut (ResMut<RenderMode>, ResMut<CharacterStyle>, Query<&mut Window, With<PrimaryWindow>>),
    audio: &mut AudioState,
    sinks: &mut Query<&mut AudioSink, With<Sfx>>,
    pending: &mut Query<&mut PlaybackSettings, (With<Sfx>, Without<AudioSink>)>,
) {
    match key {
        "render_mode" | "character_style" => {
            let (m, s) = crate::settings::resolve_render(&set.render_mode, &set.character_style, None, None);
            (**mode, **style) = (m, s);
        }
        "window" => {
            if let Ok(mut w) = win.single_mut() {
                w.mode = if set.borderless() { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
            }
        }
        "volume" | "muted" => {
            (audio.volume, audio.muted) = (set.volume, set.muted);
            crate::audio::apply_gain(audio, sinks, pending);
        }
        _ => {} // camera_scroll is read on every room change; the rest after a restart
    }
}

/// Fills the panel for the open menu (title menu on the title, pause menu while paused).
pub fn draw(
    ui: Res<MenuUi>,
    state: Res<State<Screen>>,
    set: Res<Settings>,
    hof: Res<Hof>,
    mut panel: Query<&mut Visibility, With<MenuPanel>>,
    mut lines: Query<(&MenuLine, &mut Text, &mut TextColor, &mut Node)>,
) {
    let m = ui.menu.as_ref().filter(|m| matches!((m.ctx, state.get()), (Ctx::Title, Screen::Title) | (Ctx::Pause, Screen::Paused)));
    for mut v in &mut panel {
        v.set_if_neq(if m.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }
    let Some(m) = m else { return };
    let mut v: Vec<(String, Color)> = vec![(m.title().into(), GOLD)];
    match m.page {
        Page::Controls => v.extend(crate::menu::controls(&set.keys).into_iter().map(|l| (l, GREY))),
        Page::Hof if hof.entries.is_empty() => v.push(("No entries yet: finish the game to get one.".into(), GREY)),
        Page::Hof => v.extend(hof.entries.iter().enumerate().map(|(i, e)| (format!("{}.  {}   {}", i + 1, e.name, e.time_text()), Color::WHITE))),
        _ => {}
    }
    for (i, it) in m.items().into_iter().enumerate() {
        let sel = i == m.sel;
        v.push((format!("{}{}", if sel { "> " } else { "" }, m.label(it, &set)), if sel { GOLD } else { Color::WHITE }));
    }
    v.push((if ui.note.is_empty() { HINT.into() } else { ui.note.clone() }, GREY));
    for (MenuLine(i), mut t, mut c, mut n) in &mut lines {
        let (s, col) = v.get(*i).cloned().unwrap_or_default();
        let display = if *i < v.len() { Display::Flex } else { Display::None };
        if n.display != display {
            n.display = display;
        }
        if t.0 != s {
            t.0 = s;
        }
        c.set_if_neq(TextColor(col));
    }
}
