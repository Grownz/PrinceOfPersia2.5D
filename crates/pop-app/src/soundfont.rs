//! Downloadable MIT-licensed General MIDI SoundFonts for the `soundfont` sound mode (M7 WP9, docs/audio.md 7).
//! Nothing is bundled: the menu (Settings > SoundFonts) asks the user to confirm, shows the licence duty, then downloads with
//! Windows' own `curl.exe` and `tar.exe` (no HTTP / archive crates) into `soundfonts/` next to the exe, together with the
//! licence text of the source. The download state lives in a static so the menu can read it without plumbing. A running
//! download (curl, then tar for the .deb) is cancelled through a flag the worker thread polls: it kills the child process and
//! removes the partial files. Installed fonts can be deleted again (SF2 and licence text).
use std::path::PathBuf;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct Font {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    /// expected size of the download (sanity check; from the server's Content-Length, 2026-11)
    pub bytes: u64,
    /// file name of the SF2 next to the exe's `soundfonts/` folder
    pub file: &'static str,
    /// licence text of the source, saved next to the font (MuseScore: direct file; FluidR3: inside the .deb)
    pub licence_url: Option<&'static str>,
    pub licence_file: &'static str,
    /// the .deb of Debian's fluid-soundfont-gm: the SF2 is unpacked from it
    pub deb_inner: Option<&'static str>,
}

pub const FONTS: [Font; 2] = [
    Font {
        id: "musescore",
        name: "MuseScore_General",
        url: "https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General.sf2",
        bytes: 215_614_036,
        file: "MuseScore_General.sf2",
        licence_url: Some("https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/MuseScore_General_License.md"),
        licence_file: "MuseScore_General_LICENSE.md",
        deb_inner: None,
    },
    Font {
        id: "fluidr3",
        name: "FluidR3_GM",
        url: "https://deb.debian.org/debian/pool/main/f/fluid-soundfont/fluid-soundfont-gm_3.1-6_all.deb",
        bytes: 119_613_032,
        file: "FluidR3_GM.sf2",
        licence_url: None,
        licence_file: "FluidR3_GM_COPYRIGHT.txt",
        deb_inner: Some("FluidR3_GM.sf2"),
    },
];

/// Short licence line for the confirmation page.
pub fn licence_line(i: usize) -> &'static str {
    match i {
        0 => "MIT, (c) F. Wen, M. Cowgill, S. C. Collins",
        _ => "MIT, (c) 2000-2008 Frank Wen",
    }
}

pub fn size_mb(i: usize) -> u64 {
    FONTS[i].bytes / 1_000_000
}

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Absent,
    /// the licence notice is shown; Enter again downloads
    Confirm,
    Busy { phase: &'static str, done: u64, total: u64 },
    Failed(String),
    Installed,
    /// installed, the user asked to delete it and has to confirm
    ConfirmDelete,
}

pub(crate) const CANCELLED: &str = "cancelled";
static CANCEL: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];

/// `None` = derive from the files on disk.
static STATE: Mutex<[Option<Status>; 2]> = Mutex::new([None, None]);

pub fn dir() -> PathBuf {
    crate::flow::exe_dir().join("soundfonts")
}

pub fn path(i: usize) -> PathBuf {
    dir().join(FONTS[i].file)
}

/// id of the font in settings (`"musescore"`), else the first installed one.
pub fn index_of(id: &str) -> Option<usize> {
    FONTS.iter().position(|f| f.id == id)
}

fn installed(i: usize) -> bool {
    std::fs::metadata(path(i)).is_ok_and(|m| m.len() > 1_000_000)
}

pub fn status(i: usize) -> Status {
    let s = STATE.lock().map(|g| g[i].clone()).unwrap_or(None);
    s.unwrap_or_else(|| if installed(i) { Status::Installed } else { Status::Absent })
}

/// Usable for music now (downloaded; the delete confirmation still counts).
pub fn available(i: usize) -> bool {
    matches!(status(i), Status::Installed | Status::ConfirmDelete)
}

