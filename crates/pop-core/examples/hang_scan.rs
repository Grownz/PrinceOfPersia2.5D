//! throwaway: enumerate hang situations on level 1
use pop_core::{dat::Dat, exe::Exe, kid::Keys, level::*, sim::{Data, World}};
use std::sync::Arc;
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
fn data() -> Arc<Data> {
    let exe = Exe::open(format!("{DIR}PRINCE.EXE")).unwrap();
    let frames = exe.kid_frames();
    let sizes = pop_core::image::image_sizes(&Dat::open(format!("{DIR}KID.DAT")).unwrap(), 401, 220);
    let gsizes = ["GUARD.DAT", "FAT.DAT", "SKEL.DAT", "VIZIER.DAT", "SHADOW.DAT"].iter().map(|f| pop_core::image::image_sizes(&Dat::open(format!("{DIR}{f}")).unwrap(), 751, 40)).collect();
    Arc::new(Data { gframes: exe.guard_frames(), exe, frames, sizes, gsizes })
}
fn k(s: &str) -> Keys { Keys { left: s.contains('l'), right: s.contains('r'), up: s.contains('u'), down: s.contains('d'), shift: s.contains('s') } }
fn tick(w: &mut World, keys: Keys) { while !w.step(keys) {} }
fn main() {
    let (sr, st, sd): (u8, u8, u8) = (std::env::var("SR").map(|v| v.parse().unwrap()).unwrap_or(2), std::env::var("ST").map(|v| v.parse().unwrap()).unwrap_or(11), std::env::var("SD").map(|v| v.parse().unwrap()).unwrap_or(255));
    let d = Dat::open(format!("{DIR}LEVELS.DAT")).unwrap();
    let lvl = Level::parse(d.get(2001).unwrap()).unwrap();
    for r in 0..9 { println!("links room{} {:?}", r+1, lvl.links[r]); } println!("start room {} tile {} dir {}", lvl.start_room, lvl.start_tile, lvl.start_dir);
    for room in 1..=9usize {
        println!("room {room}");
        for r in 0..3 { let mut s = String::new(); for c in 0..10 { s += &format!("{:>3}", lvl.tile(room, c, r).kind as u8); } println!("{s}"); }
    }
    let dt = data();
    if std::env::var("TRACE").is_ok() {
        let mut l2 = lvl.clone(); l2.start_room = sr; l2.start_tile = st; l2.start_dir = sd; if let Ok(m) = std::env::var("MOD") { let v: Vec<usize> = m.split(':').map(|x| x.parse().unwrap()).collect(); l2.set_tile(v[0], v[1] + 10 * v[2], Tile::from_u8(v[3] as u8)); } let mut w = World::new(dt.clone(), l2, 1);
        let script = std::env::var("TRACE").unwrap();
        let mut t = 0;
        for part in script.split(',') { let (ks, n) = part.split_once(':').unwrap(); for _ in 0..n.parse::<usize>().unwrap() { tick(&mut w, k(if ks=="-" {""} else {ks})); t+=1; let kd=&w.kid; println!("t{} tick {} room {} x {} y {} row {} left {} frame {} action {}", t, w.tick, kd.room, kd.x, kd.y, kd.row, kd.facing_left, kd.frame, kd.action); } }
        return;
    }
    let mut seen = std::collections::BTreeSet::new();
    for pre in ["", "l", "r"] { for npre in [0usize, 2, 4, 8, 12, 16, 20, 30, 40] { for up_at in 0..14usize { for dir in ["", "l", "r", "lu", "ru"] {
        let mut l2 = lvl.clone(); l2.start_room = sr; l2.start_tile = st; l2.start_dir = sd; let mut w = World::new(dt.clone(), l2, 1);
        for _ in 0..npre { tick(&mut w, k(pre)); }
        for t in 0..up_at { let _ = t; tick(&mut w, k(pre)); }
        let mut hung = None;
        for t in 0..70 {
            let keys = if t < 20 { k(&format!("u{dir}")) } else { k("u") };
            tick(&mut w, keys);
            let kd = &w.kid;
            if (78..=99).contains(&kd.frame) || kd.action == 2 { hung = Some((w.tick, kd.room, kd.x, kd.y, kd.row, kd.facing_left, kd.frame, kd.action)); break; }
        }
        if let Some(h) = hung {
            let (_, room, x, y, row, fl, ..) = h;
            let key = (room, x, y, row, fl);
            if seen.insert(key) {
                let col = ((x as i32 - 58) / 14).clamp(0, 9) as usize;
                let near: Vec<u8> = (0..10).map(|c| lvl.tile(room as usize, c, row.clamp(0, 2) as usize).kind as u8).collect();
                println!("pre={pre:?}x{npre}+{up_at} dir={dir:?} => tick {} room {room} x {x} y {y} row {row} left {fl} frame {} action {} col~{col} rowtiles {near:?}", h.0, h.6, h.7);
            }
        }
    }}}}
}
