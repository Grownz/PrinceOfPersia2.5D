//! settings.json next to the exe: window mode and size, volume, mute, sound mode, kid key bindings, data path, render mode,
//! character style.
//! Unknown keys are ignored and missing ones take their defaults, so older and newer files both load.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// "windowed" or "borderless" (F11 toggles)
    pub window: String,
    pub width: u32,
    pub height: u32,
    pub volume: f32,
    pub muted: bool,
    /// "synth" or "original" (POP_SOUND overrides)
    pub sound: String,
    pub keys: Bindings,
    /// folder with the user's own game files (LEVELS.DAT, PRINCE.EXE, ...); tried after POP_DATA, exe dir and cwd
    pub data_path: Option<String>,
    /// "classic" (today's renderer), "enhanced" or "rt_experimental" (reserved: accepted, run as classic with a log line
    /// until implemented); POP_RENDER overrides
    pub render_mode: String,
    /// "sprite" (the original images on quads) or "voxel" (built from them at runtime); POP_CHARS overrides, F6 toggles
    pub character_style: String,
}

pub const RENDER_MODES: [&str; 3] = ["classic", "enhanced", "rt_experimental"];
pub const CHARACTER_STYLES: [&str; 2] = ["sprite", "voxel"];

/// The renderer actually used. Only `Classic` exists so far; the other `render_mode` values fall back to it.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Classic,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterStyle {
    Sprite,
    Voxel,
}

/// The effective (mode, style) from the settings values and the env overrides (`env` = POP_RENDER, POP_CHARS); unknown
/// values and modes that are not implemented yet fall back with a log line.
pub fn resolve_render(mode: &str, style: &str, env_mode: Option<&str>, env_style: Option<&str>) -> (RenderMode, CharacterStyle) {
    let mode = env_mode.unwrap_or(mode);
    match mode {
        "classic" => {}
        m if RENDER_MODES.contains(&m) => info!("render mode {m:?} is not implemented yet; using classic"),
        m => warn!("unknown render mode {m:?}; using classic"),
    }
    let style = match env_style.unwrap_or(style) {
        "voxel" => CharacterStyle::Voxel,
        "sprite" => CharacterStyle::Sprite,
        s => {
            warn!("unknown character style {s:?}; using sprite");
            CharacterStyle::Sprite
        }
    };
    (RenderMode::Classic, style)
}

/// Bevy `KeyCode` names per kid key, e.g. "ArrowLeft", "KeyA", "ShiftLeft".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Bindings {
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub shift: Vec<String>,
}

impl Default for Bindings {
    fn default() -> Self {
        let v = |s: &[&str]| s.iter().map(|s| s.to_string()).collect();
        Bindings { left: v(&["ArrowLeft"]), right: v(&["ArrowRight"]), up: v(&["ArrowUp"]), down: v(&["ArrowDown"]), shift: v(&["ShiftLeft", "ShiftRight"]) }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings { window: "windowed".into(), width: 1280, height: 720, volume: 0.7, muted: false, sound: "synth".into(), keys: Bindings::default(), data_path: None, render_mode: "classic".into(), character_style: "sprite".into() }
    }
}