fn set(i: usize, s: Option<Status>) {
    if let Ok(mut g) = STATE.lock() {
        g[i] = s;
    }
}

/// First Enter on a missing font: show the licence notice.
pub fn ask(i: usize) {
    if status(i) == Status::Absent || matches!(status(i), Status::Failed(_)) {
        set(i, Some(Status::Confirm));
    }
}

/// Close every open confirmation (licence notice, delete question).
pub fn cancel_all() {
    for i in 0..FONTS.len() {
        if matches!(status(i), Status::Confirm | Status::ConfirmDelete) {
            set(i, None);
        }
    }
}

/// Ask before deleting an installed font.
pub fn ask_delete(i: usize) {
    if status(i) == Status::Installed {
        set(i, Some(Status::ConfirmDelete));
    }
}

/// Abort a running download: the worker thread sees the flag within 100 ms, kills curl / tar and removes the partial files.
pub fn cancel(i: usize) {
    if matches!(status(i), Status::Busy { .. }) {
        CANCEL[i].store(true, Ordering::SeqCst);
    }
}

/// Remove the SF2 and its licence text from `d` (missing files are fine).
fn delete_in(d: &std::path::Path, i: usize) -> std::io::Result<()> {
    for f in [FONTS[i].file, FONTS[i].licence_file] {
        match std::fs::remove_file(d.join(f)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Delete an installed font (after the user confirmed); a failure shows as `Failed`.
pub fn delete(i: usize) {
    set(i, delete_in(&dir(), i).err().map(|e| Status::Failed(format!("cannot delete: {e}"))));
}

/// Is any font waiting for the user's confirmation (the menu then shows the notice)?
pub fn confirming() -> Option<usize> {
    (0..FONTS.len()).find(|&i| status(i) == Status::Confirm)
}

/// Second Enter: download in a thread. The curl process is the only network access; nothing else is sent.
pub fn start(i: usize) {
    if status(i) != Status::Confirm {
        return;
    }
    CANCEL[i].store(false, Ordering::SeqCst);
    set(i, Some(Status::Busy { phase: "downloading", done: 0, total: FONTS[i].bytes }));
    std::thread::spawn(move || match download(i) {
        Ok(()) => set(i, None),
        Err(e) => {
            let _ = std::fs::remove_file(dir().join(format!("{}.part", FONTS[i].file)));
            let _ = std::fs::remove_dir_all(dir().join("unpack.tmp"));
            if e == CANCELLED {
                return set(i, None); // back to "absent"
            }
            bevy::log::warn!("soundfont {}: {e}", FONTS[i].id);
            set(i, Some(Status::Failed(e)));
        }
    });
}

/// A system program as a command that never opens a console window (the game is a GUI program: a console child would flash one).
pub(crate) fn cmd(exe: &str) -> Command {
    let mut c = Command::new(sys(exe));
    // the release exe has no console, so no standard handles: a child that inherits them fails with "invalid handle"
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // CREATE_NO_WINDOW
    c
}

pub(crate) fn sys(exe: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    root.join("System32").join(exe)
}

/// Run a child to the end; `cancel` kills it (Err(CANCELLED)). Only the last stderr line is kept (small outputs, no pipe stall).
pub(crate) fn run(cmd: &mut Command, cancel: &AtomicBool) -> Result<(), String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("cannot run {:?}: {e}", cmd.get_program()))?;
    loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            let mut err = String::new();
            if let Some(mut r) = child.stderr.take() {
                let _ = r.read_to_string(&mut err);
            }
            return if st.success() { Ok(()) } else { Err(err.lines().last().unwrap_or("failed").to_string()) };
        }
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED.into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub(crate) fn curl(url: &str, to: &std::path::Path) -> Command {
    let mut c = cmd("curl.exe");
    c.args(["-L", "--fail", "--silent", "--show-error", "--proto", "=https", "--max-redirs", "5", "-o"]).arg(to).arg(url);
    c
}

fn download(i: usize) -> Result<(), String> {
    let f = &FONTS[i];
    let d = dir();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let part = d.join(format!("{}.part", f.file));
    let _ = std::fs::remove_file(&part);
    let mut child = curl(f.url, &part).stdin(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("cannot run curl.exe: {e}"))?;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if CANCEL[i].load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED.into());
        }
        let done = std::fs::metadata(&part).map_or(0, |m| m.len());
        set(i, Some(Status::Busy { phase: "downloading", done, total: f.bytes }));
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err("download failed (network?)".into());
    }
    if std::fs::metadata(&part).map_or(0, |m| m.len()) != f.bytes {
        let _ = std::fs::remove_file(&part);
        return Err("unexpected file size (the source changed?)".into());
    }
    if let Some(url) = f.licence_url {
        run(&mut curl(url, &d.join(f.licence_file)), &CANCEL[i])?;
    }
    match f.deb_inner {
        None => {
            if CANCEL[i].load(Ordering::SeqCst) {
                return Err(CANCELLED.into());
            }
            std::fs::rename(&part, path(i)).map_err(|e| e.to_string())?
        }
        Some(name) => {
            set(i, Some(Status::Busy { phase: "unpacking", done: 0, total: 0 }));
            let tmp = d.join("unpack.tmp");
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            let res = unpack_deb(&part, &tmp, name, &CANCEL[i]).and_then(|(sf2, copyright)| {
                if CANCEL[i].load(Ordering::SeqCst) {
                    return Err(CANCELLED.to_string());
                }
                if let Some(c) = copyright {
                    let _ = std::fs::copy(c, d.join(f.licence_file));
                }
                std::fs::rename(sf2, path(i)).map_err(|e| e.to_string())
            });
            let _ = std::fs::remove_dir_all(&tmp);
            let _ = std::fs::remove_file(&part);
            res?;
        }
    }
    Ok(())
}

