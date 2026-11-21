//! The menus on screen (M6): reads keys, mouse and controller for menu.rs, applies live settings, draws the panel.
//! Esc / Start opens the title menu (the attract loop holds while it is open) and pauses the game (the pause menu);
//! P still pauses and resumes directly. Actions that change the game go to `flow::menu_action`.
//!
//! The panel is a pool of widget rows (label, badge, slider, checkbox, progress bar, value text, buttons, drop-down popup)
//! that `draw` fills from the menu model each frame; `mouse` hit-tests the same nodes (`Part`) with the window's cursor.
use crate::audio::{AudioState, Sfx};
use crate::flow::Screen;
use crate::hof::Hof;
use crate::menu::{self, Action, Ctl, Ctx, Menu, Nav, Page, Row, OPTS, ROWS, TABS};
use crate::settings::{CharacterStyle, RenderMode, Settings};
use crate::soundfont;
use crate::Game;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
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
    /// pool slot of the slider being dragged
    drag: Option<usize>,
    /// the cursor of the last frame (hover only follows a moving mouse, so the keyboard keeps its focus)
    cursor: Option<Vec2>,
}

#[derive(Component)]
pub struct MenuPanel;

/// One line of the Controls / Hall of fame pages.
#[derive(Component)]
pub struct MenuLine(usize);

/// A node of the widget pool: the pool slot (tab number for tabs) and what it is.
#[derive(Component, Clone, Copy)]
pub struct Part(usize, P);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum P {
    Card,
    Title,
    Hint,
    TabBar,
    Tab,
    TabText,
    Row,
    Left,
    Label,
    Sub,
    Badge,
    Field,
    Value,
    Caret,
    Track,
    Fill,
    Thumb,
    Check,
    Tick,
    Bar,
    BarFill,
    Btn(usize),
    BtnText(usize),
    Popup,
    Opt(usize),
    OptText(usize),
}

/// The panel has one fixed size, that of the largest page.
const CARD_W: f32 = 112.0;
const CARD_H: f32 = 85.9; // the Graphics tab (measured: 618 px at 720 px window height)
const INFO: usize = 16;
const GOLD: Color = Color::srgb(1.0, 0.85, 0.5);
const GREY: Color = Color::srgb(0.8, 0.8, 0.85);
const DIM: Color = Color::srgb(0.5, 0.5, 0.58);
const FOCUS_BG: Color = Color::srgba(1.0, 0.85, 0.5, 0.13);
const TRACK_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.18);
const BTN_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);
const POPUP_BG: Color = Color::srgb(0.07, 0.07, 0.1);
const BLACK: Color = Color::BLACK;

/// The menu font: Fixedsys, as a fallback Consolas and any monospace. Fixedsys itself is a Windows bitmap font that Bevy cannot
/// read; it is used when a vector font of that name (e.g. Fixedsys Excelsior) is installed.
fn font(size: f32) -> TextFont {
    TextFont { font: FontSource::Families("Fixedsys, Fixedsys Excelsior, Consolas, monospace".into()), font_size: FontSize::VMin(size), ..default() }
}

fn txt(p: &mut ChildSpawnerCommands, part: Part, size: f32, col: Color, node: Node) {
    p.spawn((Text::new(""), font(size), TextColor(col), BorderColor::all(Color::NONE), if matches!(part.1, P::Value | P::Caret | P::Badge | P::BtnText(_) | P::OptText(_)) { TextLayout::no_wrap() } else { TextLayout::default() }, node, part));
}

fn boxed(part: Part, node: Node, bg: Color) -> impl Bundle {
    (node, BackgroundColor(bg), BorderColor::all(Color::NONE), part)
}

