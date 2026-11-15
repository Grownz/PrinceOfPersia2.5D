//! Audio (M7): plays the simulation's sound log (`World::sounds`) once per fixed step, so sounds follow the logic ticks
//! (including the 5/6-unit frame length). Default: the procedural set of `synth`; `POP_SOUND=original` (or "sound": "original" in settings.json) uses the game's own
//! digitized effects (DIGISND*.DAT) and PC-speaker tunes (IBM_SND*.DAT) instead. M = mute, - / = volume. docs/audio.md.
use crate::settings::Settings;
use crate::Game;
use bevy::audio::{AddAudioSource, ChannelCount, Decodable, Sample, SampleRate, Source, Volume};
use bevy::prelude::*;
use pop_core::dat::Dat;
use std::sync::Arc;

/// Mono PCM clip generated in memory.
#[derive(Asset, TypePath, Clone)]
pub struct Pcm {
    rate: u32,
    samples: Arc<[f32]>,
}

pub struct PcmDecoder(Pcm, usize);

impl Iterator for PcmDecoder {
    type Item = Sample;
    fn next(&mut self) -> Option<Sample> {
        let s = *self.0.samples.get(self.1)?;
        self.1 += 1;
        Some(s as Sample)
    }
}

impl Source for PcmDecoder {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.0.samples.len() - self.1)
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.0.rate).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs_f32(self.0.samples.len() as f32 / self.0.rate as f32))
    }
}

impl Decodable for Pcm {
    type Decoder = PcmDecoder;
    fn decoder(&self) -> PcmDecoder {
        PcmDecoder(self.clone(), 0)
    }
}

/// One clip per sound id (index = id) and its length in fixed steps (60 Hz).
#[derive(Resource)]
pub struct Sounds(Vec<Option<(Handle<Pcm>, u32)>>);

#[derive(Resource)]
pub struct AudioState {
    /// fixed steps until every started sound has ended ("a sound is playing" in the original)
    pub busy: u32,
    gen: u32,
    volume: f32,
    muted: bool,
}

/// A playing sound effect / music cue.
#[derive(Component)]
pub struct Sfx(u8);

const IDS: u8 = 57;

pub fn plugin(app: &mut App) {
    app.add_audio_source::<Pcm>()
        .insert_resource(AudioState { busy: 0, gen: 0, volume: 0.7, muted: false })
        .add_systems(Startup, setup);
}

/// Original sound `id`: digitized (type 1: u16 rate, u16 count, u16 ?, u8 bits = 8, unsigned samples; type bit 7 = flag) or
/// PC speaker (type 0: u16 tempo, notes {u16 Hz (0/1 rest, 0x12 end), u8 length in 1/tempo s}), rendered as a square wave.
fn original(dats: &[Dat], id: u8) -> Option<Pcm> {
    decode(&dats.iter().find_map(|d| d.get(10000 + id as u16))?)
}

fn decode(r: &[u8]) -> Option<Pcm> {
    let u16at = |o: usize| r.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    match r.first()? & 0x7F {
        1 if r.get(7) == Some(&8) => {
            let n = (u16at(3)? as usize).min(r.len() - 8);
            let rate = u16at(1)? as u32;
            // untrusted header: an implausible rate falls back to the synthesized clip
            (2000..=48000).contains(&rate).then(|| Pcm { rate, samples: r[8..8 + n].iter().map(|&b| (b as f32 - 128.0) / 128.0 * 0.5).collect() })
        }
        0 => {
            let tempo = u16at(1)?.max(1) as f32;
            let rate = crate::synth::RATE;
            let (mut v, mut ph, mut o) = (vec![], 0.0f32, 3);
            while let (Some(f), Some(&l)) = (u16at(o), r.get(o + 2)) {
                if f == 0x12 {
                    break;
                }
                for _ in 0..(l as f32 / tempo * rate as f32) as usize {
                    ph = (ph + f as f32 / rate as f32).fract();
                    v.push(if f <= 1 { 0.0 } else if ph < 0.5 { 0.25 } else { -0.25 });
                }
                o += 3;
            }
            (!v.is_empty()).then(|| Pcm { rate, samples: v.into() })
        }
        _ => None, // MIDI (type 2) is not decoded
    }
}

