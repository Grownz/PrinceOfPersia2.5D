//! Optional download of the user's own game files from the Internet Archive (https://archive.org/details/PERSIA_VGA) when
//! the app finds no game data at start. Same approach as `soundfont.rs`: Windows' own `curl.exe` and `tar.exe` (no HTTP or
//! archive crates), a worker thread, a cancel flag, state in a static for the window to read. Nothing is bundled or
//! fetched without the user confirming the licence sentence. The zip is checked (size and SHA-1) and unpacked into a
//! temporary folder first; only the `.DAT` files and PRINCE.EXE are copied to `Original_data` (existing files stay).
use crate::soundfont::{curl, run, sys, CANCELLED};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const PAGE: &str = "https://archive.org/details/PERSIA_VGA";
const URL: &str = "https://archive.org/download/PERSIA_VGA/PERSIA.zip";
const BYTES: u64 = 353_211;
const SHA1: &str = "8705853e34426ed216fe2769ddcacb61f16513a3";
pub const LICENCE: &str = "User confirms that they will comply with the licenses in accordance with the laws of their country.";

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Idle,
    /// the licence sentence is shown; Enter downloads
    Confirm,
    Busy { phase: &'static str, done: u64 },
    Failed(String),
    Done,
}

static STATE: Mutex<Status> = Mutex::new(Status::Idle);
static CANCEL: AtomicBool = AtomicBool::new(false);

pub fn status() -> Status {
    STATE.lock().map(|s| s.clone()).unwrap_or(Status::Idle)
}

fn set(s: Status) {
    if let Ok(mut g) = STATE.lock() {
        *g = s;
    }
}

/// First step: show the licence sentence.
pub fn ask() {
    if matches!(status(), Status::Idle | Status::Failed(_)) {
        set(Status::Confirm);
    }
}

/// Back out of the licence sentence.
pub fn back() {
    if status() == Status::Confirm {
        set(Status::Idle);
    }
}

/// Abort a running download (the worker kills curl / tar and removes the partial files within 100 ms).
pub fn cancel() {
    if matches!(status(), Status::Busy { .. }) {
        CANCEL.store(true, Ordering::SeqCst);
    }
}

/// Second step: download and unpack into `dest` in a thread.
pub fn start(dest: PathBuf) {
    if status() != Status::Confirm {
        return;
    }
    CANCEL.store(false, Ordering::SeqCst);
    set(Status::Busy { phase: "downloading", done: 0 });
    std::thread::spawn(move || match install(&dest) {
        Ok(()) => set(Status::Done),
        Err(e) if e == CANCELLED => set(Status::Idle),
        Err(e) => {
            bevy::log::warn!("game data download: {e}");
            set(Status::Failed(e));
        }
    });
}

/// Files worth keeping from the archive: the game's data and executable (not the DOS installer).
fn wanted(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    n.ends_with(".DAT") && !n.starts_with("INSTALL") || n == "PRINCE.EXE"
}

fn install(dest: &Path) -> Result<(), String> {
    let parent = dest.parent().ok_or("no folder to install into")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let (part, tmp) = (parent.join("PERSIA.zip.part"), parent.join("Original_data.tmp"));
    let res = fetch(&part, &tmp, dest);
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

fn fetch(part: &Path, tmp: &Path, dest: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(part);
    let mut child = curl(URL, part).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().map_err(|e| format!("cannot run curl.exe: {e}"))?;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if CANCEL.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED.into());
        }
        set(Status::Busy { phase: "downloading", done: std::fs::metadata(part).map_or(0, |m| m.len()) });
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    if !status.success() {
        return Err("download failed (network?)".into());
    }
    if std::fs::metadata(part).map_or(0, |m| m.len()) != BYTES || sha1(part)? != SHA1 {
        return Err("the downloaded file is not the expected archive (the source changed?)".into());
    }
    set(Status::Busy { phase: "unpacking", done: 0 });
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp).map_err(|e| e.to_string())?;
    let mut tar = Command::new(sys("tar.exe"));
    tar.arg("-xf").arg(part).arg("-C").arg(tmp);
    run(&mut tar, &CANCEL)?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(tmp).map_err(|e| e.to_string())?.flatten() {
        let (name, to) = (e.file_name(), dest.join(e.file_name()));
        if e.path().is_file() && wanted(&name.to_string_lossy()) && !to.exists() {
            std::fs::copy(e.path(), &to).map_err(|e| format!("cannot write {}: {e}", to.display()))?;
        }
    }
    match crate::missing_files(dest) {
        m if m.is_empty() => Ok(()),
        m => Err(format!("the archive lacks {}", m.join(", "))),
    }
}

/// SHA-1 of a file with Windows' `certutil` (lower-case hex).
fn sha1(f: &Path) -> Result<String, String> {
    let out = Command::new(sys("certutil.exe")).arg("-hashfile").arg(f).arg("SHA1").output().map_err(|e| format!("cannot run certutil.exe: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().nth(1).unwrap_or("").replace(' ', "").to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_game_files_are_kept() {
        assert!(wanted("LEVELS.DAT") && wanted("prince.exe") && wanted("VDUNGEON.DAT"));
        assert!(!wanted("INSTALL.EXE") && !wanted("INSTALL.PDM") && !wanted("DESKTOPD.CFG") && !wanted("PRINCE.PDM"));
    }

    /// Network: downloads the real archive into a temp folder and checks that the game data is complete.
    #[test]
    #[ignore]
    fn real_download() {
        let d = std::env::temp_dir().join("pop_getdata_test");
        let _ = std::fs::remove_dir_all(&d);
        install(&d.join("Original_data")).unwrap();
        assert!(crate::missing_files(&d.join("Original_data")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
