//! settings.json next to the exe: window mode and size, volume, mute, sound mode, kid key bindings, data path, render mode,
//! character style and the voxel characters' normals / shading, tile textures, column style, camera scroll.
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
    /// "synth", "original", "adlib" (original effects, music through the AdLib driver + OPL2 emulation) or "soundfont" (POP_SOUND overrides)
    pub sound: String,
    /// sound = "soundfont": which downloaded SoundFont plays the music ("musescore" or "fluidr3", Settings > SoundFonts)
    pub soundfont: String,
    pub keys: Bindings,
    /// folder with the user's own game files (LEVELS.DAT, PRINCE.EXE, ...); tried after POP_DATA, exe dir and cwd
    pub data_path: Option<String>,
    /// "classic" (the original look), "enhanced" (shadows, SSAO, bloom) or "rt_experimental" (Bevy Solari, only in builds
    /// with the `solari` feature, else run as classic with a log line); POP_RENDER overrides, F5 cycles classic / enhanced
    /// (/ rt_experimental when the game started in it)
    pub render_mode: String,
    /// "sprite" (the original images on quads) or "voxel" (built from them at runtime); POP_CHARS overrides, F6 toggles
    pub character_style: String,
    /// enhanced mode only: "off", "low" (2 torch shadows, small maps) or "high" (4 torch shadows)
    pub shadow_quality: String,
    /// enhanced mode only: screen-space ambient occlusion (turns MSAA off and uses SMAA instead)
    pub ssao: bool,
    /// "original" (the tile images of VDUNGEON.DAT / VPALACE.DAT on the room pieces) or "procedural" (plain coloured
    /// boxes); POP_TILES overrides
    pub tile_textures: String,
    /// "voxel" (pillars and door posts built as voxel columns from their images) or "flat" (the image on a box); only with
    /// tile_textures = original; POP_COLUMNS overrides
    pub column_style: String,
    /// "voxel" (doors, spikes, chompers, torch flames and debris built as voxel objects from their images) or "flat" (the
    /// images on quads and the procedural boxes); only with tile_textures = original; POP_OBJECTS overrides
    pub object_style: String,
    /// voxel characters: "smooth" (normals from the voxel surface's gradient, rounded shading) or "flat" (per cube face);
    /// POP_VOXEL_NORMALS overrides
    pub voxel_normals: String,
    /// voxel characters: "lit" (shaded by torches, sun and ambient) or "unlit" (the palette colours with the baked occlusion
    /// only, closest to the sprites); POP_VOXEL_SHADING overrides
    pub voxel_shading: String,
    /// room change: "smooth" (a short eased pan to the new room, the neighbours visible at the margins) or "flip" (the
    /// original's hard cut); POP_CAMERA overrides
    pub camera_scroll: String,
    /// "position" (kid and guard are drawn between the last two logic frames, 60 Hz motion at the cost of one logic frame,
    /// 83 ms, of delay) or "off" (the original's 12 Hz motion); POP_SMOOTH overrides
    pub motion_smoothing: String,
}

pub const CAMERA_SCROLLS: [&str; 2] = ["smooth", "flip"];
pub const MOTION_SMOOTHINGS: [&str; 2] = ["position", "off"];
pub const RENDER_MODES: [&str; 3] = ["classic", "enhanced", "rt_experimental"];
pub const CHARACTER_STYLES: [&str; 2] = ["sprite", "voxel"];
pub const SHADOW_QUALITIES: [&str; 3] = ["off", "low", "high"];
pub const TILE_TEXTURES: [&str; 2] = ["original", "procedural"];
pub const COLUMN_STYLES: [&str; 2] = ["voxel", "flat"];
pub const OBJECT_STYLES: [&str; 2] = ["voxel", "flat"];
pub const VOXEL_NORMALS: [&str; 2] = ["smooth", "flat"];
pub const VOXEL_SHADINGS: [&str; 2] = ["lit", "unlit"];

/// The renderer actually used; `rt_experimental` falls back to `Classic` unless built with the `solari` feature.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Classic,
    Enhanced,
    /// R3 trial: Bevy Solari (solari.rs)
    #[cfg(feature = "solari")]
    Rt,
}

