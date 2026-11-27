//! Self-update from the GitHub releases of the project. At every start, before anything else is loaded, the app asks
//! GitHub for the latest release (`curl.exe`, a few seconds at most, silent when offline). If it is newer than this
//! build, a small window offers it (Y / N, or the buttons; C shows the changelog since this build, which stays open during the
//! download); a yes downloads the release zip with a progress bar and a
//! Cancel button, checks it, unpacks it, replaces the exe and the other released files and starts the new version.
//! Same approach as `soundfont.rs` / `getdata.rs`: Windows' own `curl.exe`, `tar.exe`, `certutil.exe`, a worker thread, a
//! cancel flag. A running exe cannot be overwritten but can be renamed: the old one becomes `<exe>.old` and is deleted at a
//! later start. Only a released `PrinceOfPersia.exe` updates itself (a `cargo run` build never asks). Switch off with
//! `"update_check": false` in settings.json or the environment variable `POP_SKIP_UPDATE`; `POP_UPDATE_AS=0.5.0` pretends
//! to be that version (tests).
use crate::soundfont::{curl, run, CANCELLED};
use bevy::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// The newest releases (newest first): one request gives the latest release and the release notes of every release since this build.
/// shortcut: only the last 20 releases, so the changelog of a build older than that is cut off; upgrade by paging when that matters.
const API: &str = "https://api.github.com/repos/Grownz/PrinceOfPersia2.5D/releases?per_page=20";
/// The page the version line of the menu opens.
pub const RELEASES: &str = "https://github.com/Grownz/PrinceOfPersia2.5D/releases";
const ASSET: &str = "PrinceOfPersia-x64.zip";
const EXE: &str = "PrinceOfPersia.exe";
/// Files of the user that an update never touches.
const KEEP: [&str; 3] = ["settings.json", "pop.log", "PRINCE.SAV"];

#[derive(Clone, PartialEq, Debug)]
pub struct Offer {
    pub tag: String,
    url: String,
    bytes: u64,
    sha256: Option<String>,
    /// the notes of every release newer than this build, newest first (plain text; moved into `Notes` when the window opens)
    notes: String,
}