pub fn setup(mut cmds: Commands) {
    let vm = Val::VMin;
    cmds.spawn((
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        GlobalZIndex(20), // above the fade layer (10), below the frame-time overlay (30)
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)), // dims the busy title pictures and the frozen game
        Visibility::Hidden,
        MenuPanel,
    ))
    .with_children(|p| {
        p.spawn((
            Node { flex_direction: FlexDirection::Column, width: vm(CARD_W), height: vm(CARD_H), max_width: Val::Percent(96.0), padding: UiRect::axes(vm(3.0), vm(2.2)), row_gap: vm(0.7), border: UiRect::all(Val::Px(2.0)), ..default() },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.88)),
            BorderColor::all(Color::srgba(1.0, 0.85, 0.5, 0.35)),
            Part(0, P::Card),
        ))
        .with_children(|p| {
            txt(p, Part(0, P::Title), 5.0, GOLD, Node { align_self: AlignSelf::Center, margin: UiRect::bottom(vm(0.8)), ..default() });
            // tab strip
            p.spawn((Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::Center, column_gap: vm(1.0), margin: UiRect::bottom(vm(0.8)), ..default() }, Part(0, P::TabBar))).with_children(|p| {
                for (k, t) in TABS.iter().enumerate() {
                    p.spawn(boxed(Part(k, P::Tab), Node { padding: UiRect::axes(vm(2.2), vm(0.8)), border: UiRect::all(Val::Px(2.0)), ..default() }, BTN_BG)).with_children(|p| {
                        p.spawn((Text::new(t.name()), font(3.0), TextColor(GREY), Part(k, P::TabText)));
                    });
                }
            });
            for i in 0..INFO {
                p.spawn((Text::new(""), font(2.6), TextColor(GREY), TextLayout::justify(Justify::Center), MenuLine(i)));
            }
            for slot in 0..ROWS {
                p.spawn((
                    boxed(Part(slot, P::Row), Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, padding: UiRect::axes(vm(1.6), vm(0.7)), border: UiRect::all(Val::Px(2.0)), column_gap: vm(2.0), min_height: vm(5.4), ..default() }, Color::NONE),
                    ZIndex(0),
                ))
                .with_children(|p| {
                    p.spawn((Node { flex_direction: FlexDirection::Column, width: Val::Percent(44.0), ..default() }, Part(slot, P::Left))).with_children(|p| {
                        txt(p, Part(slot, P::Label), 3.3, Color::WHITE, Node::default());
                        txt(p, Part(slot, P::Sub), 2.1, GREY, Node::default());
                    });
                    p.spawn(Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd, align_items: AlignItems::Center, column_gap: vm(1.4), ..default() }).with_children(|p| {
                        txt(p, Part(slot, P::Badge), 1.9, DIM, Node { padding: UiRect::axes(vm(0.8), vm(0.2)), border: UiRect::all(Val::Px(1.0)), ..default() });
                        p.spawn(boxed(Part(slot, P::Bar), Node { width: vm(16.0), height: vm(1.8), border: UiRect::all(Val::Px(1.0)), ..default() }, TRACK_BG)).with_children(|p| {
                            p.spawn(boxed(Part(slot, P::BarFill), Node { height: Val::Percent(100.0), width: Val::Percent(0.0), ..default() }, GOLD));
                        });
                        p.spawn(boxed(Part(slot, P::Track), Node { width: vm(26.0), height: vm(1.2), margin: UiRect::axes(vm(0.9), vm(0.0)), ..default() }, TRACK_BG)).with_children(|p| {
                            p.spawn(boxed(Part(slot, P::Fill), Node { height: Val::Percent(100.0), width: Val::Percent(0.0), ..default() }, GOLD));
                            p.spawn(boxed(Part(slot, P::Thumb), Node { position_type: PositionType::Absolute, width: vm(1.8), height: vm(3.0), top: vm(-0.9), left: Val::Percent(0.0), margin: UiRect::left(vm(-0.9)), ..default() }, Color::WHITE));
                        });
                        p.spawn(boxed(Part(slot, P::Check), Node { width: vm(3.2), height: vm(3.2), border: UiRect::all(Val::Px(2.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, TRACK_BG)).with_children(|p| {
                            p.spawn(boxed(Part(slot, P::Tick), Node { width: vm(1.9), height: vm(1.9), ..default() }, GOLD));
                        });
                        // value box: the text left, the drop-down's "v" at the right edge
                        p.spawn((Node { min_width: vm(9.0), padding: UiRect::axes(vm(1.2), vm(0.4)), border: UiRect::all(Val::Px(2.0)), justify_content: JustifyContent::SpaceBetween, column_gap: vm(1.2), ..default() }, BorderColor::all(Color::NONE), Part(slot, P::Field))).with_children(|p| {
                            txt(p, Part(slot, P::Value), 3.0, Color::WHITE, Node::default());
                            txt(p, Part(slot, P::Caret), 3.0, Color::WHITE, Node::default());
                        });
                        for b in 0..2 {
                            p.spawn(boxed(Part(slot, P::Btn(b)), Node { padding: UiRect::axes(vm(1.8), vm(0.6)), border: UiRect::all(Val::Px(2.0)), ..default() }, BTN_BG)).with_children(|p| {
                                txt(p, Part(slot, P::BtnText(b)), 2.8, Color::WHITE, Node::default());
                            });
                        }
                    });
                    // drop-down list, opens below the row
                    p.spawn((
                        Node { position_type: PositionType::Absolute, right: vm(1.6), top: Val::Percent(100.0), flex_direction: FlexDirection::Column, min_width: vm(30.0), border: UiRect::all(Val::Px(2.0)), padding: UiRect::all(vm(0.4)), display: Display::None, ..default() },
                        BackgroundColor(POPUP_BG),
                        BorderColor::all(GOLD),
                        Part(slot, P::Popup),
                    ))
                    .with_children(|p| {
                        for k in 0..4 {
                            p.spawn(boxed(Part(slot, P::Opt(k)), Node { padding: UiRect::axes(vm(1.6), vm(0.7)), ..default() }, Color::NONE)).with_children(|p| {
                                txt(p, Part(slot, P::OptText(k)), 3.0, Color::WHITE, Node::default());
                            });
                        }
                    });
                });
            }
            txt(p, Part(0, P::Hint), 2.1, GREY, Node { align_self: AlignSelf::Center, margin: UiRect::top(vm(0.8)), ..default() });
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
    } else if key(&[PageUp, KeyQ]) || pad(&[B::LeftTrigger]) {
        Some(Nav::PrevTab)
    } else if key(&[PageDown, KeyE]) || pad(&[B::RightTrigger]) {
        Some(Nav::NextTab)
    } else {
        None
    }
}

