//! Tables embedded in PRINCE.EXE (version 1.1): frame tables (kid, guard-type characters, cutscenes)
//! and animation sequences.
//! File offset = data-segment offset + `DS_BASE`. Layout found by analysis of the shipped binary.
use std::{fs, io, path::Path};

const DS_BASE: usize = 112832;
const VERSION_AT: usize = 112986;
const VERSION: &[u8] = b"PRINCE OF PERSIA  V1.1";
// Frame records are 5 bytes: image, sword, dx, dy, flags.
const KID_FRAMES_AT: usize = 116916; // verified in the EXE's load_frame (see docs/mechanics.md)
pub const KID_FRAMES: usize = 241;
const GUARD_FRAMES_AT: usize = 118127; // = frame 150 (only frames 150.. are stored)
pub const GUARD_FRAMES: usize = 41;
const CUT_FRAMES_AT: usize = 118328;
pub const CUT_FRAMES: usize = 86;
const SEQ_OFFSETS_AT: usize = 119134;
pub const SEQUENCES: usize = 114;

/// One animation frame (5 bytes in the file). Kid: image `i` is KID.DAT resource 401 + i; guard-type characters:
/// GUARD/FAT/VIZIER.DAT resource 751 + i. Image 255 = none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// 255 = no image
    pub image: u8,
    /// index into the sword position table (low bits); high bits unverified
    pub sword: u8,
    pub dx: i8,
    pub dy: i8,
    /// low 5 bits: foot offset (x weight); 0x20 thin (collision box 4 narrower each side); 0x40 needs floor;
    /// 0x80 x-parity (half step when drawing). Verified in the EXE code.
    pub flags: u8,
}

/// Sequence instruction (opcode meanings verified in the EXE code, see docs/mechanics.md). Frame numbers index the frame table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Frame(u8),
    Dx(i8),
    Dy(i8),
    Flip,
    Up,
    Down,
    Action(u8),
    SetFall(i8, i8),
    /// jump to data-segment offset (also used as sequence-local label)
    Jmp(u16),
    JmpIfFeather(u16),
    /// 0xF4 / 0xF5
    KnockDown,
    KnockUp,
    Sound(u8),
    GetItem(u8),
    /// 0xF1: increments next_level (level-end sequence)
    EndLevel,
    /// 0xF6: no effect
    Nop,
}

pub struct Exe(Vec<u8>);

impl Exe {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let d = fs::read(path)?;
        if d.get(VERSION_AT..VERSION_AT + VERSION.len()) != Some(VERSION) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported PRINCE.EXE (expected v1.1)"));
        }
        Ok(Self(d))
    }

    fn frames(&self, at: usize, n: usize) -> Vec<Frame> {
        self.0[at..][..n * 5]
            .chunks(5)
            .map(|b| Frame { image: b[0], sword: b[1], dx: b[2] as i8, dy: b[3] as i8, flags: b[4] })
            .collect()
    }

    pub fn kid_frames(&self) -> Vec<Frame> {
        self.frames(KID_FRAMES_AT, KID_FRAMES)
    }

    /// Shared by guard, fat guard, vizier (and skeleton, which has fewer images).
    pub fn guard_frames(&self) -> Vec<Frame> {
        self.frames(GUARD_FRAMES_AT, GUARD_FRAMES)
    }

    /// Cutscene frames (charid 5 princess, 6 vizier): `sword` 64 = PV.DAT 800 set, 128 = 850 / 900 set (docs/cutscenes.md 1.6);
    /// image i = set base + 1 + i.
    pub fn cutscene_frames(&self) -> Vec<Frame> {
        self.frames(CUT_FRAMES_AT, CUT_FRAMES)
    }

    /// Sword position table (ds:1726, 51 entries): (image in PRINCE.DAT resource 701 + i, dx, dy) in screen pixels;
    /// a frame selects its entry with `sword & 0x3F` (entry 0 = no sword).
    pub fn sword_table(&self) -> Vec<(u8, i8, i8)> {
        self.0[DS_BASE + 0x1726..][..51 * 3].chunks(3).map(|b| (b[0], b[1] as i8, b[2] as i8)).collect()
    }

    /// Sound priorities (ds:2528, one byte per sound id 0..56): play_sound (0000:12F0) keeps the queued sound when the new
    /// one has a higher value.
    pub fn sound_priorities(&self) -> Vec<u8> {
        self.0[DS_BASE + 0x2528..][..57].to_vec()
    }

    /// Data-segment offsets of sequences 1..=114.
    pub fn sequence_offsets(&self) -> Vec<u16> {
        self.0[SEQ_OFFSETS_AT..][..SEQUENCES * 2].chunks(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect()
    }

    /// Instructions from data-segment offset `ds` up to and including the next `Jmp`.
    pub fn ops_from(&self, ds: u16) -> Option<Vec<Op>> {
        let d = self.0.get(DS_BASE..)?;
        let (mut p, mut out) = (ds as usize, Vec::new());
        loop {
            let a = |i: usize| d.get(p + i).copied();
            let b = a(0)?;
            let (op, n) = match b {
                0xFF => (Op::Jmp(u16::from_le_bytes([a(1)?, a(2)?])), 3),
                0xFE => (Op::Flip, 1),
                0xFD => (Op::Up, 1),
                0xFC => (Op::Down, 1),
                0xFB => (Op::Dx(a(1)? as i8), 2),
                0xFA => (Op::Dy(a(1)? as i8), 2),
                0xF9 => (Op::Action(a(1)?), 2),
                0xF8 => (Op::SetFall(a(1)? as i8, a(2)? as i8), 3),
                0xF7 => (Op::JmpIfFeather(u16::from_le_bytes([a(1)?, a(2)?])), 3),
                0xF6 => (Op::Nop, 1),
                0xF5 => (Op::KnockUp, 1),
                0xF4 => (Op::KnockDown, 1),
                0xF3 => (Op::GetItem(a(1)?), 2),
                0xF2 => (Op::Sound(a(1)?), 2),
                0xF1 => (Op::EndLevel, 1),
                f => (Op::Frame(f), 1),
            };
            out.push(op);
            p += n;
            if matches!(op, Op::Jmp(_)) {
                return Some(out);
            }
        }
    }

    /// Data-segment offset of sequence `n` (1-based).
    pub fn seq_offset(&self, n: usize) -> Option<u16> {
        self.sequence_offsets().get(n.checked_sub(1)?).copied()
    }

    /// Sequence `n` (1-based).
    pub fn sequence(&self, n: usize) -> Option<Vec<Op>> {
        self.ops_from(self.seq_offset(n)?)
    }
}