/// A .deb is an `ar` archive holding `data.tar.*`; Windows' bsdtar reads both. Returns (the SF2, the copyright file if found).
fn unpack_deb(deb: &std::path::Path, tmp: &std::path::Path, name: &str, cancel: &AtomicBool) -> Result<(PathBuf, Option<PathBuf>), String> {
    let tar = |a: &std::path::Path| {
        let mut c = cmd("tar.exe");
        c.arg("-xf").arg(a).arg("-C").arg(tmp);
        run(&mut c, cancel)
    };
    tar(deb)?;
    let data = std::fs::read_dir(tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("data.tar")));
    tar(&data.ok_or("no data.tar in the package")?)?;
    let sf2 = find(tmp, &|n| n.eq_ignore_ascii_case(name)).ok_or("the SoundFont is not in the package")?;
    let copyright = find(tmp, &|n| n == "copyright");
    Ok((sf2, copyright))
}

fn find(dir: &std::path::Path, want: &dyn Fn(&str) -> bool) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find(&p, want) {
                return Some(f);
            }
        } else if want(&e.file_name().to_string_lossy()) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
pub static LOCK: Mutex<()> = Mutex::new(()); // tests share the static state

pub fn debug_clear(i: usize) {
    set(i, None);
}

/// Screenshot hook (POP_MENU_FAKE): show a made-up state without a download.
pub fn debug_set(i: usize, s: Status) {
    set(i, Some(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_and_delete_transitions() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // index 1 only: the confirm test below uses font 0 and calls cancel_all
        set(1, Some(Status::Busy { phase: "downloading", done: 5, total: 10 }));
        CANCEL[1].store(false, Ordering::SeqCst);
        cancel(1);
        assert!(CANCEL[1].load(Ordering::SeqCst), "a running download gets the flag");
        CANCEL[1].store(false, Ordering::SeqCst);
        set(1, Some(Status::Failed("x".into())));
        cancel(1);
        assert!(!CANCEL[1].load(Ordering::SeqCst), "nothing to cancel");
        set(1, Some(Status::ConfirmDelete));
        cancel_all(); // Esc / No: back to what is on disk
        assert!(matches!(status(1), Status::Absent | Status::Installed));
        set(1, None);
        // delete removes the SF2 and the licence, nothing else
        let d = std::env::temp_dir().join(format!("pop_sf_delete_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for f in [FONTS[1].file, FONTS[1].licence_file, "keep.txt"] {
            std::fs::write(d.join(f), "x").unwrap();
        }
        delete_in(&d, 1).unwrap();
        delete_in(&d, 1).unwrap(); // already gone is fine
        assert!(!d.join(FONTS[1].file).exists() && !d.join(FONTS[1].licence_file).exists() && d.join("keep.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A stand-in child process (ping, 30 s) is killed by the flag well before it would end.
    #[test]
    fn run_is_cancellable() {
        let flag = std::sync::Arc::new(AtomicBool::new(false));
        let f2 = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            f2.store(true, Ordering::SeqCst);
        });
        let t = std::time::Instant::now();
        let r = run(Command::new(sys("PING.EXE")).args(["-n", "30", "127.0.0.1"]), &flag);
        assert_eq!(r, Err(CANCELLED.to_string()));
        assert!(t.elapsed().as_secs() < 10);
        assert!(run(Command::new(sys("PING.EXE")).args(["-n", "1", "127.0.0.1"]), &AtomicBool::new(false)).is_ok());
    }

    #[test]
    fn catalog_is_consistent() {
        assert!(FONTS.iter().all(|f| f.url.starts_with("https://") && f.bytes > 50_000_000 && f.file.ends_with(".sf2")));
        assert_eq!(index_of("fluidr3"), Some(1));
        assert!(licence_line(0).contains("MIT") && licence_line(1).contains("MIT"));
        // a font without a licence text source must come from a package that carries one
        assert!(FONTS.iter().all(|f| f.licence_url.is_some() || f.deb_inner.is_some()));
    }

    #[test]
    fn confirm_flow_does_not_download() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set(0, None);
        if installed(0) { return; }
        ask(0);
        assert_eq!(status(0), Status::Confirm);
        assert_eq!(confirming(), Some(0));
        cancel_all();
        assert_eq!(status(0), Status::Absent);
    }

    /// Network test (user-approved downloads, ~335 MB): `cargo test -p pop-app --release real_download -- --ignored --nocapture`.
    /// Downloads through the same code path as the menu, then renders one cue with each font.
    #[test]
    #[ignore]
    fn real_download_and_render() {
        for i in 0..FONTS.len() {
            if !installed(i) {
                ask(i);
                start(i);
                let t = std::time::Instant::now();
                while matches!(status(i), Status::Confirm | Status::Busy { .. }) {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    eprintln!("{}: {:?}", FONTS[i].name, status(i));
                    assert!(t.elapsed().as_secs() < 900, "timeout");
                }
            }
            assert_eq!(status(i), Status::Installed, "{}", FONTS[i].name);
            assert!(dir().join(FONTS[i].licence_file).is_file(), "licence file missing for {}", FONTS[i].name);
            eprintln!("{} installed: {} bytes, licence {} bytes", FONTS[i].name, std::fs::metadata(path(i)).unwrap().len(), std::fs::metadata(dir().join(FONTS[i].licence_file)).unwrap().len());
            let dat = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/MIDISND1.DAT");
            let Ok(snd) = pop_core::dat::Dat::open(dat) else { continue };
            let smf = pop_core::midi::Smf::parse(&snd.get(10024).unwrap()[1..]).unwrap();
            let t = std::time::Instant::now();
            let sf = std::sync::Arc::new(rustysynth::SoundFont::new(&mut std::io::BufReader::new(std::fs::File::open(path(i)).unwrap())).unwrap());
            let load = t.elapsed();
            let t = std::time::Instant::now();
            let pcm = crate::music::render_sf(&smf, &sf, crate::music::SF_RATE);
            eprintln!("{}: load {load:?}, render {:.1} s of audio in {:?}", FONTS[i].name, pcm.len() as f32 / crate::music::SF_RATE as f32, t.elapsed());
            assert!(pcm.iter().filter(|s| s.abs() > 0.01).count() > pcm.len() / 20, "mostly silent");
        }
    }
}