/// The kid's keys resolved to `KeyCode`s (unknown names are dropped with a warning; an empty list falls back to the default).
#[derive(Resource, Clone)]
pub struct KeyMap {
    pub left: Vec<KeyCode>,
    pub right: Vec<KeyCode>,
    pub up: Vec<KeyCode>,
    pub down: Vec<KeyCode>,
    pub shift: Vec<KeyCode>,
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        let Ok(serde_json::Value::Object(o)) = serde_json::from_str(text) else {
            eprintln!("settings.json: not a JSON object; using defaults");
            return Settings::default();
        };
        let d = Settings::default();
        let k = o.get("keys").and_then(|k| k.as_object()).cloned().unwrap_or_default();
        let kd = Bindings::default();
        let keys = Bindings { left: field(&k, "left", kd.left, |_| true), right: field(&k, "right", kd.right, |_| true), up: field(&k, "up", kd.up, |_| true), down: field(&k, "down", kd.down, |_| true), shift: field(&k, "shift", kd.shift, |_| true) };
        Settings {
            window: field(&o, "window", d.window, |_| true),
            width: field(&o, "width", d.width, |&w| w > 0),
            height: field(&o, "height", d.height, |&h| h > 0),
            volume: field(&o, "volume", d.volume, |v| (0.0..=1.0).contains(v)),
            muted: field(&o, "muted", d.muted, |_| true),
            sound: field(&o, "sound", d.sound, |_| true),
            keys,
            data_path: field(&o, "data_path", d.data_path, |_| true),
            render_mode: field(&o, "render_mode", d.render_mode, |m| RENDER_MODES.contains(&m.as_str())),
            character_style: field(&o, "character_style", d.character_style, |s| CHARACTER_STYLES.contains(&s.as_str())),
        }
    }

    /// Effective render mode and character style (settings plus POP_RENDER / POP_CHARS).
    pub fn render(&self) -> (RenderMode, CharacterStyle) {
        let (m, s) = (std::env::var("POP_RENDER").ok(), std::env::var("POP_CHARS").ok());
        resolve_render(&self.render_mode, &self.character_style, m.as_deref(), s.as_deref())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings serialize")
    }

    pub fn borderless(&self) -> bool {
        self.window == "borderless"
    }

    pub fn path() -> std::path::PathBuf {
        crate::flow::exe_dir().join("settings.json")
    }

    /// The file next to the exe, or the defaults (written out on first run so the user can edit them).
    pub fn load() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(t) => {
                if !matches!(serde_json::from_str(&t), Ok(serde_json::Value::Object(_))) {
                    // the next save() would overwrite it: keep a copy (best effort)
                    let _ = std::fs::write(Self::path().with_extension("json.bad"), &t);
                }
                Self::parse(&t)
            }
            Err(_) => {
                let s = Settings::default();
                s.save();
                s
            }
        }
    }

    pub fn save(&self) {
        if let Err(e) = crate::flow::write_atomic(&Self::path(), &self.to_json()) {
            warn!("cannot write settings.json: {e}");
        }
    }

    pub fn key_map(&self) -> KeyMap {
        let d = Bindings::default();
        let res = |names: &[String], def: &[String]| {
            let v: Vec<KeyCode> = names
                .iter()
                .filter_map(|n| {
                    let k = key_code(n);
                    if k.is_none() {
                        warn!("settings.json: unknown key {n:?}");
                    }
                    k
                })
                .collect();
            if v.is_empty() { def.iter().filter_map(|n| key_code(n)).collect() } else { v }
        };
        let k = &self.keys;
        KeyMap { left: res(&k.left, &d.left), right: res(&k.right, &d.right), up: res(&k.up, &d.up), down: res(&k.down, &d.down), shift: res(&k.shift, &d.shift) }
    }
}

/// One field of a settings object: its value if present, well-typed and `ok`, else the default.
fn field<T: serde::de::DeserializeOwned>(o: &serde_json::Map<String, serde_json::Value>, key: &str, default: T, ok: impl Fn(&T) -> bool) -> T {
    match o.get(key).map(|v| serde_json::from_value::<T>(v.clone())) {
        Some(Ok(v)) if ok(&v) => v,
        Some(_) => {
            eprintln!("settings.json: bad value for {key:?}; using default");
            default
        }
        None => default,
    }
}

