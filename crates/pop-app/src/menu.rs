//! Menu logic (M6), no Bevy: pages, tabs, widgets, focus and settings editing. menu_ui.rs reads keys, mouse and controller and
//! draws it (hit testing is there), flow.rs carries out the actions. The title menu opens with Esc / Start, the pause menu
//! whenever the game is paused.
//!
//! Model: a page is a list of `Row`s; `sel` is the focused row, `sub` the focused button inside a download row, `open` the
//! highlighted entry of an open drop-down. The Settings page has a tab row (row 0) and one tab of option rows each; every
//! option (`Opt`) is drawn as a slider, a checkbox or a drop-down (`Ctl`).
use crate::settings::{self, Settings};
use crate::soundfont::{self, Status};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ctx {
    Title,
    Pause,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Main,
    Settings,
    Controls,
    Hof,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Video,
    Audio,
    Graphics,
    Downloads,
    /// the tabs of the Controls page
    Keyboard,
    Controller,
}

pub const TABS: [Tab; 4] = [Tab::Video, Tab::Audio, Tab::Graphics, Tab::Downloads];
pub const CONTROL_TABS: [Tab; 2] = [Tab::Keyboard, Tab::Controller];

impl Tab {
    pub fn name(self) -> &'static str {
        match self {
            Tab::Video => "Video",
            Tab::Audio => "Audio",
            Tab::Graphics => "Graphics",
            Tab::Downloads => "Downloads",
            Tab::Keyboard => "Keyboard",
            Tab::Controller => "Controller",
        }
    }
}

/// The plain buttons of the main / pause menu, and Back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Start,
    Load,
    Resume,
    Restart,
    Save,
    Settings,
    Controls,
    Hof,
    QuitTitle,
    Quit,
    Back,
    /// the installed version at the bottom of the main menu; opens the GitHub release page
    Version,
}

impl Item {
    pub fn label(self) -> &'static str {
        match self {
            Item::Start => "Start game",
            Item::Load => "Load game",
            Item::Resume => "Resume",
            Item::Restart => "Restart level",
            Item::Save => "Save game",
            Item::Settings => "Settings",
            Item::Controls => "Controls",
            Item::Hof => "Hall of fame",
            Item::QuitTitle => "Quit to title",
            Item::Quit => "Quit game",
            Item::Back => "Back",
            Item::Version => concat!("Version ", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// One focusable line of a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    /// the tab strip of the Settings page
    Tabs,
    /// index into OPTS
    Opt(usize),
    /// a downloadable SoundFont (index into soundfont::FONTS)
    Font(usize),
    Item(Item),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
    PrevTab,
    NextTab,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// close the menu (pause: resume the game)
    Close,
    Start,
    Load,
    Restart,
    Save,
    QuitTitle,
    Quit,
    /// OPTS[i] changed in the settings: apply it and save settings.json
    Changed(usize),
    /// start downloading soundfont::FONTS[i] (the user confirmed the licence notice)
    Download(usize),
    /// soundfont::FONTS[i] was deleted
    Deleted(usize),
    /// open the project's release page in the browser
    Releases,
}

/// How an option is edited.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ctl {
    /// a number in percent (the setting is a fraction)
    Slider { min: i32, max: i32, step: i32 },
    /// ordered values on a slider (off / low / high)
    Steps,
    Checkbox,
    Dropdown,
}

/// One settings.json key: shown label, the values it takes, whether it applies at once (else after a restart).
pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub values: &'static [&'static str],
    pub live: bool,
    pub tab: Tab,
    pub ctl: Ctl,
}

const ON_OFF: &[&str] = &["true", "false"];
const PERCENT: Ctl = Ctl::Slider { min: 0, max: 100, step: 5 };

pub const OPTS: [Opt; 25] = [
    Opt { key: "render_mode", label: "Rendering", values: &["classic", "enhanced"], live: true, tab: Tab::Video, ctl: Ctl::Dropdown },
    Opt { key: "character_style", label: "Characters", values: &settings::CHARACTER_STYLES, live: true, tab: Tab::Video, ctl: Ctl::Dropdown },
    Opt { key: "camera_scroll", label: "Room change", values: &settings::CAMERA_SCROLLS, live: true, tab: Tab::Video, ctl: Ctl::Dropdown },
    Opt { key: "motion_smoothing", label: "Motion smoothing", values: &settings::MOTION_SMOOTHINGS, live: true, tab: Tab::Video, ctl: Ctl::Dropdown },
    Opt { key: "window", label: "Window mode", values: &["windowed", "borderless"], live: true, tab: Tab::Video, ctl: Ctl::Dropdown },
    Opt { key: "external_terminal", label: "External Terminal", values: ON_OFF, live: true, tab: Tab::Video, ctl: Ctl::Checkbox },
    Opt { key: "volume", label: "Volume", values: &[], live: true, tab: Tab::Audio, ctl: PERCENT },
    Opt { key: "muted", label: "Mute", values: ON_OFF, live: true, tab: Tab::Audio, ctl: Ctl::Checkbox },
    Opt { key: "sound", label: "Sound set", values: &["synth", "original", "adlib", "soundfont"], live: true, tab: Tab::Audio, ctl: Ctl::Dropdown },
    Opt { key: "soundfont", label: "SoundFont", values: &["musescore", "fluidr3"], live: true, tab: Tab::Audio, ctl: Ctl::Dropdown },
    Opt { key: "tile_textures", label: "Tile textures", values: &settings::TILE_TEXTURES, live: true, tab: Tab::Graphics, ctl: Ctl::Dropdown },
    Opt { key: "quick_input", label: "Quick input", values: ON_OFF, live: true, tab: Tab::Video, ctl: Ctl::Checkbox },
    Opt { key: "low_latency_present", label: "Low latency", values: ON_OFF, live: false, tab: Tab::Video, ctl: Ctl::Checkbox },
    Opt { key: "tile_relief", label: "Tile relief (MMPX)", values: ON_OFF, live: true, tab: Tab::Graphics, ctl: Ctl::Checkbox },
    Opt { key: "column_style", label: "Columns", values: &settings::COLUMN_STYLES, live: true, tab: Tab::Graphics, ctl: Ctl::Dropdown },
    Opt { key: "object_style", label: "Objects", values: &settings::OBJECT_STYLES, live: true, tab: Tab::Graphics, ctl: Ctl::Dropdown },
    Opt { key: "voxel_normals", label: "Voxel normals", values: &settings::VOXEL_NORMALS, live: true, tab: Tab::Graphics, ctl: Ctl::Dropdown },
    Opt { key: "voxel_shading", label: "Voxel shading", values: &settings::VOXEL_SHADINGS, live: true, tab: Tab::Graphics, ctl: Ctl::Dropdown },
    Opt { key: "shadow_quality", label: "Shadows (enhanced)", values: &settings::SHADOW_QUALITIES, live: true, tab: Tab::Graphics, ctl: Ctl::Steps },
    Opt { key: "sun_brightness", label: "Sun light (enhanced)", values: &settings::SUN_BRIGHTNESS, live: true, tab: Tab::Graphics, ctl: Ctl::Steps },
    Opt { key: "pad_layout", label: "Controller layout", values: &settings::PAD_LAYOUTS, live: true, tab: Tab::Controller, ctl: Ctl::Dropdown },
    Opt { key: "ssao", label: "SSAO (enhanced)", values: ON_OFF, live: true, tab: Tab::Graphics, ctl: Ctl::Checkbox },
    Opt { key: "atmosphere", label: "Atmosphere (enhanced)", values: ON_OFF, live: true, tab: Tab::Graphics, ctl: Ctl::Checkbox },
    Opt { key: "cinematic_fights", label: "Cinematic fights (enhanced)", values: ON_OFF, live: true, tab: Tab::Graphics, ctl: Ctl::Checkbox },
    Opt { key: "minimap", label: "Minimap (F8)", values: ON_OFF, live: true, tab: Tab::Graphics, ctl: Ctl::Checkbox },
];

