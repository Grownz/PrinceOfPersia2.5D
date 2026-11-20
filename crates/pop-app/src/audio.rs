//! Audio (M7): plays the simulation's sound log (`World::sounds`) once per fixed step, so sounds follow the logic ticks
//! (including the 5/6-unit frame length). Default: the procedural set of `synth`; `POP_SOUND=original` (or "sound": "original" in settings.json) uses the game's own
//! digitized effects (DIGISND*.DAT) and PC-speaker tunes (IBM_SND*.DAT) instead. M = mute, - / = volume. docs/audio.md.
use crate::settings::Settings;
use crate::Game;
use crate::mixer::{Cmd, Mixer, Shared};
use bevy::audio::{AddAudioSource, Volume};
use bevy::prelude::*;
use pop_core::dat::Dat;
use std::sync::Arc;

/// Mono PCM clip generated in memory.
#[derive(Clone)]
pub struct Pcm {
    rate: u32,
    samples: Arc<[f32]>,
}

/// One clip per sound id (index = id) and its length in fixed steps (60 Hz).
#[derive(Resource)]
pub struct Sounds(Vec<Option<(Pcm, u32)>>);

#[derive(Resource, Default)]
pub struct AudioState {
    /// command queue of the single mixer source (limiter, docs/audio.md 7)
    mix: Arc<Shared>,
    /// fixed steps until every started sound has ended ("a sound is playing" in the original)
    pub busy: u32,
    gen: u32,
    pub volume: f32,
    pub muted: bool,
}

