//! Cutscene player (docs/cutscenes.md): the princess-room scenes of PRINCE.EXE v1.1 as data-driven scripts, played frame by
//! frame like proc_cutscene_frame (0000:2540). Headless and deterministic: the app renders the exposed state and plays the
//! started sounds; "a sound is playing" comes from the caller.
use crate::exe::{Frame, Op};
use crate::kid::Rng;
use crate::sim::Data;
use std::sync::Arc;

/// Fade length in cutscene frames (fade_in_1 / fade_out_1). UNVERIFIED: 64 palette steps with retrace waits, estimated
/// 1.83 s = about 18 frames of 6 units on VGA (docs/cutscenes.md 1.5); machine dependent in the original.
pub const FADE_FRAMES: u16 = 18;

/// The two character records of a cutscene: Kid (vizier, mouse or kid) and Guard (the princess).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Kid,
    Princess,
}

/// Actor setup routines (docs/cutscenes.md 1.6): (charid, x, y, facing left, sequence).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setup {
    InitPrincess,
    PrincessRight,
    PrincessX156,
    PrincessStand,
    PrincessCrouching,
    PrincessLying,
    EndingPrincess,
    Vizier,
    EndingKid,
    MouseGo,
    MouseCu8,
    MouseEnding,
}

impl Setup {
    fn params(self) -> (u8, u8, u8, bool, usize) {
        use Setup::*;
        match self {
            InitPrincess => (5, 120, 166, true, 94),
            PrincessRight => (5, 120, 166, false, 94),
            PrincessX156 => (5, 156, 166, true, 94),
            PrincessStand => (5, 144, 169, false, 94),
            PrincessCrouching => (5, 131, 169, true, 110),
            PrincessLying => (5, 92, 162, true, 103),
            EndingPrincess => (5, 136, 164, true, 109),
            Vizier => (6, 198, 166, true, 95),
            EndingKid => (0, 198, 164, true, 1),
            MouseGo => (24, 199, 167, true, 105),
            MouseCu8 => (24, 144, 167, true, 106),
            MouseEnding => (24, 197, 164, true, 105),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// set up an actor and play its first frame at once
    Spawn(Slot, Setup),
    /// 2724 / 273D: new sequence, shown from the next frame
    Seq(Slot, u8),
    /// Char.frame = 0: not played, not drawn
    Hide(Slot),
    /// proc_cutscene_frame(n)
    Frames(u16),
    /// do P(1) while a sound plays
    WaitSound,
    /// play_sound: queue the sound
    Sound(u8),
    FrameTime(u8),
    /// set_hourglass_state: state and sandflow = 0
    SetHourglass(u8),
    /// hourglass_state only
    HourglassState(u8),
    Sandflow(i8),
    Flash { frames: u8, colour: u8 },
    FadeIn,
    FadeOut,
    DisableKeys,
    /// after the scene: wait (black screen, no frames drawn) until no sound plays
    WaitSoundEnd,
}

/// Which scene: the cutscene before a level (2, 4, 6, 8, 9, 12), the time out, the ending, the title's intro (pv_scene).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Level(usize),
    TimeOut,
    Ending,
    Intro,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    /// 0: PV.DAT 850 set (Jaffar) for chtab 4, 1: the 900 set (princess)
    pub which_imgs: u8,
    pub steps: Vec<Step>,
}

/// hourglass_frame (0000:2D32): 6 - number of thresholds {6, 17, 33, 65} <= rem_min.
pub fn hourglass_frame(rem_min: i32) -> u8 {
    6 - [6, 17, 33, 65].iter().filter(|&&t| t <= rem_min).count() as u8
}

/// get_torch_frame (0000:8CC5) with r = prandom(255).
pub fn torch_frame(prev: u8, r: u32) -> u8 {
    if r < 9 && r != prev as u32 { r as u8 } else { (prev + 1) % 9 }
}

impl Scene {
    /// The scene's script (section 4); `rem_min` picks the hourglass and the level-12 branch. None: no cutscene there.
    pub fn script(self, rem_min: i32) -> Option<Script> {
        use Setup::*;
        use Slot::*;
        use Step::*;
        let hf = hourglass_frame(rem_min);
        let c2_6 = |w| Script { which_imgs: w, steps: vec![Sound(27), SetHourglass(hf), Spawn(Princess, PrincessRight), FadeIn, Frames(26), FadeOut] };
        Some(match self {
            Scene::Level(2) => c2_6(0),
            Scene::Level(6) => c2_6(1),
            Scene::Level(4) => Script { which_imgs: 1, steps: vec![Sound(27), SetHourglass(hf), Spawn(Princess, PrincessLying), FadeIn, Frames(26), FadeOut] },
            Scene::Level(8) => Script {
                which_imgs: 1,
                steps: vec![Sound(35), SetHourglass(hf), Spawn(Kid, MouseCu8), Spawn(Princess, PrincessCrouching), FadeIn, Frames(20), Seq(Kid, 107), Frames(20), Seq(Princess, 111), Frames(20), Hide(Kid), FadeOut],
            },
            Scene::Level(9) => Script {
                which_imgs: 1,
                steps: vec![Sound(35), SetHourglass(hf), Spawn(Princess, PrincessStand), FadeIn, Spawn(Kid, MouseGo), Frames(5), Seq(Princess, 112), Frames(9), Seq(Kid, 114), Frames(58), FadeOut],
            },
            Scene::Level(12) if hf >= 6 => Script { which_imgs: 1, steps: vec![SetHourglass(hf), Spawn(Princess, PrincessX156), Sound(40), FadeIn, Frames(2), Seq(Princess, 98), Frames(24), FadeOut] },
            Scene::Level(12) => c2_6(1),
            Scene::Level(_) => return None,
            Scene::TimeOut => Script { which_imgs: 1, steps: vec![DisableKeys, SetHourglass(7), Sandflow(-1), Sound(36), FadeIn, Frames(2), Frames(100), FadeOut, WaitSoundEnd] },
            Scene::Ending => Script {
                which_imgs: 1,
                steps: vec![
                    DisableKeys, Sound(26), Spawn(Princess, EndingPrincess), Spawn(Kid, EndingKid), Frames(8), Seq(Princess, 108), Frames(5), Seq(Kid, 13), Frames(2), Hide(Kid), Frames(39),
                    Spawn(Kid, MouseEnding), Frames(9), Seq(Kid, 101), Frames(41), FadeOut, WaitSoundEnd,
                ],
            },
            Scene::Intro => Script {
                which_imgs: 0,
                steps: vec![
                    Spawn(Princess, InitPrincess), FadeIn, Spawn(Kid, Vizier), Frames(2), Sound(50), WaitSound, FrameTime(8), Frames(5), Sound(4), WaitSound, Sound(51), Frames(3),
                    Seq(Princess, 98), Frames(5), Seq(Kid, 96), Frames(6), Sound(53), Seq(Kid, 97), Frames(4), Frames(18), Seq(Kid, 96), Frames(30), Seq(Kid, 97), Frames(35),
                    Seq(Kid, 102), FrameTime(7), Frames(1), Seq(Princess, 99), Frames(17), HourglassState(1), Flash { frames: 5, colour: 15 }, WaitSound, Seq(Kid, 100),
                    Sandflow(0), Frames(6), Sound(52), Frames(24), HourglassState(2), Frames(9), Seq(Princess, 113), Frames(28), FadeOut,
                ],
            },
        })
    }
}

/// A character record driven by its sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    /// 5 princess, 6 vizier, 24 mouse, 0 kid
    pub charid: u8,
    pub x: u8,
    pub y: u8,
    pub facing_left: bool,
    pub frame: u8,
    cur: Vec<Op>,
    pc: usize,
}