/// What can change at once (the same as F5, F6, F11, M and - / =).
#[derive(SystemParam)]
pub struct Live<'w, 's> {
    mode: ResMut<'w, RenderMode>,
    style: ResMut<'w, CharacterStyle>,
    win: Query<'w, 's, &'static mut Window, With<PrimaryWindow>>,
    audio: ResMut<'w, AudioState>,
    sinks: Query<'w, 's, &'static mut AudioSink, With<Sfx>>,
    pending: Query<'w, 's, &'static mut PlaybackSettings, (With<Sfx>, Without<AudioSink>)>,
}

impl Live<'_, '_> {
    /// Settings that take effect at once; the others are read at startup.
    fn apply(&mut self, key: &str, set: &Settings) {
        match key {
            "render_mode" | "character_style" => {
                let (m, s) = crate::settings::resolve_render(&set.render_mode, &set.character_style, None, None);
                (*self.mode, *self.style) = (m, s);
            }
            "window" => {
                if let Ok(mut w) = self.win.single_mut() {
                    w.mode = if set.borderless() { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
                }
            }
            "external_terminal" => crate::flow::console(set.external_terminal),
            "volume" | "muted" => {
                (self.audio.volume, self.audio.muted) = (set.volume, set.muted);
                crate::audio::apply_gain(&self.audio, &mut self.sinks, &mut self.pending);
            }
            _ => {} // camera_scroll is read on every room change, motion_smoothing every frame; the rest after a restart
        }
    }
}

/// Carries out what a menu step returned (keys and mouse alike).
fn dispatch(act: Option<Action>, ui: &mut MenuUi, set: &mut Settings, live: &mut Live, next: &mut NextState<Screen>) {
    // `mouse` runs after `input` every frame, usually with nothing to do: it must not wipe the action or note `input` just set
    let Some(act) = act else { return };
    let Some(ctx) = ui.menu.as_ref().map(|m| m.ctx) else { return };
    ui.note.clear();
    match Some(act) {
        Some(Action::Close) => {
            ui.menu = None;
            if ctx == Ctx::Pause {
                next.set(Screen::Playing);
            }
        }
        Some(Action::Changed(i)) => {
            live.apply(OPTS[i].key, set);
            set.save();
        }
        Some(Action::Deleted(i)) => ui.note = menu::deleted_note(i, set),
        a => ui.action = a,
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
    mut live: Live,
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
    dispatch(act, &mut ui, &mut set, &mut live, &mut next);
}

/// What the hit test knows of a pool node: priority (drop-down entries first) and where the cursor is inside it.
fn hit_priority(p: P) -> Option<u8> {
    match p {
        P::Opt(_) => Some(5),
        P::Btn(_) => Some(4),
        P::Track => Some(3),
        P::Tab => Some(2),
        P::Row => Some(1),
        _ => None,
    }
}

/// Mouse: hover focuses the row under a moving cursor, click presses, the slider drags, the wheel scrolls, a click outside
/// an open drop-down closes it.
#[allow(clippy::too_many_arguments)]
pub fn mouse(
    mb: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    state: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
    mut ui: ResMut<MenuUi>,
    mut set: ResMut<Settings>,
    mut live: Live,
    parts: Query<(&Part, &ComputedNode, &UiGlobalTransform)>,
) {
    let dy: f32 = wheel.read().map(|e| e.y).sum(); // always drain
    let shown = ui.menu.as_ref().is_some_and(|m| matches!((m.ctx, state.get()), (Ctx::Title, Screen::Title) | (Ctx::Pause, Screen::Paused)));
    if !shown {
        ui.drag = None;
        return;
    }
    let cur = live.win.single().ok().and_then(|w| w.physical_cursor_position());
    let ui = &mut *ui;
    let moved = cur != ui.cursor;
    ui.cursor = cur;
    let (click, held) = (mb.just_pressed(MouseButton::Left), mb.pressed(MouseButton::Left));
    let Some(m) = ui.menu.as_mut() else { return };
    let rows = m.rows();
    // which pool nodes exist right now
    let valid = |p: Part| -> bool {
        let row = m.slot_idx(p.0).and_then(|i| rows.get(i).copied());
        match p.1 {
            P::Tab => m.page == Page::Settings,
            P::Row => row.is_some(),
            P::Track => matches!(row, Some(Row::Opt(i)) if matches!(OPTS[i].ctl, Ctl::Slider { .. } | Ctl::Steps)),
            P::Btn(b) => matches!(row, Some(Row::Font(i)) if b < menu::font_buttons(&soundfont::status(i)).len()),
            P::Opt(k) => m.open.is_some() && m.slot_idx(p.0).is_some_and(|i| i == m.sel) && matches!(row, Some(Row::Opt(i)) if k < OPTS[i].values.len()),
            _ => false,
        }
    };
    let (mut best, mut drag_x): (Option<(u8, Part)>, Option<f32>) = (None, None);
    if let Some(c) = cur {
        for (&p, cn, tf) in &parts {
            if ui.drag == Some(p.0) && p.1 == P::Track {
                drag_x = cn.normalize_point(*tf, c).map(|v| v.x + 0.5);
            }
            let Some(pr) = hit_priority(p.1) else { continue };
            if valid(p) && cn.contains_point(*tf, c) && best.is_none_or(|b| pr > b.0) {
                best = Some((pr, p));
            }
        }
    }
    let was_open = m.open.is_some();
    let mut act = None;
    if let Some(slot) = ui.drag {
        if held {
            if let (Some(x), Some(i)) = (drag_x, m.slot_idx(slot)) {
                act = m.slide(i, x, &mut set);
            }
        } else {
            ui.drag = None;
        }
    } else if let Some((_, p)) = best {
        let idx = m.slot_idx(p.0);
        match (p.1, idx) {
            (P::Opt(k), _) => {
                if moved {
                    m.open = Some(k);
                }
                if click {
                    act = m.pick(k, &mut set);
                }
            }
            _ if was_open => {
                if click {
                    m.open = None; // a click elsewhere closes the list
                }
            }
            (P::Tab, _) => {
                if click {
                    m.focus(0);
                    m.set_tab(p.0);
                }
            }
            (P::Btn(b), Some(i)) => {
                if moved {
                    m.focus(i);
                }
                if click {
                    act = m.press(i, b, &mut set);
                }
            }
            (P::Track, Some(i)) => {
                if moved || click {
                    m.focus(i);
                }
                if click {
                    ui.drag = Some(p.0);
                    act = m.slide(i, drag_x.unwrap_or(0.0), &mut set);
                }
            }
            (P::Row, Some(i)) => {
                if moved {
                    m.focus(i);
                }
                if click && !matches!(rows[i], Row::Font(_)) {
                    act = m.press(i, 0, &mut set);
                }
            }
            _ => {}
        }
    } else if click && was_open {
        m.open = None;
    }
    if dy != 0.0 && !was_open {
        m.scroll(-(dy.signum() as i32));
    }
    dispatch(act, ui, &mut set, &mut live, &mut next);
}

/// What one pool row shows.
#[derive(Default, Clone)]
struct View {
    focus: bool,
    centered: bool,
    label: String,
    sub: String,
    sub_warn: bool,
    /// the version line: quieter than the buttons
    dim: bool,
    badge: bool,
    value: String,
    /// the value is a drop-down box
    drop: bool,
    /// download row: a narrower label column
    wide: bool,
    frac: Option<f32>,
    check: Option<bool>,
    bar: Option<f32>,
    btns: Vec<&'static str>,
    btn_focus: Option<usize>,
    /// open drop-down: entries, highlighted, current
    popup: Option<(Vec<String>, usize, usize)>,
}

fn view(m: &Menu, idx: usize, row: Row, set: &Settings) -> View {
    let focus = idx == m.sel;
    match row {
        Row::Item(it) => View { focus, centered: true, dim: it == menu::Item::Version, label: it.label().into(), ..default() },
        Row::Tabs => View::default(),
        Row::Opt(i) => {
            let o = &OPTS[i];
            let cur = menu::value(set, o.key);
            let mut v = View { focus, label: o.label.into(), badge: !o.live, value: menu::opt_value(set, i), ..default() };
            match o.ctl {
                Ctl::Slider { .. } | Ctl::Steps => v.frac = Some(menu::frac(set, i)),
                Ctl::Checkbox => (v.check, v.value) = (Some(cur == "true"), String::new()),
                Ctl::Dropdown => {
                    v.drop = true;
                    if let (true, Some(h)) = (focus, m.open) {
                        v.popup = Some((o.values.iter().map(|x| menu::entry_label(o.key, x)).collect(), h, o.values.iter().position(|x| *x == cur).unwrap_or(usize::MAX)));
                    }
                }
            }
            if o.key == "soundfont" && set.sound == "soundfont" && soundfont::index_of(&set.soundfont).is_some_and(|f| soundfont::status(f) == soundfont::Status::Absent) {
                (v.sub, v.sub_warn) = ("Not downloaded yet: see the Downloads tab. Music uses AdLib until then.".into(), true);
            }
            v
        }
        Row::Font(i) => {
            let fi = menu::font_info(i, set, &soundfont::status(i));
            let nb = fi.btns.len();
            View { focus, wide: true, label: fi.title, sub: fi.sub, sub_warn: fi.warn, value: fi.status, bar: fi.bar, btns: fi.btns.iter().map(|b| b.label()).collect(), btn_focus: focus.then(|| m.sub.min(nb - 1)), ..default() }
        }
    }
}

fn disp(n: &mut Node, on: bool) {
    let d = if on { Display::Flex } else { Display::None };
    if n.display != d {
        n.display = d;
    }
}

type PartItem = (&'static Part, &'static mut Node, Option<&'static mut BackgroundColor>, Option<&'static mut BorderColor>, Option<&'static mut Text>, Option<&'static mut TextColor>, Option<&'static mut ZIndex>);

/// Fills the panel for the open menu (title menu on the title, pause menu while paused).
#[allow(clippy::type_complexity)]
pub fn draw(
    ui: Res<MenuUi>,
    state: Res<State<Screen>>,
    set: Res<Settings>,
    hof: Res<Hof>,
    mut panel: Query<&mut Visibility, With<MenuPanel>>,
    mut lines: Query<(&MenuLine, &mut Text, &mut Node), Without<Part>>,
    mut parts: Query<PartItem>,
) {
    let m = ui.menu.as_ref().filter(|m| matches!((m.ctx, state.get()), (Ctx::Title, Screen::Title) | (Ctx::Pause, Screen::Paused)));
    for mut v in &mut panel {
        v.set_if_neq(if m.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }
    let Some(m) = m else { return };
    // info lines (Controls, Hall of fame)
    let info: Vec<String> = match m.page {
        Page::Controls => menu::controls(&set.keys),
        Page::Hof if hof.entries.is_empty() => vec!["No entries yet: finish the game to get one.".into()],
        Page::Hof => hof.entries.iter().enumerate().map(|(i, e)| format!("{}.  {}   {}", i + 1, e.name, e.time_text())).collect(),
        _ => vec![],
    };
    for (MenuLine(i), mut t, mut n) in &mut lines {
        disp(&mut n, *i < info.len());
        let s = info.get(*i).map_or("", |s| s.as_str());
        if t.0 != s {
            t.0 = s.into();
        }
    }
    let views: Vec<View> = m.visible().into_iter().map(|(idx, row)| view(m, idx, row, &set)).collect();
    let on_tabs = m.page == Page::Settings;
    let tabs_focus = on_tabs && m.sel == 0;
    let width = CARD_W;
    let hint = ui.note.clone();
    for (&Part(slot, p), mut n, bg, border, text, tcol, z) in &mut parts {
        let v = views.get(slot);
        let (mut set_bg, mut set_border, mut set_text, mut set_col) = (None, None, None, None);
        match p {
            P::Card => {
                let w = Val::VMin(width);
                if n.width != w {
                    n.width = w;
                }
            }
            P::Title => set_text = Some(m.title().to_string()),
            P::Hint => {
                disp(&mut n, !ui.note.is_empty()); // the key hints are gone; only notes ("GAME SAVED") show
                (set_text, set_col) = (Some(hint.clone()), Some(GOLD));
            }
            P::TabBar => disp(&mut n, on_tabs),
            P::Tab => {
                let active = TABS[slot] == m.tab;
                set_bg = Some(if active { GOLD } else { BTN_BG });
                set_border = Some(if tabs_focus && active { Color::WHITE } else if active { GOLD } else { Color::NONE });
            }
            P::TabText => set_col = Some(if TABS[slot] == m.tab { BLACK } else { GREY }),
            P::Row => {
                disp(&mut n, v.is_some());
                let v = v.cloned().unwrap_or_default();
                set_bg = Some(if v.focus { FOCUS_BG } else { Color::NONE });
                set_border = Some(if v.focus { GOLD } else { Color::NONE });
                let j = if v.centered { JustifyContent::Center } else { JustifyContent::SpaceBetween };
                if n.justify_content != j {
                    n.justify_content = j;
                }
                if let Some(mut z) = z {
                    let want = ZIndex(v.popup.is_some() as i32);
                    if *z != want {
                        *z = want;
                    }
                }
            }
            _ => {
                let Some(v) = v else { continue };
                match p {
                    P::Left => {
                        let w = if v.centered { Val::Percent(100.0) } else { Val::Percent(if v.wide { 36.0 } else { 44.0 }) };
                        if n.width != w {
                            n.width = w;
                        }
                        let a = if v.centered { AlignItems::Center } else { AlignItems::Stretch };
                        if n.align_items != a {
                            n.align_items = a;
                        }
                    }
                    P::Label => (set_text, set_col) = (Some(v.label.clone()), Some(if v.focus { GOLD } else if v.dim { GREY } else { Color::WHITE })),
                    P::Sub => {
                        disp(&mut n, !v.sub.is_empty());
                        (set_text, set_col) = (Some(v.sub.clone()), Some(if v.sub_warn { GOLD } else { GREY }));
                    }
                    P::Badge => {
                        disp(&mut n, v.badge);
                        (set_text, set_border) = (Some("after restart".into()), Some(DIM));
                    }
                    P::Field => {
                        disp(&mut n, !v.value.is_empty());
                        let w = Val::VMin(if v.drop { 24.0 } else { 9.0 });
                        if n.min_width != w {
                            n.min_width = w;
                        }
                        set_border = Some(if v.drop { if v.focus { GOLD } else { DIM } } else { Color::NONE });
                    }
                    P::Value => (set_text, set_col) = (Some(v.value.clone()), Some(if v.focus { GOLD } else { Color::WHITE })),
                    P::Caret => {
                        disp(&mut n, v.drop);
                        (set_text, set_col) = (Some("v".into()), Some(if v.focus { GOLD } else { Color::WHITE }));
                    }
                    P::Track => {
                        disp(&mut n, v.frac.is_some());
                        set_bg = Some(if v.focus { Color::srgba(1.0, 1.0, 1.0, 0.28) } else { TRACK_BG });
                    }
                    P::Fill | P::Thumb | P::BarFill => {
                        let f = if p == P::BarFill { v.bar } else { v.frac }.unwrap_or(0.0) * 100.0;
                        let w = Val::Percent(f);
                        if p == P::Thumb {
                            if n.left != w {
                                n.left = w;
                            }
                            set_bg = Some(if v.focus { GOLD } else { Color::WHITE });
                        } else if n.width != w {
                            n.width = w;
                        }
                    }
                    P::Check => {
                        disp(&mut n, v.check.is_some());
                        set_border = Some(if v.focus { GOLD } else { GREY });
                    }
                    P::Tick => disp(&mut n, v.check == Some(true)),
                    P::Bar => {
                        disp(&mut n, v.bar.is_some());
                        set_border = Some(GREY);
                    }
                    P::Btn(b) => {
                        disp(&mut n, b < v.btns.len());
                        let f = v.btn_focus == Some(b);
                        set_bg = Some(if f { GOLD } else { BTN_BG });
                        set_border = Some(if f { Color::WHITE } else { GREY });
                    }
                    P::BtnText(b) => (set_text, set_col) = (v.btns.get(b).map(|s| s.to_string()), Some(if v.btn_focus == Some(b) { BLACK } else { Color::WHITE })),
                    P::Popup => disp(&mut n, v.popup.is_some()),
                    P::Opt(k) => {
                        let on = v.popup.as_ref().is_some_and(|p| k < p.0.len());
                        disp(&mut n, on);
                        set_bg = Some(if v.popup.as_ref().is_some_and(|p| p.1 == k) { GOLD } else { Color::NONE });
                    }
                    P::OptText(k) => {
                        if let Some((items, hi, cur)) = &v.popup {
                            let mark = if *cur == k { "* " } else { "  " };
                            (set_text, set_col) = (items.get(k).map(|s| format!("{mark}{s}")), Some(if *hi == k { BLACK } else { Color::WHITE }));
                        }
                    }
                    _ => {}
                }
            }
        }
        if let (Some(c), Some(mut b)) = (set_bg, bg) {
            b.set_if_neq(BackgroundColor(c));
        }
        if let (Some(c), Some(mut b)) = (set_border, border) {
            b.set_if_neq(BorderColor::all(c));
        }
        if let (Some(s), Some(mut t)) = (set_text, text) {
            if t.0 != s {
                t.0 = s;
            }
        }
        if let (Some(c), Some(mut t)) = (set_col, tcol) {
            t.set_if_neq(TextColor(c));
        }
    }
}