#[derive(Clone, PartialEq, Debug)]
enum Status {
    Offer(Offer),
    Busy { phase: &'static str, done: u64, total: u64 },
    Failed(String),
    /// start the game without updating
    Skip,
    /// installed: start the new version
    Done,
}

static STATE: Mutex<Option<Status>> = Mutex::new(None);
static CANCEL: AtomicBool = AtomicBool::new(false);

fn status() -> Option<Status> {
    STATE.lock().ok().and_then(|g| g.clone())
}

fn set(s: Status) {
    if let Ok(mut g) = STATE.lock() {
        *g = Some(s);
    }
}

/// `"v0.6.1"` -> (0, 6, 1)
fn version(s: &str) -> Option<(u32, u32, u32)> {
    let mut p = s.trim().trim_start_matches('v').split('.').map(|n| n.split(['-', '+']).next().unwrap_or("").parse::<u32>());
    Some((p.next()?.ok()?, p.next()?.ok()?, p.next()?.ok()?))
}

fn current() -> String {
    std::env::var("POP_UPDATE_AS").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

/// Release notes (the Markdown of a CHANGELOG.md section) as plain text for the window.
fn plain(md: &str) -> String {
    md.lines()
        .map(|l| {
            let l = l.trim_end().replace(['`', '*'], "");
            match l.strip_prefix("### ") {
                Some(h) => format!("{h}:"),
                None => l.strip_prefix("- ").map_or(l.clone(), |b| format!("  \u{2022} {b}")),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Is the newest release in `releases_json` (GitHub's release list) newer than `have`, with a zip we can use? The offer carries the
/// notes of all releases newer than `have`, newest first.
fn parse_offer(releases_json: &str, have: &str) -> Option<Offer> {
    let v: serde_json::Value = serde_json::from_str(releases_json).ok()?;
    let have = version(have)?;
    let mut newer: Vec<_> = v.as_array()?.iter().filter(|r| !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false)).filter_map(|r| Some((version(r["tag_name"].as_str()?)?, r))).filter(|(n, _)| *n > have).collect();
    newer.sort_by(|a, b| b.0.cmp(&a.0));
    let latest = newer.first()?.1;
    let tag = latest["tag_name"].as_str()?;
    let a = latest["assets"].as_array()?.iter().find(|a| a["name"] == ASSET)?;
    let url = a["browser_download_url"].as_str()?.to_string();
    if !url.starts_with("https://") {
        return None;
    }
    let notes = newer.iter().map(|(_, r)| format!("Version {}\n{}", r["tag_name"].as_str().unwrap_or("?").trim_start_matches('v'), plain(r["body"].as_str().unwrap_or("")))).collect::<Vec<_>>().join("\n\n");
    Some(Offer { tag: tag.into(), url, bytes: a["size"].as_u64()?, sha256: a["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase), notes })
}

fn exe_path() -> Option<PathBuf> {
    std::env::current_exe().ok().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(EXE)))
}

/// Start of the app: clean up the old exe of the last update, then look for a newer release.
pub fn check(update_check: bool) -> Option<Offer> {
    let exe = exe_path()?; // a build from `cargo run` never asks
    let _ = std::fs::remove_file(old_exe(&exe));
    if !update_check || std::env::var_os("POP_SKIP_UPDATE").is_some() {
        return None;
    }
    let tmp = std::env::temp_dir().join("pop_latest_release.json");
    let _ = std::fs::remove_file(&tmp);
    let mut c = curl(API, &tmp);
    c.args(["--max-time", "4", "--connect-timeout", "2"]).stdin(std::process::Stdio::null());
    run(&mut c, &AtomicBool::new(false)).ok()?;
    let text = std::fs::read_to_string(&tmp).ok();
    let _ = std::fs::remove_file(&tmp);
    parse_offer(&text?, &current())
}

fn old_exe(exe: &Path) -> PathBuf {
    exe.with_extension("exe.old")
}

fn start(o: Offer) {
    CANCEL.store(false, Ordering::SeqCst);
    set(Status::Busy { phase: "downloading", done: 0, total: o.bytes });
    std::thread::spawn(move || match install(&o) {
        Ok(()) => set(Status::Done),
        Err(e) if e == CANCELLED => set(Status::Skip),
        Err(e) => {
            bevy::log::warn!("update: {e}");
            set(Status::Failed(e));
        }
    });
}

fn install(o: &Offer) -> Result<(), String> {
    let exe = exe_path().ok_or("only the released PrinceOfPersia.exe can update itself")?;
    let dir = exe.parent().ok_or("no folder")?.to_path_buf();
    let (part, tmp) = (dir.join("update.zip.part"), dir.join("update.tmp"));
    let res = fetch(o, &part, &tmp, &exe, &dir);
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

fn fetch(o: &Offer, part: &Path, tmp: &Path, exe: &Path, dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(part);
    let mut child = curl(&o.url, part).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().map_err(|e| format!("cannot run curl.exe: {e}"))?;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if CANCEL.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED.into());
        }
        set(Status::Busy { phase: "downloading", done: std::fs::metadata(part).map_or(0, |m| m.len()), total: o.bytes });
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    if !status.success() {
        return Err("download failed (network?)".into());
    }
    if std::fs::metadata(part).map_or(0, |m| m.len()) != o.bytes {
        return Err("unexpected file size".into());
    }
    if let Some(want) = &o.sha256 {
        let out = crate::soundfont::cmd("certutil.exe").stdout(std::process::Stdio::piped()).arg("-hashfile").arg(part).arg("SHA256").output().map_err(|e| format!("cannot run certutil.exe: {e}"))?;
        let got = String::from_utf8_lossy(&out.stdout).lines().nth(1).unwrap_or("").replace(' ', "").to_lowercase();
        if &got != want {
            return Err("the download does not match GitHub's checksum".into());
        }
    }
    set(Status::Busy { phase: "installing", done: 0, total: 0 });
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let mut tar = crate::soundfont::cmd("tar.exe");
    tar.arg("-xf").arg(part).arg("-C").arg(tmp);
    run(&mut tar, &CANCEL)?;
    let src = find_exe_dir(tmp).ok_or("the release has no PrinceOfPersia.exe")?;
    // point of no return: from here on no cancel
    let old = old_exe(exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("cannot replace the exe (no write access?): {e}"))?;
    if let Err(e) = std::fs::copy(src.join(EXE), exe) {
        let _ = std::fs::remove_file(exe);
        let _ = std::fs::rename(&old, exe); // put the old one back
        return Err(format!("cannot write the new exe: {e}"));
    }
    for e in std::fs::read_dir(&src).map_err(|e| e.to_string())?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.path().is_file() && !name.eq_ignore_ascii_case(EXE) && !KEEP.iter().any(|k| k.eq_ignore_ascii_case(&name)) {
            let _ = std::fs::copy(e.path(), dir.join(&name));
        }
    }
    Ok(())
}

/// The folder of the unpacked release that holds the exe (the zip has one top folder).
fn find_exe_dir(root: &Path) -> Option<PathBuf> {
    if root.join(EXE).is_file() {
        return Some(root.to_path_buf());
    }
    std::fs::read_dir(root).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_exe_dir(&e.path()))
}

/// Start the app again (`skip`: without asking for the update again).
fn relaunch(skip: bool) {
    if let Ok(exe) = std::env::current_exe() {
        let mut c = Command::new(exe);
        if skip {
            c.env("POP_SKIP_UPDATE", "1");
        }
        let _ = c.spawn();
    }
}

#[derive(Component)]
struct Msg;
#[derive(Component)]
struct Bar;
#[derive(Component)]
struct BarFill;
#[derive(Component)]
struct Btn(usize);
#[derive(Component)]
struct BtnText(usize);
/// The scrolling box of the changelog.
#[derive(Component)]
struct NotesBox;

/// The changelog since this build, and whether it is shown (it stays open while the download runs).
#[derive(Resource)]
struct Notes {
    text: String,
    open: bool,
}

const WIN_W: f32 = 720.0;
const WIN_H: f32 = 320.0;
/// window height with the changelog open
const WIN_H_NOTES: f32 = 600.0;

/// The update window. Returns when the app quits: the game (or the new version) is started by a new process.
pub fn window(mut offer: Offer) {
    let notes = Notes { text: std::mem::take(&mut offer.notes), open: false };
    set(Status::Offer(offer));
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "Prince of Persia x64 - update".into(), resolution: (WIN_W as u32, WIN_H as u32).into(), ..default() }), ..default() }).set(bevy::log::LogPlugin { custom_layer: crate::log_file_layer, ..default() }))
        .insert_resource(notes)
        .add_systems(Startup, setup)
        .add_systems(Update, ui)
        .run();
}