/// Which file an actor's image comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteFile {
    /// PV.DAT; the palette is the set's base resource (800, 850, 900)
    Pv,
    /// KID.DAT, palette 400
    Kid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Done,
    /// a key: the rest of the scene is skipped (black, sounds stopped)
    Skipped,
}

pub struct Cutscene {
    data: Arc<Data>,
    cut_frames: Vec<Frame>,
    prio: Vec<u8>,
    pub which_imgs: u8,
    steps: Vec<Step>,
    pc: usize,
    /// frames left in the current Frames / fade step (0 = not started)
    left: u16,
    pub kid: Option<Actor>,
    pub princess: Option<Actor>,
    /// 60 Hz units per frame (6, 7, 8)
    pub frame_time: u8,
    pub hourglass_state: u8,
    pub sandflow: i8,
    pub torch: [u8; 2],
    which_torch: usize,
    /// (x, y, colour index) per star (ds:0DD6); colour = STAR_COLOURS[idx]
    pub stars: [(u16, u16, u8); 6],
    /// (frames left, EGA colour); the flash shows on the frame it is counted down
    flash: (u8, u8),
    pub flash_now: Option<u8>,
    pub disable_keys: bool,
    queued: Option<u8>,
    /// sounds started by the last frame (play them now)
    pub started: Vec<u8>,
    /// 0 = black .. 1 = full brightness
    pub fade: f32,
    /// true while the screen is black waiting for the music (WaitSoundEnd)
    pub blank: bool,
    rng: Rng,
    /// frames played so far (debug / tests)
    pub frames: u32,
}

