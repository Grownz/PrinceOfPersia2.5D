//! 16-bit real-mode disassembly helper for PRINCE.EXE (DOS MZ). See README.md.
use iced_x86::{Code, Decoder, DecoderOptions, Formatter, Instruction, IntelFormatter, OpKind};
use std::collections::BTreeMap;

struct Exe {
    img: Vec<u8>, // load image (file minus MZ header)
    hdr: usize,
    segs: Vec<u16>, // code segments (from relocation fixups), sorted
    ds: u16,
}

fn rd16(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d[i], d[i + 1]])
}

impl Exe {
    fn load(path: &str) -> Exe {
        let f = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&f[..2], b"MZ", "not an MZ exe");
        let hdr = rd16(&f, 8) as usize * 16;
        let (n, at) = (rd16(&f, 6) as usize, rd16(&f, 0x18) as usize);
        let img = f[hdr..].to_vec();
        // PRINCE.EXE v1.1: data segment found from the known table layout (file = ds offset + 112832).
        let ds = ((112832 - hdr) / 16) as u16;
        let mut segs: Vec<u16> = (0..n)
            .map(|i| {
                let (o, s) = (rd16(&f, at + i * 4) as usize, rd16(&f, at + i * 4 + 2) as usize);
                rd16(&img, s * 16 + o)
            })
            .filter(|&s| s < ds)
            .collect();
        segs.push(0);
        segs.sort();
        segs.dedup();
        Exe { img, hdr, segs, ds }
    }

    fn file(&self, seg: u16, off: u16) -> usize {
        self.hdr + seg as usize * 16 + off as usize
    }

    /// Decoder positioned at seg:off; IP is the segment offset.
    fn decoder(&self, seg: u16, off: u16, end_lin: usize) -> Decoder<'_> {
        let lin = seg as usize * 16 + off as usize;
        let mut d = Decoder::with_ip(16, &self.img[lin..end_lin.min(self.img.len())], off as u64, DecoderOptions::NONE);
        d.set_ip(off as u64);
        d
    }

    /// Every instruction of every code segment, linear sweep (segment base .. next segment base).
    fn sweep(&self, mut f: impl FnMut(u16, &Instruction)) {
        let ends: Vec<usize> = self.segs.iter().skip(1).map(|&s| s as usize * 16).chain([self.ds as usize * 16]).collect();
        for (&s, &end) in self.segs.iter().zip(&ends) {
            let mut d = self.decoder(s, 0, end);
            let mut ins = Instruction::default();
            while d.can_decode() {
                d.decode_out(&mut ins);
                f(s, &ins);
            }
        }
    }

    fn fmt(&self, seg: u16, ins: &Instruction, out: &mut String) -> String {
        out.clear();
        IntelFormatter::new().format(ins, out);
        let lin = seg as usize * 16 + ins.ip() as usize;
        let bytes: String = self.img[lin..lin + ins.len()].iter().map(|b| format!("{b:02X}")).collect();
        let mut s = format!("{seg:04X}:{:04X} [{:06X}] {bytes:<14} {out}", ins.ip(), self.hdr + lin);
        if let Some((ts, to)) = far_target(ins) {
            s += &format!("    ; far -> {ts:04X}:{to:04X}");
        }
        s
    }
}

fn far_target(ins: &Instruction) -> Option<(u16, u16)> {
    matches!(ins.code(), Code::Call_ptr1616 | Code::Jmp_ptr1616).then(|| (ins.far_branch_selector(), ins.far_branch16()))
}

fn num(s: &str) -> u16 {
    let s = s.trim();
    let r = if let Some(h) = s.strip_prefix("0x") { u32::from_str_radix(h, 16) } else if let Some(h) = s.strip_suffix('h') { u32::from_str_radix(h, 16) } else { s.parse() };
    r.unwrap_or_else(|_| panic!("bad number {s}")) as u16
}

