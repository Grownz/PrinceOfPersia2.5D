//! Deterministic game simulation. The original game runs its logic at 12 ticks per second.
//! We step at 60 Hz (`SUBSTEPS` = 5 steps per logic tick): the original logic runs unchanged on every
//! 5th step, the steps in between only advance `alpha()` so rendering can interpolate and input can
//! be sampled at 60 Hz. Mechanics therefore stay identical to the original.
//!
//! One logic tick (docs/tiles.md 2.1): falling loose floors (mobs), then all moving tiles (trobs), then the
//! kid. What the kid triggers in tick N first moves in tick N+1. Rules and addresses: docs/tiles.md.
//! Torches, potion bubbles and the sword glint animate as trobs of the drawn room and draw from the shared prandom stream in
//! the original order (docs/tiles.md 8.2-8.4, 10); guards, shadow, mirror and the rest live in `combat`.
use crate::exe::{Exe, Frame};
use crate::kid::{Env, Event, Input, Keys, Kid, Rng, Shared, Y_LAND};
use crate::level::{Level, Tile, COLS, ROWS, TILES_PER_ROOM};
use std::sync::Arc;

mod combat;
pub use combat::{GuardSlot, Hit,ShadowState, GUARD_TYPE, mk_env};

pub const TICKS_PER_SEC: u32 = 12;
/// Logic ticks from the pick-up of a potion (frame 191) until the kid stands again (frame 15); checked in world_tests.
pub const DRINK_TICKS: usize = 21;
/// 60 Hz steps per logic frame normally; `SWORD_SUBSTEPS` while the kid's sword is drawn (0000:4D6A, docs/edge_cases.md 5.2)
pub const SUBSTEPS: u32 = 5;
pub const SWORD_SUBSTEPS: u32 = 6;
pub const STEP_HZ: f64 = (TICKS_PER_SEC * SUBSTEPS) as f64;
/// Feather fall length. The original ends it in timers (0000:4FD0) on the first frame with no sound playing, i.e. when sound
/// 39 (PC speaker, 18.61 s, docs/audio.md) ends. Set during frame T0's play_frame, the timers of frame T0+k run k * 5/60 s
/// later: the tune still plays at k = 223 (18.58 s) and has ended at k = 224 (18.67 s). The port decrements before use, so
/// feather > 0 on ticks T0+1..T0+223 needs 224. INFERRED: a sound that interrupts the tune (one channel) or sound off ends it
/// earlier in the original; the speaker length is assumed (AdLib/MIDI length UNVERIFIED).
pub const FEATHER_TICKS: u8 = 224;
/// Ctrl+G saves only on levels 3..13 (0000:05EA: level > 2 and < 14); the loader accepts the same range.
pub const SAVE_LEVELS: std::ops::RangeInclusive<usize> = 3..=13;

/// prandom seed of a new World. The original seeds from the clock on its first prandom call (0C83:0418 -> 0C83:B416), so
/// any value is a possible original run. The demo's fight depends on it (12 of 60 nearby seeds let the demo kid win in
/// the port; whether the original demo can lose is UNVERIFIED): this one plays the demo to its end in room 24.
pub(crate) const SEED: u32 = 0x1234_567A;
const MAX_TROBS: usize = 30;
const MAX_MOBS: usize = 14;
/// y_loose_land (ds:2298) and y_something (ds:228E), indexed by row + 1
const Y_LOOSE_LAND: [u8; 5] = [2, 65, 128, 191, 254];
const Y_SOMETHING: [i32; 5] = [-1, 62, 125, 188, 251];
const DOOR_DELTA: [i8; 3] = [-1, 4, 4];
const GATE_CLOSE: [u8; 9] = [0, 0, 0, 20, 40, 60, 80, 100, 120];
const LEVELDOOR_CLOSE: [u8; 8] = [0, 0, 0, 0, 5, 17, 99, 0];
const LOOSE_SOUND: [u8; 12] = [0, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0];

/// Static game data shared by all worlds.
pub struct Data {
    pub exe: Exe,
    pub frames: Vec<Frame>,
    /// (height, width) of kid image i (KID.DAT resource 401 + i)
    pub sizes: Vec<(u16, u16)>,
    /// guard frame table (frames 150..190)
    pub gframes: Vec<Frame>,
    /// image sizes of the guard-type files, indexed by guard type (GUARD, FAT, SKEL, VIZIER, SHADOW)
    pub gsizes: Vec<Vec<(u16, u16)>>,
}

/// A moving tile ("trob"): `ty` < 0 = finished. Meaning of `ty` depends on the tile type (docs/tiles.md 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trob {
    pub tilepos: u8,
    pub room: u8,
    pub ty: i8,
}

/// A falling loose floor ("mob"). `xh` = column * 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mob {
    pub xh: u8,
    pub y: u8,
    pub room: u8,
    pub speed: i8,
    pub ty: u8,
    pub row: u8,
}

/// What carries over between levels and restarts (docs/edge_cases.md 3.4). A restart or the next level builds a new
/// `World` from this (the original reloads the level completely).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub level: usize,
    /// time left (ds:50BA / ds:50C0): 720 ticks per minute
    pub rem_min: i32,
    pub rem_tick: i32,
    pub hitp_beg_lev: i32,
    /// level-3 checkpoint (ds:4CAC); on level 0 it marks the demo guard as killed
    pub checkpoint: bool,
    /// level 12 -> 13 is seamless: the kid keeps his hit points (0000:4A8B)
    pub carry_hp: Option<i32>,
}

impl Progress {
    /// New game (0000:4893): 3 hit points, 60 minutes.
    pub fn new_game(level: usize) -> Progress {
        Progress { level, rem_min: 60, rem_tick: 719, hitp_beg_lev: 3, checkpoint: false, carry_hp: None }
    }

    /// Save file text. The original (Ctrl+G, 0000:1D78, levels 3..13) writes rem_min, rem_tick, level and hitp_beg_lev as
    /// four words to PRINCE.SAV; this one is JSON with the same values (`hitp_max` is informational, a level always
    /// starts with hitp_max = hitp_beg_lev).
    pub fn save_text(&self, hitp_max: i32) -> String {
        format!(
            "{{\"level\": {}, \"hitp_max\": {}, \"rem_min\": {}, \"rem_tick\": {}, \"hitp_beg_lev\": {}}}
",
            self.level, hitp_max, self.rem_min, self.rem_tick, self.hitp_beg_lev
        )
    }

