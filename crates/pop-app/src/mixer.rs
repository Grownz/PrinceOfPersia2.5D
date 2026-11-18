//! One endless audio source that sums every playing clip and limits the sum (M7 WP8, docs/audio.md 7).
//! bevy_audio would play each clip as its own source and clip their sum on the device; here the game sends commands
//! (`play` / `stop`) and the audio thread mixes, so the peak limiter sees the whole sum. Timing never depends on this:
//! `AudioState::busy` still counts fixed steps from the clip lengths.
use bevy::audio::{ChannelCount, Decodable, Sample, SampleRate, Source};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};

pub const RATE: u32 = 44100;
/// -1 dBFS: the limiter keeps |out| at or below this.
const CEILING: f32 = 0.89;
/// the gain recovers from 0 to 1 in about this many seconds
const RELEASE_SECS: f32 = 0.08;
/// commands are picked up this often (samples)
const BLOCK: u32 = 256;

pub enum Cmd {
    /// start clip `id` (an identical id restarts: the original has one channel per sound)
    Play { id: u8, rate: u32, samples: Arc<[f32]> },
    StopAll,
}

#[derive(Default)]
pub struct Shared(Mutex<Vec<Cmd>>);

impl Shared {
    pub fn send(&self, c: Cmd) {
        if let Ok(mut q) = self.0.lock() {
            q.push(c);
        }
    }
}

struct Voice {
    id: u8,
    rate: f32,
    samples: Arc<[f32]>,
    pos: f64,
}

/// The mixing state, independent of bevy so it can be tested headless.
pub struct Core {
    voices: Vec<Voice>,
    gain: f32,
}

impl Core {
    pub fn new() -> Core {
        Core { voices: vec![], gain: 1.0 }
    }
    pub fn apply(&mut self, c: Cmd) {
        match c {
            Cmd::Play { id, rate, samples } => {
                self.voices.retain(|v| v.id != id);
                self.voices.push(Voice { id, rate: rate as f32, samples, pos: 0.0 });
            }
            Cmd::StopAll => self.voices.clear(),
        }
    }
    #[cfg(test)]
    pub fn playing(&self) -> usize {
        self.voices.len()
    }
    /// The next output sample: sum of the voices (linear resampling to `RATE`), then the limiter (instant attack, ~80 ms release).
    pub fn next(&mut self) -> f32 {
        let mut sum = 0.0;
        for v in &mut self.voices {
            let i = v.pos as usize;
            if let Some(&a) = v.samples.get(i) {
                let b = v.samples.get(i + 1).copied().unwrap_or(a);
                sum += a + (b - a) * (v.pos - i as f64) as f32;
            }
            v.pos += (v.rate / RATE as f32) as f64;
        }
        self.voices.retain(|v| (v.pos as usize) < v.samples.len());
        let peak = sum.abs();
        let target = if peak > CEILING { CEILING / peak } else { 1.0 };
        self.gain = if target < self.gain { target } else { (self.gain + 1.0 / (RELEASE_SECS * RATE as f32)).min(target) };
        sum * self.gain
    }
}

/// The asset behind the single `AudioPlayer` of the game.
#[derive(Asset, TypePath, Clone, Default)]
pub struct Mixer(pub Arc<Shared>);

pub struct MixDecoder {
    shared: Arc<Shared>,
    core: Core,
    n: u32,
}

impl Iterator for MixDecoder {
    type Item = Sample;
    fn next(&mut self) -> Option<Sample> {
        if self.n % BLOCK == 0 {
            // never block the audio thread: a busy lock only delays the commands by one block
            if let Ok(mut q) = self.shared.0.try_lock() {
                q.drain(..).for_each(|c| self.core.apply(c));
            }
        }
        self.n = self.n.wrapping_add(1);
        Some(self.core.next() as Sample)
    }
}

impl Source for MixDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Decodable for Mixer {
    type Decoder = MixDecoder;
    fn decoder(&self) -> MixDecoder {
        MixDecoder { shared: self.0.clone(), core: Core::new(), n: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(level: f32, secs: f32) -> Arc<[f32]> {
        (0..(RATE as f32 * secs) as usize).map(|i| if i % 100 < 50 { level } else { -level }).collect()
    }

    #[test]
    fn full_scale_sum_never_exceeds_the_ceiling() {
        let mut c = Core::new();
        for id in 0..4 {
            c.apply(Cmd::Play { id, rate: RATE, samples: clip(1.0, 0.5) });
        }
        let peak = (0..RATE / 2).map(|_| c.next().abs()).fold(0.0, f32::max);
        assert!(peak <= CEILING + 1e-4, "{peak}");
    }

    #[test]
    fn transparent_below_the_threshold_and_recovers() {
        let mut c = Core::new();
        c.apply(Cmd::Play { id: 1, rate: RATE, samples: clip(0.3, 0.2) });
        let out: Vec<f32> = (0..1000).map(|_| c.next()).collect();
        assert!(out.iter().zip(clip(0.3, 0.2).iter()).all(|(a, b)| (a - b).abs() < 1e-5));
        // a loud burst pulls the gain down, then it returns to 1 within the release time
        c.apply(Cmd::Play { id: 2, rate: RATE, samples: clip(3.0, 0.01) });
        for _ in 0..(RATE / 100 + 10) { c.next(); }
        assert!(c.gain < 0.5);
        for _ in 0..(RATE / 10) { c.next(); }
        assert!((c.gain - 1.0).abs() < 1e-4);
    }

    #[test]
    fn same_id_restarts_and_clips_end() {
        let mut c = Core::new();
        c.apply(Cmd::Play { id: 7, rate: 22050, samples: clip(0.2, 0.1) });
        c.apply(Cmd::Play { id: 7, rate: 22050, samples: clip(0.2, 0.1) });
        assert_eq!(c.playing(), 1);
        for _ in 0..RATE { c.next(); }
        assert_eq!(c.playing(), 0);
    }
}
