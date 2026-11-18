//! Throwaway analysis for docs/audio.md 6.6: cargo run -p pop-core --example midi_channels --release [-- <dir>]
//! Prints markdown: per-cue track/meta layout, per-channel statistics, instrument decode, instrument x cue cross-table.
use pop_core::{dat::Dat, midi::{Msg, Smf}};
use std::collections::BTreeMap;

fn vlq(d: &[u8], p: &mut usize) -> usize {
    let mut v = 0usize;
    loop {
        let b = d[*p];
        *p += 1;
        v = v << 7 | (b & 0x7F) as usize;
        if b < 0x80 { return v; }
    }
}

/// (track index, channels used, name meta 03, other text metas (type, text), meta type counts)
struct Trk { ch: Vec<u8>, texts: Vec<(u8, String)>, metas: BTreeMap<u8, usize>, events: usize }

fn tracks(d: &[u8]) -> (u16, Vec<Trk>) {
    let fmt = u16::from_be_bytes([d[8], d[9]]);
    let mut o = 8 + u32::from_be_bytes([d[4], d[5], d[6], d[7]]) as usize;
    let mut v = vec![];
    while o + 8 <= d.len() && &d[o..o + 4] == b"MTrk" {
        let len = u32::from_be_bytes([d[o + 4], d[o + 5], d[o + 6], d[o + 7]]) as usize;
        let t = &d[o + 8..o + 8 + len];
        let (mut p, mut st) = (0, 0u8);
        let mut k = Trk { ch: vec![], texts: vec![], metas: BTreeMap::new(), events: 0 };
        while p < t.len() {
            vlq(t, &mut p);
            let mut b = t[p];
            if b >= 0x80 { p += 1; st = b } else { b = st }
            k.events += 1;
            match b {
                0xFF => {
                    let ty = t[p]; p += 1;
                    let n = vlq(t, &mut p);
                    let s = &t[p..p + n]; p += n;
                    *k.metas.entry(ty).or_default() += 1;
                    if (1..=7).contains(&ty) { k.texts.push((ty, String::from_utf8_lossy(s).into_owned())) }
                    if ty == 0x2F { break }
                }
                0xF0 | 0xF7 => { let n = vlq(t, &mut p); p += n; }
                _ => {
                    let c = b & 15;
                    if !k.ch.contains(&c) { k.ch.push(c) }
                    p += if matches!(b >> 4, 0xC | 0xD) { 1 } else { 2 };
                }
            }
        }
        v.push(k);
        o += 8 + len;
    }
    (fmt, v)
}

fn kname(k: i32) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[k.rem_euclid(12) as usize], k.div_euclid(12) - 1)
}

fn timbre(i: &[u8; 16]) -> (String, String) {
    let (m, c) = (&i[3..8], &i[8..13]);
    let fb = i[2] >> 1 & 7;
    let add = i[2] & 1 == 1;
    let op = |o: &[u8]| format!("mult {} wave {} AM{} VIB{} EGT{} KSR{} KSL{} TL {} AR{} DR{} SL{} RR{}", o[0] & 15, o[4] & 3, o[0] >> 7, o[0] >> 6 & 1, o[0] >> 5 & 1, o[0] >> 4 & 1, o[1] >> 6, o[1] & 63, o[2] >> 4, o[2] & 15, o[3] >> 4, o[3] & 15);
    let desc = format!("fb {} {} | mod: {} | car: {}", fb, if add { "additive" } else { "FM" }, op(m), op(c));
    // heuristics, INFERRED
    let (cegt, cdr, car, crr, csl) = (c[0] >> 5 & 1, c[2] & 15, c[2] >> 4, c[3] & 15, c[3] >> 4);
    let (mm, cm, mtl) = (m[0] & 15, c[0] & 15, m[1] & 63);
    let mut g = vec![];
    if add { g.push("additive, organ/reed-like".to_string()) }
    else if mtl < 16 && fb >= 4 { g.push("bright/harsh FM (strong modulator)".into()) }
    else if mtl < 24 { g.push("brassy/bright FM".into()) }
    else if mtl > 40 { g.push("soft/near-sine".into()) }
    if cegt == 0 && cdr >= 5 { g.push("percussive/plucked (decays to silence)".into()) }
    else if cegt == 0 { g.push("slow-decay pluck/bell".into()) }
    else if csl >= 8 { g.push("sustained, ducks to a low plateau".into()) }
    else { g.push("sustained, organ/string-like".into()) }
    if car >= 12 { g.push("instant attack".into()) } else if car <= 8 { g.push("slow attack/swell".into()) }
    if crr <= 3 { g.push("long release".into()) }
    if cm >= 4 || mm >= 4 { g.push(format!("high multiples ({}/{}): metallic/bell-ish", mm, cm)) }
    if cm == 0 { g.push("carrier mult 0: sub-octave".into()) }
    (desc, g.join("; "))
}

