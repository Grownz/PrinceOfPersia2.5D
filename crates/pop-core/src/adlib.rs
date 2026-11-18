//! The original AdLib/OPL2 MIDI driver (PRINCE.EXE `0C83:690A`, docs/audio.md 6.4) as a pure function:
//! SMF events + the instrument bank (PRINCE.DAT resource 1) -> a timed OPL2 register-write list.
//! The emulator (pop-app) only plays the list; nothing here touches audio. The 240 Hz timer quirk and the
//! tempo-byte swap of the original sequencer (6.4.8) are not applied: times use the nominal tempo map.
use crate::midi::{Msg, Smf};

pub const FNUM: [u16; 12] = [0x200, 0x21E, 0x23F, 0x261, 0x285, 0x2AB, 0x2D4, 0x300, 0x32E, 0x35E, 0x390, 0x3C7];
/// (modulator, carrier) operator offsets of voices 0..8
pub const OPS: [(u8, u8); 9] = [(0, 3), (1, 4), (2, 5), (8, 0xB), (9, 0xC), (0xA, 0xD), (0x10, 0x13), (0x11, 0x14), (0x12, 0x15)];
const VOICES: usize = 9;

/// One OPL register write at a time (microseconds from the start of the music).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Write {
    pub micros: u64,
    pub reg: u8,
    pub val: u8,
}

/// PRINCE.DAT resource 1: `[count, 16 * count bytes]` (the DAT checksum byte in front is not part of `Dat::get`'s slice).
pub struct Bank(pub Vec<[u8; 16]>);

impl Bank {
    pub fn parse(r: &[u8]) -> Option<Bank> {
        let n = *r.first()? as usize;
        let body = r.get(1..1 + 16 * n)?;
        Some(Bank(body.chunks(16).map(|c| c.try_into().unwrap()).collect()))
    }
}

fn tl_scaled(byte: u8, vel: u8) -> u8 {
    let v = (((byte as u32 & 0x3F) + 64) * 225 / (vel as u32 + 161)).clamp(64, 127) - 64;
    (byte & 0xC0) | v as u8 // master volume is always 15 in the game: floor 0
}

fn key_regs(key: u8, transpose: i8) -> (u8, u8) {
    let mut n = key as i32 - 31 + transpose as i32;
    while n < 0 {
        n += 12;
    }
    let (oct, idx) = (n / 12, (n % 12) as usize);
    ((FNUM[idx] & 0xFF) as u8, 0x20 | ((oct << 2) as u8) | (FNUM[idx] >> 8) as u8)
}

struct Driver<'a> {
    bank: &'a Bank,
    key: [u8; VOICES], // 0 = free
    chan: [u8; VOICES],
    b0: [u8; VOICES],
    next: usize,
    prog: [u8; 16],
    transpose: i8,
    enabled: u16,
    out: Vec<Write>,
    now: u64,
}

impl<'a> Driver<'a> {
    fn new(bank: &'a Bank) -> Self {
        let mut d = Driver { bank, key: [0; 9], chan: [0; 9], b0: [0; 9], next: 0, prog: [0; 16], transpose: 0, enabled: 0xFFFF, out: vec![], now: 0 };
        for (r, v) in [(0x01, 0), (0xBD, 0), (0x01, 0x20)] {
            d.w(r, v);
        }
        d.reset();
        d
    }
    fn w(&mut self, reg: u8, val: u8) {
        self.out.push(Write { micros: self.now, reg, val });
    }
    fn reset(&mut self) {
        for v in 0..VOICES as u8 {
            self.w(0xA0 + v, 0);
            self.w(0xB0 + v, 0);
        }
        self.enabled = 0xFFFF;
        self.transpose = 0;
        let n = self.bank.0.len();
        for c in 0..16 {
            self.prog[c] = if c < n { c as u8 } else { 0 };
        }
        (self.key, self.chan, self.next) = ([0; 9], [0; 9], 0);
    }
    fn on(&mut self, ch: u8, key: u8, vel: u8) {
        let Some(inst) = self.bank.0.get(self.prog[ch as usize] as usize).copied() else { return };
        if self.enabled >> ch & 1 == 0 {
            return;
        }
        let start = self.next;
        self.next = (self.next + 1) % VOICES;
        let Some(v) = (0..VOICES).map(|i| (start + i) % VOICES).find(|&v| self.key[v] == 0) else { return };
        let (m, c) = OPS[v];
        self.key[v] = key;
        self.chan[v] = ch;
        self.w(0xC0 + v as u8, inst[2]);
        let additive = inst[2] & 1 != 0;
        let mod_tl = if additive { tl_scaled(inst[4], vel) } else { inst[4] };
        for (base, op, val) in [(0x20, m, inst[3]), (0x40, m, mod_tl), (0x60, m, inst[5]), (0x80, m, inst[6]), (0xE0, m, inst[7])] {
            self.w(base + op, val);
        }
        for (base, op, val) in [(0x20, c, inst[8]), (0x40, c, tl_scaled(inst[9], vel)), (0x60, c, inst[10]), (0x80, c, inst[11]), (0xE0, c, inst[12])] {
            self.w(base + op, val);
        }
        let (a0, b0) = key_regs(key, self.transpose);
        self.b0[v] = b0;
        self.w(0xA0 + v as u8, a0);
        self.w(0xB0 + v as u8, b0);
    }
    fn off(&mut self, ch: u8, key: u8) {
        if let Some(v) = (0..VOICES).find(|&v| self.key[v] == key && self.chan[v] == ch) {
            self.key[v] = 0;
            self.w(0xB0 + v as u8, self.b0[v] & !0x20);
        }
    }
    fn msg(&mut self, m: Msg) {
        match m {
            Msg::NoteOn { ch, key, vel } if vel > 0 => self.on(ch, key, vel),
            Msg::NoteOn { ch, key, .. } | Msg::NoteOff { ch, key } => self.off(ch, key),
            Msg::Program { ch, prog } if (prog as usize) < self.bank.0.len() => self.prog[ch as usize] = prog,
            Msg::Sysex { sub: 0, arg } => self.transpose = (arg << 1) as i8 >> 1, // signed 7 bit
            Msg::Sysex { sub: 1, arg } => self.enabled &= !(1u16 << (arg & 15)),
            Msg::Sysex { sub: 2, arg } => self.enabled |= 1u16 << (arg & 15),
            _ => {} // controllers, pressure, pitch bend: ignored by the original driver
        }
    }
}

