//! Procedural sound set (M7): every effect and music cue the simulation emits, synthesized at startup (no audio assets).
//! The music cues are short original jingles, not the game's melodies. Ids and meanings: docs/audio.md.
use std::f32::consts::TAU;

pub const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Tri,
}

fn len(secs: f32) -> usize {
    (secs * RATE as f32) as usize
}

/// Oscillator gliding linearly from `f0` to `f1` Hz, 3 ms attack, exponential decay `exp(-decay * t)`.
fn tone(secs: f32, f0: f32, f1: f32, w: Wave, decay: f32) -> Vec<f32> {
    let n = len(secs);
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            ph = (ph + (f0 + (f1 - f0) * i as f32 / n as f32) / RATE as f32).fract();
            let s = match w {
                Wave::Sine => (ph * TAU).sin(),
                Wave::Square => if ph < 0.5 { 0.6 } else { -0.6 },
                Wave::Tri => 1.0 - 4.0 * (ph - 0.5).abs(),
            };
            s * (t / 0.003).min(1.0) * (-decay * t).exp()
        })
        .collect()
}

/// White noise through a one-pole low-pass whose cutoff glides from `c0` to `c1` Hz, normalized, then decaying.
fn noise(secs: f32, c0: f32, c1: f32, decay: f32, seed: u32) -> Vec<f32> {
    let n = len(secs);
    let mut s = seed | 1;
    let mut y = 0.0f32;
    let mut v: Vec<f32> = (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let x = (s as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let c = c0 + (c1 - c0) * i as f32 / n as f32;
            y += (1.0 - (-TAU * c / RATE as f32).exp()) * (x - y);
            y
        })
        .collect();
    normalize(&mut v, 1.0);
    for (i, x) in v.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        *x *= (t / 0.002).min(1.0) * (-decay * t).exp();
    }
    v
}

/// Sum of (signal, gain, start seconds).
fn mix(parts: &[(Vec<f32>, f32, f32)]) -> Vec<f32> {
    let n = parts.iter().map(|(v, _, at)| len(*at) + v.len()).max().unwrap_or(0);
    let mut out = vec![0.0; n];
    for (v, g, at) in parts {
        for (o, x) in out[len(*at)..].iter_mut().zip(v) {
            *o += x * g;
        }
    }
    out
}

fn normalize(v: &mut [f32], peak: f32) {
    let m = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    if m > 0.0 {
        v.iter_mut().for_each(|x| *x *= peak / m);
    }
}

/// Multiply by a swell sin(pi * t / T) (for sounds without a hard attack).
fn swell(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.len() as f32;
    v.iter_mut().enumerate().for_each(|(i, x)| *x *= (std::f32::consts::PI * i as f32 / n).sin());
    v
}

const REST: i8 = i8::MIN;

/// A jingle: (semitones from C4 or REST, beats) at `bpm`, cut or padded to `total` seconds (fade-out over the last 0.3 s).
/// Voice: triangle + a little square for edge + a sine an octave down; a quiet drone on `root` underneath.
fn tune(notes: &[(i8, f32)], bpm: f32, total: f32, root: i8) -> Vec<f32> {
    let beat = 60.0 / bpm;
    let hz = |n: i8| 261.63 * 2f32.powf(n as f32 / 12.0);
    let mut parts = vec![];
    let mut at = 0.0;
    for &(n, b) in notes {
        let d = b * beat;
        if n != REST {
            let f = hz(n);
            let mut v = mix(&[(tone(d, f, f, Wave::Tri, 1.5), 0.6, 0.0), (tone(d, f, f, Wave::Square, 4.0), 0.15, 0.0), (tone(d, f / 2.0, f / 2.0, Wave::Sine, 1.0), 0.3, 0.0)]);
            let r = len(0.03).min(v.len());
            let k = v.len();
            v[k - r..].iter_mut().enumerate().for_each(|(i, x)| *x *= 1.0 - i as f32 / r as f32);
            parts.push((v, 1.0, at));
        }
        at += d;
    }
    let drone = hz(root - 12);
    parts.push((tone(total, drone, drone, Wave::Sine, 0.25), 0.12, 0.0));
    let mut v = mix(&parts);
    v.resize(len(total), 0.0);
    let f = len(0.3).min(v.len());
    let k = v.len();
    v[k - f..].iter_mut().enumerate().for_each(|(i, x)| *x *= 1.0 - i as f32 / f as f32);
    v
}