fn setup(mut cmds: Commands, mut pcms: ResMut<Assets<Pcm>>, mut a: ResMut<AudioState>, set: Res<Settings>) {
    (a.volume, a.muted) = (set.volume.clamp(0.0, 1.0), set.muted);
    let orig = std::env::var("POP_SOUND").map_or(set.sound == "original", |v| v == "original");
    let dir = crate::data_dir();
    let dats: Vec<Dat> = if orig { ["DIGISND1", "DIGISND2", "DIGISND3", "IBM_SND1", "IBM_SND2"].iter().filter_map(|f| Dat::open(format!("{dir}/{f}.DAT")).ok()).collect() } else { vec![] };
    let clips = (0..IDS)
        .map(|id| {
            // original mode: the game's clip where it has one, else the synthesized one
            let pcm = orig.then(|| original(&dats, id)).flatten().or_else(|| crate::synth::sound(id).map(|v| Pcm { rate: crate::synth::RATE, samples: v.into() }))?;
            let steps = (pcm.samples.len() as u64 * pop_core::sim::STEP_HZ as u64).div_ceil(pcm.rate as u64) as u32;
            Some((pcms.add(pcm), steps))
        })
        .collect::<Vec<_>>();
    info!("audio: {} mode, {} clips", if orig { "original" } else { "synthesized" }, clips.iter().flatten().count());
    cmds.insert_resource(Sounds(clips));
}

impl AudioState {
    fn gain(&self) -> Volume {
        Volume::Linear(if self.muted { 0.0 } else { self.volume })
    }
}

fn play(cmds: &mut Commands, s: &Sounds, a: &mut AudioState, id: u8, playing: &Query<(Entity, &Sfx)>) {
    let Some(Some((h, steps))) = s.0.get(id as usize) else { return };
    // the same sound again restarts it (the original has one channel; this keeps repeated door/gate ticks from piling up)
    for (e, _) in playing.iter().filter(|(_, x)| x.0 == id) {
        cmds.entity(e).despawn();
    }
    debug!("sound {id} ({steps} steps)");
    cmds.spawn((AudioPlayer::<Pcm>(h.clone()), PlaybackSettings::DESPAWN.with_volume(a.gain()), Sfx(id)));
    a.busy = a.busy.max(*steps);
}

impl AudioState {
    /// Before a step: a new World (gen changed) clears the busy counter; returns (new world, hold_death_counter for this
    /// step). Then one step of the counter elapses.
    fn begin_step(&mut self, gen: u32) -> (bool, bool) {
        let fresh = std::mem::replace(&mut self.gen, gen) != gen;
        if fresh {
            self.busy = 0;
        }
        let hold = self.busy > 0;
        self.busy = self.busy.saturating_sub(1);
        (fresh, hold)
    }
}

/// FixedUpdate, before the world step: reset on a new World, set `hold_death_counter` (owned by the app) for this step.
pub fn before_step(mut cmds: Commands, mut g: ResMut<Game>, mut a: ResMut<AudioState>, playing: Query<(Entity, &Sfx)>) {
    let (fresh, hold) = a.begin_step(g.gen);
    if fresh {
        stop_all(&mut cmds, &mut a, &playing); // a new world (restart, next level): the original stops all sounds
    }
    g.world.hold_death_counter = hold;
}

/// FixedUpdate, after the world step: drain the sound log; hold the death counter while a sound plays (edge_cases.md 3.2).
/// The timing counts fixed steps (not the audio device), so it is deterministic and independent of mute and volume.
pub fn fixed(mut cmds: Commands, mut g: ResMut<Game>, s: Res<Sounds>, mut a: ResMut<AudioState>, playing: Query<(Entity, &Sfx)>) {
    let mut ids = std::mem::take(&mut g.world.sounds);
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        play(&mut cmds, &s, &mut a, id, &playing);
    }
}

fn stop_all(cmds: &mut Commands, a: &mut AudioState, playing: &Query<(Entity, &Sfx)>) {
    for (e, _) in playing {
        cmds.entity(e).despawn();
    }
    a.busy = 0;
}

/// Title and cards silence the game; a card plays its cue (the cutscene music stands in for the cutscene).
pub fn enter_title(mut cmds: Commands, mut a: ResMut<AudioState>, playing: Query<(Entity, &Sfx)>) {
    stop_all(&mut cmds, &mut a, &playing);
}

pub fn enter_card(mut cmds: Commands, s: Res<Sounds>, mut a: ResMut<AudioState>, f: Res<crate::flow::Flow>, playing: Query<(Entity, &Sfx)>) {
    stop_all(&mut cmds, &mut a, &playing);
    if let Some(id) = f.card_sound {
        play(&mut cmds, &s, &mut a, id, &playing);
    }
    a.busy = 0; // cards play in real time; only Playing steps count busy
}

pub fn pause(sinks: Query<&AudioSink, With<Sfx>>) {
    sinks.iter().for_each(|s| s.pause());
}

pub fn resume(sinks: Query<&AudioSink, With<Sfx>>) {
    sinks.iter().for_each(|s| s.play());
}