/// "SSSS:OOOO" in hex (load-relative segment).
fn addr(s: &str) -> (u16, u16) {
    let (a, b) = s.split_once(':').expect("address must be SEG:OFF (hex)");
    (u16::from_str_radix(a, 16).unwrap(), u16::from_str_radix(b, 16).unwrap())
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let path = std::env::var("POP_EXE").unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PRINCE.EXE").into());
    let exe = Exe::load(&path);
    let mut buf = String::new();
    match a.first().map(String::as_str) {
        Some("info") => {
            println!("header {:#x} bytes, DS = {:04X} (file = ds_off + {})", exe.hdr, exe.ds, exe.file(exe.ds, 0));
            for s in &exe.segs {
                println!("code seg {s:04X} file {:06X}", exe.file(*s, 0));
            }
        }
        // dis SEG:OFF [N]: N instructions; without N, stop at a ret/retf that is followed by `push bp` (next function).
        Some("dis") => {
            let (s, o) = addr(&a[1]);
            let n: Option<usize> = a.get(2).map(|x| x.parse().unwrap());
            let mut d = exe.decoder(s, o, exe.img.len());
            let mut ins = Instruction::default();
            for _ in 0..n.unwrap_or(2000) {
                d.decode_out(&mut ins);
                println!("{}", exe.fmt(s, &ins, &mut buf));
                let next = exe.img.get(s as usize * 16 + ins.next_ip() as usize).copied();
                if n.is_none() && matches!(ins.mnemonic(), iced_x86::Mnemonic::Ret | iced_x86::Mnemonic::Retf) && next == Some(0x55) {
                    break;
                }
            }
        }
        // farcalls: every far call target with its call count
        Some("farcalls") => {
            let mut m: BTreeMap<(u16, u16), usize> = BTreeMap::new();
            exe.sweep(|_, ins| {
                if let Some(t) = far_target(ins) {
                    *m.entry(t).or_default() += 1;
                }
            });
            for ((s, o), n) in m {
                println!("{s:04X}:{o:04X} file {:06X} calls {n}", exe.file(s, o));
            }
        }
        // callers SEG:OFF: far calls to SEG:OFF and near calls/jumps to OFF inside SEG
        Some("callers") => {
            let t = addr(&a[1]);
            exe.sweep(|s, ins| {
                let near = s == t.0 && ins.op_count() > 0 && ins.op0_kind() == OpKind::NearBranch16 && ins.near_branch16() == t.1 && ins.is_call_near();
                if far_target(ins) == Some(t) || near {
                    println!("{}", exe.fmt(s, ins, &mut String::new()));
                }
            });
        }
        // xref VALUE [mem|imm]: instructions using VALUE as memory displacement and/or immediate
        Some("xref") => {
            let v = num(&a[1]);
            let only = a.get(2).map(String::as_str);
            exe.sweep(|s, ins| {
                let hit = (0..ins.op_count()).any(|i| match ins.op_kind(i) {
                    OpKind::Memory => only != Some("imm") && ins.memory_displacement32() as u16 == v && ins.memory_size() != iced_x86::MemorySize::Unknown,
                    OpKind::Immediate8 | OpKind::Immediate8to16 | OpKind::Immediate16 => only != Some("mem") && ins.immediate(i) as u16 == v,
                    _ => false,
                });
                if hit {
                    println!("{}", exe.fmt(s, ins, &mut String::new()));
                }
            });
        }
        // ds OFF LEN: hex dump of the data segment
        Some("ds") => {
            let (o, n) = (num(&a[1]) as usize, num(&a[2]) as usize);
            let base = exe.ds as usize * 16;
            for row in (o..o + n).step_by(16) {
                let b = &exe.img[base + row..base + (row + 16).min(o + n)];
                println!("ds:{row:04X} [{:06X}] {}", exe.hdr + base + row, b.iter().map(|x| format!("{x:02X} ")).collect::<String>());
            }
        }
        _ => eprintln!(
            "usage: pop-disasm info | dis SEG:OFF [N] | farcalls | callers SEG:OFF | xref VALUE [mem|imm] | ds OFF LEN\n\
             SEG:OFF in hex, load-relative; VALUE/OFF/LEN decimal or 0x.. hex. EXE path: $POP_EXE or <repo>/Original_data/PRINCE.EXE"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Self-check against PRINCE.EXE v1.1 (skipped when the original file is absent).
    #[test]
    fn finds_seqtbl_reader() {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PRINCE.EXE");
        if !std::path::Path::new(p).exists() {
            return;
        }
        let exe = Exe::load(p);
        assert_eq!((exe.hdr, exe.ds), (0x890, 0x1B03));
        let mut hits = Vec::new();
        exe.sweep(|s, i| if (0..i.op_count()).any(|k| i.op_kind(k) == OpKind::Memory) && i.memory_displacement32() == 0x189C { hits.push((s, i.ip())) });
        assert_eq!(hits, [(0, 0x5E7A), (0, 0x5E8D)]);
        assert_eq!((num("0x10"), num("16"), addr("0C83:A87C")), (16, 16, (0xC83, 0xA87C)));
    }
}