    /// Parse `save_text` (a flat object of integer values); `None` if the shape is wrong, a key is duplicated or missing,
    /// or a value is out of range (the file is user-editable). Unknown keys are ignored (forward compatible).
    pub fn from_save(text: &str) -> Option<Progress> {
        let body = text.trim().strip_prefix('{')?.strip_suffix('}')?;
        let mut v = [None; 4];
        for pair in body.split(',') {
            let (key, num) = pair.split_once(':')?;
            let key = key.trim().strip_prefix('"')?.strip_suffix('"')?;
            let num: i32 = num.trim().parse().ok()?;
            let i = match key {
                "level" => 0,
                "rem_min" => 1,
                "rem_tick" => 2,
                "hitp_beg_lev" => 3,
                _ => continue, // hitp_max is informational; unknown keys come from newer versions
            };
            if v[i].replace(num).is_some() {
                return None;
            }
        }
        let [Some(level), Some(rem_min), Some(rem_tick), Some(hp)] = v else { return None };
        let ok = SAVE_LEVELS.contains(&(level.max(0) as usize)) && (1..=60).contains(&rem_min) && (1..=719).contains(&rem_tick) && (1..=10).contains(&hp);
        ok.then_some(Progress { level: level as usize, rem_min, rem_tick, hitp_beg_lev: hp, checkpoint: false, carry_hp: None })
    }
}

/// Why the world wants to be replaced; the app builds the next `World` with `World::progress_after`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// next_level++ (exit stairs, level 6 fall, level 12 room 23 = seamless)
    NextLevel { seamless: bool },
    /// restart the same level (Shift/Enter after death, Ctrl+A)
    Restart,
    /// back to the title: demo over, "Press Button" timed out, time out (the app plays the time-out cutscene first)
    Title,
    /// a key during the demo starts the game
    NewGame,
    /// level 14, the kid entered room 5 (check_the_end 0000:100A): the ending, then the title
    End,
}

/// Demo move list (ds:0F36): (time, move) for do_auto_moves.
pub const DEMO_MOVES: [(i16, i16); 25] = [
    (0, 0), (1, 1), (13, 0), (30, 1), (37, 5), (47, 0), (48, 1), (65, 0), (73, 2), (75, 0), (99, 2), (100, 0), (115, 5),
    (128, 6), (136, 3), (157, 7), (158, 0), (159, 1), (171, 4), (177, 0), (178, 1), (188, 0), (193, 1), (205, 0), (233, -1),
];

/// What happened on a tile (`World::impacts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactKind {
    /// a loose floor let go (the tile became empty)
    LooseFall,
    /// a falling loose floor hit the floor below
    Crash,
    /// a gate finished its fast close
    GateSlam,
    /// a chomper closed
    Chomp,
}

/// A tile impact in `room` at tile index `tilepos` (row * 10 + column).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Impact {
    pub kind: ImpactKind,
    pub room: u8,
    pub tilepos: u8,
}

pub struct World {
    /// original logic ticks (12 Hz)
    pub tick: u64,
    /// carried over to the next world; time and the checkpoint are updated while playing
    pub progress: Progress,
    /// set once when the world is over (see `Exit`); later exits are ignored
    pub exit: Option<Exit>,
    /// Kid.alive after death (0..8, play_kid 0000:792A); 0 while alive
    pub death_count: u8,
    /// app hook: while true the death counter waits (the original waits for the death sounds; pop-core treats them as finished)
    pub hold_death_counter: bool,
    /// "Press Button to Continue" timer (ds:60C8): 288 (36 on level 0) ticks, then back to the title
    pub text_timer: u16,
    /// level 0: the kid is driven by DEMO_MOVES / the guard AI; the control flags persist between ticks
    pub demo: bool,
    demo_ctrl: crate::kid::Ctrl,
    /// jumped_through_mirror (ds:42E0), handled after the tick
    mirror_jump: bool,
    sub: u32,
    /// 60 Hz steps of the current logic frame (5, or 6 with the kid's sword drawn)
    frame_len: u32,
    /// screen flash (colour index, frames left), docs/edge_cases.md 3.6: shown while frames > 0
    pub flash: (u8, u8),
    /// upside-down display (ds:4FD0, invert potion); display only
    pub upside: bool,
    pub data: Arc<Data>,
    /// runtime tile state (the level as loaded, modified by the mechanics)
    pub level: Level,
    pub level_no: usize,
    pub kid: Kid,
    pub trobs: Vec<Trob>,
    pub mobs: Vec<Mob>,
    /// sound ids triggered since the consumer last cleared this list (docs/tiles.md section 9)
    pub sounds: Vec<u8>,
    /// sword hits since the consumer last cleared this list (renderers: blood)
    pub hits: Vec<Hit>,
    /// tile impacts (loose floor falls / crashes, gate slams, chompers) since the consumer last cleared this list (renderers: dust puffs); display only
    pub impacts: Vec<Impact>,
    /// 0 = exit door closed, 1 = open (set when the door finished opening)
    pub leveldoor_open: u8,
    /// bumped whenever tiles, trobs or mobs changed; renderers use it to know when to rebuild
    pub version: u32,
    dirty: bool,
    /// drawn_room (ds:40DE) as check_the_end keeps it: the room whose torch/potion/sword trobs animate
    drawn_room: u8,
    rng: Rng,
    last_loose_sound: u8,
    /// the one active guard (or skeleton, shadow); `None` = no guard (Guard.direction == 0x56)
    pub guard: Option<Kid>,
    pub slots: [GuardSlot; 24],
    /// guard type of this level (tbl_guard_type), -1 = none
    pub guard_type: i8,
    pub shared: Shared,
    pub shadow: ShadowState,
    /// the Guard record as last saved (saveshad); it outlives clear_char and is the kid's Opp for check_killed_shadow
    guard_rec: crate::kid::Opp,
}

impl World {
    /// A new game started on `level_no`.
    pub fn new(data: Arc<Data>, level: Level, level_no: usize) -> World {
        World::from_progress(data, level, Progress::new_game(level_no))
    }

    /// play_level (0000:48C0) + do_startpos: the level as loaded, the kid from `progress` (docs/edge_cases.md 3.4).
    pub fn from_progress(data: Arc<Data>, level: Level, progress: Progress) -> World {
        World::seeded(data, level, progress, SEED)
    }

