//! The kid: sequence player. One logic tick = one frame of the current animation sequence.
//! Movement semantics are provisional until verified against the EXE code (see docs/mechanics.md):
//! `Dx`/`Dy` ops and the frame table's dx/dy are both applied in facing direction / downwards-positive y.
use crate::exe::{Exe, Frame, Op};

#[derive(Clone, Debug, Default)]
pub struct Kid {
    /// logic coordinates relative to the start position (provisional units: original x/y units)
    pub x: i32,
    pub y: i32,
    pub facing_left: bool,
    pub frame: u8,
    pub action: u8,
    pub seq: usize,
    cur: Vec<Op>,
    pc: usize,
}

impl Kid {
    pub fn start(&mut self, exe: &Exe, seq: usize) {
        self.seq = seq;
        self.cur = exe.sequence(seq).unwrap_or_default();
        self.pc = 0;
    }

    /// Run ops until the next `Frame`, which ends the tick.
    pub fn tick(&mut self, exe: &Exe, frames: &[Frame]) {
        // guard against sequences without any Frame op (would loop forever)
        for _ in 0..256 {
            let Some(&op) = self.cur.get(self.pc) else { return };
            self.pc += 1;
            let dir = if self.facing_left { -1 } else { 1 };
            match op {
                Op::Frame(n) => {
                    self.frame = n;
                    if let Some(f) = frames.get(n as usize) {
                        self.x += dir * f.dx as i32;
                        self.y += f.dy as i32;
                    }
                    return;
                }
                Op::Dx(d) => self.x += dir * d as i32,
                Op::Dy(d) => self.y += d as i32,
                Op::Flip => self.facing_left = !self.facing_left,
                Op::Action(a) => self.action = a,
                Op::Jmp(t) => {
                    self.cur = exe.ops_from(t).unwrap_or_default();
                    self.pc = 0;
                }
                // not yet implemented: Up/Down, SetFall, JmpIfFeather, Knock*, Sound, EndLevel, GetItem, Die
                _ => {}
            }
        }
    }
}
