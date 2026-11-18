//! Menu logic (M6), no Bevy: pages, items, navigation and settings value cycling. menu_ui.rs reads the keys and draws it,
//! flow.rs carries out the actions. The title menu opens with Esc / Start, the pause menu whenever the game is paused.
use crate::settings::{self, Bindings, Settings};

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
    /// index into OPTS
    Opt(usize),
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
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
}

/// One settings.json key: shown label, the values it cycles through, whether it applies at once (else after a restart).
pub struct Opt {
    pub key: &'static str,
    pub label: &'static str,
    pub values: &'static [&'static str],
    pub live: bool,
}

const ON_OFF: &[&str] = &["true", "false"];
const VOLUMES: &[&str] = &["0", "0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8", "0.9", "1"];

pub const OPTS: [Opt; 14] = [
    Opt { key: "render_mode", label: "Rendering", values: &["classic", "enhanced"], live: true },
    Opt { key: "character_style", label: "Characters", values: &settings::CHARACTER_STYLES, live: true },
    Opt { key: "camera_scroll", label: "Room change", values: &settings::CAMERA_SCROLLS, live: true },
    Opt { key: "window", label: "Window", values: &["windowed", "borderless"], live: true },
    Opt { key: "volume", label: "Volume", values: VOLUMES, live: true },
    Opt { key: "muted", label: "Mute", values: ON_OFF, live: true },
    Opt { key: "sound", label: "Sound set", values: &["synth", "original"], live: false },
    Opt { key: "tile_textures", label: "Tile textures", values: &settings::TILE_TEXTURES, live: false },
    Opt { key: "column_style", label: "Columns", values: &settings::COLUMN_STYLES, live: false },
    Opt { key: "object_style", label: "Objects", values: &settings::OBJECT_STYLES, live: false },
    Opt { key: "voxel_normals", label: "Voxel normals", values: &settings::VOXEL_NORMALS, live: false },
    Opt { key: "voxel_shading", label: "Voxel shading", values: &settings::VOXEL_SHADINGS, live: false },
    Opt { key: "shadow_quality", label: "Shadows (enhanced)", values: &settings::SHADOW_QUALITIES, live: false },
    Opt { key: "ssao", label: "SSAO (enhanced)", values: ON_OFF, live: false },
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

/// Step OPTS[i] by `dir` (+1 / -1, wrapping); a current value outside the list (rt_experimental, volume 0.65) starts at
/// the nearest end.
pub fn cycle(s: &Settings, i: usize, dir: i32) -> Settings {
    let o = &OPTS[i];
    let cur = value(s, o.key);
    let n = o.values.len() as i32;
    let pos = if o.key == "volume" {
        Some((s.volume * 10.0).round() as i32) // f32 sums such as 0.70000005 still find their step
    } else {
        o.values.iter().position(|v| *v == cur).map(|p| p as i32)
    };
    let next = match pos {
        Some(p) if o.key == "volume" => (p + dir).clamp(0, n - 1), // volume stops at 0 and 100 %
        Some(p) => (p + dir).rem_euclid(n),
        None => if dir > 0 { 0 } else { n - 1 },
    };
    with_value(s, o.key, o.values[next as usize])
}

/// What the value looks like in the menu.
fn shown(key: &str, v: &str) -> String {
    match (key, v) {
        ("volume", v) => format!("{}%", (v.parse::<f32>().unwrap_or(0.0) * 100.0).round()),
        (_, "true") => "on".into(),
        (_, "false") => "off".into(),
        (_, v) => v.into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub ctx: Ctx,
    pub page: Page,
    pub sel: usize,
    /// pause: Ctrl+G would save here (levels 3..13)
    pub can_save: bool,
    /// title: PRINCE.SAV exists
    pub has_save: bool,
}

impl Menu {
    pub fn new(ctx: Ctx, can_save: bool, has_save: bool) -> Menu {
        Menu { ctx, page: Page::Main, sel: 0, can_save, has_save }
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

    pub fn items(&self) -> Vec<Item> {
        use Item::*;
        match (self.page, self.ctx) {
            (Page::Main, Ctx::Title) => [Start, Load, Settings, Controls, Hof, Quit].into_iter().filter(|&i| i != Load || self.has_save).collect(),
            (Page::Main, Ctx::Pause) => [Resume, Restart, Save, Settings, Controls, QuitTitle, Quit].into_iter().filter(|&i| i != Save || self.can_save).collect(),
            (Page::Settings, _) => (0..OPTS.len()).map(Item::Opt).chain([Back]).collect(),
            (Page::Controls | Page::Hof, _) => vec![Back],
        }
    }

    pub fn label(&self, item: Item, s: &Settings) -> String {
        match item {
            Item::Start => "Start game".into(),
            Item::Load => "Load game".into(),
            Item::Resume => "Resume".into(),
            Item::Restart => "Restart level".into(),
            Item::Save => "Save game".into(),
            Item::Settings => "Settings".into(),
            Item::Controls => "Controls".into(),
            Item::Hof => "Hall of fame".into(),
            Item::QuitTitle => "Quit to title".into(),
            Item::Quit => "Quit game".into(),
            Item::Back => "Back".into(),
            Item::Opt(i) => {
                let o = &OPTS[i];
                format!("{}:  < {} >{}", o.label, shown(o.key, &value(s, o.key)), if o.live { "" } else { "  (after restart)" })
            }
        }
    }

    /// One key press. Settings changes are made in `s`; the caller applies and saves them on `Changed`.
    pub fn nav(&mut self, n: Nav, s: &mut Settings) -> Option<Action> {
        let items = self.items();
        self.sel = self.sel.min(items.len() - 1);
        let item = items[self.sel];
        match (n, item) {
            (Nav::Up, _) => self.sel = (self.sel + items.len() - 1) % items.len(),
            (Nav::Down, _) => self.sel = (self.sel + 1) % items.len(),
            (Nav::Left | Nav::Right | Nav::Enter, Item::Opt(i)) => {
                *s = cycle(s, i, if n == Nav::Left { -1 } else { 1 });
                return Some(Action::Changed(i));
            }
            (Nav::Left | Nav::Right, _) => {}
            (Nav::Back, _) | (Nav::Enter, Item::Back) => {
                if self.page == Page::Main {
                    return Some(Action::Close);
                }
                let from = match self.page {
                    Page::Settings => Item::Settings,
                    Page::Controls => Item::Controls,
                    _ => Item::Hof,
                };
                self.page = Page::Main;
                self.sel = self.items().iter().position(|&i| i == from).unwrap_or(0);
            }
            (Nav::Enter, Item::Settings) => (self.page, self.sel) = (Page::Settings, 0),
            (Nav::Enter, Item::Controls) => (self.page, self.sel) = (Page::Controls, 0),
            (Nav::Enter, Item::Hof) => (self.page, self.sel) = (Page::Hof, 0),
            (Nav::Enter, Item::Resume) => return Some(Action::Close),
            (Nav::Enter, Item::Start) => return Some(Action::Start),
            (Nav::Enter, Item::Load) => return Some(Action::Load),
            (Nav::Enter, Item::Restart) => return Some(Action::Restart),
            (Nav::Enter, Item::Save) => return Some(Action::Save),
            (Nav::Enter, Item::QuitTitle) => return Some(Action::QuitTitle),
            (Nav::Enter, Item::Quit) => return Some(Action::Quit),
        }
        None
    }
}

/// The Controls page: the kid's keys from settings.json, then the fixed ones (docs/controls.md).
pub fn controls(k: &Bindings) -> Vec<String> {
    let j = |v: &[String]| v.join(", ");
    let mut v = vec![
        format!("Left: {}   Right: {}", j(&k.left), j(&k.right)),
        format!("Up / jump: {}   Down / crouch: {}", j(&k.up), j(&k.down)),
        format!("Action (grab, strike, drink): {}", j(&k.shift)),
    ];
    v.extend(
        [
            "Numpad 7 8 9 / 4 5 6 / 2: move (7 / 9 running jump)",
            "Esc / P: pause menu   Ctrl+A: restart level",
            "Ctrl+G: save (levels 3-13)   Ctrl+L: load (title, demo)",
            "M: mute   - / =: volume   F11: fullscreen",
            "F5: classic / enhanced   F6: sprite / voxel   F9: frame times",
            "Controller: d-pad / stick move, A / X action,",
            "Start pause, Back restart; menus: A ok, B back",
        ]
        .map(String::from),
    );
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_menu_navigation_and_back() {
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Pause, false, false);
        assert!(!m.items().contains(&Item::Save), "no save outside levels 3..13");
        assert_eq!(m.nav(Nav::Up, &mut s), None);
        assert_eq!(m.items()[m.sel], Item::Quit, "up wraps to the bottom");
        assert_eq!(m.nav(Nav::Down, &mut s), None);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Close), "Resume");
        m.sel = 1;
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Restart));
        // into settings and back with Esc: the cursor returns to "Settings"
        m.sel = m.items().iter().position(|&i| i == Item::Settings).unwrap();
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.sel), (Page::Settings, 0));
        assert_eq!(m.nav(Nav::Back, &mut s), None);
        assert_eq!((m.page, m.items()[m.sel]), (Page::Main, Item::Settings));
        assert_eq!(m.nav(Nav::Back, &mut s), Some(Action::Close), "Esc on the main page resumes");
        assert!(Menu::new(Ctx::Pause, true, false).items().contains(&Item::Save));
    }

    #[test]
    fn title_menu_items() {
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Title, false, false);
        assert_eq!(m.items(), [Item::Start, Item::Settings, Item::Controls, Item::Hof, Item::Quit]);
        assert_eq!(Menu::new(Ctx::Title, false, true).items()[1], Item::Load);
        assert_eq!(m.nav(Nav::Enter, &mut s), Some(Action::Start));
        m.sel = 3;
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.items()), (Page::Hof, vec![Item::Back]));
        m.nav(Nav::Enter, &mut s);
        assert_eq!((m.page, m.sel), (Page::Main, 3));
        assert_eq!(m.title(), "MENU");
    }

    #[test]
    fn settings_values_cycle_and_wrap() {
        let mut s = Settings::default();
        let mut m = Menu::new(Ctx::Title, false, false);
        m.page = Page::Settings;
        assert_eq!(m.nav(Nav::Right, &mut s), Some(Action::Changed(0)));
        assert_eq!(s.render_mode, "enhanced");
        m.nav(Nav::Enter, &mut s);
        assert_eq!(s.render_mode, "classic", "wraps");
        m.nav(Nav::Left, &mut s);
        assert_eq!(s.render_mode, "enhanced");
        // every option cycles through all its values and comes back, through Settings::parse
        for (i, o) in OPTS.iter().enumerate() {
            let start = value(&s, o.key);
            let mut seen = vec![];
            let mut t = s.clone();
            for _ in 0..o.values.len() {
                t = cycle(&t, i, 1);
                seen.push(value(&t, o.key));
            }
            if o.key != "volume" {
                assert_eq!(value(&t, o.key), start, "{}", o.key);
                assert_eq!(seen.len(), o.values.len());
            }
            assert!(seen.iter().all(|v| !v.is_empty()), "{}", o.key);
        }
        // a value outside the list starts at an end; the rest of the settings stay
        let rt = with_value(&Settings::default(), "render_mode", "rt_experimental");
        assert_eq!(cycle(&rt, 0, 1).render_mode, "classic");
        assert_eq!(cycle(&rt, 0, 1).volume, rt.volume);
    }

    #[test]
    fn volume_steps_and_labels() {
        let mut s = Settings::default();
        s.volume = 0.70000005;
        let v = OPTS.iter().position(|o| o.key == "volume").unwrap();
        assert!((cycle(&s, v, 1).volume - 0.8).abs() < 1e-6);
        s.volume = 1.0;
        assert_eq!(cycle(&s, v, 1).volume, 1.0, "stops at 100 %");
        s.volume = 0.0;
        assert_eq!(cycle(&s, v, -1).volume, 0.0);
        let m = Menu::new(Ctx::Pause, false, false);
        assert_eq!(m.label(Item::Opt(v), &Settings::default()), "Volume:  < 70% >");
        let mute = OPTS.iter().position(|o| o.key == "muted").unwrap();
        assert_eq!(m.label(Item::Opt(mute), &Settings::default()), "Mute:  < off >");
        assert!(m.label(Item::Opt(7), &Settings::default()).ends_with("(after restart)"));
        assert!(controls(&Bindings::default())[0].contains("ArrowLeft"));
    }
}