    fn seeded(data: Arc<Data>, level: Level, progress: Progress, seed: u32) -> World {
        let level_no = progress.level;
        let guard_type = GUARD_TYPE[level_no.min(15)];
        let kid = {
            let mut start = level.clone();
            if level_no == 3 && progress.checkpoint {
                // 0000:4A24: dir 0xFF, then NOT (4A86) -> 0 (right) and seq 5's flip: the kid ends facing left (VERIFIED)
                (start.start_room, start.start_tile, start.start_dir) = (2, 6, 0xFF);
            }
            let env = mk_env(&data, &start, guard_type);
            let mut k = Kid::spawn(&env, level_no);
            k.have_sword = level_no != 1; // 0000:49A6: the sword is picked up on level 1 (the demo level 0 has it)
            k.hp_max = if level_no == 0 { 4 } else { progress.hitp_beg_lev }; // 0000:4AA9
            k.hp = progress.carry_hp.unwrap_or(k.hp_max);
            k
        };
        let mut level = level;
        if level_no == 3 && progress.checkpoint {
            level.set_tile(7, 4, Tile::Empty); // do_startpos clears tile (room 7, col 4, row 0)
        }
        let slots = World::init_guard_slots(&level, level_no);
        let shared = Shared { level: level_no, drawn_room: kid.room, level1_music: (level_no == 1) as u8, ..Shared::default() };
        let mut w = World {
            tick: 0,
            progress: Progress { carry_hp: None, ..progress },
            exit: None,
            death_count: 0,
            hold_death_counter: false,
            text_timer: 0,
            demo: level_no == 0,
            demo_ctrl: Default::default(),
            mirror_jump: false,
            sub: 0,
            frame_len: SUBSTEPS,
            flash: (0, 0),
            upside: false,
            guard_rec: Default::default(),
            data,
            level,
            level_no,
            kid,
            trobs: vec![],
            mobs: vec![],
            sounds: vec![],
            hits: vec![],
            impacts: vec![],
            leveldoor_open: 0,
            version: 0,
            dirty: true,
            drawn_room: 0,
            rng: Rng(seed),
            last_loose_sound: 0,
            guard: None,
            slots,
            guard_type,
            shared,
            shadow: ShadowState::default(),
        };
        // level start (do_startpos): level 1 presses the button at room 5 tile 2; the entry door closes; chompers wake up
        if level_no == 1 {
            w.trigger_button(false, 0, -1, 5, 2);
        }
        let room = w.kid.room;
        if let Some(idx) = (0..TILES_PER_ROOM).find(|&i| (1..=24).contains(&room) && w.level.tile_at(room as usize, i).kind == Tile::ExitLeft) {
            w.set_mod(room, idx as u8, 43);
            w.add_trob(room, idx as u8, 3);
        }
        w.check_the_end(true); // 0000:4B92: next_room = Kid.room, check_the_end
        w.check_shadow(); // enter_guard / the scripted shadows for the first room
        w.apply_kid_events();
        w
    }

    /// One 60 Hz step; returns true if a logic tick ran.
    pub fn step(&mut self, keys: Keys) -> bool {
        if self.sub == 0 {
            self.frame_len = self.next_frame_len();
        }
        self.sub += 1;
        if self.sub < self.frame_len {
            return false;
        }
        self.sub = 0;
        self.tick += 1;
        self.logic_tick(keys);
        true
    }

    /// Frame length decided at the start of a frame from the kid's sword as the last tick left it.
    fn next_frame_len(&self) -> u32 {
        if self.kid.sword == 2 { SWORD_SUBSTEPS } else { SUBSTEPS }
    }

    /// True if the next `step` runs a logic tick (lets callers snapshot state for interpolation first).
    pub fn next_is_tick(&self) -> bool {
        self.sub + 1 >= if self.sub == 0 { self.next_frame_len() } else { self.frame_len }
    }

    /// Input latency option: makes the next `step` run the logic tick instead of waiting out the rest of the frame. Only for a
    /// controlled kid standing idle (frame 15, action 0, no sword), so no motion is cut short, and only after 2 steps of the frame.
    /// The tick itself is unchanged; only the wall time of this one frame gets shorter. Returns whether it did.
    pub fn tick_now(&mut self) -> bool {
        let k = &self.kid;
        if self.sub < 2 || self.sub + 1 >= self.frame_len || !k.controlled || !k.alive || k.sword != 0 || k.action != 0 || k.frame != 15 {
            return false;
        }
        self.sub = self.frame_len - 1;
        true
    }

    /// Sequence viewer: play sequence `seq` without control and physics.
    pub fn kid_start_seq(&mut self, seq: usize) {
        let env = mk_env(&self.data, &self.level, self.guard_type);
        self.kid.controlled = false;
        self.kid.start_seq(&env, seq);
    }

    /// Progress from the last logic tick to the next, in [0, 1), for render interpolation.
    pub fn alpha(&self) -> f32 {
        self.sub as f32 / self.frame_len as f32
    }