impl RenderMode {
    /// The settings.json value.
    pub fn key(self) -> &'static str {
        match self {
            RenderMode::Classic => "classic",
            RenderMode::Enhanced => "enhanced",
            #[cfg(feature = "solari")]
            RenderMode::Rt => "rt_experimental",
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterStyle {
    Sprite,
    Voxel,
}

/// The effective (mode, style) from the settings values and the env overrides (`env` = POP_RENDER, POP_CHARS); unknown
/// values and modes that are not implemented yet fall back with a log line.
pub fn resolve_render(mode: &str, style: &str, env_mode: Option<&str>, env_style: Option<&str>) -> (RenderMode, CharacterStyle) {
    let mode = match env_mode.unwrap_or(mode) {
        "classic" => RenderMode::Classic,
        "enhanced" => RenderMode::Enhanced,
        #[cfg(feature = "solari")]
        "rt_experimental" => RenderMode::Rt,
        m if RENDER_MODES.contains(&m) => {
            info!("render mode {m:?} is not implemented yet; using classic");
            RenderMode::Classic
        }
        m => {
            warn!("unknown render mode {m:?}; using classic");
            RenderMode::Classic
        }
    };
    let style = match env_style.unwrap_or(style) {
        "voxel" => CharacterStyle::Voxel,
        "sprite" => CharacterStyle::Sprite,
        s => {
            warn!("unknown character style {s:?}; using sprite");
            CharacterStyle::Sprite
        }
    };
    (mode, style)
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
        Settings { window: "windowed".into(), width: 1280, height: 720, volume: 0.7, muted: false, sound: "synth".into(), soundfont: "musescore".into(), keys: Bindings::default(), data_path: None, render_mode: "classic".into(), character_style: "sprite".into(), shadow_quality: "high".into(), ssao: true, tile_textures: "original".into(), column_style: "voxel".into(), object_style: "voxel".into(), voxel_normals: "smooth".into(), voxel_shading: "lit".into(), camera_scroll: "smooth".into(), motion_smoothing: "position".into() }
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
            soundfont: field(&o, "soundfont", d.soundfont, |s| crate::soundfont::index_of(s).is_some()),
            keys,
            data_path: field(&o, "data_path", d.data_path, |_| true),
            render_mode: field(&o, "render_mode", d.render_mode, |m| RENDER_MODES.contains(&m.as_str())),
            character_style: field(&o, "character_style", d.character_style, |s| CHARACTER_STYLES.contains(&s.as_str())),
            shadow_quality: field(&o, "shadow_quality", d.shadow_quality, |s| SHADOW_QUALITIES.contains(&s.as_str())),
            ssao: field(&o, "ssao", d.ssao, |_| true),
            tile_textures: field(&o, "tile_textures", d.tile_textures, |s| TILE_TEXTURES.contains(&s.as_str())),
            column_style: field(&o, "column_style", d.column_style, |s| COLUMN_STYLES.contains(&s.as_str())),
            object_style: field(&o, "object_style", d.object_style, |s| OBJECT_STYLES.contains(&s.as_str())),
            voxel_normals: field(&o, "voxel_normals", d.voxel_normals, |s| VOXEL_NORMALS.contains(&s.as_str())),
            voxel_shading: field(&o, "voxel_shading", d.voxel_shading, |s| VOXEL_SHADINGS.contains(&s.as_str())),
            camera_scroll: field(&o, "camera_scroll", d.camera_scroll, |s| CAMERA_SCROLLS.contains(&s.as_str())),
            motion_smoothing: field(&o, "motion_smoothing", d.motion_smoothing, |s| MOTION_SMOOTHINGS.contains(&s.as_str())),
        }
    }

    /// Effective render mode and character style (settings plus POP_RENDER / POP_CHARS).
    pub fn render(&self) -> (RenderMode, CharacterStyle) {
        let (m, s) = (std::env::var("POP_RENDER").ok(), std::env::var("POP_CHARS").ok());
        resolve_render(&self.render_mode, &self.character_style, m.as_deref(), s.as_deref())
    }

    /// Original tile images on the room pieces (settings plus POP_TILES).
    pub fn tile_textures(&self) -> bool {
        crate::tiles::use_textures(&self.tile_textures, std::env::var("POP_TILES").ok().as_deref())
    }

    /// Voxel columns (settings plus POP_COLUMNS; anything but "flat" means voxel).
    pub fn voxel_columns(&self) -> bool {
        voxel_columns(&self.column_style, std::env::var("POP_COLUMNS").ok().as_deref())
    }