/// M = mute, - / = = master volume (10 % steps); applied to every sound (effects, music cues, card cues all carry `Sfx`),
/// including ones spawned this frame whose sink does not exist yet.
pub fn keys(
    k: Res<ButtonInput<KeyCode>>,
    mut a: ResMut<AudioState>,
    mut g: ResMut<Game>,
    mut set: ResMut<Settings>,
    mut sinks: Query<&mut AudioSink, With<Sfx>>,
    mut pending: Query<&mut PlaybackSettings, (With<Sfx>, Without<AudioSink>)>,
) {
    let step = (k.just_pressed(KeyCode::Equal) || k.just_pressed(KeyCode::NumpadAdd)) as i32 - (k.just_pressed(KeyCode::Minus) || k.just_pressed(KeyCode::NumpadSubtract)) as i32;
    let mute = k.just_pressed(KeyCode::KeyM);
    if step == 0 && !mute {
        return;
    }
    a.muted ^= mute;
    a.volume = (a.volume + step as f32 * 0.1).clamp(0.0, 1.0);
    if step != 0 {
        a.muted = false;
    }
    let v = a.gain();
    sinks.iter_mut().for_each(|mut s| s.set_volume(v));
    pending.iter_mut().for_each(|mut p| p.volume = v);
    (set.volume, set.muted) = (a.volume, a.muted);
    set.save();
    g.banner = (24, if a.muted { "SOUND OFF".into() } else { format!("VOLUME {}%", (a.volume * 100.0).round()) });
}

#[cfg(test)]
mod tests {
    #[test]
    fn bad_rate_does_not_panic_and_busy_resets() {
        let mut r = vec![1u8, 0, 0, 4, 0, 0, 0, 8, 128, 128, 128, 128]; // type 1, rate 0, 4 samples
        assert!(super::decode(&r).is_none());
        r[1] = 0x2B; r[2] = 0x2B; // 11051 Hz
        assert_eq!(super::decode(&r).unwrap().samples.len(), 4);
        let mut a = super::AudioState { busy: 3, gen: 0, volume: 1.0, muted: false };
        assert_eq!(a.begin_step(0), (false, true)); // busy 3 -> hold, drains
        assert_eq!(a.begin_step(1), (true, false)); // new World: cleared
        assert_eq!(a.busy, 0);
    }

    /// busy counts fixed steps: a 3-step sound holds exactly the next 3 steps, then releases; a new World clears it.
    #[test]
    fn busy_drains_one_step_at_a_time() {
        let mut a = super::AudioState { busy: 3, gen: 7, volume: 1.0, muted: false };
        let holds: Vec<_> = (0..5).map(|_| a.begin_step(7)).collect();
        assert_eq!(holds, [(false, true), (false, true), (false, true), (false, false), (false, false)]);
        a.busy = 100;
        assert_eq!(a.begin_step(8), (true, false));
        assert_eq!((a.busy, a.begin_step(8)), (0, (false, false)));
        // mute does not change the timing, only the gain
        let m = super::AudioState { busy: 2, gen: 0, volume: 0.5, muted: true };
        assert!(matches!(m.gain(), bevy::audio::Volume::Linear(v) if v == 0.0));
    }

    /// Feather fall ends with sound 39 (edge_cases.md 5.1): the speaker tune's length, rounded up to whole 5/60 s frames,
    /// is sim::FEATHER_TICKS (see its doc comment for the off-by-one).
    #[test]
    fn feather_ticks_match_the_speaker_tune() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data");
        let (Ok(s1), Ok(s2)) = (pop_core::dat::Dat::open(format!("{dir}/IBM_SND1.DAT")), pop_core::dat::Dat::open(format!("{dir}/IBM_SND2.DAT"))) else { return };
        let dats = [s1, s2];
        let p = super::original(&dats, 39).unwrap();
        let secs = p.samples.len() as f64 / p.rate as f64;
        assert!((secs - 18.61).abs() < 0.02, "{secs}");
        assert_eq!(pop_core::sim::FEATHER_TICKS as f64, (secs * 12.0).ceil());
    }

    /// The original decoders: digitized footstep-like click (id 0, 11000 Hz) and the PC-speaker death tune (id 24, 2.97 s).
    #[test]
    fn decodes_original_sounds() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data");
        let Ok(d) = pop_core::dat::Dat::open(format!("{dir}/DIGISND1.DAT")) else { return }; // no game data: nothing to test
        let s = pop_core::dat::Dat::open(format!("{dir}/IBM_SND1.DAT")).unwrap();
        let dats = [d, s];
        let p = super::original(&dats, 0).unwrap();
        assert_eq!((p.rate, p.samples.len()), (11000, 1655));
        assert!(p.samples.iter().any(|x| x.abs() > 0.1));
        let m = super::original(&dats, 24).unwrap();
        assert!((m.samples.len() as f32 / m.rate as f32 - 2.97).abs() < 0.02);
        assert!(super::original(&dats, 31).is_none());
    }
}