/// The settings.json value of `key` as text ("classic", "true", "0.7").
pub fn value(s: &Settings, key: &str) -> String {
    match serde_json::to_value(s).ok().and_then(|o| o.get(key).cloned()) {
        Some(serde_json::Value::String(v)) => v,
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// Settings with `key` set to `v` (through Settings::parse, so a bad value falls back like a hand-edited file).
pub fn with_value(s: &Settings, key: &str, v: &str) -> Settings {
    let mut o = serde_json::to_value(s).expect("settings serialize");
    o[key] = serde_json::from_str(v).unwrap_or_else(|_| serde_json::Value::String(v.into()));
    Settings::parse(&o.to_string())
}

/// Index of the current value in OPTS[i].values (None for a value outside the list, e.g. rt_experimental).
fn pos(s: &Settings, i: usize) -> Option<usize> {
    let cur = value(s, OPTS[i].key);
    OPTS[i].values.iter().position(|v| *v == cur)
}

/// The slider's percent value (volume is the only one).
fn percent(s: &Settings) -> i32 {
    (s.volume * 100.0).round() as i32 // f32 sums such as 0.70000005 still land on their step
}

fn set_percent(s: &Settings, i: usize, p: i32) -> Settings {
    with_value(s, OPTS[i].key, &format!("{}", p as f32 / 100.0))
}

/// Slider position 0..1 of a slider / steps option.
pub fn frac(s: &Settings, i: usize) -> f32 {
    match OPTS[i].ctl {
        Ctl::Slider { min, max, .. } => (percent(s) - min) as f32 / (max - min) as f32,
        _ => pos(s, i).map_or(0.0, |p| p as f32 / (OPTS[i].values.len() - 1).max(1) as f32),
    }
}

/// Step OPTS[i] by `dir` (+1 / -1): sliders stop at their ends, everything else wraps; a current value outside the list
/// (rt_experimental) starts at the nearest end.
pub fn cycle(s: &Settings, i: usize, dir: i32) -> Settings {
    let o = &OPTS[i];
    if let Ctl::Slider { min, max, step } = o.ctl {
        let c = percent(s);
        let next = if dir > 0 { (c.div_euclid(step) + 1) * step } else { (c + step - 1).div_euclid(step) * step - step };
        return set_percent(s, i, next.clamp(min, max));
    }
    let n = o.values.len() as i32;
    let mut next = match pos(s, i) {
        Some(p) if o.ctl == Ctl::Steps => (p as i32 + dir).clamp(0, n - 1),
        Some(p) => (p as i32 + dir).rem_euclid(n),
        None => if dir > 0 { 0 } else { n - 1 },
    };
    // values that cannot be picked yet (a SoundFont that is not downloaded) are skipped
    for _ in 0..n {
        if available(o.key, o.values[next as usize]) {
            break;
        }
        next = (next + dir.signum()).rem_euclid(n);
    }
    if !available(o.key, o.values[next as usize]) {
        return s.clone(); // nothing to pick
    }
    with_value(s, o.key, o.values[next as usize])
}

/// Whether value `v` of option `key` can be picked: a SoundFont (and the "soundfont" sound set) only once it is downloaded.
pub fn available(key: &str, v: &str) -> bool {
    match (key, v) {
        ("soundfont", v) => crate::soundfont::index_of(v).is_some_and(crate::soundfont::available),
        ("sound", "soundfont") => (0..crate::soundfont::FONTS.len()).any(crate::soundfont::available),
        _ => true,
    }
}

/// A drop-down entry: the value, with a note when it cannot be picked yet.
pub fn entry_label(key: &str, v: &str) -> String {
    if available(key, v) { shown(key, v) } else { format!("{} (not installed)", shown(key, v)) }
}

/// What a value looks like in the menu.
pub fn shown(key: &str, v: &str) -> String {
    match (key, v) {
        ("volume", v) => format!("{}%", (v.parse::<f32>().unwrap_or(0.0) * 100.0).round()),
        ("soundfont", v) => crate::soundfont::index_of(v).map_or(v.into(), |i| crate::soundfont::FONTS[i].name.into()),
        (_, "true") => "on".into(),
        (_, "false") => "off".into(),
        (_, v) => {
            let mut c = v.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

/// The text beside OPTS[i]'s widget.
pub fn opt_value(s: &Settings, i: usize) -> String {
    shown(OPTS[i].key, &value(s, OPTS[i].key))
}

/// The buttons of a font's row for its current state (the first one is the default).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontBtn {
    Download,
    Accept,
    Decline,
    Abort,
    Delete,
    Yes,
    No,
    Retry,
}

impl FontBtn {
    pub fn label(self) -> &'static str {
        match self {
            FontBtn::Download => "Download",
            FontBtn::Accept => "Accept",
            FontBtn::Decline | FontBtn::Abort => "Cancel",
            FontBtn::Delete => "Delete",
            FontBtn::Yes => "Yes",
            FontBtn::No => "No",
            FontBtn::Retry => "Retry",
        }
    }
}

pub fn font_buttons(st: &Status) -> Vec<FontBtn> {
    match st {
        Status::Absent => vec![FontBtn::Download],
        Status::Confirm => vec![FontBtn::Accept, FontBtn::Decline],
        Status::Busy { .. } => vec![FontBtn::Abort],
        Status::Installed => vec![FontBtn::Delete],
        Status::ConfirmDelete => vec![FontBtn::Yes, FontBtn::No],
        Status::Failed(_) => vec![FontBtn::Retry],
    }
}

/// Everything the Downloads row of font `i` shows.
pub struct FontInfo {
    pub title: String,
    pub sub: String,
    /// the sub line is a notice / warning (accent colour)
    pub warn: bool,
    pub status: String,
    /// progress bar fill 0..1 while downloading
    pub bar: Option<f32>,
    pub btns: Vec<FontBtn>,
}

pub fn font_info(i: usize, s: &Settings, st: &Status) -> FontInfo {
    let f = &soundfont::FONTS[i];
    // the font chosen in the settings is gone: the sound set falls back to AdLib at once, the setting stays
    let selected = s.sound == "soundfont" && s.soundfont == f.id;
    let licence = soundfont::licence_line(i).to_string();
    let (mut sub, mut warn) = (licence, false);
    let mut bar = None;
    let status = match st {
        Status::Absent => "not installed".to_string(),
        Status::Confirm => {
            (sub, warn) = (format!("{} Downloaded from the source's server: you are responsible for respecting its licence.", sub), true);
            "licence notice".into()
        }
        Status::Busy { done, total, .. } if *total > 0 => {
            bar = Some((*done as f32 / *total as f32).clamp(0.0, 1.0));
            format!("{} / {} MB  {}%", done / 1_000_000, total / 1_000_000, (done * 100 + total / 2) / total)
        }
        Status::Busy { phase, .. } => {
            bar = Some(1.0);
            format!("{phase}...")
        }
        Status::Installed => format!("{} MB on disk", std::fs::metadata(soundfont::path(i)).map_or(f.bytes, |m| m.len()) / 1_000_000),
        Status::ConfirmDelete => {
            (sub, warn) = ("Deletes the font file and its licence text.".into(), true);
            if selected {
                sub += " It is the selected SoundFont: music uses AdLib until you download it again.";
            }
            "Delete?".into()
        }
        Status::Failed(e) => {
            (sub, warn) = (e.clone(), true);
            "failed".into()
        }
    };
    if selected && matches!(st, Status::Absent) {
        (sub, warn) = ("Selected SoundFont is missing: music uses AdLib until it is downloaded.".into(), true);
    }
    FontInfo { title: f.name.to_string(), sub: if warn { sub } else { format!("{} MB. {sub}", soundfont::size_mb(i)) }, warn, status, bar, btns: font_buttons(st) }
}

/// Visible pooled rows (the tab strip is drawn apart).
pub const ROWS: usize = 9;

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub ctx: Ctx,
    pub page: Page,
    pub tab: Tab,
    /// focused row
    pub sel: usize,
    /// focused button of a download row
    pub sub: usize,
    /// highlighted entry of the open drop-down of the focused row
    pub open: Option<usize>,
    /// scroll: first visible pooled row
    pub top: usize,
    /// pause: Ctrl+G would save here (levels 3..13)
    pub can_save: bool,
    /// title: PRINCE.SAV exists
    pub has_save: bool,
}

fn confirming_any() -> bool {
    (0..soundfont::FONTS.len()).any(|i| matches!(soundfont::status(i), Status::Confirm | Status::ConfirmDelete))
}

impl Menu {
    pub fn new(ctx: Ctx, can_save: bool, has_save: bool) -> Menu {
        Menu { ctx, page: Page::Main, tab: Tab::Video, sel: 0, sub: 0, open: None, top: 0, can_save, has_save }
    }

    pub fn title(&self) -> &'static str {
        match (self.page, self.ctx) {
            (Page::Main, Ctx::Title) => "MENU",
            (Page::Main, Ctx::Pause) => "PAUSED",
            (Page::Settings, _) => "SETTINGS",
            (Page::Controls, _) => "CONTROLS",
            (Page::Hof, _) => "HALL OF FAME",
        }
    }

    pub fn rows(&self) -> Vec<Row> {
        use Item::*;
        let items = |v: &[Item]| v.iter().copied().map(Row::Item).collect::<Vec<_>>();
        match (self.page, self.ctx) {
            (Page::Main, Ctx::Title) => items(&[Start, Load, Settings, Controls, Hof, Quit, Version]).into_iter().filter(|&r| r != Row::Item(Load) || self.has_save).collect(),
            (Page::Main, Ctx::Pause) => items(&[Resume, Restart, Save, Settings, Controls, QuitTitle, Quit, Version]).into_iter().filter(|&r| r != Row::Item(Save) || self.can_save).collect(),
            (Page::Settings, _) => {
                let body: Vec<Row> = if self.tab == Tab::Downloads { (0..soundfont::FONTS.len()).map(Row::Font).collect() } else { (0..OPTS.len()).filter(|&i| OPTS[i].tab == self.tab).map(Row::Opt).collect() };
                [Row::Tabs].into_iter().chain(body).chain([Row::Item(Back)]).collect()
            }
            (Page::Controls, _) => [Row::Tabs].into_iter().chain((0..OPTS.len()).filter(|&i| OPTS[i].tab == self.tab).map(Row::Opt)).chain(items(&[Back])).collect(),
            (Page::Hof, _) => items(&[Back]),
        }
    }

    /// Rows drawn in the pool: (row index, row), scrolled by `top`; the tab strip is not part of it.
    pub fn visible(&self) -> Vec<(usize, Row)> {
        self.rows().into_iter().enumerate().skip(self.first()).skip(self.top).take(ROWS).collect()
    }

    fn first(&self) -> usize {
        self.has_tabs() as usize
    }

    /// Settings and Controls have a tab strip.
    pub fn has_tabs(&self) -> bool {
        matches!(self.page, Page::Settings | Page::Controls)
    }

    /// The tabs of this page.
    pub fn tabs(&self) -> &'static [Tab] {
        if self.page == Page::Controls { &CONTROL_TABS } else { &TABS }
    }

    pub(crate) fn keep_visible(&mut self) {
        let (n, first) = (self.rows().len() - self.first(), self.first());
        self.top = self.top.min(n.saturating_sub(ROWS));
        if self.sel >= first {
            let p = self.sel - first;
            if p < self.top {
                self.top = p;
            } else if p >= self.top + ROWS {
                self.top = p + 1 - ROWS;
            }
        }
    }

    /// Select the tab `k` (strip click, Q / E).
    pub fn set_tab(&mut self, k: usize) {
        self.tab = self.tabs()[k % self.tabs().len()];
        (self.top, self.sub, self.open) = (0, 0, None);
        self.sel = self.sel.min(self.rows().len() - 1);
    }

    /// Move the focus to row `idx` (mouse hover); a drop-down stays open only on its own row.
    pub fn focus(&mut self, idx: usize) {
        if idx != self.sel {
            (self.sel, self.sub, self.open) = (idx.min(self.rows().len() - 1), 0, None);
        }
    }

    /// The row index of pool slot `slot` (None past the end).
    pub fn slot_idx(&self, slot: usize) -> Option<usize> {
        let i = self.first() + self.top + slot;
        (slot < ROWS && i < self.rows().len()).then_some(i)
    }

    /// Mouse wheel: scroll the pooled rows by `d` (the focus follows the view).
    pub fn scroll(&mut self, d: i32) {
        let n = self.rows().len() - self.first();
        self.top = (self.top as i32 + d).clamp(0, n.saturating_sub(ROWS) as i32) as usize;
        let (lo, hi) = (self.first() + self.top, self.first() + self.top + ROWS - 1);
        if self.sel >= self.first() && !(lo..=hi).contains(&self.sel) {
            self.focus(self.sel.clamp(lo, hi));
        }
    }

    /// Scroll bar: (first visible row, rows in all) while more rows exist than fit.
    pub fn scroll_thumb(&self) -> Option<(usize, usize)> {
        let n = self.rows().len() - self.first();
        (n > ROWS).then_some((self.top, n))
    }

    /// Scroll bar drag: the thumb's middle follows `f` (0..1 down the track).
    pub fn scroll_to(&mut self, f: f32) {
        let Some((top, n)) = self.scroll_thumb() else { return };
        let thumb = ROWS as f32 / n as f32;
        let want = ((f - thumb / 2.0) / (1.0 - thumb) * (n - ROWS) as f32).round().clamp(0.0, (n - ROWS) as f32);
        self.scroll(want as i32 - top as i32);
    }

    /// Mouse press on row `idx` (button `sub` of a download row): focus it and do what Enter does.
    pub fn press(&mut self, idx: usize, sub: usize, s: &mut Settings) -> Option<Action> {
        self.focus(idx);
        self.sub = sub;
        self.nav(Nav::Enter, s)
    }

    /// Mouse press on entry `k` of the open drop-down.
    pub fn pick(&mut self, k: usize, s: &mut Settings) -> Option<Action> {
        self.open = Some(k);
        self.nav(Nav::Enter, s)
    }

    /// Set a slider row (mouse press / drag at `f` 0..1 of the track).
    pub fn slide(&mut self, idx: usize, f: f32, s: &mut Settings) -> Option<Action> {
        self.focus(idx);
        let Some(&Row::Opt(i)) = self.rows().get(idx) else { return None };
        let f = f.clamp(0.0, 1.0);
        let next = match OPTS[i].ctl {
            Ctl::Slider { min, max, step } => set_percent(s, i, (((f * (max - min) as f32) / step as f32).round() as i32 * step + min).clamp(min, max)),
            Ctl::Steps => with_value(s, OPTS[i].key, OPTS[i].values[(f * (OPTS[i].values.len() - 1) as f32).round() as usize]),
            _ => return None,
        };
        (next != *s).then(|| {
            *s = next;
            Action::Changed(i)
        })
    }

    fn back_to_main(&mut self) {
        let from = match self.page {
            Page::Settings => Item::Settings,
            Page::Controls => Item::Controls,
            _ => Item::Hof,
        };
        self.page = Page::Main;
        (self.sub, self.top, self.open) = (0, 0, None);
        self.sel = self.rows().iter().position(|&r| r == Row::Item(from)).unwrap_or(0);
    }

    fn open_page(&mut self, p: Page) {
        (self.page, self.sel, self.sub, self.top, self.open) = (p, 0, 0, 0, None);
        match p {
            Page::Controls => self.tab = Tab::Keyboard,
            Page::Settings if !TABS.contains(&self.tab) => self.tab = Tab::Video,
            _ => {}
        }
    }

    fn font_press(&mut self, i: usize) -> Option<Action> {
        let b = font_buttons(&soundfont::status(i));
        let b = b[self.sub.min(b.len() - 1)];
        self.sub = 0;
        match b {
            FontBtn::Download | FontBtn::Retry => soundfont::ask(i),
            FontBtn::Accept => return Some(Action::Download(i)),
            FontBtn::Decline | FontBtn::No => soundfont::cancel_all(),
            FontBtn::Abort => soundfont::cancel(i),
            FontBtn::Delete => {
                soundfont::ask_delete(i);
                self.sub = 1; // "No" is the default of a delete question
            }
            FontBtn::Yes => {
                soundfont::delete(i);
                return Some(Action::Deleted(i));
            }
        }
        None
    }

    /// One key press (or its mouse equivalent). Settings changes are made in `s`; the caller applies and saves them on `Changed`.
    pub fn nav(&mut self, n: Nav, s: &mut Settings) -> Option<Action> {
        let rows = self.rows();
        self.sel = self.sel.min(rows.len() - 1);
        let row = rows[self.sel];
        let act = self.nav_row(n, row, rows.len(), s);
        self.keep_visible();
        act
    }

    fn nav_row(&mut self, n: Nav, row: Row, count: usize, s: &mut Settings) -> Option<Action> {
        if let (Some(h), Row::Opt(i)) = (self.open, row) {
            let len = OPTS[i].values.len();
            let ok = |k: usize| available(OPTS[i].key, OPTS[i].values[k]);
            match n {
                // the highlight skips entries that cannot be picked yet
                Nav::Up => self.open = (1..=len).map(|d| (h + len - d) % len).find(|&k| ok(k)).or(Some(h)),
                Nav::Down => self.open = (1..=len).map(|d| (h + d) % len).find(|&k| ok(k)).or(Some(h)),
                Nav::Enter if !ok(h) => {} // stays open
                Nav::Enter => {
                    self.open = None;
                    *s = with_value(s, OPTS[i].key, OPTS[i].values[h]);
                    return Some(Action::Changed(i));
                }
                Nav::Back => self.open = None,
                _ => {}
            }
            return None;
        }
        self.open = None;
        match (n, row) {
            (Nav::Up, _) => (self.sel, self.sub) = ((self.sel + count - 1) % count, 0),
            (Nav::Down, _) => (self.sel, self.sub) = ((self.sel + 1) % count, 0),
            (Nav::PrevTab | Nav::NextTab, _) if self.has_tabs() => self.set_tab((self.tabs().iter().position(|&t| t == self.tab).unwrap_or(0) + if n == Nav::NextTab { 1 } else { self.tabs().len() - 1 }) % self.tabs().len()),
            (Nav::PrevTab | Nav::NextTab, _) => {}
            (Nav::Left | Nav::Right, Row::Tabs) => self.set_tab((self.tabs().iter().position(|&t| t == self.tab).unwrap_or(0) + if n == Nav::Right { 1 } else { self.tabs().len() - 1 }) % self.tabs().len()),
            (Nav::Left | Nav::Right, Row::Opt(i)) => {
                *s = cycle(s, i, if n == Nav::Left { -1 } else { 1 });
                return Some(Action::Changed(i));
            }
            (Nav::Left | Nav::Right, Row::Font(i)) => {
                let k = font_buttons(&soundfont::status(i)).len();
                self.sub = if n == Nav::Right { (self.sub + 1).min(k - 1) } else { self.sub.saturating_sub(1) };
            }
            (Nav::Left | Nav::Right, _) => {}
            (Nav::Enter, Row::Tabs) => self.sel = (self.sel + 1).min(count - 1), // into the options of the tab
            (Nav::Enter, Row::Opt(i)) => match OPTS[i].ctl {
                Ctl::Checkbox => {
                    *s = cycle(s, i, 1);
                    return Some(Action::Changed(i));
                }
                Ctl::Dropdown => self.open = Some(pos(s, i).unwrap_or(0)),
                // Enter steps a slider or step row on, from the last value back to the first
                _ => {
                    let mut next = cycle(s, i, 1);
                    if next == *s {
                        next = match OPTS[i].ctl {
                            Ctl::Slider { min, .. } => set_percent(s, i, min),
                            _ => with_value(s, OPTS[i].key, OPTS[i].values[0]),
                        };
                    }
                    if next != *s {
                        *s = next;
                        return Some(Action::Changed(i));
                    }
                }
            },
            (Nav::Enter, Row::Font(i)) => return self.font_press(i),
            (Nav::Back, _) if self.page == Page::Settings && confirming_any() => soundfont::cancel_all(),
            (Nav::Back, _) | (Nav::Enter, Row::Item(Item::Back)) => {
                if self.page == Page::Main {
                    return Some(Action::Close);
                }
                self.back_to_main();
            }
            (Nav::Enter, Row::Item(it)) => match it {
                Item::Settings => self.open_page(Page::Settings),
                Item::Controls => self.open_page(Page::Controls),
                Item::Hof => self.open_page(Page::Hof),
                Item::Resume => return Some(Action::Close),
                Item::Start => return Some(Action::Start),
                Item::Load => return Some(Action::Load),
                Item::Restart => return Some(Action::Restart),
                Item::Save => return Some(Action::Save),
                Item::QuitTitle => return Some(Action::QuitTitle),
                Item::Quit => return Some(Action::Quit),
                Item::Version => return Some(Action::Releases),
                Item::Back => {}
            },
        }
        None
    }
}

