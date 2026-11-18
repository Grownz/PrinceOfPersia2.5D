//! Read-only inventory of the original sound archives (DIGISND*, IBM_SND*, MIDISND*.DAT) for docs/audio.md.
//! `cargo run --release` prints Markdown; `--write` replaces the block between the inventory markers in docs/audio.md.
//! Reads the user's own Original_data (or POP_DATA); writes nothing but that doc section.
use pop_core::dat::Dat;
use std::fmt::Write as _;

const FILES: [&str; 7] = ["DIGISND1", "DIGISND2", "DIGISND3", "IBM_SND1", "IBM_SND2", "MIDISND1", "MIDISND2"];
const BEGIN: &str = "<!-- inventory:begin (tools/sound_inventory --write) -->";
const END: &str = "<!-- inventory:end -->";

fn u16le(r: &[u8], o: usize) -> Option<u16> {
    r.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

/// What a standard MIDI file contains, as far as the remake needs it.
#[derive(Debug, Default, PartialEq)]
struct Midi {
    format: u16,
    tracks: u16,
    division: u16,
    secs: f64,
    notes: u32,
    channels: u16, // bit mask
    programs: Vec<(u8, u8)>, // (channel, program), first per channel
    tempos: usize,
    sysex: u32,
    sysex_head: Vec<u8>, // first bytes of the first sysex
    seq_specific: u32, // meta 0x7F
    extra: usize, // bytes after the last chunk
}

fn vlq(d: &[u8], p: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for _ in 0..4 {
        let b = *d.get(*p)?;
        *p += 1;
        v = (v << 7) | (b & 0x7F) as u32;
        if b < 0x80 {
            return Some(v);
        }
    }
    None
}

fn parse_midi(d: &[u8]) -> Option<Midi> {
    let be16 = |o: usize| d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let be32 = |o: usize| d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    if d.get(0..4)? != b"MThd" {
        return None;
    }
    let mut m = Midi { format: be16(8)?, tracks: be16(10)?, division: be16(12)?, ..Default::default() };
    if m.division & 0x8000 != 0 || m.division == 0 {
        return None; // SMPTE timing: not expected here
    }
    let mut tempo_map: Vec<(u64, u32)> = vec![];
    let mut end = 0u64;
    let mut o = 8 + be32(4)?;
    while let (Some(b"MTrk"), Some(len)) = (d.get(o..o + 4), be32(o + 4)) {
        let t = d.get(o + 8..o + 8 + len)?;
        let (mut p, mut tick, mut status) = (0usize, 0u64, 0u8);
        while p < t.len() {
            tick += vlq(t, &mut p)? as u64;
            let mut b = *t.get(p)?;
            if b >= 0x80 {
                p += 1;
                status = b;
            } else {
                b = status; // running status
            }
            match b {
                0xFF => {
                    let ty = *t.get(p)?;
                    p += 1;
                    let n = vlq(t, &mut p)? as usize;
                    let data = t.get(p..p + n)?;
                    p += n;
                    match ty {
                        0x51 if n == 3 => tempo_map.push((tick, u32::from_be_bytes([0, data[0], data[1], data[2]]))),
                        0x7F => m.seq_specific += 1,
                        0x2F => break,
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let n = vlq(t, &mut p)? as usize;
                    if m.sysex == 0 {
                        m.sysex_head = t.get(p..p + n.min(6))?.to_vec();
                    }
                    p += n;
                    m.sysex += 1;
                }
                0x80..=0xEF => {
                    let ch = b & 15;
                    m.channels |= 1 << ch;
                    let n = if matches!(b >> 4, 0xC | 0xD) { 1 } else { 2 };
                    let a = t.get(p..p + n)?;
                    p += n;
                    match b >> 4 {
                        0x9 if a[1] > 0 => m.notes += 1,
                        0xC if !m.programs.iter().any(|&(c, _)| c == ch) => m.programs.push((ch, a[0])),
                        _ => {}
                    }
                }
                _ => return None,
            }
        }
        end = end.max(tick);
        o += 8 + len;
    }
    m.extra = d.len().saturating_sub(o);
    m.tempos = tempo_map.len();
    tempo_map.sort_by_key(|&(t, _)| t);
    let (mut secs, mut at, mut us) = (0f64, 0u64, 500_000u32);
    for &(t, u) in tempo_map.iter().filter(|&&(t, _)| t <= end) {
        secs += (t - at) as f64 * us as f64 / m.division as f64 / 1e6;
        (at, us) = (t, u);
    }
    m.secs = secs + (end - at) as f64 * us as f64 / m.division as f64 / 1e6;
    Some(m)
}

/// One resource as a table row: (kind, details, seconds).
fn describe(r: &[u8]) -> (String, String, Option<f64>) {
    let Some(&t) = r.first() else { return ("empty".into(), String::new(), None) };
    let lp = if t & 0x80 != 0 { ", bit 7 (loop)" } else { "" };
    match t & 0x7F {
        _ if r.len() == 1 => ("empty".into(), format!("type {t}"), None),
        1 => {
            let (rate, n, bits) = (u16le(r, 1).unwrap_or(0), u16le(r, 3).unwrap_or(0), r.get(7).copied().unwrap_or(0));
            let secs = (rate > 0).then(|| n as f64 / rate as f64);
            ("digitized".into(), format!("{rate} Hz, {n} samples, {bits} bit{lp}"), secs)
        }
        0 => {
            let tempo = u16le(r, 1).unwrap_or(0).max(1) as f64;
            let (mut o, mut notes, mut units) = (3, 0, 0u32);
            while let (Some(f), Some(&l)) = (u16le(r, o), r.get(o + 2)) {
                if f == 0x12 {
                    break;
                }
                notes += 1;
                units += l as u32;
                o += 3;
            }
            ("speaker".into(), format!("tempo {tempo}, {notes} notes{lp}"), Some(units as f64 / tempo))
        }
        2 => match parse_midi(&r[1..]) {
            Some(m) => {
                let mut chs: Vec<String> = (0..16).filter(|c| m.channels & (1 << c) != 0).map(|c| (c + 1).to_string()).collect();
                if chs.is_empty() {
                    chs.push("-".into());
                }
                let progs: Vec<String> = m.programs.iter().map(|(c, p)| format!("{}:{p}", c + 1)).collect();
                let d = format!(
                    "SMF {} x{} trk, {} tpq, {} tempo, {} notes, ch {}, prog {}, sysex {}{}, meta7F {}, tail {} B{lp}",
                    m.format, m.tracks, m.division, m.tempos, m.notes, chs.join(","), if progs.is_empty() { "-".into() } else { progs.join(" ") }, m.sysex, if m.sysex_head.is_empty() { String::new() } else { format!(" ({})", m.sysex_head.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")) }, m.seq_specific, m.extra
                );
                ("MIDI".into(), d, Some(m.secs))
            }
            None => ("MIDI?".into(), "unparsed".into(), None),
        },
        k => (format!("unknown type {k}"), String::new(), None),
    }
}

fn data_dir() -> String {
    std::env::var("POP_DATA").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data").into())
}

fn inventory(dir: &str) -> Option<String> {
    let mut summary = String::from("| file | bytes | resources | kinds | ids |\n|---|---|---|---|---|\n");
    let mut rows = String::from("| file | id | sound | bytes | kind | seconds | details |\n|---|---|---|---|---|---|---|\n");
    let mut found = false;
    for f in FILES {
        let path = format!("{dir}/{f}.DAT");
        let Ok(dat) = Dat::open(&path) else { continue };
        found = true;
        let bytes = std::fs::metadata(&path).map_or(0, |m| m.len());
        let mut ids: Vec<u16> = dat.ids().collect();
        ids.sort_unstable();
        let mut kinds: Vec<String> = vec![];
        for &id in &ids {
            let r = dat.get(id).unwrap_or(&[]);
            let (kind, det, secs) = describe(r);
            if !kinds.contains(&kind) {
                kinds.push(kind.clone());
            }
            let snd = id.checked_sub(10000).map_or("-".into(), |s| s.to_string());
            let secs = secs.map_or("-".into(), |s| format!("{s:.2}"));
            writeln!(rows, "| {f} | {id} | {snd} | {} | {kind} | {secs} | {det} |", r.len()).ok();
        }
        let span = match (ids.first(), ids.last()) {
            (Some(a), Some(b)) => format!("{a}..{b}"),
            _ => "-".into(),
        };
        writeln!(summary, "| {f}.DAT | {bytes} | {} | {} | {span} |", ids.len(), kinds.join(", ")).ok();
    }
    found.then(|| format!("{summary}\n{rows}"))
}

fn main() {
    let Some(md) = inventory(&data_dir()) else {
        eprintln!("no sound archives in {} (set POP_DATA)", data_dir());
        std::process::exit(1);
    };
    if std::env::args().any(|a| a == "--write") {
        let doc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/audio.md");
        let text = std::fs::read_to_string(doc).expect("docs/audio.md");
        let (Some(a), Some(b)) = (text.find(BEGIN), text.find(END)) else { panic!("inventory markers missing in docs/audio.md") };
        let out = format!("{}{BEGIN}\n\n{md}\n{}", &text[..a], &text[b..]);
        std::fs::write(doc, out).expect("write docs/audio.md");
    } else {
        print!("{md}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-track SMF: 96 tpq, tempo 1 s per quarter after a quarter at the default 0.5 s, a note on channel 10 with program 5.
    #[test]
    fn midi_length_follows_the_tempo_map() {
        let trk: &[u8] = &[
            0x00, 0xC9, 0x05, // program 5 on channel 10
            0x00, 0x99, 60, 100, // note on
            0x60, 0xFF, 0x51, 0x03, 0x0F, 0x42, 0x40, // after 96 ticks: tempo 1_000_000
            0x60, 60, 0, // running status note off (vel 0), after 96 ticks
            0x00, 0xFF, 0x2F, 0x00,
        ];
        let mut d = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
        d.extend((trk.len() as u32).to_be_bytes());
        d.extend(trk);
        let m = parse_midi(&d).unwrap();
        assert!((m.secs - 1.5).abs() < 1e-9, "{}", m.secs);
        assert_eq!((m.notes, m.channels, m.programs.clone(), m.tempos, m.extra), (1, 1 << 9, vec![(9, 5)], 1, 0));
        assert!(parse_midi(b"RIFF").is_none());
    }

    #[test]
    fn speaker_and_digi_headers() {
        let (k, _, s) = describe(&[0, 80, 0, 0xB8, 1, 40, 0x12, 0, 0]); // 440 Hz for 40/80 s
        assert_eq!((k.as_str(), s), ("speaker", Some(0.5)));
        let (k, d, s) = describe(&[0x81, 0xF8, 0x2A, 4, 0, 0, 0, 8, 1, 2, 3, 4]);
        assert_eq!((k.as_str(), s), ("digitized", Some(4.0 / 11000.0)));
        assert!(d.contains("loop"));
    }

    /// With the user's data: every resource of the sound archives is recognised (skips without Original_data).
    #[test]
    fn user_data_parses() {
        let Some(md) = inventory(&data_dir()) else { return };
        assert!(!md.contains("MIDI?") && !md.contains("unknown type"), "{md}");
    }
}
