//! Standard MIDI file reader for the music resources (type 2 sound resources: the file follows the type byte).
//! Produces one time-ordered event list; times are absolute microseconds through the tempo map.
//! Same walker as tools/sound_inventory (running status, tempo, sysex), now keeping the events.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    NoteOff { ch: u8, key: u8 },
    NoteOn { ch: u8, key: u8, vel: u8 },
    Control { ch: u8, num: u8, val: u8 },
    Program { ch: u8, prog: u8 },
    /// 14-bit value, 0x2000 = centre
    Bend { ch: u8, val: u16 },
    /// Channel pressure / key pressure: kept so the driver trace can decide what to ignore.
    Pressure { ch: u8, val: u8 },
    /// The AdLib driver's own sysex: 0 = transpose (signed 7 bit), 1 / 2 = disable / enable channel. Others are dropped.
    Sysex { sub: u8, arg: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub tick: u64,
    pub micros: u64,
    pub msg: Msg,
}

#[derive(Debug, Default, PartialEq)]
pub struct Smf {
    pub division: u16,
    pub events: Vec<Event>,
    /// End of the longest track (End of Track meta), microseconds.
    pub length_micros: u64,
}

impl Smf {
    pub fn secs(&self) -> f64 {
        self.length_micros as f64 / 1e6
    }
    /// Time of the last note-off/note-on, microseconds (the "last note" end, vs the End of Track in `length_micros`).
    pub fn last_note_micros(&self) -> u64 {
        self.events.iter().filter(|e| matches!(e.msg, Msg::NoteOn { .. } | Msg::NoteOff { .. })).map(|e| e.micros).max().unwrap_or(0)
    }
    /// 60 Hz timer steps (the game's clock) for a time.
    pub fn steps60(micros: u64) -> u64 {
        micros * 60 / 1_000_000
    }

    pub fn parse(d: &[u8]) -> Option<Smf> {
        let be16 = |o: usize| d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
        let be32 = |o: usize| d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
        if d.get(0..4)? != b"MThd" {
            return None;
        }
        let division = be16(12)?;
        if division & 0x8000 != 0 || division == 0 {
            return None; // SMPTE timing: not used by the game
        }
        let mut tempos: Vec<(u64, u32)> = vec![];
        let mut raw: Vec<(u64, Msg)> = vec![];
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
                    b = status;
                }
                match b {
                    0xFF => {
                        let ty = *t.get(p)?;
                        p += 1;
                        let n = vlq(t, &mut p)? as usize;
                        let data = t.get(p..p + n)?;
                        p += n;
                        match ty {
                            0x51 if n == 3 => tempos.push((tick, u32::from_be_bytes([0, data[0], data[1], data[2]]))),
                            0x2F => break,
                            _ => {}
                        }
                    }
                    0xF0 | 0xF7 => {
                        let n = vlq(t, &mut p)? as usize;
                        let s = t.get(p..p + n)?;
                        p += n;
                        // the driver accepts only `00 00 34 <00|01> sub arg` (docs/audio.md 6.4.6)
                        if let [0, 0, 0x34, 0 | 1, sub, arg, ..] = *s {
                            raw.push((tick, Msg::Sysex { sub, arg }));
                        }
                    }
                    0x80..=0xEF => {
                        let ch = b & 15;
                        let n = if matches!(b >> 4, 0xC | 0xD) { 1 } else { 2 };
                        let a = t.get(p..p + n)?;
                        p += n;
                        let msg = match b >> 4 {
                            0x8 => Msg::NoteOff { ch, key: a[0] },
                            0x9 if a[1] == 0 => Msg::NoteOff { ch, key: a[0] },
                            0x9 => Msg::NoteOn { ch, key: a[0], vel: a[1] },
                            0xA => Msg::Pressure { ch, val: a[1] },
                            0xB => Msg::Control { ch, num: a[0], val: a[1] },
                            0xC => Msg::Program { ch, prog: a[0] },
                            0xD => Msg::Pressure { ch, val: a[0] },
                            _ => Msg::Bend { ch, val: (a[0] as u16 & 0x7F) | ((a[1] as u16 & 0x7F) << 7) },
                        };
                        raw.push((tick, msg));
                    }
                    _ => return None,
                }
            }
            end = end.max(tick);
            o += 8 + len;
        }
        tempos.sort_by_key(|&(t, _)| t);
        let to_micros = |tick: u64| {
            let (mut us, mut at, mut tempo) = (0f64, 0u64, 500_000u32);
            for &(t, u) in tempos.iter().filter(|&&(t, _)| t <= tick) {
                us += (t - at) as f64 * tempo as f64 / division as f64;
                (at, tempo) = (t, u);
            }
            (us + (tick - at) as f64 * tempo as f64 / division as f64).round() as u64
        };
        raw.sort_by_key(|&(t, _)| t); // stable: tracks keep their order within a tick
        let events = raw.into_iter().map(|(tick, msg)| Event { tick, micros: to_micros(tick), msg }).collect();
        Some(Smf { division, events, length_micros: to_micros(end) })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Format 0, 480 ticks, one track: tempo 500000, program 3, note on 60 at 0, off at 480, End of Track at 960.
    fn tiny() -> Vec<u8> {
        let mut t = vec![0, 0xFF, 0x51, 3, 0x07, 0xA1, 0x20, 0, 0xC0, 3, 0, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0, 0x83, 0x60, 0xFF, 0x2F, 0];
        let mut d = b"MThd".to_vec();
        d.extend([0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xE0]);
        d.extend(b"MTrk");
        d.extend((t.len() as u32).to_be_bytes());
        d.append(&mut t);
        d
    }

    #[test]
    fn walks_events_and_times() {
        let s = Smf::parse(&tiny()).unwrap();
        assert_eq!(s.events.len(), 3);
        assert_eq!(s.events[1].msg, Msg::NoteOn { ch: 0, key: 60, vel: 100 });
        assert_eq!((s.events[2].tick, s.events[2].micros), (480, 500_000));
        assert_eq!(s.length_micros, 1_000_000);
        assert_eq!(Smf::steps60(s.length_micros), 60);
        assert_eq!(s.last_note_micros(), 500_000);
    }

    #[test]
    fn running_status_and_velocity_zero() {
        let t = [0, 0x90, 60, 100, 0, 62, 100, 0, 62, 0, 0, 0xFF, 0x2F, 0];
        let mut d = b"MThd".to_vec();
        d.extend([0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xE0]);
        d.extend(b"MTrk");
        d.extend((t.len() as u32).to_be_bytes());
        d.extend(t);
        let s = Smf::parse(&d).unwrap();
        let m: Vec<_> = s.events.iter().map(|e| e.msg).collect();
        assert_eq!(m[1], Msg::NoteOn { ch: 0, key: 62, vel: 100 });
        assert_eq!(m[2], Msg::NoteOff { ch: 0, key: 62 });
        assert!(Smf::parse(b"RIFF").is_none());
    }

    #[test]
    fn game_music_lengths_match_inventory() {
        if crate::no_game_data() { return; }
        let d = crate::dat::Dat::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/MIDISND2.DAT")).unwrap();
        // docs/audio.md 3.1: sound 54 = 24.29 s
        let s = Smf::parse(&d.get(10054).unwrap()[1..]).unwrap();
        assert!((s.secs() - 24.29).abs() < 0.01, "{}", s.secs());
        assert!(s.events.iter().all(|e| match e.msg { Msg::Program { prog, .. } => prog < 18, _ => true }));
    }
}
