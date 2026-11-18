//! Music from the original MIDI files (M7 WP6): the AdLib driver of `pop_core::adlib` writes OPL2 registers, `oplon` renders them.
//! A whole cue is rendered once at startup into a mono clip whose length is exactly the SMF length, so `busy`, the death counter
//! and the cutscene waits depend on the file and not on the audio thread (docs/audio.md 7).
use oplon::Opl2;
use pop_core::{adlib, midi::Smf};

pub const RATE: u32 = 44100;
/// Peak of a rendered cue (effects stay <= 0.5; leaves room for one overlapping effect before the mixer's limiter).
const PEAK: f32 = 0.35;

/// Render `smf` through the original driver and the emulator. The closing reset of the driver silences every voice at the end.
pub fn render_opl(smf: &Smf, bank: &adlib::Bank) -> Vec<f32> {
    let total = (smf.length_micros * RATE as u64 / 1_000_000) as usize;
    let mut opl = Opl2::new(RATE);
    let mut out = Vec::with_capacity(total);
    let render_to =|opl: &mut Opl2, n: usize, out: &mut Vec<f32>| {
        while out.len() < n.min(total) {
            let (l, r) = opl.render_frame();
            out.push((l + r) as f32 / 2.0);
        }
    };
    for w in adlib::trace(smf, bank) {
        render_to(&mut opl, (w.micros * RATE as u64 / 1_000_000) as usize, &mut out);
        opl.write_reg(w.reg, w.val);
    }
    render_to(&mut opl, total, &mut out);
    let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        let g = PEAK / peak;
        out.iter_mut().for_each(|s| *s *= g);
    }
    out
}

/// The AdLib bank has 18 instruments without names; no document names them (docs/audio.md 6.6). This maps program 0..17 to
/// General MIDI by what the OPL patches and the track roles say (INFERRED, to be corrected by ear): 0 soft reed lead -> Oboe,
/// 1 bell -> Glockenspiel, 2 bass (every cue) -> Fingered Bass, 3 slow pad -> Choir Aahs, 4 thump -> Timpani, 5 long bell ->
/// Tubular Bells, 6 mellow mid voice -> Strings, 7 shimmering organ chords -> Church Organ, 8 bright sustained -> Brass
/// Section, 9 vibrato lead -> Flute, 10 (unused), 11 plucked bass pad -> Acoustic Bass, 12 reed -> Clarinet, 13 glassy pad ->
/// Warm Pad, 14 bright pluck -> Harp, 15 heartbeat -> Melodic Tom, 16 buzzy -> Synth Brass, 17 reedy -> Accordion.
pub const GM_MAP: [u8; 18] = [68, 9, 33, 52, 47, 14, 48, 19, 61, 73, 0, 32, 71, 89, 46, 117, 62, 21];
pub const SF_RATE: u32 = 32000;

/// Render `smf` with a SoundFont (rustysynth). Same rules as the original driver where they matter for the sound: controllers,
/// pressure and pitch bend are ignored; the file's transpose sysex is applied relative to General MIDI numbering (+12 in the
/// original driver equals GM pitch, so a +24 file sounds one octave up). Channel 10 (percussion in GM) is moved to a spare
/// channel because the driver has no drum channel.
pub fn render_sf(smf: &Smf, sf: &std::sync::Arc<rustysynth::SoundFont>, rate: u32) -> Vec<f32> {
    use pop_core::midi::Msg;
    let mut st = rustysynth::SynthesizerSettings::new(rate as i32);
    st.enable_reverb_and_chorus = false;
    let Ok(mut syn) = rustysynth::Synthesizer::new(sf, &st) else { return vec![] };
    let used = smf.events.iter().filter_map(|e| match e.msg { Msg::NoteOn { ch, .. } | Msg::NoteOff { ch, .. } | Msg::Program { ch, .. } => Some(ch), _ => None }).fold(0u16, |m, c| m | 1 << c);
    let ch = |c: u8| if c == 9 { (0..16).find(|&x| used >> x & 1 == 0).unwrap_or(9) } else { c as i32 };
    for c in 0..16u8 {
        syn.process_midi_message(ch(c), 0xC0, GM_MAP.get(c as usize).copied().unwrap_or(0) as i32, 0);
    }
    let total = (smf.length_micros * rate as u64 / 1_000_000) as usize;
    let (mut out, mut transpose) = (Vec::with_capacity(total), 0i32);
    let (mut l, mut r) = (vec![0f32; 512], vec![0f32; 512]);
    let mut render_to = |syn: &mut rustysynth::Synthesizer, n: usize, out: &mut Vec<f32>| {
        while out.len() < n.min(total) {
            let k = (n.min(total) - out.len()).min(512);
            syn.render(&mut l[..k], &mut r[..k]);
            out.extend(l[..k].iter().zip(&r[..k]).map(|(a, b)| (a + b) / 2.0));
        }
    };
    for e in &smf.events {
        render_to(&mut syn, (e.micros * rate as u64 / 1_000_000) as usize, &mut out);
        let key = |k: u8| (k as i32 + transpose - 12).clamp(0, 127);
        match e.msg {
            Msg::NoteOn { ch: c, key: k, vel } => syn.note_on(ch(c), key(k), vel as i32),
            Msg::NoteOff { ch: c, key: k } => syn.note_off(ch(c), key(k)),
            Msg::Program { ch: c, prog } if (prog as usize) < GM_MAP.len() => syn.process_midi_message(ch(c), 0xC0, GM_MAP[prog as usize] as i32, 0),
            Msg::Sysex { sub: 0, arg } => transpose = ((arg << 1) as i8 >> 1) as i32,
            _ => {}
        }
    }
    render_to(&mut syn, total, &mut out);
    let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        out.iter_mut().for_each(|s| *s *= PEAK / peak);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pop_core::dat::Dat;

    /// WP7: in adlib mode a cue is exactly as long as its SMF, so every Len::Quiet / death-counter wait follows the MIDI length,
    /// not the speaker one (docs/audio.md 6.5: cue 50 = 14.50 s MIDI vs 9.50 s speaker).
    #[test]
    fn cue_length_is_the_midi_length_not_the_speaker_one() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let Ok(snd) = Dat::open(format!("{dir}MIDISND2.DAT")) else { return eprintln!("skipping: no Original_data") };
        let bank = adlib::Bank::parse(Dat::open(format!("{dir}PRINCE.DAT")).unwrap().get(1).unwrap()).unwrap();
        let pcm = render_opl(&Smf::parse(&snd.get(10050).unwrap()[1..]).unwrap(), &bank);
        let secs = pcm.len() as f32 / RATE as f32;
        assert!((secs - 14.50).abs() < 0.02 && (secs - 9.50).abs() > 4.0, "{secs}");
    }

    #[test]
    fn cue_renders_non_silent_finite_and_exact_length() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let Ok(snd) = Dat::open(format!("{dir}MIDISND1.DAT")) else { return eprintln!("skipping: no Original_data") };
        let bank = adlib::Bank::parse(Dat::open(format!("{dir}PRINCE.DAT")).unwrap().get(1).unwrap()).unwrap();
        let smf = Smf::parse(&snd.get(10024).unwrap()[1..]).unwrap();
        let pcm = render_opl(&smf, &bank);
        assert_eq!(pcm.len(), (smf.length_micros * RATE as u64 / 1_000_000) as usize);
        assert!(pcm.iter().all(|s| s.is_finite() && s.abs() <= PEAK + 1e-4));
        assert!(pcm.iter().filter(|s| s.abs() > 0.01).count() > pcm.len() / 20, "mostly silent");
    }
}