/// A playing sound effect / music cue.
#[derive(Component)]
pub struct Sfx(#[allow(dead_code)] u8);

const IDS: u8 = 57;

pub fn plugin(app: &mut App) {
    app.add_audio_source::<Mixer>()
        .insert_resource(AudioState { volume: 0.7, ..default() })
        .add_systems(Startup, setup)
        .add_systems(Update, sound_live);
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

/// The SoundFont chosen in settings ("soundfont" key), loaded from `soundfonts/` next to the exe.
fn load_soundfont(id: &str) -> Option<Arc<rustysynth::SoundFont>> {
    let i = crate::soundfont::index_of(id)?;
    let path = crate::soundfont::path(i);
    let mut r = std::io::BufReader::new(std::fs::File::open(&path).map_err(|e| warn!("soundfont {}: {e} (download it in Settings > SoundFonts); using adlib", path.display())).ok()?);
    rustysynth::SoundFont::new(&mut r).map_err(|e| warn!("soundfont {}: {e:?}; using adlib", path.display())).ok().map(Arc::new)
}

/// The sound mode in use: POP_SOUND, else the `sound` setting.
fn sound_mode(set: &Settings) -> String {
    std::env::var("POP_SOUND").unwrap_or_else(|_| set.sound.clone())
}

/// Renders every clip of sound set `mode` (the OPL / SoundFont renders take a second or more).
fn render_clips(mode: &str, soundfont: &str) -> Vec<Option<(Pcm, u32)>> {
    // soundfont: the downloaded SoundFont plays the music; without a usable file it falls back to adlib (log line)
    let sf = (mode == "soundfont").then(|| load_soundfont(soundfont)).flatten();
    let adlib = mode == "adlib" || (mode == "soundfont" && sf.is_none()); // original effects + MIDI through the AdLib driver and an OPL2 emulator
    let orig = adlib || sf.is_some() || mode == "original";
    let dir = crate::data_dir();
    let dats: Vec<Dat> = if orig { ["DIGISND1", "DIGISND2", "DIGISND3", "IBM_SND1", "IBM_SND2"].iter().filter_map(|f| Dat::open(format!("{dir}/{f}.DAT")).ok()).collect() } else { vec![] };
    // every set but `original` plays the real melodies from the MIDI files (the synth set's own phrases were invented, docs/audio.md 6.7)
    let opl_music = sf.is_none() && mode != "original";
    let midi: Vec<Dat> = if opl_music || sf.is_some() { ["MIDISND1", "MIDISND2"].iter().filter_map(|f| Dat::open(format!("{dir}/{f}.DAT")).ok()).collect() } else { vec![] };
    let bank = opl_music.then(|| Dat::open(format!("{dir}/PRINCE.DAT")).ok().and_then(|d| pop_core::adlib::Bank::parse(d.get(1)?))).flatten();
    let t0 = std::time::Instant::now();
    let clips = (0..IDS)
        .map(|id| {
            // original mode: the game's clip where it has one, else the synthesized one; adlib / soundfont: MIDI first
            let smf = || pop_core::midi::Smf::parse(midi.iter().find_map(|d| d.get(10000 + id as u16))?.get(1..)?);
            let music = match (&sf, &bank) {
                (Some(f), _) => smf().map(|m| Pcm { rate: crate::music::SF_RATE, samples: crate::music::render_sf(&m, f, crate::music::SF_RATE).into() }),
                (None, Some(b)) => smf().map(|m| Pcm { rate: crate::music::RATE, samples: crate::music::render_opl(&m, b).into() }),
                _ => None,
            };
            let pcm = music.or_else(|| orig.then(|| original(&dats, id)).flatten()).or_else(|| crate::synth::sound(id).map(|v| Pcm { rate: crate::synth::RATE, samples: v.into() }))?;
            let steps = (pcm.samples.len() as u64 * pop_core::sim::STEP_HZ as u64).div_ceil(pcm.rate as u64) as u32;
            Some((pcm, steps))
        })
        .collect::<Vec<_>>();
    info!("audio: {mode} mode, {} clips, rendered in {:?}", clips.iter().flatten().count(), t0.elapsed());
    clips
}

fn setup(mut cmds: Commands, mut mixers: ResMut<Assets<Mixer>>, mut a: ResMut<AudioState>, set: Res<Settings>) {
    (a.volume, a.muted) = (set.volume.clamp(0.0, 1.0), set.muted);
    let clips = render_clips(&sound_mode(&set), &set.soundfont);
    cmds.insert_resource(Sounds(clips));
    // the one source that mixes and limits everything; `Sfx(255)` lets pause / volume reach its sink
    cmds.spawn((AudioPlayer::<Mixer>(mixers.add(Mixer(a.mix.clone()))), PlaybackSettings::ONCE.with_volume(a.gain()), Sfx(255)));
}

/// `sound` and `soundfont` apply live: the clips are rendered again on a thread and swapped in when ready (what plays stops;
/// the clip lengths, and with them the waits, follow the new set). The mixer source stays.
#[allow(clippy::type_complexity)]
pub fn sound_live(set: Res<Settings>, mut a: ResMut<AudioState>, mut cmds: Commands, mut last: Local<Option<(String, String, bool)>>, mut job: Local<Option<std::sync::mpsc::Receiver<Vec<Option<(Pcm, u32)>>>>>) {
    let mode = sound_mode(&set);
    let font = if mode == "soundfont" { set.soundfont.clone() } else { String::new() }; // the font only matters in that set
    // a download that finishes (or a delete) of the chosen font changes the key: it is loaded (or dropped) at once
    let ready = mode == "soundfont" && crate::soundfont::index_of(&set.soundfont).is_some_and(crate::soundfont::available);
    let now = (mode, font, ready);
    if last.is_none() {
        *last = Some(now.clone());
    }
    if last.as_ref() != Some(&now) {
        *last = Some(now.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(render_clips(&now.0, &now.1)); // a newer request drops this receiver: the send fails, harmlessly
        });
        *job = Some(rx);
    }
    if let Some(clips) = job.as_ref().and_then(|rx| rx.try_recv().ok()) {
        *job = None;
        a.mix.send(Cmd::StopAll);
        a.busy = 0;
        cmds.insert_resource(Sounds(clips));
    }
}

impl AudioState {
    fn gain(&self) -> Volume {
        Volume::Linear(if self.muted { 0.0 } else { self.volume })
    }
}

/// Fixed steps sound `id` lasts by the known (synth) length table; 0 for ids without one.
fn synth_steps(id: u8) -> u32 {
    crate::synth::sound(id).map_or(0, |v| (v.len() as u64 * pop_core::sim::STEP_HZ as u64).div_ceil(crate::synth::RATE as u64) as u32)
}

pub fn play(_cmds: &mut Commands, s: &Sounds, a: &mut AudioState, id: u8, _playing: &Query<(Entity, &Sfx)>) {
    let Some(Some((pcm, steps))) = s.0.get(id as usize) else {
        a.busy = a.busy.max(synth_steps(id)); // no clip: still "playing" for its known length, so Len::Quiet waits the same
        return;
    };
    // the same sound again restarts it (the original has one channel; this keeps repeated door/gate ticks from piling up)
    debug!("sound {id} ({steps} steps)");
    a.mix.send(Cmd::Play { id, rate: pcm.rate, samples: pcm.samples.clone() });
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

pub fn stop_all(_cmds: &mut Commands, a: &mut AudioState, _playing: &Query<(Entity, &Sfx)>) {
    a.mix.send(Cmd::StopAll);
    a.busy = 0;
}

/// The title silences the game, except when it comes back from the intro scene (its music 52 plays on over page 43).
pub fn enter_title(mut cmds: Commands, mut a: ResMut<AudioState>, f: Res<crate::flow::Flow>, playing: Query<(Entity, &Sfx)>) {
    if f.fresh() {
        stop_all(&mut cmds, &mut a, &playing);
    }
}

/// A cutscene starts silent (the original waits for the level music before load_intro; the title waits for its story music).
pub fn enter_cutscene(mut cmds: Commands, mut a: ResMut<AudioState>, playing: Query<(Entity, &Sfx)>) {
    stop_all(&mut cmds, &mut a, &playing);
}

/// FixedUpdate outside Playing (title, cutscenes): one step of the "sound playing" counter elapses.
pub fn drain(mut a: ResMut<AudioState>) {
    a.busy = a.busy.saturating_sub(1);
}

pub fn pause(sinks: Query<&AudioSink, With<Sfx>>) {
    sinks.iter().for_each(|s| s.pause());
}

pub fn resume(sinks: Query<&AudioSink, With<Sfx>>) {
    sinks.iter().for_each(|s| s.play());
}

/// The volume / mute of `a` on every playing sound and on the ones spawned this frame (no sink yet).
pub fn apply_gain(a: &AudioState, sinks: &mut Query<&mut AudioSink, With<Sfx>>, pending: &mut Query<&mut PlaybackSettings, (With<Sfx>, Without<AudioSink>)>) {
    let v = a.gain();
    sinks.iter_mut().for_each(|mut s| s.set_volume(v));
    pending.iter_mut().for_each(|mut p| p.volume = v);
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
    apply_gain(&a, &mut sinks, &mut pending);
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
        let mut a = super::AudioState { busy: 3, volume: 1.0, ..Default::default() };
        assert_eq!(a.begin_step(0), (false, true)); // busy 3 -> hold, drains
        assert_eq!(a.begin_step(1), (true, false)); // new World: cleared
        assert_eq!(a.busy, 0);
    }

    /// A missing clip still holds the wait for the cue's known length (intro music 54 = 24.74 s).
    #[test]
    fn missing_clip_keeps_its_length() {
        let want = (24.74f32 * pop_core::sim::STEP_HZ as f32) as u32;
        assert!(super::synth_steps(54).abs_diff(want) <= 2, "{}", super::synth_steps(54));
        assert_eq!(super::synth_steps(255), 0);
    }

    /// busy counts fixed steps: a 3-step sound holds exactly the next 3 steps, then releases; a new World clears it.
    #[test]
    fn busy_drains_one_step_at_a_time() {
        let mut a = super::AudioState { busy: 3, gen: 7, volume: 1.0, ..Default::default() };
        let holds: Vec<_> = (0..5).map(|_| a.begin_step(7)).collect();
        assert_eq!(holds, [(false, true), (false, true), (false, true), (false, false), (false, false)]);
        a.busy = 100;
        assert_eq!(a.begin_step(8), (true, false));
        assert_eq!((a.busy, a.begin_step(8)), (0, (false, false)));
        // mute does not change the timing, only the gain
        let m = super::AudioState { busy: 2, volume: 0.5, muted: true, ..Default::default() };
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
