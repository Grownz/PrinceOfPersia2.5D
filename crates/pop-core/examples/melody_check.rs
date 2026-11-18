//! Throwaway analysis (melody audit): speaker tune vs MIDI melody (part 1) and OPL trace vs SMF notes (part 2).
//! cargo run -p pop-core --example melody_check --release [-- <data dir> <out.txt>]   prints a summary, writes the note lists to out.txt
use pop_core::{adlib::{self, Bank}, dat::Dat, midi::{Msg, Smf}};
use std::{collections::BTreeMap, fmt::Write as _};

const NM: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
fn kn(k: i32) -> String { format!("{}{}", NM[k.rem_euclid(12) as usize], k.div_euclid(12) - 1) }
fn hz_key(f: f64) -> f64 { 69.0 + 12.0 * (f / 440.0).log2() }

/// speaker tune -> (key or None for rest, start s, dur s); consecutive equal notes are NOT merged (a repeated note is a new note)
fn speaker(r: &[u8]) -> Option<(u16, Vec<(Option<i32>, f64, f64)>)> {
    if r[0] & 0x7F != 0 { return None; }
    let u = |o: usize| u16::from_le_bytes([r[o], r[o + 1]]);
    let tempo = u(1);
    let (mut o, mut t, mut v) = (3, 0.0, vec![]);
    while o + 2 < r.len() {
        let (f, l) = (u(o), r[o + 2] as f64 / tempo as f64);
        if f == 0x12 { break; }
        v.push((if f <= 1 { None } else { Some(hz_key(f as f64).round() as i32) }, t, l));
        t += l;
        o += 3;
    }
    Some((tempo, v))
}

/// channel -> track name (every track has one channel and one meta 03)
fn names(d: &[u8]) -> BTreeMap<u8, String> {
    let vlq = |t: &[u8], p: &mut usize| { let mut v = 0usize; loop { let b = t[*p]; *p += 1; v = v << 7 | (b & 0x7F) as usize; if b < 0x80 { return v; } } };
    let mut o = 8 + u32::from_be_bytes([d[4], d[5], d[6], d[7]]) as usize;
    let mut m = BTreeMap::new();
    while o + 8 <= d.len() && &d[o..o + 4] == b"MTrk" {
        let len = u32::from_be_bytes([d[o + 4], d[o + 5], d[o + 6], d[o + 7]]) as usize;
        let t = &d[o + 8..o + 8 + len];
        let (mut p, mut st, mut name, mut ch) = (0, 0u8, String::new(), None);
        while p < t.len() {
            vlq(t, &mut p);
            let mut b = t[p];
            if b >= 0x80 { p += 1; st = b } else { b = st }
            match b {
                0xFF => { let ty = t[p]; p += 1; let n = vlq(t, &mut p); if ty == 3 { name = String::from_utf8_lossy(&t[p..p + n]).into(); } p += n; if ty == 0x2F { break } }
                0xF0 | 0xF7 => { let n = vlq(t, &mut p); p += n; }
                _ => { ch = Some(b & 15); p += if matches!(b >> 4, 0xC | 0xD) { 1 } else { 2 }; }
            }
        }
        if let Some(c) = ch { m.insert(c, name); }
        o += 8 + len;
    }
    m
}

#[derive(Clone, Debug)]
struct Note { ch: u8, raw: u8, key: i32, on: u64, off: u64 } // key = sounding GM key (raw + transpose - 12), micros

fn notes(s: &Smf) -> Vec<Note> {
    let (mut t, mut open, mut v): (i32, Vec<(u8, u8, usize)>, Vec<Note>) = (0, vec![], vec![]);
    for e in &s.events {
        match e.msg {
            Msg::Sysex { sub: 0, arg } => t = ((arg << 1) as i8 >> 1) as i32,
            Msg::NoteOn { ch, key, .. } => { open.push((ch, key, v.len())); v.push(Note { ch, raw: key, key: key as i32 + t - 12, on: e.micros, off: s.length_micros }); }
            Msg::NoteOff { ch, key } => if let Some(i) = open.iter().position(|&(c, k, _)| c == ch && k == key) { let (_, _, n) = open.remove(i); v[n].off = e.micros; },
            _ => {}
        }
    }
    v
}

