//! settings.json next to the exe: window mode and size, volume, mute, sound mode, kid key bindings, data path.
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
        Settings { window: "windowed".into(), width: 1280, height: 720, volume: 0.7, muted: false, sound: "synth".into(), keys: Bindings::default(), data_path: None }
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
        serde_json::from_str(text).unwrap_or_else(|e| {
            eprintln!("settings.json: {e}; using defaults");
            Settings::default()
        })
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
            Ok(t) => Self::parse(&t),
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