/// `KeyCode` by its Debug name, for the keys a player would bind.
pub fn key_code(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    const KEYS: &[KeyCode] = &[
        ArrowLeft, ArrowRight, ArrowUp, ArrowDown, ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, Space, Enter, Tab,
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadEnter,
        Insert, Delete, Home, End, PageUp, PageDown, Comma, Period, Slash, Semicolon, Quote, BracketLeft, BracketRight, Backslash,
    ];
    KEYS.iter().copied().find(|k| format!("{k:?}") == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let mut s = Settings::default();
        (s.volume, s.muted, s.window, s.data_path) = (0.3, true, "borderless".into(), Some("D:/pop".into()));
        s.keys.shift = vec!["KeyX".into()];
        assert_eq!(Settings::parse(&s.to_json()), s);
        assert_eq!(Settings::parse("{}"), Settings::default());
    }

    #[test]
    fn unknown_keys_ignored_and_bad_file_falls_back() {
        let s = Settings::parse(r#"{"volume": 0.25, "future_option": [1, 2], "keys": {"left": ["KeyA"], "jump": ["Space"]}}"#);
        assert_eq!((s.volume, s.keys.left.clone(), s.keys.right.clone()), (0.25, vec!["KeyA".to_string()], vec!["ArrowRight".to_string()]));
        assert_eq!(Settings::parse("not json"), Settings::default());
    }

    #[test]
    fn bad_fields_fall_back_individually() {
        let s = Settings::parse(r#"{"volume": "loud", "width": -5, "height": 900, "data_path": "D:/pop", "keys": {"left": ["KeyA"], "up": 3}}"#);
        let d = Settings::default();
        assert_eq!((s.volume, s.width, s.height), (d.volume, d.width, 900));
        assert_eq!(s.data_path.as_deref(), Some("D:/pop"));
        assert_eq!((s.keys.left, s.keys.up), (vec!["KeyA".to_string()], d.keys.up));
        assert_eq!(Settings::parse(r#"{"volume": 7}"#).volume, d.volume);
    }

    #[test]
    fn non_object_file_falls_back() {
        for t in ["[1,2]", "42", "", "{\"volume\": "] {
            assert_eq!(Settings::parse(t), Settings::default());
        }
    }

    #[test]
    fn render_settings_parse_and_fall_back() {
        let d = Settings::default();
        assert_eq!((d.render_mode.as_str(), d.character_style.as_str()), ("classic", "sprite"));
        let s = Settings::parse(r#"{"render_mode": "rt_experimental", "character_style": "voxel"}"#);
        assert_eq!((s.render_mode.as_str(), s.character_style.as_str()), ("rt_experimental", "voxel"));
        assert_eq!(Settings::parse(&s.to_json()), s);
        // unknown or ill-typed values fall back per field, the rest of the file still loads
        let s = Settings::parse(r#"{"render_mode": "pathtraced", "character_style": 3, "volume": 0.5}"#);
        assert_eq!((s.render_mode, s.character_style, s.volume), (d.render_mode, d.character_style, 0.5));
    }

    #[test]
    fn render_resolution_env_overrides_and_fallback() {
        use super::{CharacterStyle::*, RenderMode::*};
        assert_eq!(resolve_render("classic", "sprite", None, None), (Classic, Sprite));
        assert_eq!(resolve_render("classic", "voxel", None, None), (Classic, Voxel));
        // reserved modes are accepted but run classic until implemented
        assert_eq!(resolve_render("enhanced", "sprite", None, None).0, Classic);
        assert_eq!(resolve_render("rt_experimental", "sprite", None, None).0, Classic);
        // env wins over the file; bad env values fall back instead of failing
        assert_eq!(resolve_render("classic", "sprite", Some("rt_experimental"), Some("voxel")), (Classic, Voxel));
        assert_eq!(resolve_render("classic", "voxel", Some("nonsense"), Some("cubes")), (Classic, Sprite));
    }

    #[test]
    fn key_map_resolves_names_and_falls_back() {
        let mut s = Settings::default();
        s.keys.left = vec!["KeyA".into(), "NoSuchKey".into()];
        s.keys.up = vec!["Bogus".into()];
        let m = s.key_map();
        assert_eq!(m.left, vec![KeyCode::KeyA]);
        assert_eq!(m.up, vec![KeyCode::ArrowUp]);
        assert_eq!(m.shift, vec![KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    }
}