/// Levenshtein on pitch classes with backtrace; returns (distance, matched index pairs (a, b))
fn align(a: &[i32], b: &[i32]) -> (usize, Vec<(usize, usize)>) {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n { d[i][0] = i; }
    for j in 0..=m { d[0][j] = j; }
    for i in 1..=n { for j in 1..=m {
        let c = (a[i - 1].rem_euclid(12) != b[j - 1].rem_euclid(12)) as usize;
        d[i][j] = (d[i - 1][j - 1] + c).min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
    } }
    let (mut i, mut j, mut pairs) = (n, m, vec![]);
    while i > 0 && j > 0 {
        let c = (a[i - 1].rem_euclid(12) != b[j - 1].rem_euclid(12)) as usize;
        if d[i][j] == d[i - 1][j - 1] + c { if c == 0 { pairs.push((i - 1, j - 1)); } i -= 1; j -= 1; }
        else if d[i][j] == d[i - 1][j] + 1 { i -= 1 } else { j -= 1 }
    }
    pairs.reverse();
    (d[n][m], pairs)
}

fn line(ns: &[&Note]) -> Vec<(i32, f64, f64)> { ns.iter().map(|n| (n.key, n.on as f64 / 1e6, (n.off - n.on) as f64 / 1e6)).collect() }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = a.get(1).cloned().unwrap_or("Original_data".into());
    let out_path = a.get(2).cloned().unwrap_or("melody_dump.txt".into());
    let open = |f: &str| Dat::open(format!("{dir}/{f}.DAT")).unwrap();
    let (s1, s2, m1, m2) = (open("IBM_SND1"), open("IBM_SND2"), open("MIDISND1"), open("MIDISND2"));
    let bank = Bank::parse(open("PRINCE").get(1).unwrap()).unwrap();
    let mut dump = String::new();
    println!("== PART 1: speaker vs MIDI (sounding key = file key + transpose - 12; offset = speaker key - midi key, semitones)");
    for id in 24u16..=56 {
        let (Some(sp), Some(mi)) = (s1.get(10000 + id).or_else(|| s2.get(10000 + id)), m1.get(10000 + id).or_else(|| m2.get(10000 + id))) else { continue };
        let Some((tempo, spk)) = speaker(&sp) else { continue };
        let smf = Smf::parse(&mi[1..]).unwrap();
        let nm = names(&mi[1..]);
        let ns = notes(&smf);
        let sk: Vec<i32> = spk.iter().filter_map(|x| x.0).collect();
        let sdur: f64 = spk.last().map_or(0.0, |x| x.1 + x.2);
        // candidate lines: each channel; skyline of all channels
        let mut lines: Vec<(String, Vec<&Note>)> = vec![];
        for (&c, name) in &nm { let mut l: Vec<&Note> = ns.iter().filter(|n| n.ch == c).collect(); l.sort_by_key(|n| n.on); lines.push((format!("ch{c} {name}"), l)); }
        let mut sky: Vec<&Note> = vec![];
        let mut onsets: Vec<u64> = ns.iter().map(|n| n.on).collect(); onsets.sort(); onsets.dedup();
        for o in onsets { if let Some(n) = ns.iter().filter(|n| n.on == o).max_by_key(|n| n.key) { sky.push(n); } }
        lines.push(("skyline(highest onset)".into(), sky));
        let mut res = vec![];
        for (name, l) in &lines {
            let k: Vec<i32> = l.iter().map(|n| n.key).collect();
            let (dist, pairs) = align(&sk, &k);
            res.push((dist, name.clone(), k.len(), pairs, l.clone()));
        }
        let mel = res.iter().position(|r| r.1.ends_with(" melody")).or_else(|| res.iter().position(|r| r.1.contains("melody") || r.1.contains(" hi"))).unwrap_or(0);
        let best = (0..res.len()).min_by_key(|&i| res[i].0).unwrap();
        let rep = |tag: &str, i: usize| {
            let (dist, name, n, pairs, l) = &res[i];
            let mut h: BTreeMap<i32, usize> = BTreeMap::new();
            for &(x, y) in pairs { *h.entry(sk[x] - l[y].key).or_default() += 1; }
            let ioi: Vec<f64> = pairs.windows(2).filter_map(|w| { let (a, b) = (spk.iter().filter(|x| x.0.is_some()).nth(w[0].0)?, spk.iter().filter(|x| x.0.is_some()).nth(w[1].0)?); let dm = (l[w[1].1].on - l[w[0].1].on) as f64 / 1e6; (dm > 0.0).then(|| (b.1 - a.1) / dm) }).collect();
            let (mn, mx) = ioi.iter().fold((f64::MAX, 0f64), |(a, b), &x| (a.min(x), b.max(x)));
            format!("  {tag}: {name} notes {n} vs speaker {} ; edit dist {dist} ; matched {} ; semitone offsets {:?} ; speaker/MIDI inter-onset ratio min {:.2} max {:.2}", sk.len(), pairs.len(), h, mn, mx)
        };
        println!("id {id}: speaker tempo {tempo}, {} notes ({} rests) {:.2}s ; midi {:.2}s ; midi notes {}", sk.len(), spk.len() - sk.len(), sdur, smf.secs(), ns.len());
        println!("{}", rep("melody-named", mel));
        if best != mel { println!("{}", rep("best-match  ", best)); }
        println!("{}", rep("skyline     ", res.len() - 1));
        writeln!(dump, "=== id {id}: speaker (tempo {tempo})").unwrap();
        for x in &spk { match x.0 { Some(k) => write!(dump, "{}@{:.2}:{:.2} ", kn(k), x.1, x.2), None => write!(dump, "rest:{:.2} ", x.2) }.unwrap(); }
        for i in [mel, best, res.len() - 1] {
            writeln!(dump, "\n--- midi {} (sounding key@start:len)", res[i].1).unwrap();
            for (k, s, l) in line(&res[i].4) { write!(dump, "{}@{:.2}:{:.2} ", kn(k), s, l).unwrap(); }
        }
        writeln!(dump).unwrap();
    }
    std::fs::write(&out_path, &dump).unwrap();

    println!("\n== PART 2: OPL trace vs SMF note-ons (f = fnum*49716/2^(20-block); expected key = raw + transpose - 12)");
    for id in [24u16, 28, 32, 41, 54, 56, 26, 36, 52, 39] {
        let Some(mi) = m1.get(10000 + id).or_else(|| m2.get(10000 + id)) else { continue };
        let smf = Smf::parse(&mi[1..]).unwrap();
        let ns = notes(&smf);
        let tr = adlib::trace(&smf, &bank);
        // key-ons / key-offs per voice
        let (mut a0, mut b0) = ([0u8; 9], [0u8; 9]);
        let mut ons: Vec<(u64, i32, f64, usize)> = vec![]; // micros, key, cents error vs equal temperament, voice
        let mut offs: Vec<Vec<u64>> = vec![vec![]; 9];
        let mut onidx: Vec<Option<usize>> = vec![None; 9];
        let mut dur: Vec<(usize, u64, u64)> = vec![]; // (ons index, on, off)
        for w in &tr {
            match w.reg {
                0xA0..=0xA8 => a0[(w.reg - 0xA0) as usize] = w.val,
                0xB0..=0xB8 => {
                    let v = (w.reg - 0xB0) as usize;
                    let (was, now) = (b0[v] & 0x20 != 0, w.val & 0x20 != 0);
                    b0[v] = w.val;
                    if !was && now {
                        let fnum = a0[v] as u32 | ((w.val as u32 & 3) << 8);
                        let blk = (w.val >> 2) & 7;
                        let f = fnum as f64 * 49716.0 / (1u64 << (20 - blk)) as f64;
                        let kf = hz_key(f);
                        ons.push((w.micros, kf.round() as i32, (kf - kf.round()) * 100.0, v));
                        onidx[v] = Some(ons.len() - 1);
                    } else if was && !now {
                        if let Some(i) = onidx[v].take() { dur.push((i, ons[i].0, w.micros)); offs[v].push(w.micros); }
                    }
                }
                _ => {}
            }
        }
        // pair by (micros, key)
        let mut exp: Vec<(u64, i32, u64)> = ns.iter().map(|n| (n.on, n.key, n.off)).collect();
        exp.sort();
        let mut got: Vec<(u64, i32, u64)> = ons.iter().enumerate().map(|(i, o)| (o.0, o.1, dur.iter().find(|d| d.0 == i).map_or(smf.length_micros, |d| d.2))).collect();
        got.sort();
        let (mut only_exp, mut only_got, mut bad_off, mut g) = (vec![], vec![], 0, got.clone());
        for e in &exp {
            if let Some(p) = g.iter().position(|x| x.0 == e.0 && x.1 == e.1) {
                if (g[p].2 as i64 - e.2 as i64).abs() > 1000 { bad_off += 1; }
                g.remove(p);
            } else { only_exp.push(*e); }
        }
        only_got.extend(g);
        let cents: Vec<f64> = ons.iter().map(|o| o.2).collect();
        let (cmin, cmax) = cents.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        println!("id {id}: SMF note-ons {} ; OPL key-ons {} ; unmatched SMF {} ; unmatched OPL {} ; off time differs >1ms {} ; OPL pitch vs equal temperament {:.1}..{:.1} cents", exp.len(), ons.len(), only_exp.len(), only_got.len(), bad_off, cmin, cmax);
        for e in only_exp.iter().take(5) { println!("    dropped (no OPL key-on at same time+pitch): {} at {:.3}s", kn(e.1), e.0 as f64 / 1e6); }
        for e in only_got.iter().take(5) { println!("    extra OPL key-on: {} at {:.3}s", kn(e.1), e.0 as f64 / 1e6); }
    }
    println!("\n== PART 3: transpose sysex per music cue (arg, micros)");
    for id in (24u16..=56).chain([]) {
        let Some(mi) = m1.get(10000 + id).or_else(|| m2.get(10000 + id)) else { continue };
        let s = Smf::parse(&mi[1..]).unwrap();
        let x: Vec<_> = s.events.iter().filter_map(|e| if let Msg::Sysex { sub, arg } = e.msg { Some((sub, arg, e.micros)) } else { None }).collect();
        let tp: Vec<String> = mi.windows(6).filter(|w| w[..3] == [0xFF, 0x51, 3]).map(|w| { let t = (w[3] as u32) << 16 | (w[4] as u32) << 8 | w[5] as u32; let q = (w[3] as u32) << 16 | (w[5] as u32) << 8 | w[4] as u32; format!("{t}->{q}({:+.1}% len)", (t as f64 / q as f64 - 1.0) * 100.0) }).take(2).collect();
        print!("{id}:{x:?} tempo {tp:?}\n");
    }
    println!("\n\n== PART 4: speaker raw format");
    for id in [24u16, 39, 54] {
        let r = s1.get(10000 + id).or_else(|| s2.get(10000 + id)).unwrap();
        let rows: Vec<String> = r[3..].chunks(3).take(4).map(|c| format!("{}Hz/{}", u16::from_le_bytes([c[0], c[1]]), c[2])).collect();
        let tail: Vec<String> = r[r.len().saturating_sub(6)..].iter().map(|b| format!("{b:02X}")).collect();
        println!("id {id}: type {} tempo {} first notes {:?}; len {} (3+3n = {}), tail bytes {:?}", r[0], u16::from_le_bytes([r[1], r[2]]), rows, r.len(), (r.len() - 3) % 3, tail);
    }
    let (mut cmin, mut cmax, mut ends) = (f64::MAX, f64::MIN, BTreeMap::new());
    for id in 24u16..=56 {
        let Some(r) = s1.get(10000 + id).or_else(|| s2.get(10000 + id)) else { continue };
        if r[0] & 0x7F != 0 { continue; }
        let (mut o, mut last) = (3, 0u16);
        while o + 2 < r.len() { let f = u16::from_le_bytes([r[o], r[o + 1]]); last = f; if f == 0x12 { break; } if f > 1 { let k = hz_key(f as f64); let c = (k - k.round()) * 100.0; cmin = cmin.min(c); cmax = cmax.max(c); } o += 3; }
        *ends.entry((last, r.len() - o)).or_insert(0) += 1; // (last freq, bytes left from the end marker)
    }
    println!("speaker Hz vs equal temperament over ids 24..56: {cmin:.1}..{cmax:.1} cents; end marker (freq, bytes from marker to EOF) histogram {ends:?}");
    println!("dump written to {out_path}");
}