/// The register writes the original driver would make for this file, with the closing reset (all voices off) at the end.
pub fn trace(smf: &Smf, bank: &Bank) -> Vec<Write> {
    let mut d = Driver::new(bank);
    for e in &smf.events {
        d.now = e.micros;
        d.msg(e.msg);
    }
    d.now = smf.length_micros;
    d.reset();
    d.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> Bank {
        // instrument 0 of the shipped bank (docs/audio.md 6.4.2), the rest copies
        let i = [0x26, 0x0A, 0x04, 0x52, 0x88, 0x41, 0x21, 0x00, 0x73, 0x86, 0x71, 0x14, 0x00, 0, 0, 0];
        Bank(vec![i; 18])
    }
    fn smf(events: &[(u64, Msg)]) -> Smf {
        Smf {
            division: 480,
            events: events.iter().map(|&(micros, msg)| crate::midi::Event { tick: 0, micros, msg }).collect(),
            length_micros: 2_000_000,
        }
    }

    #[test]
    fn note_on_register_order_and_pitch() {
        let t = trace(&smf(&[(0, Msg::Sysex { sub: 0, arg: 12 }), (1000, Msg::NoteOn { ch: 0, key: 60, vel: 100 })]), &bank());
        let n: Vec<_> = t.iter().filter(|w| w.micros == 1000).map(|w| (w.reg, w.val)).collect();
        // C0, modulator 20/40/60/80/E0 (op 0), carrier (op 3), A0, B0 with key on. key 60+12: n = 41 -> oct 3, idx 5
        assert_eq!(n[0], (0xC0, 0x04));
        assert_eq!(&n[1..6].iter().map(|x| x.0).collect::<Vec<_>>(), &[0x20, 0x40, 0x60, 0x80, 0xE0]);
        assert_eq!(&n[6..11].iter().map(|x| x.0).collect::<Vec<_>>(), &[0x23, 0x43, 0x63, 0x83, 0xE3]);
        assert_eq!(n[1].1, 0x52);
        assert_eq!(n[2].1, 0x88); // non-additive modulator TL unscaled
        assert_eq!(n[7], (0x43, 0x80 | tl_scaled(0x86, 100) & 0x3F));
        assert_eq!(n[11], (0xA0, (FNUM[5] & 0xFF) as u8));
        assert_eq!(n[12], (0xB0, 0x20 | (3 << 2) | (FNUM[5] >> 8) as u8));
    }

    #[test]
    fn round_robin_no_stealing_and_note_off() {
        let on = |k| (0u64, Msg::NoteOn { ch: 0, key: k, vel: 90 });
        let mut ev: Vec<_> = (40..50).map(on).collect(); // 10 notes, 9 voices: the 10th is dropped
        ev.push((500, Msg::NoteOff { ch: 0, key: 40 }));
        let t = trace(&smf(&ev), &bank());
        let keyons = t.iter().filter(|w| w.micros == 0 && (0xB0..0xB9).contains(&w.reg) && w.val & 0x20 != 0).count();
        assert_eq!(keyons, 9);
        let off: Vec<_> = t.iter().filter(|w| w.micros == 500).map(|w| (w.reg, w.val & 0x20)).collect();
        assert_eq!(off, [(0xB0, 0)]); // first voice released only
    }

    #[test]
    fn ignored_messages_and_bad_program() {
        let t = trace(&smf(&[(10, Msg::Control { ch: 0, num: 7, val: 0 }), (20, Msg::Bend { ch: 0, val: 0 }), (30, Msg::Program { ch: 0, prog: 99 }), (40, Msg::NoteOn { ch: 0, key: 60, vel: 80 })]), &bank());
        assert!(t.iter().all(|w| w.micros != 10 && w.micros != 20 && w.micros != 30));
        assert!(t.iter().any(|w| w.micros == 40)); // program 99 ignored: channel keeps program 0
    }

    #[test]
    fn music_traces_on_the_real_bank() {
        if crate::no_game_data() { return; }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let bank = Bank::parse(crate::dat::Dat::open(format!("{dir}PRINCE.DAT")).unwrap().get(1).unwrap()).unwrap();
        assert_eq!(bank.0.len(), 18);
        let s = Smf::parse(&crate::dat::Dat::open(format!("{dir}MIDISND2.DAT")).unwrap().get(10054).unwrap()[1..]).unwrap();
        let t = trace(&s, &bank);
        assert!(t.len() > 500);
        assert!(t.windows(2).all(|w| w[0].micros <= w[1].micros));
        assert_eq!(t.last().unwrap().micros, s.length_micros);
    }
}