    fn logic_tick(&mut self, keys: Keys) {
        let room_before = self.kid.room;
        self.flash.1 = self.flash.1.saturating_sub(1); // remove_flash_if_hurt of the previous frame
        if self.text_timer > 0 {
            self.text_timer -= 1;
            if self.text_timer == 1 {
                self.set_exit(Exit::Title); // 0000:0AAE: nobody pressed a key
            }
        }
        if self.demo && keys != Keys::default() {
            self.set_exit(Exit::NewGame);
        }
        // timers (0000:4FA9), level 8: the counter in leveldoor_open runs while Char.room is 16 (INFERRED: the kid's room)
        if self.level_no == 8 && self.kid.room == 16 && self.leveldoor_open != 0 {
            self.leveldoor_open = self.leveldoor_open.saturating_add(1); // a word in the original: must not wrap to "closed"
            if self.leveldoor_open == 150 {
                self.do_mouse();
            }
        }
        self.shared.feather = self.shared.feather.saturating_sub(1);
        if !self.kid.alive {
            self.upside = false; // 0000:0EE7
        }
        // hit point deltas are cleared at the start of the frame and applied at its end (0000:4D7F, 0000:12A6)
        self.kid.hp_delta = 0;
        if let Some(g) = self.guard.as_mut() {
            g.hp_delta = 0;
        }
        self.do_mobs();
        self.process_trobs();
        self.check_skel();
        self.shared.drawn_room = self.kid.room;
        self.shared.can_see = self.can_guard_see_kid();
        self.shared.notice_timer = self.shared.notice_timer.saturating_sub(1);
        if self.shadow.united > 0 {
            self.shadow.united -= 1;
            if self.shadow.united == 2 {
                self.sounds.push(41); // 0000:BDFF: drawn with ds:4FCE == 2 (INFERRED: a tile of the room is drawn that frame)
            }
            if self.shadow.united == 0 {
                self.shadow.united = -1; // the hidden floor trick of level 12 is enabled from here
            }
        }
        let opp = self.guard.as_ref().map_or(self.guard_rec, crate::kid::Opp::of);
        if check_killed_shadow(self.level_no, &mut self.kid, &opp) {
            self.flash = (15, 5);
        }
        self.play_kid_counter();
        self.kid.opp = self.guard.as_ref().map_or_else(Default::default, crate::kid::Opp::of);
        self.shared.leveldoor_open = self.leveldoor_open != 0;
        self.shared.sound_busy = self.hold_death_counter;
        self.kid.sh = self.shared;
        let input = if self.demo { Input::Moves(self.demo_control()) } else { Input::Keys(keys) };
        {
            let env = mk_env(&self.data, &self.level, self.guard_type);
            self.kid.tick_input(&env, input);
        }
        if self.demo {
            self.demo_ctrl = self.kid.last_ctrl;
        }
        self.shared = self.kid.sh;
        self.apply_kid_events();
        self.play_guard_frame();
        self.check_sword_hurting();
        self.check_sword_hurt();
        if self.kid.frame == 167 || self.guard.as_ref().is_some_and(|g| g.frame == 167) {
            self.sounds.push(10); // swords clash
        }
        self.do_delta_hp();
        // exit_room / check_the_end: the kid changed room
        if let Some(dir) = self.kid.leave_dir.take() {
            self.on_room_left(dir, room_before);
            self.guard_after_room_change(dir);
            self.shared.drawn_room = self.kid.room;
            self.check_the_end(false); // start_chompers already ran from the kid's room-change event
            self.check_shadow();
        }
        self.check_guard_fallout();
        // play_frame (0000:095B, 0000:09AA): demo over in room 24; level 12 room 23 leads seamlessly into level 13
        if self.demo && self.kid.room == 24 {
            self.set_exit(Exit::Title);
        }
        if self.level_no == 12 && self.kid.room == 23 {
            self.set_exit(Exit::NextLevel { seamless: true });
        }
        if self.level_no == 14 && self.kid.room == 5 {
            self.set_exit(Exit::End); // check_the_end runs on room entry; the exit is set once, so testing every tick is the same
        }
        self.show_time();
        if std::mem::take(&mut self.mirror_jump) {
            self.jump_through_mirror(); // check_mirror runs in the drawing pass, after play_frame
        }
        if self.flash.1 == 0 && self.kid.hp_delta < 0 {
            self.flash = (12, 1); // flash_if_hurt (0000:5374): red for the frame the kid lost hit points
        }
        if self.dirty {
            self.dirty = false;
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Level events at a room change (docs/combat.md 7.5): Jaffar is met, the exit door opens after his death.
    fn on_room_left(&mut self, dir: u8, from: u8) {
        if self.level_no == 3 && dir == 0 && from == 7 {
            self.progress.checkpoint = true; // 0000:3D59
            self.progress.hitp_beg_lev = self.kid.hp_max;
        }
        if self.level_no == 4 && dir == 0 && from == 11 && self.kid.row == 0 && self.leveldoor_open != 0 && self.leveldoor_open != 0x4D {
            self.sounds.push(25); // 0000:3DC7: mirror music, once
            self.leveldoor_open = 0x4D;
        }
        if self.level_no == 13 && dir == 1 && self.leveldoor_open == 0 && from == 3 {
            self.sounds.push(29);
            self.shared.notice_timer = 28;
        }
        if self.level_no == 13 && dir == 0 && self.leveldoor_open == 2 {
            self.trigger_button(false, 0, -1, 24, 0); // opens the exit door through room 24
        }
    }

    fn apply_kid_events(&mut self) {
        let evs = std::mem::take(&mut self.kid.events);
        for e in evs {
            self.apply_event(e);
        }
    }

    fn apply_event(&mut self, e: Event) {
        {
            match e {
                Event::Pickup { room, tilepos } => {
                    self.set_tile(room, tilepos, Tile::Floor);
                    self.set_mod(room, tilepos, 0);
                }
                Event::Button { room, tilepos, dead } => {
                    if dead { self.died_on_button(room, tilepos) } else { self.trigger_button(true, 0, -1, room, tilepos) }
                }
                Event::LooseFall { room, tilepos } => self.make_loose_fall(room, tilepos, 1),
                Event::Knock { room, row } => self.do_knock(room, row),
                Event::StartSpike { room, tilepos } => self.start_anim_spike(room, tilepos),
                Event::Spiked { room, tilepos } => self.set_mod(room, tilepos, 255),
                Event::Chomped { room, tilepos } => {
                    let m = self.modifier(room, tilepos);
                    self.set_mod(room, tilepos, m | 0x80);
                }
                Event::StartChompers { room, row } => self.start_chompers(room, row),
                // seq op F2 4 (0000:7049): shadow music 32 on level 4, nothing on levels 13 and 15, else end-level music 41
                Event::Sound(41) => match self.level_no {
                    4 => self.sounds.push(32),
                    13 | 15 => {}
                    _ => self.sounds.push(41),
                },
                Event::Sound(id) => self.sounds.push(id),
                Event::GetObject(kind) => self.get_object(kind),
                Event::NextLevel => self.set_exit(Exit::NextLevel { seamless: false }),
                // is_obstacle fires on every probe of frames 39..43; the 0x56 modifier marks the jump as done (one shadow per jump)
                Event::Mirror { room, tilepos } if self.modifier(room, tilepos) != 0x56 => {
                    self.set_mod(room, tilepos, 0x56);
                    self.mirror_jump = true;
                }
                Event::Mirror { .. } => {}
            }
        }
    }

    /// proc_get_object (0000:81AB), the world part; hit points and have_sword were set by the kid. (Stop-sounds not modelled.)
    fn get_object(&mut self, kind: i8) {
        match kind {
            -1 => {
                self.sound(37);
                self.flash = (14, 8);
            }
            1 => {
                self.sound(33);
                self.flash = (4, 2);
            }
            2 => {
                self.sound(30);
                self.flash = (4, 4);
            }
            3 => {
                self.shared.feather = FEATHER_TICKS; // set_feather_fall (0000:162B)
                self.sound(39);
                self.flash = (2, 3);
            }
            4 => {
                self.upside = !self.upside; // toggle_upside (0000:161C)
                self.dirty = true;
            }
            5 => self.sound(13),
            6 => self.trigger_button(false, 0, -1, 8, 0), // level 15: get_tile(8, 0, 0); trigger_button(0, 0, -1)
            _ => {}
        }
    }

    // ------------------------------------------------------------------ death, restart, time (docs/edge_cases.md 3.2-3.5)

    /// The first exit of a tick wins, except that the ending replaces a time-out / title or restart set earlier in the
    /// same tick (Exit::End must not be lost to a race on the final tick).
    fn set_exit(&mut self, e: Exit) {
        if e == Exit::End && matches!(self.exit, Some(Exit::Title | Exit::Restart)) {
            self.exit = Some(e);
        }
        self.exit.get_or_insert(e);
    }

    /// Key handler (0000:054D..05E8): `continue_key` = Shift held or Enter, `ctrl_a` = Ctrl+A (any time).
    pub fn restart_input(&mut self, continue_key: bool, ctrl_a: bool) {
        if self.level_no == 15 {
            return;
        }
        if ctrl_a || (continue_key && self.progress.rem_min != 0 && self.death_count > 6) {
            self.set_exit(Exit::Restart);
        }
    }

    /// "Press Button to Continue" is shown (blinking with period 12 in the last 72 ticks).
    pub fn continue_text(&self) -> bool {
        self.death_count >= 8 && !matches!(self.level_no, 0 | 15) && self.text_timer > 0 && (self.text_timer > 72 || self.text_timer % 12 <= 3)
    }

    /// The progress the next world starts from after `exit` (docs/edge_cases.md 3.4, play_level_2 0000:4DA2).
    pub fn progress_after(&self, exit: Exit) -> Progress {
        let mut p = Progress { carry_hp: None, ..self.progress };
        match exit {
            Exit::Restart => p.checkpoint &= p.level != 0, // level 0: the checkpoint is the demo's guard-killed flag
            Exit::NextLevel { seamless } => {
                p.level += 1;
                p.hitp_beg_lev = self.kid.hp_max;
                p.checkpoint = false;
                p.carry_hp = seamless.then_some(self.kid.hp);
            }
            Exit::Title | Exit::End => p = Progress::new_game(0),
            Exit::NewGame => p = Progress::new_game(1),
        }
        p
    }

    /// fell_out (0000:7900), control_kid's death test (0000:7A1F) and the death counter of play_kid (0000:792A).
    /// Sounds count as finished at once (deterministic); the app may set `hold_death_counter` while they play.
    fn play_kid_counter(&mut self) {
        let k = &mut self.kid;
        if k.alive && k.room == 0 {
            k.take_hp(100);
            k.alive = false;
            k.frame = 185;
        }
        if k.alive && k.hp <= 0 && k.controlled {
            k.alive = false;
        }
        if k.alive || !k.is_dead() || self.hold_death_counter {
            return;
        }
        match self.death_count {
            0..=5 => {}
            6 => {
                if !matches!(self.level_no, 0 | 15) {
                    let id = if self.guard_rec.charid == 1 { 32 } else if self.shared.holding_sword { 28 } else { 24 };
                    self.sounds.push(id); // death music (0000:8309)
                }
            }
            7 => {
                if self.progress.rem_min == 0 {
                    self.set_exit(Exit::Title); // expired (0000:31F4); the app plays the time-out cutscene
                }
                self.text_timer = if matches!(self.level_no, 0 | 15) { 36 } else { 288 };
            }
            _ => return,
        }
        self.death_count += 1;
    }

    /// show_time (0000:C170) and the time-out test of play_frame (0000:09C9).
    fn show_time(&mut self) {
        let p = &mut self.progress;
        // 0000:C176..C199: alive, rem_min != 0, level <= 13 (13 only while leveldoor_open == 0: Jaffar's death sets 2, the
        // opened door then 1, time stays stopped), level < 15. Level 0 is not excluded: the clock also runs in the demo.
        // 0000:C18D (VERIFIED): `cmp word [40DC],0; jne skip`, i.e. level 13 stops for any leveldoor_open != 0.
        let runs = self.level_no < 13 || (self.level_no == 13 && self.leveldoor_open == 0);
        if self.kid.alive && p.rem_min != 0 && runs {
            p.rem_tick -= 1;
            if p.rem_tick == 0 {
                p.rem_tick = 719;
                p.rem_min -= 1;
            }
        }
        if self.level_no < 13 && self.progress.rem_min == 0 {
            self.set_exit(Exit::Title);
        }
    }

    // ------------------------------------------------------------------ helpers

    fn kind(&self, room: u8, idx: u8) -> Tile {
        if !(1..=24).contains(&room) || idx as usize >= TILES_PER_ROOM {
            return Tile::Wall;
        }
        self.level.tile_at(room as usize, idx as usize).kind
    }

    fn modifier(&self, room: u8, idx: u8) -> u8 {
        if !(1..=24).contains(&room) || idx as usize >= TILES_PER_ROOM {
            return 0;
        }
        self.level.tile_at(room as usize, idx as usize).modifier
    }

    fn set_mod(&mut self, room: u8, idx: u8, m: u8) {
        if (1..=24).contains(&room) && (idx as usize) < TILES_PER_ROOM {
            self.level.set_modifier(room as usize, idx as usize, m);
            self.dirty = true;
        }
    }

    fn set_tile(&mut self, room: u8, idx: u8, t: Tile) {
        if (1..=24).contains(&room) && (idx as usize) < TILES_PER_ROOM {
            self.level.set_tile(room as usize, idx as usize, t);
            self.dirty = true;
        }
    }

    fn sound(&mut self, id: u8) {
        self.sounds.push(id);
    }

    /// Logs a tile impact for the renderers (capped, so a consumer that never clears it cannot make it grow).
    fn impact(&mut self, kind: ImpactKind, room: u8, tilepos: u8) {
        if self.impacts.len() < 32 {
            self.impacts.push(Impact { kind, room, tilepos });
        }
    }

    /// door_sound_if_visible (simplified: only the room the kid is in counts as visible)
    fn sound_if_visible(&mut self, room: u8, id: u8) {
        if room == self.kid.room {
            self.sounds.push(id);
        }
    }

    /// prandom (0C83:0412)
    fn prandom(&mut self, max: u32) -> u32 {
        self.rng.next(max)
    }

    /// add_trob (0000:9052): an existing entry only gets its type replaced.
    fn add_trob(&mut self, room: u8, tilepos: u8, ty: i8) {
        self.dirty = true;
        if let Some(t) = self.trobs.iter_mut().find(|t| t.tilepos == tilepos && t.room == room) {
            t.ty = ty;
        } else if self.trobs.len() < MAX_TROBS {
            self.trobs.push(Trob { tilepos, room, ty });
        }
    }

    fn add_mob(&mut self, m: Mob) {
        if self.mobs.len() < MAX_MOBS {
            self.mobs.push(m);
        }
    }

    // ------------------------------------------------------------------ triggers (docs/tiles.md 3.4, 4)

    /// trigger_button (0000:924B) for the button tile at (room, tilepos). `button_type` 0 = the tile's own type,
    /// `modifier` -1 = the tile's own link index.
    fn trigger_button(&mut self, playsound: bool, button_type: u8, modifier: i32, room: u8, tilepos: u8) {
        let bt = if button_type == 0 { self.kind(room, tilepos) as u8 } else { button_type };
        let m = if modifier == -1 { self.modifier(room, tilepos) } else { modifier as u8 };
        let t = self.level.link_timer(m);
        if t != 0x1F {
            self.level.set_link_timer(m, 5);
            if t < 2 {
                self.add_trob(room, tilepos, 1);
                self.shared.guard_notice = true;
                if playsound {
                    self.sound(3);
                }
            }
            self.do_trigger_list(m, bt);
        }
    }

    /// do_trigger_list (0000:8FDD)
    fn do_trigger_list(&mut self, index: u8, bt: u8) {
        for link in self.level.door_links(index) {
            let (room, idx) = (link.room, link.tile);
            let r = match self.kind(room, idx) {
                Tile::Gate => self.trigger_gate(room, idx, bt),
                Tile::ExitLeft => if self.modifier(room, idx) != 0 { -1 } else { 1 },
                _ => -1,
            };
            if r >= 0 {
                self.add_trob(room, idx, r);
            }
        }
    }

    /// trigger_gate (0000:8F24)
    fn trigger_gate(&mut self, room: u8, idx: u8, bt: u8) -> i8 {
        let m = self.modifier(room, idx);
        match bt {
            15 => {
                if m == 255 {
                    -1
                } else if m >= 188 {
                    self.set_mod(room, idx, 238);
                    -1
                } else {
                    self.set_mod(room, idx, m.wrapping_add(3) & 0xFC);
                    1
                }
            }
            14 => {
                if m < 188 {
                    2
                } else {
                    self.set_mod(room, idx, 255);
                    -1
                }
            }
            _ => if m != 0 { 3 } else { -1 },
        }
    }

    /// died_on_button (0000:92D1)
    fn died_on_button(&mut self, room: u8, tilepos: u8) {
        let bt = self.kind(room, tilepos);
        let old = self.modifier(room, tilepos);
        let bt = if bt == Tile::RaiseButton {
            self.set_tile(room, tilepos, Tile::Floor);
            self.set_mod(room, tilepos, 0);
            14
        } else {
            self.set_tile(room, tilepos, Tile::StuckButton);
            bt as u8
        };
        self.trigger_button(true, bt, old as i32, room, tilepos);
    }

    // ------------------------------------------------------------------ trobs

    fn process_trobs(&mut self) {
        let mut i = 0;
        while i < self.trobs.len() {
            let mut t = self.trobs[i];
            self.animate_tile(&mut t);
            if i < self.trobs.len() {
                self.trobs[i] = t;
            }
            i += 1;
        }
        self.trobs.retain(|t| t.ty >= 0);
    }

    /// animate_tile (0000:86A3): dispatch on the current tile type, then write the modifier back.
    fn animate_tile(&mut self, t: &mut Trob) {
        let (room, tp) = (t.room, t.tilepos);
        let mut m = self.modifier(room, tp);
        match self.kind(room, tp) {
            Tile::Spikes => self.animate_spike(t, &mut m),
            Tile::Gate => self.animate_door(t, &mut m),
            Tile::DropButton | Tile::RaiseButton => {
                let timer = self.level.link_timer(m);
                if timer == 0 {
                    t.ty = -1;
                } else {
                    let nt = timer - 1;
                    self.level.set_link_timer(m, nt);
                    if nt < 2 {
                        t.ty = -1;
                    }
                }
                self.dirty = true;
            }
            Tile::LooseFloor => self.animate_loose(t, &mut m),
            Tile::ExitLeft => self.animate_leveldoor(t, &mut m),
            Tile::Chomper => self.animate_chomper(t, &mut m),
            // animate_potion (0000:89DD): bubble frame + 1, 8 -> 1 (0000:8CA5); kind bits kept
            Tile::Potion if t.ty >= 0 && self.in_drawn_room(t) => {
                let f = (m & 0x1F) + 1;
                m = (m & 0xE0) | if f >= 8 { 1 } else { f };
            }
            // animate_torch (0000:89C3): get_torch_frame (0000:8CC5); no type test
            Tile::Torch | Tile::TorchDebris if self.in_drawn_room(t) => {
                let r = self.prandom(255);
                m = crate::cutscene::torch_frame(m, r);
            }
            // animate_sword (0000:8A19): glint countdown, the glint image shows at 1 (0000:A910); no type test
            Tile::Sword if self.in_drawn_room(t) => {
                m = m.wrapping_sub(1);
                if m == 0 {
                    m = (self.prandom(255) as u8 & 0x3F) + 40;
                }
            }
            Tile::Potion | Tile::Torch | Tile::TorchDebris | Tile::Sword => {} // not in the drawn room: dropped
            _ => t.ty = -1, // empty, stuck button, debris, ...
        }
        // the original writes the modifier back unconditionally (a fallen loose floor sets it in remove_loose)
        if self.modifier(room, tp) != m {
            match self.kind(room, tp) {
                // visual only: no `version` bump, renderers read the frame every frame (Level::anim_frame)
                Tile::Potion | Tile::Torch | Tile::TorchDebris | Tile::Sword => self.level.set_modifier(room as usize, tp as usize, m),
                _ => self.set_mod(room, tp, m),
            }
        }
    }

    /// is_trob_in_drawn_room (0000:875A): a trob of another room is dropped (type -1).
    fn in_drawn_room(&self, t: &mut Trob) -> bool {
        if t.room != self.drawn_room {
            t.ty = -1;
        }
        t.room == self.drawn_room
    }

    /// check_the_end (0000:0FE8) when the kid's room becomes the drawn room: anim_tile_modif (0000:0B3D) gives every potion,
    /// torch and sword of the room a random phase and a trob, then start_chompers (`chompers`) and check_fall_flo (0000:1034).
    /// (The level-14 end and check_shadow are done by the callers.)
    fn check_the_end(&mut self, chompers: bool) {
        let room = self.kid.room;
        if room == 0 || room == self.drawn_room || room > 24 {
            return;
        }
        self.drawn_room = room;
        for tp in 0..TILES_PER_ROOM as u8 {
            let m = self.modifier(room, tp);
            let m = match self.kind(room, tp) {
                Tile::Potion => (m & 0xE0) | (self.prandom(6) as u8 + 1), // start_anim_potion (0000:8E3F)
                Tile::Torch | Tile::TorchDebris => self.prandom(8) as u8,  // start_anim_torch (0000:8E16)
                Tile::Sword => self.prandom(255) as u8 & 0x1F,             // start_anim_sword (0000:8E74)
                _ => continue,
            };
            self.level.set_modifier(room as usize, tp as usize, m);
            self.add_trob(room, tp, 1);
        }
        if chompers {
            self.start_chompers(room, self.kid.row);
        }
        // check_fall_flo: level 13 rooms 23 and 16, tiles 22..27 of the room above start falling, staggered by prandom
        if self.level_no == 13 && (room == 23 || room == 16) {
            let above = self.level.links[room as usize - 1][2];
            // shortcut: with no room above the original works on stale room pointers; both rooms have one in the data
            if above != 0 {
                for tp in 22..=27 {
                    let r = self.prandom(255) as u8 & 15;
                    self.make_loose_fall(above, tp, r.wrapping_neg());
                }
            }
        }
    }

    /// animate_spike (0000:8AC7)
    fn animate_spike(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 || *m == 255 {
            return;
        }
        if *m & 0x80 != 0 {
            *m -= 1;
            if *m & 0x7F == 0 {
                *m = 6;
            } else {
                return;
            }
        } else {
            *m += 1;
            if *m == 5 {
                *m = 0x8F;
            } else if *m == 9 {
                *m = 0;
                t.ty = -1;
            }
        }
    }

    /// animate_door (0000:8B16)
    fn animate_door(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        let room = t.room;
        if t.ty >= 3 {
            if t.ty < 8 {
                t.ty += 1;
            }
            let sp = GATE_CLOSE[t.ty as usize];
            if *m < sp {
                *m = 0;
                t.ty = -1;
                self.sound(6);
                self.impact(ImpactKind::GateSlam, room, t.tilepos);
            } else {
                *m -= sp;
            }
        } else if *m == 255 {
            t.ty = -1;
            self.sound_if_visible(room, 7);
        } else {
            *m = m.wrapping_add(DOOR_DELTA[t.ty as usize] as u8);
            if t.ty == 0 {
                if *m == 0 {
                    t.ty = -1;
                    self.sound_if_visible(room, 7);
                } else if *m < 188 && *m & 3 == 3 {
                    self.sound_if_visible(room, 4);
                }
            } else if *m < 188 {
                if *m & 7 == 0 {
                    self.sound(5);
                }
            } else if t.ty < 2 {
                *m = 238;
                t.ty = 0;
                self.sound(7);
            } else {
                *m = 255;
                t.ty = -1;
                self.sound_if_visible(room, 7);
            }
        }
    }

    /// animate_loose (0000:9395)
    fn animate_loose(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        *m = m.wrapping_add(1);
        if *m & 0x80 != 0 {
            if self.level_no == 13 {
                return;
            }
            if *m >= 0x84 {
                *m = 0;
                t.ty = -1;
            }
            self.loose_shake(*m, *m == 0);
        } else if *m >= 11 {
            // falls: the tile becomes empty (remove_loose) and a mob takes over
            self.set_tile(t.room, t.tilepos, Tile::Empty);
            self.impact(ImpactKind::LooseFall, t.room, t.tilepos);
            *m = 0;
            t.ty = -1;
            let row = t.tilepos / COLS as u8;
            self.add_mob(Mob { xh: (t.tilepos % COLS as u8) * 4, y: Y_LOOSE_LAND[row as usize + 1], room: t.room, speed: 0, ty: 0, row });
        } else {
            self.loose_shake(*m, false);
        }
    }

    /// loose_shake (0000:944D): shake sounds 20..22, never the same twice in a row. (The extra prandom(1000) for the
    /// digital sample rate is not consumed here.)
    fn loose_shake(&mut self, m: u8, force: bool) {
        if force || LOOSE_SOUND.get((m & 0x7F) as usize).copied().unwrap_or(0) != 0 {
            let mut s;
            loop {
                s = self.prandom(2) as u8 + 20;
                if s != self.last_loose_sound {
                    break;
                }
            }
            self.last_loose_sound = s;
            self.sound(s);
        }
    }

    /// animate_leveldoor (0000:8BE5)
    fn animate_leveldoor(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        if t.ty >= 3 {
            t.ty = (t.ty + 1).min(7);
            let sp = LEVELDOOR_CLOSE[t.ty as usize];
            if *m < sp {
                *m = 0;
                t.ty = -1;
                self.sound(14);
            } else {
                *m -= sp;
            }
        } else {
            *m = m.wrapping_add(1);
            if *m >= 43 {
                t.ty = -1;
                if self.leveldoor_open == 0 || self.leveldoor_open == 2 {
                    self.leveldoor_open = 1;
                    if self.level_no == 4 {
                        self.set_tile(4, 4, Tile::Mirror); // tiles.md 8.1: (room 4, col 4, row 0)
                    }
                }
            } else {
                self.sound(15);
            }
        }
    }

    /// animate_chomper (0000:8A3C)
    fn animate_chomper(&mut self, t: &mut Trob, m: &mut u8) {
        if t.ty < 0 {
            return;
        }
        let blood = *m & 0x80;
        let mut f = (*m & 0x7F) + 1;
        if f > 15 {
            f = 1;
        }
        *m = blood | f;
        if f == 2 {
            self.sound_if_visible(t.room, 47);
            self.impact(ImpactKind::Chomp, t.room, t.tilepos);
        }
        if (t.room != self.kid.room || (t.tilepos / COLS as u8) as i32 != self.kid.row || (self.kid.alive && blood == 0)) && f >= 6 {
            t.ty = -1;
        }
    }

    // ------------------------------------------------------------------ mechanics started by events

    /// start_anim_spike (0000:8EDB)
    fn start_anim_spike(&mut self, room: u8, tp: u8) {
        let m = self.modifier(room, tp);
        if (m as i8) <= 0 {
            if m == 0 {
                self.add_trob(room, tp, 1);
                self.sound(49);
            } else if m != 255 {
                self.set_mod(room, tp, 0x8F);
            }
        }
    }

    /// make_loose_fall (0000:94CD)
    fn make_loose_fall(&mut self, room: u8, tp: u8, m: u8) {
        if !(1..=24).contains(&room) || tp as usize >= TILES_PER_ROOM {
            return;
        }
        let cur = self.modifier(room, tp);
        if self.level.fg_byte(room as usize, tp as usize) & 0x20 == 0 && (cur as i8) <= 0 {
            self.set_mod(room, tp, m);
            self.add_trob(room, tp, 0);
        }
    }

    /// do_knock (0000:95D8): shake every loose floor of a row
    fn do_knock(&mut self, room: u8, row: i32) {
        if !(0..ROWS as i32).contains(&row) {
            return;
        }
        for col in 0..COLS as u8 {
            let tp = row as u8 * COLS as u8 + col;
            if self.kind(room, tp) == Tile::LooseFloor && self.modifier(room, tp) == 0 && self.level_no != 13 {
                self.set_mod(room, tp, 0x80);
                self.add_trob(room, tp, 1);
            }
        }
    }

    /// start_chompers (0000:950B)
    fn start_chompers(&mut self, room: u8, row: i32) {
        if !(0..ROWS as i32).contains(&row) || !(1..=24).contains(&room) {
            return;
        }
        let mut timing: u8 = 15;
        for col in 0..COLS as u8 {
            let tp = row as u8 * COLS as u8 + col;
            if self.kind(room, tp) != Tile::Chomper {
                continue;
            }
            let m = self.modifier(room, tp);
            if m & 0x7F == 0 || m & 0x7F >= 6 {
                if m == 0 || m >= 6 {
                    self.set_mod(room, tp, timing | (m & 0x80));
                    self.add_trob(room, tp, 1);
                }
                timing = timing.wrapping_sub(3);
                if timing < 6 {
                    timing = timing.wrapping_add(10);
                }
            }
        }
    }

    // ------------------------------------------------------------------ mobs (falling loose floors)

    fn do_mobs(&mut self) {
        let n = self.mobs.len();
        for i in 0..n {
            let mut m = self.mobs[i];
            self.move_mob(&mut m);
            self.check_loose_fall_on_kid(&m);
            self.mobs[i] = m;
            self.dirty = true;
        }
        self.mobs.retain(|m| m.speed != -1);
    }

    fn move_mob(&mut self, m: &mut Mob) {
        if m.ty == 0 {
            self.move_loose(m);
        }
        if m.speed <= 0 {
            m.speed = m.speed.wrapping_add(1);
        }
    }

    /// move_loose (0000:971E)
    fn move_loose(&mut self, m: &mut Mob) {
        if m.speed < 0 {
            return;
        }
        if m.speed < 29 {
            m.speed += 3;
        }
        m.y = m.y.wrapping_add(m.speed as u8);
        if m.room == 0 {
            if m.y >= 210 {
                m.speed = -2;
            }
            return;
        }
        if m.y >= 226 || (m.y as i32) < Y_SOMETHING[(m.row as usize + 1).min(4)] {
            return;
        }
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        let t = self.kind(m.room, idx);
        if t == Tile::LooseFloor {
            self.loose_fall(m);
        }
        if matches!(t, Tile::Empty | Tile::LooseFloor) {
            self.mob_down_a_row(m);
            return;
        }
        self.sound(2);
        self.impact(ImpactKind::Crash, m.room, idx);
        self.do_knock(m.room, m.row as i32);
        m.y = Y_SOMETHING[m.row as usize + 1] as u8;
        m.speed = -2;
        self.loose_land(m);
    }

    /// loose_fall (0000:98C3): the floor the mob hits is knocked loose too (approximation: the copy is dropped
    /// when it would start below row 2).
    fn loose_fall(&mut self, m: &mut Mob) {
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        self.set_tile(m.room, idx, Tile::Empty);
        self.set_mod(m.room, idx, 0);
        m.speed >>= 1;
        if m.row < 2 {
            self.add_mob(Mob { xh: m.xh, y: m.y.wrapping_add(6), room: m.room, speed: m.speed, ty: 0, row: m.row + 1 });
        }
    }

    /// mob_down_a_row (0000:997F)
    fn mob_down_a_row(&mut self, m: &mut Mob) {
        m.row += 1;
        if m.row >= 3 {
            m.y = m.y.wrapping_sub(192);
            m.row = 0;
            m.room = if (1..=24).contains(&m.room) { self.level.links[m.room as usize - 1][3] } else { 0 };
        }
    }

    /// loose_land (0000:97E0)
    fn loose_land(&mut self, m: &Mob) {
        let idx = m.row * COLS as u8 + (m.xh >> 2);
        match self.kind(m.room, idx) {
            Tile::RaiseButton => {
                self.set_tile(m.room, idx, Tile::Debris);
                self.trigger_button(true, 14, -1, m.room, idx);
            }
            Tile::DropButton => {
                self.trigger_button(true, 0, -1, m.room, idx);
                self.set_tile(m.room, idx, Tile::Debris);
            }
            Tile::Floor | Tile::Spikes | Tile::Potion => self.set_tile(m.room, idx, Tile::Debris),
            Tile::Torch | Tile::TorchDebris => self.set_tile(m.room, idx, Tile::TorchDebris),
            _ => {}
        }
    }

    /// check_loose_fall_on_kid (0000:9B89) / fell_on_your_head (0000:9BCB)
    fn check_loose_fall_on_kid(&mut self, m: &Mob) {
        let k = &self.kid;
        if m.room != k.room || !k.alive {
            return;
        }
        let col = {
            let env = mk_env(&self.data, &self.level, self.guard_type);
            k.col(&env)
        };
        let ky = k.y as i32;
        if (m.xh >> 2) as i32 != col || !((m.y as i32) < ky && ky - 30 < m.y as i32) {
            return;
        }
        let (f, a) = (k.frame, k.action);
        if !((self.level_no == 13 || f < 5 || f >= 15) && (a < 2 || a == 7)) {
            return;
        }
        let env = mk_env(&self.data, &self.level, self.guard_type);
        let k = &mut self.kid;
        k.y = Y_LAND[(k.row + 1).clamp(0, 4) as usize] as u8;
        let died = k.take_hp(1);
        let dir = if k.facing_left { -1 } else { 1 };
        if died {
            if f == 177 {
                k.x = (k.x as i32 - 12 * dir) as u8;
            }
            k.start_seq(&env, 22);
        } else if f != 109 {
            let behind = env.level.tile_wrapped(k.room as i32, col - dir, k.row);
            if behind == Tile::Empty {
                k.x = (k.x as i32 - 2 * dir) as u8;
            }
            k.start_seq(&env, 52);
        }
    }
}

/// check_killed_shadow (0000:84B1): level 12, the kid and the shadow; `ch` alive and `opp` dead -> `ch` dies too.
/// Called at the start of the kid tick (Char = Kid) and of the guard tick (Char = Guard). Returns true if it fired
/// (the caller flashes white for 5 frames).
pub(crate) fn check_killed_shadow(level_no: usize, ch: &mut Kid, opp: &crate::kid::Opp) -> bool {
    if level_no != 12 || (ch.charid | opp.charid) != 1 || !ch.alive || opp.alive {
        return false;
    }
    ch.take_hp(100);
    true
}

#[cfg(test)]
impl World {
    pub(crate) fn set_mod_for_test(&mut self, room: u8, idx: u8, m: u8) {
        self.set_mod(room, idx, m);
    }
    pub(crate) fn set_tile_for_test(&mut self, room: u8, idx: u8, t: Tile) {
        self.set_tile(room, idx, t);
    }
    /// Sets the prandom state (the original's seed at ds:40C4/40C6), for tools/tick_trace.
    pub(crate) fn set_seed_for_test(&mut self, seed: u32) {
        self.rng = crate::kid::Rng(seed);
    }
    pub(crate) fn seed_for_test(&self) -> u32 {
        self.rng.0
    }
    pub(crate) fn seeded_for_test(data: Arc<Data>, level: Level, progress: Progress, seed: u32) -> World {
        World::seeded(data, level, progress, seed)
    }
    pub(crate) fn start_spike_for_test(&mut self, room: u8, idx: u8) {
        self.start_anim_spike(room, idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kid::Keys;

    #[test]
    fn logic_runs_every_fifth_step() {
        if crate::no_game_data() { return; }
        // a World needs game data; the timing logic is tested through the counters only
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Original_data/");
        let exe = Exe::open(format!("{dir}PRINCE.EXE")).unwrap();
        let frames = exe.kid_frames();
        let data = Arc::new(Data { exe, frames, sizes: vec![(0, 0); 220], gframes: vec![], gsizes: vec![] });
        let lv = crate::dat::Dat::open(format!("{dir}LEVELS.DAT")).unwrap();
        let level = Level::parse(lv.get(2002).unwrap()).unwrap();
        let mut w = World::new(data, level, 2);
        let ran: Vec<bool> = (0..10).map(|_| w.step(Keys::default())).collect();
        assert_eq!(ran, [false, false, false, false, true, false, false, false, false, true]);
        assert_eq!(w.tick, 2);
        assert_eq!(w.alpha(), 0.0);
        w.step(Keys::default());
        assert_eq!(w.alpha(), 0.2);
        assert_eq!(STEP_HZ, 60.0);
    }
}