/// ds:0DFA
pub const STAR_COLOURS: [u8; 5] = [8, 7, 15, 15, 7];

impl Cutscene {
    /// load_intro (0000:322D) + reset_cutscene (2E22): stars drawn once (each advances its colour), empty slots.
    pub fn new(data: Arc<Data>, script: Script) -> Cutscene {
        let cut_frames = data.exe.cutscene_frames();
        let prio = data.exe.sound_priorities();
        let fade = if script.steps.contains(&Step::FadeIn) { 0.0 } else { 1.0 };
        let mut stars = [(20, 97, 0), (16, 104, 1), (23, 110, 2), (17, 116, 3), (24, 120, 4), (18, 128, 0)];
        for s in &mut stars {
            s.2 = (s.2 + 1) % 5;
        }
        Cutscene {
            data, cut_frames, prio, which_imgs: script.which_imgs, steps: script.steps, pc: 0, left: 0, kid: None, princess: None, frame_time: 6,
            hourglass_state: 0, sandflow: -1, torch: [1, 6], which_torch: 0, stars, flash: (0, 0), flash_now: None, disable_keys: false, queued: None,
            started: vec![], fade, blank: false, rng: Rng(0x5EED), frames: 0,
        }
    }

    /// The frame record of an actor (charid 5/6: cutscene table, else the kid table).
    pub fn frame_rec(&self, a: &Actor) -> Frame {
        let t = if matches!(a.charid, 5 | 6) { &self.cut_frames } else { &self.data.frames };
        t.get(a.frame as usize).copied().unwrap_or(Frame { image: 255, ..Frame::default() })
    }

    /// Image of an actor: chtab = 2 + (sword >> 6); 3 = PV 800 set, 4 = PV 850 + 50 * which_imgs, else KID.DAT.
    /// Returns (file, resource id, palette resource id).
    pub fn sprite(&self, a: &Actor) -> Option<(SpriteFile, u16, u16)> {
        let f = self.frame_rec(a);
        if f.image == 255 {
            return None;
        }
        Some(match f.sword >> 6 {
            1 => (SpriteFile::Pv, 801 + f.image as u16, 800),
            2 => {
                let base = 850 + 50 * self.which_imgs as u16;
                (SpriteFile::Pv, base + 1 + f.image as u16, base)
            }
            _ => (SpriteFile::Kid, 401 + f.image as u16, 400),
        })
    }

    /// draw_hourglass: (glass PV resource, sand stream PV resource) as drawn on the last frame.
    pub fn hourglass_images(&self) -> (Option<u16>, Option<u16>) {
        if self.sandflow >= 0 && self.hourglass_state >= 7 {
            return (None, None);
        }
        let stream = (self.sandflow >= 0).then(|| 960 + self.sandflow as u16);
        let glass = (self.hourglass_state != 0).then(|| 952 + self.hourglass_state as u16);
        (glass, stream)
    }