fn setup(mut cmds: Commands, notes: Res<Notes>) {
    cmds.spawn(Camera2d);
    cmds.spawn(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Px(18.0), padding: UiRect::all(Val::Px(24.0)), ..default() }).with_children(|p| {
        p.spawn((Text::new(""), TextFont::from_font_size(20.0), TextLayout::justify(Justify::Center), Msg));
        p.spawn((Node { display: Display::None, width: Val::Px(660.0), flex_basis: Val::Px(0.0), flex_grow: 1.0, min_height: Val::Px(0.0), padding: UiRect::all(Val::Px(10.0)), border: UiRect::all(Val::Px(2.0)), overflow: Overflow::scroll_y(), ..default() }, BorderColor::all(Color::srgb(0.8, 0.8, 0.85)), BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)), ScrollPosition::default(), NotesBox)).with_children(|p| {
            // flex_shrink 0: the text keeps its full height, the box scrolls
            p.spawn((Text::new(notes.text.clone()), TextFont::from_font_size(16.0), Node { width: Val::Percent(100.0), flex_shrink: 0.0, ..default() }));
        });
        p.spawn((Node { width: Val::Px(480.0), height: Val::Px(22.0), border: UiRect::all(Val::Px(2.0)), ..default() }, BorderColor::all(Color::srgb(0.8, 0.8, 0.85)), BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)), Bar)).with_children(|p| {
            p.spawn((Node { height: Val::Percent(100.0), width: Val::Percent(0.0), ..default() }, BackgroundColor(Color::srgb(1.0, 0.85, 0.5)), BarFill));
        });
        p.spawn(Node { column_gap: Val::Px(20.0), ..default() }).with_children(|p| {
            for i in 0..3 {
                p.spawn((Node { padding: UiRect::axes(Val::Px(26.0), Val::Px(9.0)), border: UiRect::all(Val::Px(2.0)), ..default() }, BorderColor::all(Color::srgb(0.8, 0.8, 0.85)), BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.10)), Btn(i))).with_children(|p| {
                    p.spawn((Text::new(""), TextFont::from_font_size(20.0), BtnText(i)));
                });
            }
        });
    });
}