    /// Voxel objects (settings plus POP_OBJECTS; anything but "flat" means voxel).
    pub fn voxel_objects(&self) -> bool {
        voxel_columns(&self.object_style, std::env::var("POP_OBJECTS").ok().as_deref())
    }

    /// Smooth normals on the voxel characters (settings plus POP_VOXEL_NORMALS; anything but "flat" means smooth).
    pub fn smooth_normals(&self) -> bool {
        voxel_columns(&self.voxel_normals, std::env::var("POP_VOXEL_NORMALS").ok().as_deref())
    }

    /// Unlit voxel characters (settings plus POP_VOXEL_SHADING; only "unlit" means unlit).
    pub fn unlit_voxels(&self) -> bool {
        unlit(&self.voxel_shading, std::env::var("POP_VOXEL_SHADING").ok().as_deref())
    }

    /// Smooth camera pan on a room change (settings plus POP_CAMERA; only "flip" means the hard cut).
    pub fn smooth_scroll(&self) -> bool {
        smooth_scroll(&self.camera_scroll, std::env::var("POP_CAMERA").ok().as_deref())
    }

    /// Interpolated kid / guard motion (settings plus POP_SMOOTH; only "off" means the 12 Hz motion).
    pub fn smooth_motion(&self) -> bool {
        smooth_motion(&self.motion_smoothing, std::env::var("POP_SMOOTH").ok().as_deref())
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

pub fn voxel_columns(setting: &str, env: Option<&str>) -> bool {
    env.unwrap_or(setting) != "flat"
}

pub fn unlit(setting: &str, env: Option<&str>) -> bool {
    env.unwrap_or(setting) == "unlit"
}

pub fn smooth_scroll(setting: &str, env: Option<&str>) -> bool {
    env.unwrap_or(setting) != "flip"
}

pub fn smooth_motion(setting: &str, env: Option<&str>) -> bool {
    env.unwrap_or(setting) != "off"
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
        assert_eq!(resolve_render("enhanced", "sprite", None, None).0, Enhanced);
        // the RT mode runs classic unless built with the solari feature
        #[cfg(not(feature = "solari"))]
        assert_eq!(resolve_render("rt_experimental", "sprite", None, None).0, Classic);
        #[cfg(feature = "solari")]
        assert_eq!(resolve_render("rt_experimental", "sprite", None, None).0, Rt);
        // env wins over the file; bad env values fall back instead of failing
        #[cfg(not(feature = "solari"))]
        assert_eq!(resolve_render("classic", "sprite", Some("rt_experimental"), Some("voxel")), (Classic, Voxel));
        #[cfg(feature = "solari")]
        assert_eq!(resolve_render("classic", "sprite", Some("rt_experimental"), Some("voxel")), (Rt, Voxel));
        assert_eq!(resolve_render("classic", "sprite", Some("enhanced"), Some("voxel")), (Enhanced, Voxel));
        assert_eq!(resolve_render("classic", "sprite", Some("enhanced"), None), (Enhanced, Sprite));
        assert_eq!(resolve_render("enhanced", "voxel", Some("nonsense"), Some("cubes")), (Classic, Sprite));
    }

    #[test]
    fn enhanced_quality_settings_parse_and_fall_back() {
        let d = Settings::default();
        assert_eq!((d.shadow_quality.as_str(), d.ssao), ("high", true));
        let s = Settings::parse(r#"{"render_mode": "enhanced", "shadow_quality": "low", "ssao": false}"#);
        assert_eq!((s.render_mode.as_str(), s.shadow_quality.as_str(), s.ssao), ("enhanced", "low", false));
        assert_eq!(Settings::parse(&s.to_json()), s);
        // a file from before R2 (no quality keys) and bad values both give the defaults, per field
        assert_eq!(Settings::parse(r#"{"render_mode": "enhanced"}"#).shadow_quality, "high");
        let s = Settings::parse(r#"{"shadow_quality": "ultra", "ssao": "yes", "volume": 0.5}"#);
        assert_eq!((s.shadow_quality, s.ssao, s.volume), (d.shadow_quality, d.ssao, 0.5));
    }

    #[test]
    fn tile_textures_parse_and_fall_back() {
        assert_eq!(Settings::default().tile_textures, "original");
        let s = Settings::parse(r#"{"tile_textures": "procedural"}"#);
        assert_eq!(s.tile_textures, "procedural");
        assert_eq!(Settings::parse(&s.to_json()), s);
        let s = Settings::parse(r#"{"tile_textures": "hd", "volume": 0.5}"#);
        assert_eq!((s.tile_textures.as_str(), s.volume), ("original", 0.5));
        assert_eq!(Settings::parse(r#"{"tile_textures": 1}"#).tile_textures, "original");
    }

    #[test]
    fn column_style_parse_env_and_fall_back() {
        assert_eq!(Settings::default().column_style, "voxel");
        let s = Settings::parse(r#"{"column_style": "flat"}"#);
        assert_eq!(s.column_style, "flat");
        assert_eq!(Settings::parse(&s.to_json()), s);
        let s = Settings::parse(r#"{"column_style": "round", "volume": 0.5}"#);
        assert_eq!((s.column_style.as_str(), s.volume), ("voxel", 0.5));
        assert!(voxel_columns("voxel", None) && !voxel_columns("flat", None));
        assert!(voxel_columns("flat", Some("voxel")) && !voxel_columns("voxel", Some("flat")));
    }

    #[test]
    fn object_style_parse_env_and_fall_back() {
        assert_eq!(Settings::default().object_style, "voxel");
        assert_eq!(Settings::parse(r#"{"object_style": "flat"}"#).object_style, "flat");
        let s = Settings::parse(r#"{"object_style": "boxes", "column_style": "flat"}"#);
        assert_eq!((s.object_style.as_str(), s.column_style.as_str()), ("voxel", "flat"));
        assert!(Settings::parse(&Settings::default().to_json()).object_style == "voxel");
        // POP_OBJECTS=flat wins over the setting (same rule as the columns)
        assert!(!voxel_columns("voxel", Some("flat")) && voxel_columns("flat", Some("voxel")));
    }

    #[test]
    fn voxel_look_parse_env_and_fall_back() {
        let d = Settings::default();
        assert_eq!((d.voxel_normals.as_str(), d.voxel_shading.as_str()), ("smooth", "lit"));
        let s = Settings::parse(r#"{"voxel_normals": "flat", "voxel_shading": "unlit"}"#);
        assert_eq!((s.voxel_normals.as_str(), s.voxel_shading.as_str()), ("flat", "unlit"));
        assert_eq!(Settings::parse(&s.to_json()), s);
        // bad values fall back per field, the rest still loads
        let s = Settings::parse(r#"{"voxel_normals": "phong", "voxel_shading": 1, "volume": 0.5}"#);
        assert_eq!((s.voxel_normals, s.voxel_shading, s.volume), (d.voxel_normals, d.voxel_shading, 0.5));
        // env wins over the file
        assert!(voxel_columns("smooth", None) && !voxel_columns("smooth", Some("flat")) && voxel_columns("flat", Some("smooth")));
        assert!(!unlit("lit", None) && unlit("lit", Some("unlit")) && !unlit("unlit", Some("lit")));
    }

    #[test]
    fn camera_scroll_parse_env_and_fall_back() {
        assert_eq!(Settings::default().camera_scroll, "smooth");
        let s = Settings::parse(r#"{"camera_scroll": "flip"}"#);
        assert_eq!(s.camera_scroll, "flip");
        assert_eq!(Settings::parse(&s.to_json()), s);
        let s = Settings::parse(r#"{"camera_scroll": "zoom", "volume": 0.5}"#);
        assert_eq!((s.camera_scroll.as_str(), s.volume), ("smooth", 0.5));
        assert_eq!(Settings::parse(r#"{"camera_scroll": 2}"#).camera_scroll, "smooth");
        // POP_CAMERA wins over the file
        assert!(smooth_scroll("smooth", None) && !smooth_scroll("flip", None));
        assert!(!smooth_scroll("smooth", Some("flip")) && smooth_scroll("flip", Some("smooth")));
    }

    #[test]
    fn motion_smoothing_parse_env_and_fall_back() {
        assert_eq!(Settings::default().motion_smoothing, "position");
        let s = Settings::parse(r#"{"motion_smoothing": "off"}"#);
        assert_eq!(s.motion_smoothing, "off");
        assert_eq!(Settings::parse(&s.to_json()), s);
        let s = Settings::parse(r#"{"motion_smoothing": "full", "volume": 0.5}"#);
        assert_eq!((s.motion_smoothing.as_str(), s.volume), ("position", 0.5));
        assert!(smooth_motion("position", None) && !smooth_motion("off", None));
        assert!(!smooth_motion("position", Some("off")) && smooth_motion("off", Some("position")));
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