    fn slot(&mut self, s: Slot) -> &mut Option<Actor> {
        match s {
            Slot::Kid => &mut self.kid,
            Slot::Princess => &mut self.princess,
        }
    }

    fn set_seq(&self, a: &mut Actor, seq: usize) {
        a.cur = self.data.exe.sequence(seq).unwrap_or_default();
        a.pc = 0;
    }

    /// play_seq subset (0000:6F90): ops up to the next frame.
    fn play_seq(&mut self, s: Slot) {
        let Some(mut a) = self.slot(s).take() else { return };
        for _ in 0..256 {
            let Some(&op) = a.cur.get(a.pc) else { break };
            a.pc += 1;
            match op {
                Op::Frame(n) => {
                    a.frame = n;
                    break;
                }
                Op::Dx(d) => a.x = (a.x as i32 + if a.facing_left { -(d as i32) } else { d as i32 }) as u8,
                Op::Dy(d) => a.y = a.y.wrapping_add(d as u8),
                Op::Flip => a.facing_left = !a.facing_left,
                Op::Jmp(t) => {
                    a.cur = self.data.exe.ops_from(t).unwrap_or_default();
                    a.pc = 0;
                }
                // F2 1 footstep, 2 bump, 3 drink (the kid's run sequence in the ending)
                Op::Sound(n) => {
                    if let Some(id) = match n { 1 => Some(23), 2 => Some(8), 3 => Some(18), _ => None } {
                        self.play_sound(id);
                    }
                }
                _ => {}
            }
        }
        *self.slot(s) = Some(a);
    }

    /// play_sound (0000:12F0): queue, unless the queued one has a higher priority (lower ds:2528 value).
    fn play_sound(&mut self, id: u8) {
        let p = |i: u8| self.prio.get(i as usize).copied().unwrap_or(255);
        if self.queued.is_none_or(|q| p(id) <= p(q)) {
            self.queued = Some(id);
        }
    }

    /// Instant steps up to the next frame-consuming one. Returns true at the end of the script.
    fn run_instant(&mut self) -> bool {
        while let Some(&st) = self.steps.get(self.pc) {
            match st {
                Step::Spawn(s, setup) => {
                    let (charid, x, y, facing_left, seq) = setup.params();
                    let mut a = Actor { charid, x, y, facing_left, frame: 0, cur: vec![], pc: 0 };
                    self.set_seq(&mut a, seq);
                    *self.slot(s) = Some(a);
                    self.play_seq(s);
                }
                Step::Seq(s, seq) => {
                    if let Some(mut a) = self.slot(s).take() {
                        self.set_seq(&mut a, seq as usize);
                        *self.slot(s) = Some(a);
                    }
                }
                Step::Hide(s) => *self.slot(s) = None,
                Step::Sound(id) => self.play_sound(id),
                Step::FrameTime(t) => self.frame_time = t,
                Step::SetHourglass(v) => (self.sandflow, self.hourglass_state) = (0, v),
                Step::HourglassState(v) => self.hourglass_state = v,
                Step::Sandflow(v) => self.sandflow = v,
                Step::Flash { frames, colour } => self.flash = (frames, colour),
                Step::DisableKeys => self.disable_keys = true,
                Step::Frames(_) | Step::WaitSound | Step::FadeIn | Step::FadeOut | Step::WaitSoundEnd => return false,
            }
            self.pc += 1;
        }
        true
    }