/// Length of a tune in seconds plus a release tail.
fn beats(notes: &[(i8, f32)], bpm: f32) -> f32 {
    notes.iter().map(|n| n.1).sum::<f32>() * 60.0 / bpm + 0.5
}

/// The synthesized clip for sound `id` (None for ids the game never plays here). Mono, `RATE` Hz, peak 0.5 (headroom: two overlapping clips at the default volume stay below 1.0).
pub fn sound(id: u8) -> Option<Vec<f32>> {
    use Wave::*;
    let jingle = |notes: &[(i8, f32)], bpm: f32, total: Option<f32>, root: i8| tune(notes, bpm, total.unwrap_or_else(|| beats(notes, bpm)), root);
    let mut v = match id {
        0 => mix(&[(noise(0.45, 900.0, 200.0, 8.0, 1), 1.0, 0.0), (tone(0.35, 90.0, 40.0, Sine, 10.0), 0.9, 0.0)]), // fell to death
        1 => {
            // falling scream: a falling, wavering voice
            let mut v = tone(0.9, 760.0, 260.0, Tri, 1.5);
            v.iter_mut().enumerate().for_each(|(i, x)| *x *= 0.75 + 0.25 * (i as f32 / RATE as f32 * TAU * 7.0).sin());
            v
        }
        2 => mix(&[(noise(0.4, 3000.0, 300.0, 9.0, 2), 1.0, 0.0), (noise(0.15, 6000.0, 6000.0, 25.0, 3), 0.5, 0.0), (noise(0.1, 2500.0, 2500.0, 30.0, 4), 0.4, 0.12)]), // floor crashes
        3 => mix(&[(tone(0.07, 220.0, 180.0, Square, 40.0), 0.6, 0.0), (noise(0.03, 4000.0, 4000.0, 60.0, 5), 0.5, 0.0)]), // button
        4 => mix(&[(noise(0.05, 2500.0, 2500.0, 60.0, 6), 0.6, 0.0), (tone(0.05, 300.0, 300.0, Square, 60.0), 0.3, 0.0)]), // gate closing tick
        5 => mix(&[(noise(0.05, 3500.0, 3500.0, 60.0, 7), 0.6, 0.0), (tone(0.05, 450.0, 450.0, Square, 60.0), 0.3, 0.0)]), // gate opening tick
        6 => mix(&[(noise(0.3, 1500.0, 200.0, 14.0, 8), 1.0, 0.0), (tone(0.25, 110.0, 60.0, Sine, 14.0), 0.8, 0.0)]), // gate slams shut
        7 => mix(&[(tone(0.25, 420.0, 420.0, Sine, 18.0), 0.8, 0.0), (tone(0.25, 1130.0, 1130.0, Sine, 25.0), 0.4, 0.0), (noise(0.04, 5000.0, 5000.0, 80.0, 9), 0.4, 0.0)]), // gate stops
        8 => mix(&[(tone(0.12, 120.0, 70.0, Sine, 30.0), 1.0, 0.0), (noise(0.06, 800.0, 800.0, 50.0, 10), 0.5, 0.0)]), // bumped
        9 => noise(0.08, 2000.0, 2000.0, 45.0, 11), // grab a ledge
        10 => mix(&[(tone(0.4, 1480.0, 1480.0, Sine, 12.0), 1.0, 0.0), (tone(0.4, 2350.0, 2350.0, Sine, 15.0), 0.7, 0.0), (tone(0.4, 3170.0, 3170.0, Sine, 18.0), 0.5, 0.0), (noise(0.03, 8000.0, 8000.0, 100.0, 12), 0.6, 0.0)]), // swords clash
        11 => swell(noise(0.16, 1500.0, 5000.0, 0.0, 13)), // sword swish
        12 => mix(&[(tone(0.22, 150.0, 95.0, Square, 10.0), 0.6, 0.0), (noise(0.08, 1200.0, 1200.0, 40.0, 14), 0.5, 0.0)]), // guard hurt
        13 => mix(&[(tone(0.22, 240.0, 160.0, Square, 10.0), 0.6, 0.0), (noise(0.08, 1500.0, 1500.0, 40.0, 15), 0.5, 0.0)]), // kid hurt
        14 => mix(&[(noise(0.5, 1200.0, 150.0, 8.0, 16), 1.0, 0.0), (tone(0.4, 70.0, 45.0, Sine, 8.0), 0.9, 0.0)]), // exit door closes
        15 => mix(&[(swell(noise(0.12, 400.0, 400.0, 0.0, 17)), 1.0, 0.0), (tone(0.12, 55.0, 55.0, Square, 5.0), 0.2, 0.0)]), // exit door sliding
        16 => mix(&[(tone(0.2, 110.0, 60.0, Sine, 18.0), 1.0, 0.0), (noise(0.12, 900.0, 900.0, 30.0, 18), 0.6, 0.0)]), // hard landing
        17 => mix(&[(tone(0.1, 90.0, 60.0, Sine, 30.0), 0.7, 0.0), (noise(0.06, 600.0, 600.0, 50.0, 19), 0.4, 0.0)]), // soft landing
        18 => mix(&[(swell(tone(0.12, 260.0, 420.0, Sine, 0.0)), 1.0, 0.0), (swell(tone(0.12, 250.0, 400.0, Sine, 0.0)), 1.0, 0.25), (swell(tone(0.14, 240.0, 380.0, Sine, 0.0)), 1.0, 0.5)]), // drink: three gulps
        19 => mix(&[(swell(noise(0.35, 6000.0, 2500.0, 3.0, 20)), 0.5, 0.0), (tone(0.35, 2100.0, 2600.0, Sine, 10.0), 0.3, 0.0)]), // draw the sword
        20..=22 => {
            let k = (id - 20) as f32;
            mix(&[(noise(0.1, 1800.0 + 400.0 * k, 1800.0 + 400.0 * k, 35.0, 21 + id as u32), 1.0, 0.0), (tone(0.1, 180.0 + 30.0 * k, 180.0 + 30.0 * k, Square, 35.0), 0.25, 0.0)]) // loose floor shakes
        }
        23 => noise(0.05, 700.0, 700.0, 70.0, 24), // footstep
        // music cues; 24/28/32 keep the PC-speaker lengths of the original (they gate the death counter, docs/audio.md)
        24 => jingle(&[(7, 0.5), (5, 0.5), (4, 0.5), (1, 0.5), (0, 2.0)], 120.0, Some(2.97), 0), // death
        25 => jingle(&[(0, 0.5), (1, 0.5), (4, 0.5), (7, 1.5)], 120.0, None, 0), // the shadow / mirror appears
        26 => jingle(&[(0, 1.0), (4, 1.0), (7, 1.0), (12, 2.0), (11, 1.0), (7, 1.0), (12, 4.0)], 110.0, None, 0), // embrace (ending)
        27 => jingle(&[(12, 1.0), (13, 0.5), (12, 0.5), (10, 1.0), (8, 1.0), (7, 2.0)], 120.0, None, 0), // princess waiting (cutscene)
        28 => jingle(&[(7, 1.0), (8, 1.0), (7, 1.0), (5, 1.0), (4, 1.0), (1, 1.0), (0, 4.0)], 100.0, Some(6.45), 0), // death in a fight
        29 => jingle(&[(-12, 0.5), (-11, 0.5), (-12, 0.5), (-11, 0.5), (-12, 1.0), (-5, 2.0)], 100.0, None, -12), // meet Jaffar
        30 => jingle(&[(0, 0.25), (4, 0.25), (7, 0.25), (12, 0.25), (16, 0.25), (19, 0.25), (24, 1.5)], 120.0, None, 0), // life potion
        32 => jingle(&[(-5, 1.0), (-4, 1.0), (-5, 1.0), (-8, 1.0), (-12, 3.0)], 90.0, Some(4.92), -12), // death by the shadow
        33 => jingle(&[(0, 0.5), (7, 0.5), (12, 1.0)], 120.0, None, 0), // heal potion
        35 => jingle(&[(12, 0.5), (16, 0.5), (19, 0.5), (16, 0.5), (12, 0.5), (16, 0.5), (19, 1.0)], 140.0, None, 0), // princess and the mouse
        36 => jingle(&[(0, 1.0), (-1, 1.0), (-4, 1.0), (-5, 1.0), (-7, 2.0)], 80.0, None, -12), // time is up
        37 => jingle(&[(7, 0.5), (7, 0.25), (7, 0.25), (12, 1.5)], 120.0, None, 0), // victory / sword found
        39 => {
            // feather fall: a floating arpeggio for the whole effect (the original's tune is 18.6 s, sim::FEATHER_TICKS 18.75 s)
            let mut notes = vec![];
            for bar in 0..20 {
                let k = [0, 5, 3, 7][bar / 2 % 4];
                notes.extend([12, 16, 19, 24, 19, 16].map(|n| (n + k, 0.25)));
            }
            jingle(&notes, 100.0, Some(18.6), 0)
        }
        40 => jingle(&[(0, 0.5), (1, 0.5), (0, 0.5), (1, 0.5), (4, 2.0)], 120.0, None, 0), // the princess's time runs short
        41 => jingle(&[(0, 0.5), (4, 0.5), (7, 0.5), (12, 1.0), (11, 0.5), (12, 0.5), (14, 0.5), (16, 2.0)], 120.0, None, 0), // level end
        43 => jingle(&[(0, 0.5), (7, 0.5), (12, 0.5), (16, 1.0), (14, 0.5), (16, 0.5), (19, 2.0)], 130.0, None, 0), // Jaffar defeated
        44 => mix(&(0..7).map(|k| (noise(0.04, 3500.0, 3500.0, 80.0, 30 + k), 0.8, k as f32 * 0.06)).chain([(tone(0.45, 80.0, 60.0, Square, 4.0), 0.2, 0.0)]).collect::<Vec<_>>()), // skeleton wakes
        45 => mix(&[(tone(0.6, 600.0, 1800.0, Sine, 4.0), 1.0, 0.0), (tone(0.6, 903.0, 2700.0, Sine, 4.0), 0.5, 0.0)]), // jump through the mirror
        46 => mix(&[(noise(0.3, 2000.0, 400.0, 12.0, 40), 1.0, 0.0), (tone(0.15, 300.0, 300.0, Square, 30.0), 0.5, 0.0)]), // chomped
        47 => mix(&[(tone(0.12, 900.0, 900.0, Square, 40.0), 0.4, 0.0), (noise(0.08, 5000.0, 5000.0, 60.0, 41), 0.6, 0.0)]), // chomper snaps
        48 => mix(&[(noise(0.3, 4000.0, 600.0, 12.0, 42), 1.0, 0.0), (tone(0.25, 500.0, 150.0, Square, 12.0), 0.4, 0.0)]), // spiked
        49 => mix(&[(tone(0.15, 1800.0, 3000.0, Sine, 20.0), 0.5, 0.0), (noise(0.1, 7000.0, 7000.0, 40.0, 43), 0.4, 0.0)]), // spikes come out
        _ => return None,
    };
    normalize(&mut v, 0.5);
    // 5 ms fade-out against clicks
    let f = len(0.005).min(v.len());
    let k = v.len();
    v[k - f..].iter_mut().enumerate().for_each(|(i, x)| *x *= 1.0 - i as f32 / f as f32);
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every id the simulation can emit (docs/audio.md) has a clip that is audible, not clipped and of a sane length.
    #[test]
    fn clips_are_audible_and_bounded() {
        let emitted = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 28, 29, 30, 32, 33, 37, 39, 41, 43, 44, 45, 46, 47, 48, 49];
        let cards = [26, 27, 35, 36, 40];
        for id in emitted.into_iter().chain(cards) {
            let v = sound(id).unwrap_or_else(|| panic!("no clip for {id}"));
            let secs = v.len() as f32 / RATE as f32;
            assert!((0.03..=19.0).contains(&secs), "{id}: {secs} s");
            let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.45 && peak <= 0.5 + 1e-6, "{id}: peak {peak}");
            assert!(v.iter().all(|x| x.is_finite()));
            let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
            assert!(rms > 0.01, "{id}: rms {rms}");
        }
        // the death music keeps the original lengths (they gate the death counter)
        for (id, secs) in [(24, 2.97), (28, 6.45), (32, 4.92)] {
            assert_eq!(sound(id).unwrap().len(), len(secs));
        }
        assert!(sound(31).is_none());
        // overlapping clips at the default volume (0.7) must not clip
        assert!(2.0 * 0.5 * 0.7 < 1.0);
    }
}