/// Message after a font was deleted.
pub fn deleted_note(i: usize, s: &Settings) -> String {
    let f = &soundfont::FONTS[i];
    if s.sound == "soundfont" && s.soundfont == f.id {
        format!("{} deleted. Music uses AdLib now (the setting stays).", f.name)
    } else {
        format!("{} deleted.", f.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soundfont::{debug_set, Status};

    fn settings_menu() -> Menu {
        let mut m = Menu::new(Ctx::Title, false, false);
        m.page = Page::Settings;
        m
    }

    #[test]
    fn enter_confirms_on_every_row_type() {
        let mut s = Settings::default();
        let mut m = settings_menu();
        assert_eq!(m.rows()[m.sel], Row::Tabs);
        m.nav(Nav::Enter, &mut s);
        assert_eq!(m.sel, 1, "Enter on the tab strip goes into the tab");
        // a step row (sun light) steps on and wraps from the last value to the first
        let i = opt("sun_brightness");
        focus_opt(&mut m, i);
        s.sun_brightness = "high".into();
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Changed(i)));
        assert_eq!(s.sun_brightness, "dim");
        // a slider (volume) steps on and wraps too
        let v = opt("volume");
        focus_opt(&mut m, v);
        s.volume = 1.0;
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Changed(v)));
        assert_eq!(s.volume, 0.0);
    }

    #[test]
    fn controls_page_has_keyboard_and_controller_tabs() {
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Title, false, false);
        m.sel = m.rows().iter().position(|&r| r == Row::Item(Item::Controls)).unwrap();
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.tab, m.sel), (Page::Controls, Tab::Keyboard, 0));
        assert_eq!(m.rows(), [Row::Tabs, Row::Item(Item::Back)]);
        m.nav(Nav::Right, &mut s);
        assert_eq!(m.tab, Tab::Controller);
        m.nav(Nav::NextTab, &mut s);
        assert_eq!(m.tab, Tab::Keyboard, "the two tabs wrap");
        // Settings after Controls starts on a Settings tab again
        m.nav(Nav::Back, &mut s);
        m.sel = m.rows().iter().position(|&r| r == Row::Item(Item::Settings)).unwrap();
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.tab), (Page::Settings, Tab::Video));
    }

    fn opt(key: &str) -> usize {
        OPTS.iter().position(|o| o.key == key).unwrap()
    }

    /// moves the focus to the row of OPTS[i] (switching the tab)
    fn focus_opt(m: &mut Menu, i: usize) {
        m.set_tab(TABS.iter().position(|&t| t == OPTS[i].tab).unwrap());
        m.sel = m.rows().iter().position(|&r| r == Row::Opt(i)).unwrap();
    }

    #[test]
    fn pause_menu_navigation_and_back() {
        let _g = soundfont::LOCK.lock().unwrap_or_else(|e| e.into_inner()); // Esc looks at the shared download state
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Pause, false, false);
        assert!(!m.rows().contains(&Row::Item(Item::Save)), "no save outside levels 3..13");
        assert_eq!(m.nav(Nav::Up, &mut s), None);
        assert_eq!(m.rows()[m.sel], Row::Item(Item::Version), "up wraps to the bottom");
        assert_eq!(m.nav(Nav::Down, &mut s), None);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Close), "Resume");
        m.sel = 1;
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Restart));
        // into settings and back with Esc: the cursor returns to "Settings"
        m.sel = m.rows().iter().position(|&r| r == Row::Item(Item::Settings)).unwrap();
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.sel), (Page::Settings, 0));
        assert_eq!(m.rows()[0], Row::Tabs);
        assert_eq!(m.nav(Nav::Back, &mut s), None);
        assert_eq!((m.page, m.rows()[m.sel]), (Page::Main, Row::Item(Item::Settings)));
        assert_eq!(m.nav(Nav::Back, &mut s), Some(Action::Close), "Esc on the main page resumes");
        assert!(Menu::new(Ctx::Pause, true, false).rows().contains(&Row::Item(Item::Save)));
    }

    #[test]
    fn title_menu_items() {
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Title, false, false);
        let items = |m: &Menu| m.rows().into_iter().map(|r| match r { Row::Item(i) => i, _ => panic!() }).collect::<Vec<_>>();
        assert_eq!(items(&m), [Item::Start, Item::Settings, Item::Controls, Item::Hof, Item::Quit, Item::Version]);
        assert_eq!(items(&Menu::new(Ctx::Title, false, true))[1], Item::Load);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Start));
        m.sel = 3;
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, items(&m)), (Page::Hof, vec![Item::Back]));
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.sel), (Page::Main, 3));
        assert_eq!(m.title(), "MENU");
    }

    #[test]
    fn settings_values_cycle_and_wrap() {
        let _g = soundfont::LOCK.lock().unwrap_or_else(|e| e.into_inner());
        soundfont::debug_set(0, Status::Installed); // the font sound sets need one installed font
        let mut s = Settings::default();
        let mut m = settings_menu();
        s.render_mode = "classic".into();
        focus_opt(&mut m, 0);
        assert_eq!(m.nav(Nav::Right, &mut s), Some(Action::Changed(0)));
        assert_eq!(s.render_mode, "enhanced");
        assert_eq!(m.nav(Nav::Right, &mut s), Some(Action::Changed(0)));
        assert_eq!(s.render_mode, "classic", "wraps");
        m.nav(Nav::Left, &mut s);
        assert_eq!(s.render_mode, "enhanced");
        // every option cycles through all its values and comes back, through Settings::parse
        for (i, o) in OPTS.iter().enumerate() {
            if matches!(o.ctl, Ctl::Slider { .. }) {
                continue;
            }
            let start = value(&s, o.key);
            let mut t = s.clone();
            for _ in 0..o.values.len() {
                t = cycle(&t, i, 1);
                assert!(!value(&t, o.key).is_empty(), "{}", o.key);
            }
            if o.ctl != Ctl::Steps {
                assert_eq!(value(&t, o.key), start, "{}", o.key);
            }
        }
        // a value outside the list starts at an end; the rest of the settings stay
        let rt = with_value(&Settings::default(), "render_mode", "rt_experimental");
        assert_eq!(cycle(&rt, 0, 1).render_mode, "classic");
        assert_eq!(cycle(&rt, 0, 1).volume, rt.volume);
        // every option is on exactly one tab; the first three tabs list them, Downloads lists the fonts
        let mut m = settings_menu();
        let n: usize = TABS[..3].iter().map(|&t| { m.tab = t; m.rows().iter().filter(|r| matches!(r, Row::Opt(_))).count() }).sum();
        m.page = Page::Controls;
        m.tab = Tab::Controller;
        let pads = m.rows().iter().filter(|r| matches!(r, Row::Opt(_))).count();
        m.page = Page::Settings;
        assert_eq!(n + pads, OPTS.len());
        m.tab = Tab::Downloads;
        assert_eq!(m.rows(), [Row::Tabs, Row::Font(0), Row::Font(1), Row::Item(Item::Back)]);
    }

    #[test]
    fn slider_steps_clamps_and_mouse_sets() {
        let mut s = Settings::default();
        let v = opt("volume");
        s.volume = 0.70000005;
        assert!((cycle(&s, v, 1).volume - 0.75).abs() < 1e-6, "5 % steps");
        s.volume = 0.67;
        assert!((cycle(&s, v, 1).volume - 0.70).abs() < 1e-6 && (cycle(&s, v, -1).volume - 0.65).abs() < 1e-6, "off-step values snap");
        s.volume = 1.0;
        assert_eq!(cycle(&s, v, 1).volume, 1.0, "stops at 100 %");
        s.volume = 0.0;
        assert_eq!(cycle(&s, v, -1).volume, 0.0);
        let mut m = settings_menu();
        focus_opt(&mut m, v);
        let idx = m.sel;
        assert_eq!(m.slide(idx, 0.5, &mut s), Some(Action::Changed(v)));
        assert_eq!(s.volume, 0.5);
        assert_eq!(m.slide(idx, 0.5, &mut s), None, "no change, no action");
        m.slide(idx, 2.0, &mut s);
        assert_eq!(s.volume, 1.0, "drag past the end clamps");
        m.slide(idx, 0.512, &mut s);
        assert_eq!(s.volume, 0.5, "snaps to a 5 % step");
        assert_eq!(frac(&s, v), 0.5);
        assert_eq!(opt_value(&s, v), "50%");
        // Left / Right on the slider row
        assert_eq!(m.nav(Nav::Left, &mut s), Some(Action::Changed(v)));
        assert!((s.volume - 0.45).abs() < 1e-6);
        // the ordered shadow values: a slider over off / low / high that does not wrap
        let sh = opt("shadow_quality");
        focus_opt(&mut m, sh);
        m.slide(m.sel, 0.0, &mut s);
        assert_eq!(s.shadow_quality, "off");
        m.nav(Nav::Left, &mut s);
        assert_eq!(s.shadow_quality, "off", "stops at the end");
        m.slide(m.sel, 0.5, &mut s);
        assert_eq!((s.shadow_quality.as_str(), frac(&s, sh)), ("low", 0.5));
    }

    #[test]
    fn checkbox_toggles() {
        let mut s = Settings::default();
        let mut m = settings_menu();
        let mute = opt("muted");
        focus_opt(&mut m, mute);
        assert_eq!(opt_value(&s, mute), "off");
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Changed(mute)));
        assert!(s.muted);
        let idx = m.sel;
        assert_eq!(m.press(idx, 0, &mut s), Some(Action::Changed(mute)), "a click toggles too");
        assert!(!s.muted);
        assert!(OPTS[opt("ssao")].ctl == Ctl::Checkbox);
    }

    #[test]
    fn fonts_that_are_not_installed_cannot_be_picked() {
        let _g = soundfont::LOCK.lock().unwrap_or_else(|e| e.into_inner());
        soundfont::debug_set(0, Status::Absent);
        soundfont::debug_set(1, Status::Absent);
        let (snd, font) = (opt("sound"), opt("soundfont"));
        let mut s = Settings::default();
        s.sound = "adlib".into();
        assert_eq!(cycle(&s, snd, 1).sound, "synth", "the soundfont set is skipped without a font (wraps past it)");
        assert!(!available("soundfont", "musescore") && available("sound", "adlib"));
        assert!(entry_label("soundfont", "musescore").ends_with("(not installed)"));
        s.soundfont = "fluidr3".into();
        assert_eq!(cycle(&s, font, 1).soundfont, "fluidr3", "nothing installed: no change");
        soundfont::debug_set(0, Status::Installed);
        assert_eq!(cycle(&s, font, 1).soundfont, "musescore");
        assert_eq!(cycle(&s, snd, 1).sound, "soundfont");
        // the open drop-down: Enter on an entry that is not installed does nothing
        soundfont::debug_set(0, Status::Absent);
        let mut m = settings_menu();
        m.page = Page::Settings;
        m.tab = Tab::Audio;
        m.sel = m.rows().iter().position(|r| *r == Row::Opt(font)).unwrap();
        m.open = Some(0);
        assert!(m.nav(Nav::Enter, &mut s).is_none() && m.open == Some(0) && s.soundfont == "fluidr3");
        soundfont::debug_clear(0);
        soundfont::debug_clear(1);
    }

    #[test]
    fn dropdown_opens_selects_wraps_and_closes() {
        let _g = soundfont::LOCK.lock().unwrap_or_else(|e| e.into_inner());
        soundfont::debug_set(0, Status::Installed); // the font sound sets need one installed font
        let mut s = Settings::default();
        let mut m = settings_menu();
        s.sound = "synth".into();
        let snd = opt("sound");
        focus_opt(&mut m, snd);
        assert_eq!(m.nav(Nav::Enter, &mut s), None);
        assert_eq!(m.open, Some(0), "opens on the current value (synth)");
        m.nav(Nav::Up, &mut s);
        assert_eq!(m.open, Some(3), "up wraps");
        assert_eq!(m.nav(Nav::Left, &mut s), None, "Left / Right do nothing while open");
        m.nav(Nav::Down, &mut s);
        m.nav(Nav::Down, &mut s);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Changed(snd)));
        assert_eq!((s.sound.as_str(), m.open), ("original", None));
        // Esc closes without changing and does not leave the page
        m.nav(Nav::Enter, &mut s);
        assert_eq!(m.open, Some(1));
        assert_eq!(m.nav(Nav::Back, &mut s), None);
        assert_eq!((m.open, m.page, s.sound.as_str()), (None, Page::Settings, "original"));
        // a mouse pick of entry 2
        m.press(m.sel, 0, &mut s);
        assert_eq!(m.pick(2, &mut s), Some(Action::Changed(snd)));
        assert_eq!(s.sound, "adlib");
        // Left / Right change the closed drop-down and wrap
        m.nav(Nav::Right, &mut s);
        m.nav(Nav::Right, &mut s);
        assert_eq!(s.sound, "synth");
        assert!(OPTS.iter().all(|o| o.live || o.key == "low_latency_present"), "every option applies at runtime, only the window present mode carries the restart badge");
        assert_eq!(shown("sound", "adlib"), "Adlib");
    }

    #[test]
    fn focus_moves_across_widgets_tabs_and_scrolls() {
        let mut s = Settings::default();
        let mut m = settings_menu();
        assert_eq!(m.rows()[m.sel], Row::Tabs);
        m.nav(Nav::Right, &mut s);
        assert_eq!(m.tab, Tab::Audio);
        m.nav(Nav::Left, &mut s);
        m.nav(Nav::Left, &mut s);
        assert_eq!(m.tab, Tab::Downloads, "tabs wrap");
        m.nav(Nav::NextTab, &mut s);
        assert_eq!(m.tab, Tab::Video, "Q / E, LB / RB");
        m.nav(Nav::Down, &mut s);
        assert_eq!(m.rows()[m.sel], Row::Opt(0));
        for _ in 0..8 {
            m.nav(Nav::Down, &mut s);
        }
        assert_eq!(m.rows()[m.sel], Row::Item(Item::Back), "Back is last");
        m.nav(Nav::Down, &mut s);
        assert_eq!(m.sel, 0, "down wraps to the tab strip");
        m.nav(Nav::Up, &mut s);
        assert_eq!(m.rows()[m.sel], Row::Item(Item::Back));
        // hover / click focus; the wheel scrolls only when the list is longer than the pool
        m.focus(2);
        assert_eq!(m.sel, 2);
        assert_eq!(m.slot_idx(0), Some(1));
        assert_eq!(m.slot_idx(10), None);
        m.scroll(3);
        assert_eq!(m.top, 0, "everything fits");
        assert_eq!(m.visible().len(), 9);
        // a tab click focuses the strip
        m.focus(0);
        m.set_tab(2);
        assert_eq!((m.tab, m.visible().len()), (Tab::Graphics, 9));
    }

    #[test]
    fn the_scroll_bar_follows_the_view_and_drags_it() {
        let mut m = settings_menu();
        m.set_tab(2); // Graphics: more rows than fit
        let (top, n) = m.scroll_thumb().expect("a scroll bar");
        assert_eq!((top, n > ROWS), (0, true));
        m.scroll_to(1.0);
        assert_eq!(m.scroll_thumb(), Some((n - ROWS, n)));
        m.scroll_to(0.0);
        assert_eq!(m.top, 0);
        assert_eq!(Menu::new(Ctx::Title, false, false).scroll_thumb(), None); // the main page fits
    }

    #[test]
    fn download_rows_transitions() {
        let _g = soundfont::LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // an empty folder: a font installed on this machine must not change the Absent states below
        let empty = std::env::temp_dir().join(format!("pop_sf_empty_{}", std::process::id()));
        *soundfont::TEST_DIR.lock().unwrap() = Some(empty);
        let mut s = Settings::default();
        let mut m = settings_menu();
        m.set_tab(3);
        m.sel = 2; // Font(1)
        debug_set(1, Status::Absent);
        assert_eq!(font_buttons(&soundfont::status(1)), [FontBtn::Download]);
        m.nav(Nav::Enter, &mut s);
        assert_eq!(soundfont::status(1), Status::Confirm);
        assert_eq!(font_buttons(&Status::Confirm), [FontBtn::Accept, FontBtn::Decline]);
        // Esc closes the notice first, the page second
        assert_eq!(m.nav(Nav::Back, &mut s), None);
        assert_eq!((m.page, soundfont::status(1)), (Page::Settings, Status::Absent));
        // accept: the caller starts the download
        m.nav(Nav::Enter, &mut s);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Download(1)));
        // Right to Cancel declines
        m.nav(Nav::Right, &mut s);
        assert_eq!(m.sub, 1);
        m.nav(Nav::Enter, &mut s);
        assert_eq!(soundfont::status(1), Status::Absent);
        // a running download has one Cancel button, progress and a percentage
        let busy = Status::Busy { phase: "downloading", done: 54_000_000, total: 120_000_000 };
        debug_set(1, busy.clone());
        let info = font_info(1, &s, &busy);
        assert_eq!((info.btns.clone(), info.bar), (vec![FontBtn::Abort], Some(0.45)));
        assert_eq!(info.status, "54 / 120 MB  45%");
        m.nav(Nav::Enter, &mut s); // Abort: sets the flag, the worker thread resets the state
        assert!(matches!(soundfont::status(1), Status::Busy { .. }));
        // delete asks first, "No" is the default; Esc keeps the font
        let inst = Status::Installed;
        debug_set(1, inst.clone());
        m.nav(Nav::Enter, &mut s);
        assert_eq!((soundfont::status(1), m.sub), (Status::ConfirmDelete, 1));
        assert_eq!(font_buttons(&Status::ConfirmDelete), [FontBtn::Yes, FontBtn::No]);
        m.nav(Nav::Enter, &mut s);
        assert_ne!(soundfont::status(1), Status::ConfirmDelete, "No keeps it");
        debug_set(1, Status::Installed);
        m.nav(Nav::Enter, &mut s);
        assert_eq!(m.nav(Nav::Back, &mut s), None);
        assert_ne!(soundfont::status(1), Status::ConfirmDelete, "Esc keeps it");
        // a failure offers Retry
        assert_eq!(font_buttons(&Status::Failed("x".into())), [FontBtn::Retry]);
        // the selected SoundFont missing or about to be deleted shows a warning, the setting stays
        s.sound = "soundfont".into();
        s.soundfont = "fluidr3".into();
        let w = font_info(1, &s, &Status::Absent);
        assert!(w.warn && w.sub.contains("missing"));
        assert!(font_info(1, &s, &Status::ConfirmDelete).sub.contains("AdLib"));
        assert!(!font_info(0, &s, &Status::Absent).warn);
        assert!(deleted_note(1, &s).contains("AdLib") && !deleted_note(0, &s).contains("AdLib"));
        soundfont::debug_clear(1);
        *soundfont::TEST_DIR.lock().unwrap() = None;
    }

    #[test]
    fn labels_and_hints() {
        let s = Settings::default();
        assert_eq!(opt_value(&s, opt("window")), "Windowed");
        assert_eq!(Item::Back.label(), "Back");
    }
}