    /// One cutscene frame (`frame_time` units). `key` = a key or Shift was pressed, `sound_busy` = a sound is playing now.
    /// Returns the end of the scene; after `Some` the cutscene is over.
    pub fn frame(&mut self, key: bool, sound_busy: bool) -> Option<End> {
        self.started.clear();
        self.flash_now = None;
        loop {
            if self.run_instant() {
                return Some(End::Done);
            }
            if self.steps[self.pc] != Step::WaitSoundEnd {
                break;
            }
            // busy wait after the fade-out: nothing drawn, keys ignored (only the time out and the ending use it)
            (self.blank, self.fade) = (true, 0.0);
            if sound_busy {
                return None;
            }
            self.pc += 1;
        }
        let st = self.steps[self.pc];
        if self.left == 0 {
            self.left = match st {
                Step::Frames(n) => n.max(1),
                Step::FadeIn | Step::FadeOut => FADE_FRAMES,
                _ => 1,
            };
        }
        // proc_cutscene_frame body
        self.frames += 1;
        self.play_seq(Slot::Kid);
        self.play_seq(Slot::Princess);
        if self.flash.0 > 0 {
            self.flash_now = Some(self.flash.1);
            self.flash.0 -= 1;
        }
        for _ in 0..2 {
            self.which_torch ^= 1;
            let r = self.rng.next(255);
            self.torch[self.which_torch] = torch_frame(self.torch[self.which_torch], r);
        }
        if self.sandflow >= 0 {
            self.sandflow = (self.sandflow + 1) % 3;
        }
        if self.left % 2 == 1 {
            let i = self.rng.next(5) as usize;
            if let Some(s) = self.stars.get_mut(i) {
                s.2 = (s.2 + 1) % 5; // shortcut: prandom(5) can give 5 (no star), the original reads past the table there
            }
        }
        if !sound_busy {
            if let Some(id) = self.queued.take() {
                self.started.push(id);
            }
        }
        if key && !self.disable_keys {
            self.queued = None;
            return Some(End::Skipped);
        }
        self.left -= 1;
        match st {
            Step::FadeIn => self.fade = 1.0 - self.left as f32 / FADE_FRAMES as f32,
            Step::FadeOut => self.fade = self.left as f32 / FADE_FRAMES as f32,
            _ => {}
        }
        if st == Step::WaitSound && (sound_busy || !self.started.is_empty()) {
            self.left = 1; // stays on this step while a sound plays
        }
        if self.left == 0 {
            self.pc += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hourglass_and_torch_vectors() {
        let h: Vec<u8> = [60, 33, 32, 17, 16, 6, 5, 1].iter().map(|&m| hourglass_frame(m)).collect();
        assert_eq!(h, [3, 3, 4, 4, 5, 5, 6, 6]);
        let t: Vec<u8> = [(1, 5), (1, 1), (8, 200), (3, 9), (8, 8)].iter().map(|&(p, r)| torch_frame(p, r)).collect();
        assert_eq!(t, [5, 2, 0, 4, 0]);
    }

    #[test]
    fn level_12_branch_and_scene_table() {
        let first_sound = |m| Scene::Level(12).script(m).unwrap().steps.iter().find_map(|s| if let Step::Sound(i) = s { Some(*i) } else { None });
        assert_eq!((first_sound(5), first_sound(6)), (Some(40), Some(27)));
        for l in 0..16 {
            assert_eq!(Scene::Level(l).script(60).is_some(), [2, 4, 6, 8, 9, 12].contains(&l), "level {l}");
        }
        assert_eq!(Scene::Level(2).script(60).unwrap().which_imgs, 0);
        assert_eq!(Scene::Level(6).script(60).unwrap().which_imgs, 1);
    }

    fn data() -> Option<Arc<Data>> {
        if crate::no_game_data() {
            return None;
        }
        let d = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let exe = crate::exe::Exe::open(format!("{d}PRINCE.EXE")).unwrap();
        let kd = crate::dat::Dat::open(format!("{d}KID.DAT")).unwrap();
        Some(Arc::new(Data { frames: exe.kid_frames(), gframes: exe.guard_frames(), sizes: crate::image::image_sizes(&kd, 401, 220), gsizes: vec![], exe }))
    }

    /// (frame, x, facing left) of one actor
    type A = Option<(u8, u8, bool)>;

    /// Speaker lengths (docs/audio.md) in 60 Hz units for the sounds the scenes use.
    fn sound_units(id: u8) -> u32 {
        let secs = match id { 26 => 11.69, 27 => 10.73, 35 => 9.75, 36 => 12.72, 40 => 5.18, 50 => 9.50, 51 => 0.54, 52 => 27.98, 53 => 14.85, 4 => 0.01, _ => 0.01 };
        (secs * 60.0f64).ceil() as u32
    }

    /// Plays a scene to the end with a one-channel sound model; returns per frame (kid, princess), the started sounds
    /// with their frame, and the end.
    fn run(c: &mut Cutscene, key_at: Option<u32>) -> (Vec<(A, A)>, Vec<(u32, u8)>, End) {
        let (mut out, mut snd, mut busy) = (vec![], vec![], 0u32);
        let a = |a: &Option<Actor>| a.as_ref().map(|a| (a.frame, a.x, a.facing_left));
        for i in 0..5000 {
            let r = c.frame(key_at == Some(i), busy > 0);
            for &id in &c.started {
                snd.push((c.frames, id));
                busy = sound_units(id);
            }
            if let Some(e) = r {
                return (out, snd, e);
            }
            if !c.blank {
                out.push((a(&c.kid), a(&c.princess)));
            }
            busy = busy.saturating_sub(c.frame_time as u32);
        }
        panic!("scene did not end");
    }

    #[test]
    fn star_colours_after_load_intro() {
        let Some(d) = data() else { return };
        let c = Cutscene::new(d, Scene::Level(2).script(60).unwrap());
        let cols: Vec<u8> = c.stars.iter().map(|s| STAR_COLOURS[s.2 as usize]).collect();
        assert_eq!(cols, [7, 15, 15, 7, 8, 7]);
    }

    #[test]
    fn hourglass_draw_vector() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Script { which_imgs: 1, steps: vec![Step::SetHourglass(3), Step::Frames(1)] });
        assert_eq!(c.frame(false, false), None);
        assert_eq!(c.sandflow, 1);
        assert_eq!(c.hourglass_images(), (Some(955), Some(961)));
        assert_eq!(c.frame(false, false), Some(End::Done));
    }