#[derive(Default, Clone)]
struct Note { on: u64, off: u64, key: i32, vel: u8, prog: u8 }

fn main() {
    let dir = std::env::args().nth(1).unwrap_or("Original_data".into());
    let bank = Dat::open(format!("{dir}/PRINCE.DAT")).unwrap().get(1).unwrap().to_vec();
    let n = bank[0] as usize;
    let ins: Vec<[u8; 16]> = bank[1..1 + 16 * n].chunks(16).map(|c| c.try_into().unwrap()).collect();

    // cross[prog] = list of (cue, ch, notes)
    let mut cross: BTreeMap<u8, Vec<(u16, u8, usize, String)>> = BTreeMap::new();
    let mut role_notes: BTreeMap<(u8, &'static str), usize> = BTreeMap::new();
    println!("## CHANNELS");
    println!("cue|ch|track name|prog(s)|notes|keys (sounding)|vel|poly|snd s|med len s|role");
    for f in ["MIDISND1.DAT", "MIDISND2.DAT"] {
        let dat = Dat::open(format!("{dir}/{f}")).unwrap();
        for id in dat.ids() {
            let r = dat.get(id).unwrap();
            let sid = id - 10000;
            if r[0] & 3 != 2 { continue }
            let d = &r[1..];
            let smf = Smf::parse(d).unwrap();
            let (fmt, trks) = tracks(d);
            let tr = smf.events.iter().find_map(|e| if let Msg::Sysex { sub: 0, arg } = e.msg { Some(((arg << 1) as i8 >> 1) as i32) } else { None }).unwrap_or(0) as i32;
            // META
            eprintln!("--- {f} cue {sid}: format {fmt}, {} tracks, transpose {tr}, {:.2}s", trks.len(), smf.secs());
            for (i, t) in trks.iter().enumerate() {
                let m: Vec<String> = t.metas.iter().map(|(k, v)| format!("{k:02X}x{v}")).collect();
                eprintln!("  trk {i}: ch {:?} events {} metas [{}] texts {:?}", t.ch, t.events, m.join(" "), t.texts);
            }
            // per channel
            let mut prog: [u8; 16] = core::array::from_fn(|c| c as u8);
            let mut changes: Vec<Vec<(f64, u8)>> = vec![vec![]; 16];
            let mut open: Vec<Vec<Note>> = vec![vec![]; 16]; // FIFO of open notes by channel
            let mut notes: Vec<Vec<Note>> = vec![vec![]; 16];
            for e in &smf.events {
                match e.msg {
                    Msg::Program { ch, prog: p } if (p as usize) < n => { prog[ch as usize] = p; changes[ch as usize].push((e.micros as f64 / 1e6, p)); }
                    Msg::NoteOn { ch, key, vel } => open[ch as usize].push(Note { on: e.micros, off: u64::MAX, key: key as i32 + tr, vel, prog: prog[ch as usize] }),
                    Msg::NoteOff { ch, key } => {
                        if let Some(i) = open[ch as usize].iter().position(|x| x.key == key as i32 + tr && x.off == u64::MAX) {
                            let mut x = open[ch as usize].remove(i);
                            x.off = e.micros;
                            notes[ch as usize].push(x);
                        }
                    }
                    _ => {}
                }
            }
            for c in 0..16 { for mut x in open[c].drain(..) { x.off = smf.length_micros; notes[c].push(x) } }
            // cue-wide polyphony
            let mut ev: Vec<(u64, i32)> = vec![];
            for c in 0..16 { for x in &notes[c] { ev.push((x.on, 1)); ev.push((x.off, -1)) } }
            ev.sort_by_key(|&(t, d)| (t, d));
            let (mut cur, mut mx) = (0, 0);
            for (_, d) in ev { cur += d; mx = mx.max(cur) }
            eprintln!("  cue-wide max polyphony {mx}");
            for c in 0..16usize {
                let ns = &notes[c];
                if ns.is_empty() { continue }
                let mut iv: Vec<(u64, i32)> = vec![];
                for x in ns { iv.push((x.on, 1)); iv.push((x.off, -1)) }
                iv.sort_by_key(|&(t, d)| (t, d));
                let (mut cur, mut poly, mut last, mut snd, mut polyt) = (0i32, 0i32, 0u64, 0u64, 0u64);
                for (t, d) in iv {
                    if cur >= 1 { snd += t - last }
                    if cur >= 2 { polyt += t - last }
                    last = t; cur += d; poly = poly.max(cur);
                }
                let mut lens: Vec<u64> = ns.iter().map(|x| x.off - x.on).collect();
                lens.sort();
                let med = lens[lens.len() / 2] as f64 / 1e6;
                let keys: Vec<i32> = ns.iter().map(|x| x.key).collect();
                let (lo, hi) = (*keys.iter().min().unwrap(), *keys.iter().max().unwrap());
                let mean = keys.iter().sum::<i32>() as f64 / keys.len() as f64;
                let mut dist = keys.clone(); dist.sort(); dist.dedup();
                let vel = ns.iter().map(|x| x.vel as f64).sum::<f64>() / ns.len() as f64;
                let polyp = if snd > 0 { polyt as f64 * 100.0 / snd as f64 } else { 0.0 };
                let role = if dist.len() <= 2 && ns.len() >= 6 { "rhythm" }
                    else if poly >= 3 || polyp > 35.0 { "chords" }
                    else if mean < 55.0 { "bass" }
                    else { "melody" };
                let mut pc: BTreeMap<u8, usize> = BTreeMap::new();
                for x in ns { *pc.entry(x.prog).or_default() += 1 }
                for (&p, &k) in &pc { cross.entry(p).or_default().push((sid, c as u8, k, trks.iter().find(|t| t.ch.contains(&(c as u8))).and_then(|t| t.texts.first()).map(|t| t.1.trim().to_string()).unwrap_or_default())); *role_notes.entry((p, role)).or_default() += k }
                let has_pc = !changes[c].is_empty();
                let ps = if !has_pc { format!("{c}") } else { let mut s = format!("{c}"); for (t, p) in &changes[c] { s += &format!(">{p}@{t:.1}s") } s };
                let tn = trks.iter().find(|t| t.ch.contains(&(c as u8))).and_then(|t| t.texts.first()).map(|t| t.1.trim().to_string()).unwrap_or_default();
                println!("{sid}|{c}|{tn}|{ps}|{}|{}-{} ({}-{})|{:.0}|{poly}|{:.1}|{:.2}|{role}", ns.len(), lo, hi, kname(lo), kname(hi), vel, snd as f64 / 1e6, med);
            }
        }
    }
    println!("## INSTRUMENTS");
    for (i, x) in ins.iter().enumerate() {
        let (d, g) = timbre(x);
        println!("{i}: {d} => {g}");
    }
    println!("## CROSS");
    for (p, v) in &cross {
        let tot: usize = v.iter().map(|x| x.2).sum();
        let roles: Vec<String> = role_notes.iter().filter(|((q, _), _)| q == p).map(|((_, r), k)| format!("{r} {k}")).collect();
        let mut cu: Vec<u16> = v.iter().map(|x| x.0).collect(); cu.dedup();
        let l: Vec<String> = cu.iter().map(|c| c.to_string()).collect();
        let mut nm: BTreeMap<String, usize> = BTreeMap::new();
        for x in v { *nm.entry(x.3.clone()).or_default() += x.2 }
        let mut nm: Vec<_> = nm.into_iter().collect(); nm.sort_by_key(|x| std::cmp::Reverse(x.1));
        let nm: Vec<String> = nm.iter().map(|(a, b)| format!("{a} {b}")).collect();
        println!("{p}: total {tot} in {} chans; roles [{}]; {}; names [{}]", v.len(), roles.join(", "), l.join(" "), nm.join(", "));
    }
}