/// What the window shows for a status: message, bar fraction, button labels. The last button of an offer and of a download is the
/// changelog toggle (`notes`: is there a changelog, and is it open).
fn shape(s: &Status, notes: Option<bool>) -> (String, Option<f32>, Vec<&'static str>) {
    let toggle = notes.map(|open| if open { "Hide changelog (C)" } else { "Changelog (C)" });
    let (text, frac, mut labels) = match s {
        Status::Offer(o) => (format!("Version {} is available (you have {}).\n\nDownload and install it now?  (Y / N)", o.tag.trim_start_matches('v'), current()), None, vec!["Yes (Y)", "No (N)"]),
        Status::Busy { phase, done, total } => (format!("{phase}..."), Some(if *total > 0 { *done as f32 / *total as f32 } else { 1.0 }), vec!["Cancel (Esc)"]),
        Status::Failed(e) => return (format!("Update failed: {e}\n\nThe game starts with the installed version."), None, vec!["Continue"]),
        Status::Skip | Status::Done => return (String::new(), None, vec![]),
    };
    labels.extend(toggle);
    (text, frac, labels)
}

#[allow(clippy::type_complexity)]
fn ui(
    k: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut exit: MessageWriter<AppExit>,
    mut msg: Single<&mut Text, (With<Msg>, Without<BtnText>)>,
    mut bar: Single<&mut Node, (With<Bar>, Without<BarFill>, Without<Btn>)>,
    mut fill: Single<&mut Node, (With<BarFill>, Without<Bar>, Without<Btn>)>,
    mb: Res<ButtonInput<MouseButton>>,
    mut win: Single<&mut Window>,
    hits: Query<(&Btn, &ComputedNode, &UiGlobalTransform)>,
    mut all_btns: Query<(&Btn, &mut Node), (Without<Bar>, Without<BarFill>)>,
    mut labels: Query<(&BtnText, &mut Text), Without<Msg>>,
    mut notes: ResMut<Notes>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut panel: Single<(&mut Node, &mut ScrollPosition), (With<NotesBox>, Without<Bar>, Without<BarFill>, Without<Btn>)>,
) {
    let Some(st) = status() else { return };
    // what was asked: button 0 is the main action of a state, button 1 (Offer only) is "No"; the changelog toggle is the last button
    // (2 in an offer, 1 during the download)
    let mut press: Option<usize> = None;
    if let (true, Some(c)) = (mb.just_pressed(MouseButton::Left), win.physical_cursor_position()) {
        for (b, cn, tf) in &hits {
            if cn.size().x > 0.0 && cn.contains_point(*tf, c) {
                press = Some(b.0);
            }
        }
    }
    // Y and N by the letter on the key, not its position (a German keyboard has Y where a US one has Z)
    let letters: Vec<String> = typed.read().filter(|e| e.state.is_pressed()).filter_map(|e| match &e.logical_key { bevy::input::keyboard::Key::Character(c) => Some(c.to_lowercase()), _ => None }).collect();
    let yes = letters.iter().any(|c| c == "y") || k.just_pressed(KeyCode::Enter);
    let no = letters.iter().any(|c| c == "n") || k.just_pressed(KeyCode::Escape);
    let has_notes = !notes.text.is_empty();
    let toggle_button = if matches!(st, Status::Offer(_)) { 2 } else { 1 };
    if has_notes && matches!(st, Status::Offer(_) | Status::Busy { .. }) && (press == Some(toggle_button) || letters.iter().any(|c| c == "c")) {
        notes.open = !notes.open;
    }
    match &st {
        Status::Offer(o) => {
            if press == Some(0) || yes {
                start(o.clone()); // the changelog, when open, stays open: the window and its state outlive the download
            } else if press == Some(1) || no {
                set(Status::Skip);
            }
        }
        Status::Busy { .. } => {
            if press == Some(0) || no {
                CANCEL.store(true, Ordering::SeqCst);
            }
        }
        Status::Failed(_) => {
            if press == Some(0) || yes || no {
                set(Status::Skip);
            }
        }
        Status::Skip => {
            relaunch(true);
            exit.write(AppExit::Success);
        }
        Status::Done => {
            relaunch(false);
            exit.write(AppExit::Success);
        }
    }
    let (text, frac, labs) = shape(&st, (has_notes && matches!(st, Status::Offer(_) | Status::Busy { .. })).then_some(notes.open));
    if msg.0 != text {
        msg.0 = text;
    }
    // the changelog box and the window height that fits it; scrolling: wheel, arrows, Page Up / Down
    let show = notes.open && has_notes && matches!(st, Status::Offer(_) | Status::Busy { .. });
    let d = if show { Display::Flex } else { Display::None };
    if panel.0.display != d {
        panel.0.display = d;
    }
    let h = if show { WIN_H_NOTES } else { WIN_H };
    if (win.resolution.height() - h).abs() > 0.5 {
        win.resolution.set(WIN_W, h);
    }
    let mut dy = 0.0;
    for e in wheel.read() {
        dy -= match e.unit {
            bevy::input::mouse::MouseScrollUnit::Line => e.y * 40.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => e.y,
        };
    }
    for (key, step) in [(KeyCode::ArrowDown, 40.0), (KeyCode::ArrowUp, -40.0), (KeyCode::PageDown, 240.0), (KeyCode::PageUp, -240.0)] {
        if k.just_pressed(key) {
            dy += step;
        }
    }
    if show && dy != 0.0 {
        panel.1.y = (panel.1.y + dy).max(0.0); // the layout clamps the far end
    }
    let d = if frac.is_some() { Display::Flex } else { Display::None };
    if bar.display != d {
        bar.display = d;
    }
    fill.width = Val::Percent(frac.unwrap_or(0.0) * 100.0);
    for (b, mut n) in &mut all_btns {
        let d = if b.0 < labs.len() { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for (b, mut t) in &mut labels {
        let s = labs.get(b.0).copied().unwrap_or("");
        if t.0 != s {
            t.0 = s.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert!(version("v0.6.1") > version("0.6.0"));
        assert!(version("v0.10.0") > version("0.9.9"));
        assert_eq!(version("v1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(version("nightly"), None);
    }

    #[test]
    fn offer_only_for_a_newer_release_with_the_zip() {
        let j = |tag: &str, name: &str| format!(r#"[{{"tag_name":"{tag}","body":"","assets":[{{"name":"{name}","size":10,"browser_download_url":"https://github.com/x/y.zip","digest":"sha256:ABC"}}]}}]"#);
        let o = parse_offer(&j("v0.6.1", ASSET), "0.6.0").unwrap();
        assert_eq!((o.bytes, o.sha256.as_deref()), (10, Some("abc")));
        assert!(parse_offer(&j("v0.6.0", ASSET), "0.6.0").is_none(), "same version");
        assert!(parse_offer(&j("v0.5.9", ASSET), "0.6.0").is_none(), "older");
        assert!(parse_offer(&j("v0.7.0", "other.zip"), "0.6.0").is_none(), "no zip");
        assert!(parse_offer("not json", "0.6.0").is_none());
        assert!(parse_offer("[]", "0.6.0").is_none(), "no releases");
    }

    /// The changelog holds every release since the build, newest first; drafts, prereleases and older releases stay out.
    #[test]
    fn the_changelog_covers_the_releases_since_this_build() {
        let r = |tag: &str, body: &str, extra: &str| format!(r#"{{"tag_name":"{tag}","body":"{body}"{extra},"assets":[{{"name":"{ASSET}","size":1,"browser_download_url":"https://github.com/x/y.zip"}}]}}"#);
        let json = format!("[{},{},{},{},{}]", r("v0.6.17", "### Added\\n- **New** `thing`", ""), r("v0.6.16", "### Fixed\\n- old fix", ""), r("v0.7.0-rc1", "rc", r#","prerelease":true"#), r("v0.6.15", "ancient", ""), r("v0.6.18", "draft", r#","draft":true"#));
        let o = parse_offer(&json, "0.6.15").unwrap();
        assert_eq!(o.tag, "v0.6.17");
        assert_eq!(o.notes, "Version 0.6.17\nAdded:\n  \u{2022} New thing\n\nVersion 0.6.16\nFixed:\n  \u{2022} old fix");
    }

    #[test]
    fn the_toggle_is_the_last_button() {
        let o = Status::Offer(Offer { tag: "v1.0.0".into(), url: String::new(), bytes: 1, sha256: None, notes: String::new() });
        assert_eq!(shape(&o, None).2, vec!["Yes (Y)", "No (N)"]);
        assert_eq!(shape(&o, Some(false)).2, vec!["Yes (Y)", "No (N)", "Changelog (C)"]);
        assert_eq!(shape(&Status::Busy { phase: "downloading", done: 0, total: 1 }, Some(true)).2, vec!["Cancel (Esc)", "Hide changelog (C)"]);
    }
}