    #[test]
    fn cutscene_8_mouse_and_princess() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Level(8).script(60).unwrap());
        let (f, snd, end) = run(&mut c, None);
        assert_eq!(end, End::Done);
        assert_eq!(snd, [(1, 35)]);
        let f = &f[FADE_FRAMES as usize..]; // f[i - 1] = frame i after the fade-in
        assert_eq!(f.len(), 60 + FADE_FRAMES as usize);
        for i in 1..=20 {
            assert_eq!(f[i - 1], (Some((186, 144, true)), Some((37, 131, true))), "frame {i}");
        }
        assert!((24..=31).all(|i| f[i - 1].0.unwrap().0 == 188));
        assert_eq!(f[31].0, Some((186, 152, false)), "frame 32: mouse flips");
        let xs: Vec<u8> = (33..=40).map(|i| f[i - 1].0.unwrap().1).collect();
        assert_eq!(xs, [157, 160, 164, 169, 172, 176, 181, 184]);
        assert_eq!(f[39].0.unwrap().0, 187);
        assert_eq!(f[40].0.unwrap().1, 188);
        assert_eq!(f[50].0.unwrap().1, 229);
        let pf: Vec<u8> = (41..=51).map(|i| f[i - 1].1.unwrap().0).collect();
        assert_eq!(pf, (37..=47).collect::<Vec<_>>());
        assert_eq!(f[51].1, Some((11, 143, false)), "frame 52: princess flips");
        assert_eq!(f[51].0.unwrap().1, 232);
        assert_eq!(f[59].0.unwrap().1, (265u16 - 256) as u8, "x is a byte: 265 wraps (off screen either way)");
        assert!(f[60..].iter().all(|x| x.0.is_none()), "mouse removed for the fade-out");
    }

    #[test]
    fn cutscene_9_mouse_enters() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Level(9).script(60).unwrap());
        let (f, _, _) = run(&mut c, None);
        assert!(f[..FADE_FRAMES as usize].iter().all(|x| x.0.is_none() && x.1 == Some((11, 144, false))));
        let f = &f[FADE_FRAMES as usize..];
        let xs: Vec<u8> = (1..=14).map(|i| f[i - 1].0.unwrap().1).collect();
        assert_eq!(xs, [194, 191, 187, 182, 179, 175, 170, 167, 163, 158, 155, 151, 146, 143]);
        assert_eq!(f[7].1, Some((47, 131, true)), "frame 8: princess flips");
        let pf: Vec<u8> = (9..=18).map(|i| f[i - 1].1.unwrap().0).collect();
        assert_eq!(pf, [46, 45, 44, 43, 42, 41, 40, 39, 38, 37]);
        assert!((15..=72).all(|i| f[i - 1].0 == Some((188, 143, true))));
        let stroke: Vec<u8> = (19..=66).map(|i| f[i - 1].1.unwrap().0).collect();
        let mut want = vec![36, 36, 36, 35, 35, 35];
        want.extend([34; 7]);
        want.extend([35, 35]);
        want.extend(want.clone());
        want.extend([36, 36, 36, 35, 35, 35]);
        want.extend([34; 9]);
        want.extend([35, 35, 35]);
        assert_eq!(stroke, want);
        assert!((67..=72).all(|i| f[i - 1].1.unwrap().0 == 36));
        assert_eq!(f.len(), 72 + FADE_FRAMES as usize);
    }

    #[test]
    fn cutscene_12_short_turns() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Level(12).script(5).unwrap());
        let (f, snd, _) = run(&mut c, None);
        assert_eq!(snd, [(1, 40)]);
        let f = &f[FADE_FRAMES as usize..];
        assert_eq!(f[0].1, Some((11, 156, true)));
        let pf: Vec<u8> = (3..=10).map(|i| f[i - 1].1.unwrap().0).collect();
        assert_eq!(pf, (2..=9).collect::<Vec<_>>());
        assert!((11..=26).all(|i| f[i - 1].1 == Some((11, 164, false))));
        assert_eq!(c.hourglass_state, 6);
    }

    #[test]
    fn ending_hug_and_mouse() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Ending.script(60).unwrap());
        assert_eq!(c.fade, 1.0, "no fade-in");
        let (f, snd, end) = run(&mut c, Some(3));
        assert_eq!(end, End::Done, "keys disabled");
        // the kid's footsteps (23) stay queued behind the music and start only once it has ended (during the fade-out here,
        // since the estimated fade makes the scene longer than the 11.69 s tune)
        assert_eq!(snd[0], (1, 26));
        assert!(snd[1..].iter().all(|&(f, id)| id == 23 && (f - 1) * 6 >= sound_units(26)), "{snd:?}");
        let kx: Vec<(u8, u8)> = (1..=15).map(|i| f[i - 1].0.map(|k| (k.0, k.1)).unwrap()).collect();
        assert_eq!(kx, [(2, 198), (3, 198), (4, 198), (5, 190), (6, 187), (7, 184), (8, 179), (9, 178), (10, 176), (11, 172), (12, 167), (13, 165), (14, 162), (53, 162), (54, 160)]);
        let px: Vec<(u8, u8)> = (1..=21).map(|i| f[i - 1].1.map(|p| (p.0, p.1)).unwrap()).collect();
        assert_eq!(&px[8..], [(21, 136), (22, 135), (23, 135), (24, 135), (25, 134), (26, 137), (27, 139), (28, 143), (29, 146), (30, 148), (31, 151), (32, 152), (33, 152)]);
        assert!((16..=54).all(|i| f[i - 1].0.is_none()));
        let mx: Vec<u8> = (55..=63).map(|i| f[i - 1].0.unwrap().1).collect();
        assert_eq!((mx[0], mx[8]), (192, 161));
        let mf: Vec<u8> = (64..=66).map(|i| f[i - 1].0.unwrap().0).collect();
        assert_eq!(mf, [186, 186, 188]);
        assert_eq!(f.len(), 104 + FADE_FRAMES as usize);
        assert!(c.blank, "waited for the music on a black screen");
    }

    #[test]
    fn time_out_empty_room_not_skippable() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::TimeOut.script(0).unwrap());
        let (f, snd, end) = run(&mut c, Some(30));
        assert_eq!((end, snd), (End::Done, vec![(1, 36)]));
        assert_eq!(f.len(), 102 + 2 * FADE_FRAMES as usize);
        assert!(f.iter().all(|x| *x == (None, None)));
        assert_eq!(c.hourglass_images(), (Some(959), None), "empty glass, no stream");
    }

    #[test]
    fn key_skips_level_scene_at_once() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Level(2).script(60).unwrap());
        let (f, _, end) = run(&mut c, Some(25));
        assert_eq!((end, f.len()), (End::Skipped, 25));
        let mut c = Cutscene::new(data().unwrap(), Scene::Level(2).script(60).unwrap());
        let (f, _, _) = run(&mut c, None);
        assert_eq!(f.len(), 26 + 2 * FADE_FRAMES as usize);
        assert!(f.iter().all(|x| x.1 == Some((11, 120, false))));
        assert_eq!(c.sprite(c.princess.as_ref().unwrap()), Some((SpriteFile::Pv, 801, 800)));
    }

    #[test]
    fn every_scene_ends_and_its_images_exist() {
        let Some(d) = data() else { return };
        let pv = crate::dat::Dat::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/PV.DAT")).unwrap();
        let kid = crate::dat::Dat::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/KID.DAT")).unwrap();
        let scenes = [Scene::Level(2), Scene::Level(4), Scene::Level(6), Scene::Level(8), Scene::Level(9), Scene::Level(12), Scene::TimeOut, Scene::Ending, Scene::Intro];
        for s in scenes {
            for m in [60, 3] {
                let mut c = Cutscene::new(d.clone(), s.script(m).unwrap());
                let mut busy = 0u32;
                for _ in 0..3000 {
                    let r = c.frame(false, busy > 0);
                    for &id in &c.started {
                        busy = sound_units(id);
                    }
                    if r.is_some() {
                        break;
                    }
                    for a in [&c.kid, &c.princess].into_iter().flatten() {
                        if let Some((f, id, _)) = c.sprite(a) {
                            let file = if f == SpriteFile::Pv { &pv } else { &kid };
                            assert!(file.get(id).is_some(), "{s:?}: frame {} -> resource {id}", a.frame);
                        }
                    }
                    busy = busy.saturating_sub(c.frame_time as u32);
                }
                assert!(c.frame(false, false).is_some(), "{s:?} ended");
            }
        }
    }

    /// pv_scene with speaker lengths: Jaffar walks in to x 136, the princess steps back to x 111, the hourglass appears with
    /// a white flash and ends with state 2; sounds start in order.
    #[test]
    fn intro_scene() {
        let Some(d) = data() else { return };
        let mut c = Cutscene::new(d, Scene::Intro.script(60).unwrap());
        let (mut flashes, mut busy, mut stops, mut snd) = (0, 0u32, vec![], vec![]);
        let mut last = None;
        loop {
            let r = c.frame(false, busy > 0);
            for &id in &c.started {
                snd.push(id);
                busy = sound_units(id);
            }
            if r.is_some() {
                break;
            }
            flashes += c.flash_now.is_some() as u32;
            if let Some(k) = c.kid.as_ref().filter(|k| k.frame == 54 && stops.last() != Some(&k.x)) {
                stops.push(k.x); // Jaffar standing (frame 54)
            }
            if let Some(p) = &c.princess {
                if p.frame == 11 || p.facing_left {
                    last = Some((p.x, p.facing_left));
                }
            }
            busy = busy.saturating_sub(c.frame_time as u32);
        }
        assert_eq!(snd, [50, 4, 51, 53, 52]);
        assert_eq!(flashes, 5);
        assert_eq!(&stops[..3], [198, 187, 136], "enters at 198, stops at 187 and 136");
        assert_eq!(last, Some((111, true)), "the princess stepped back to x 111");
        assert_eq!(c.hourglass_state, 2);
    }
}
