# Changelog

All notable changes. Format: [Keep a Changelog](https://keepachangelog.com/); versions follow `Cargo.toml`.
The original game data is never part of the repository or a release; see `docs/PROVENANCE.md`.

## [Unreleased]

## [0.6.13] - 2026-11-25

### Added
- Option `cinematic_fights` (Settings > Graphics, off by default, enhanced modes only): while the prince has his sword drawn against a guard in the same room the camera zooms smoothly on the middle of the two (at most 1.8x, both always in the picture with a margin); a sword hit sprays blood from the blade tip that stays on the floor, a killed figure gets a growing pool; each hit makes the attacker's sword bloodier (flecked, then covered), sheathing cleans it. `World::hits` reports every landed sword hit (`sim::Hit`).
- Debug hook `POP_SET=...,guard=TILE`: one guard on that tile of the start room, a plain floor row and a sword for the kid (sword-fight screenshots).
- Dev profile: dependencies are built with `debug = "line-tables-only"` (cold debug build 24 % faster, `target/debug` about 47 % smaller; no variable info inside Bevy).
- `tools/hygiene.ps1`, run at the end of `tools/package.ps1`: removes merged, clean, unused agent worktrees and the incremental caches (`target/*/incremental`).

## [0.6.12] - 2026-11-25

### Changed
- Palace tapestries are turned like the gates (plate in the y-z plane next to the door post, from the back wall to the floor's front edge); door posts now stand against the back wall, and gates reach from the post's front face to the floor's front edge (they stopped at z 0.45 and began at -0.15).
- The light of a voxel torch moves with the centre of its flame's pixels (frame by frame, eased) instead of sitting at a fixed point above the holder.
- A drunk potion's bubble light no longer vanishes at once: it flies from the bottle's place to the kid's face during the first 12 ticks of the drinking
  animation and fades exponentially until its end (21 ticks, `sim::DRINK_TICKS`).

## [0.6.11] - 2026-11-25

### Changed
- Potions stand in the middle of their tile instead of at its front edge; their bubble light follows the bubbles.
- Sword-clash sparks burn out: random life 1.5-2 s, the glow cools from yellow-white to dark red and goes out instead of staying orange until the end.

## [0.6.10] - 2026-11-25

### Added
- Input latency options: `quick_input` (on by default) starts the logic tick at once when a direction key arrives while the kid stands idle (saves
  0-83 ms, 42 ms on average, the tick itself is unchanged); `low_latency_present` (off by default) turns vsync off and keeps one frame in flight (restart needed,
  tearing possible). The ignored test `input_latency_in_ticks` prints the ticks from key to pose and movement.

## [0.6.9] - 2026-11-25

### Fixed
- The tag `v0.6.8` points at the commit before the version bump (a repository rule keeps tags from being moved); this release carries the same
  program as v0.6.8, with the version number and sources that match its tag.

## [0.6.8] - 2026-11-25

### Added
- Tile relief (off by default; Settings > Graphics `tile_relief`, `POP_RELIEF=1`): the tile images enlarged 4x with MMPX (palette and single pixels
  kept) and a normal map derived from them, so the torches light the brick edges. About 17x the tile texture memory (156 MB instead of 9 MB). Cube meshes now carry tangents.
- Texture atlas (`atlas.rs`): the tile images of a set are packed into a few pages, materials select their rectangle with `uv_transform`; gutters of
  replicated edge pixels keep neighbours from showing through, and the atlas grows by pages when more images are added. 881 textures became 2-22:
  frame time with relief and 4 mip levels on a palace level went from about 75 ms to 14 ms (dev build).
- `POP_MIPS=<levels>` (1-4, default 1) builds mip maps for the enlarged tile textures on the CPU (sRGB colour in linear light, renormalised normals).
- Test hook `POP_UPSCALE=nearest|epx|mmpx|sharp` for the tile colour images (MMPX ported to Rust) and `tools/tile_filters/compare.py`, the filter
  comparison on the game's own tiles; results in `docs/tile_filters.md`.

## [0.6.7] - 2026-11-24

### Added
- The princess chamber of the cutscenes is rebuilt from the original picture PV 951: a level camera in which the picture plane is the screen,
  the floor, the carpet (lying on it), the far wall with the dotted band and the arcade columns at the depths the painted lines give,
  picture layers cut out of PV 951 for the arches, the left wall with its door and the front piers, thin and front columns cut out of the
  picture as voxel and flat variants. Compared with the picture by an overlay: the same room. The carpet, which the picture cuts at its left
  edge, gets its slanted left end (procedural, from the picture's own margin and border).

### Changed
- Release build time: the release workflow skips the tests (CI runs them on the commit), reads a release cache that the new workflow
  `release-cache.yml` keeps on `main`, and does nothing when the release already carries a locally built zip
  (`tools/package.ps1 -SkipTests` for the workflow; the local release procedure is in docs/DEVELOPING.md).

## [0.6.6] - 2026-11-24

### Changed
- Controller layouts: **Modern** is the new default (A jump, B crouch with no down on the stick or D-pad, X action / strike, Y parry, RB / RT
  hold to grab); **Combat** (parry on LB / LT, down also on stick and D-pad) and **Classic** (the previous layout) are chosen on Controls >
  Controller (`pad_layout` in settings.json). The pad pictures show the chosen layout.

### Added
- Palace tapestries (wall hangings, VPALACE 1278 / 1280 / 1282, tops 1281 / 1283) right of every Doortop, chosen by its modifier.

- Palace lattice tiles use the lip image 1202 (record +8 = 2) instead of 243.
- `tools/room_shots/loose_floors.py` captures a loose floor shaking, falling and landing in the original (images 269..274 identified).

### Fixed
- The test `shift_next_to_a_potion_picks_it_up` skips without game data (it failed on GitHub CI); `tools/test_without_data.ps1` runs the suite
  as CI does and `tools/package.ps1` calls it first.

## [0.6.5] - 2026-11-23

### Fixed
- Level exit doors sat one column too far left: the original draws the frame, lintel, panel and stairs one column to the right of the
  exit cell (pixel evidence in `docs/level_render_check.md`).
- The dark doorway box of the exit doors now reaches back to the back wall, so no light passes between door frame and wall.

## [0.6.4] - 2026-11-22

### Fixed
- Potions and swords: with Shift (shift2) the kid reaches the item in front of him, also from standing (check_get_item `0000:655C`: step next
  to it, crouch, take it); the sim only looked for an item under a crouching kid, so a kid standing next to a potion did something else
  (found by the tick-trace comparison, test `shift_next_to_a_potion_picks_it_up`).
- Guards and the kid fell through the lattice pillars (tile type 25) of palace rooms such as level 4 room 2: `tile_is_floor` listed types 25..=28 as
  not floor instead of the original's 26..=29. Type 25 is floor again, type 29 (lattice right) is not; test `tile_is_floor_table_of_the_original`.
- Palace walls: the speckle strips are drawn in the dark tan the original shows (192, 152, 80), not the light cream of palette colour 6.
- Torches: holder, flame and light sit one column to the right of the torch's tile, as in the original's picture (found by comparing
  the original's room pixels, `docs/level_render_check.md`).

### Added
- `tools/tick_trace`: the original in DOSBox against the sim, tick by tick, with injected keys (`docs/DEVELOPING.md`); 38 of 39 random runs over
  13 levels match.
- Level checks against the original: invariants, golden hashes per room (`crates/pop-core/src/level_audit.rs`) and a comparison with the
  RAM of the original running in DOSBox, frozen before its first tick (`tools/level_ram`). Levels 1..14 match the original's level
  arrays: all tile types and modifiers (gates, buttons, loose floors, torches, swords, potions, spikes, exit doors, chompers, wall
  drawing bits), room links and start. See `docs/DEVELOPING.md`.

## [0.6.3] - 2026-11-21

### Changed
- Controls page redone as pictures with two tabs: Keyboard (the actions on the left, their key caps on the right, following the bindings of `settings.json`) and Controller (the project's own schematic drawing of an Xbox and a PlayStation pad, embedded in the exe from `crates/pop-app/assets/`, with the parts the game uses outlined in the colour of their action and named by callouts with leader lines). The drawing matches the connected controller (by USB vendor or name; Xbox when none is connected). Keys read as "←", "Shift", "Num 7" instead of Bevy's key names.

## [0.6.2] - 2026-11-21

### Added
- Settings, Video: "External Terminal" checkbox (`external_terminal`, off by default) opens a console window with the log at run time and closes it again. Without it no console window ever shows: curl, tar and certutil (downloads, updates) run without one too. Closing the console window with its X button ends the game.

### Changed
- New defaults for a fresh install (an existing `settings.json` keeps its values): rendering enhanced, characters voxel, volume 50 %, sound set AdLib (clips the AdLib set cannot make fall back to the synth), sun light low.

### Fixed
- Enter did nothing on the menu entries that start a game action (Start game, Quit game, Restart level, Save game, Quit to title, the version line, the SoundFont download button), and the note "GAME SAVED" vanished at once: the mouse handler overwrote the pending action every frame.
- The released exe (no console) could not start any download or update (curl failed with "invalid handle"): the helper programs now get explicit null handles.
- The update window reads Y and N by the letter on the key, not the key position (on a German keyboard Y did nothing).

## [0.6.1] - 2026-11-21

### Added
- Auto-update: at every start, before the game data is loaded, the app asks GitHub for the latest release. If a newer one exists, a window offers it (Y / N or the buttons). A yes downloads the zip with a progress bar and a Cancel button, checks it (size and, when GitHub gives one, the SHA-256), unpacks it, replaces the exe (the old one is renamed to `.exe.old` and removed at a later start) and the other released files, and starts the new version. Silent when offline or when the check fails. Only the released `PrinceOfPersia.exe` asks (not a `cargo run` build). Off with `"update_check": false` in settings.json or the environment variable `POP_SKIP_UPDATE`.
- No game data at start: the window offers to download the original files from the Internet Archive (https://archive.org/details/PERSIA_VGA). D asks, Enter confirms the licence sentence ("User confirms that they will comply with the licenses in accordance with the laws of their country."), Esc cancels. The archive is checked (size and SHA-1), unpacked, the game's `.DAT` files and PRINCE.EXE are copied to `Original_data` next to the exe, and the game restarts. Uses Windows' own `curl.exe`, `tar.exe` and `certutil.exe`.

### Changed
- Menus: the key hints at the bottom of every tab are gone, the panel has one fixed size (that of the Graphics tab, the largest), the "v" of a drop-down sits at the right edge of its field, and the font is Fixedsys when a vector font of that name is installed (Windows' own Fixedsys is a bitmap font that Bevy cannot draw), else Consolas or any monospace font.
- Menus: the main and pause menu entries are centered, and the last line shows the installed version; clicking it or pressing Enter opens the GitHub release page in the default browser.
- Menus: Enter confirms on every row: on the tab strip it goes into the tab, on a step or slider row it steps on (from the last value back to the first).

## [0.6.0] - 2026-11-20

### Added
- Sword sparks: when two swords clash, 20 tiny, glowing white-yellow sparks fly radially out of the point where the blades cross, fall on a ballistic path (at most half the prince's sprite height up) and stay on the floor the prince stands on. They burn out within 2 s; in enhanced mode each is a small light, together as strong as a torch at the start, fading exponentially.
- Potion bubbles give off a faint light in their own colour (enhanced mode). It casts shadows, so it does not shine through walls or slabs; only the two potions nearest to the view shine, and none with shadows off.

### Changed
- Torches flicker between 80 % and 100 % of their strength, following the visible pixels of the current flame frame, with smooth transitions.

## [0.5.3] - 2026-11-20

### Fixed
- The prince climbing the exit stairs no longer overlaps the door frame: he now walks between the stairs and the frame (sprite and 3D figure alike) and disappears behind the right post.

## [0.5.2] - 2026-11-20

### Added
- Settings, Graphics: "Sun light (enhanced)" (`sun_brightness` = `dim|low|medium|high`, applies live) so the torches can be the main light.

### Changed
- Every setting now applies at runtime, none needs a restart any more: tile textures, columns and objects (rooms and the princess chamber are rebuilt), voxel normals and shading, shadow quality and SSAO, and the sound set / SoundFont (the clips are re-rendered on a thread and swapped in). The "after restart" badges are gone. A tile palette that is no longer used stays cached for 5 minutes (switching back is instant), then its meshes, materials and images are freed.
- SoundFonts that are not downloaded cannot be picked (the drop-down marks them "not installed", the arrows skip them, and the "Soundfont" sound set needs one). A SoundFont that finishes downloading is loaded at once; deleting the one in use falls back to AdLib at once.
- Enhanced torch shadows: much smaller depth and normal bias, so torch light no longer leaks through thin floor slabs onto the wall behind them.

## [0.5.1] - 2026-11-20

### Changed
- Smaller release exe: Bevy default features trimmed to the ones the game uses (no glTF, animation, scene, picking, gizmos, Vorbis, KTX2, wayland/x11), `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`. The exe shrinks from 97.1 MB to 47.3 MB (zip 31.8 MB to 18.4 MB); screenshots are unchanged.

## [0.5.0] - 2026-11-20

### Changed
- The settings menu is built from widgets: tabs Video / Audio / Graphics / Downloads, a volume slider (5 % steps), a slider
  over the shadow quality, checkboxes (mute, SSAO), drop-down lists for the other options, a focus ring and "after restart"
  badges. Mouse support (hover, click, slider drag, wheel) next to keyboard (Q / E or PgUp / PgDn switch tabs) and controller
  (LB / RB switch tabs).
- The SoundFonts page is the Downloads tab: a row per font with size, licence line, status and buttons.

### Fixed
- The cross bars of the voxel gates are horizontal, parallel to the floor and the ceiling above and below the gate and
  perspectively correct: the oblique source drawing (rails rising 1 row per 2 columns) is un-sheared when the gate is voxelized
  (docs/texture_inventory.md 21).

### Added
- SoundFont downloads show a progress bar (MB and percent) with a Cancel button that kills curl / tar and removes the partial
  files; installed fonts can be deleted (SF2 and licence text, with a Yes / No question). Deleting the selected font keeps the
  setting and warns that music uses AdLib until it is downloaded again.
- Screenshot hooks `POP_MENU=video|audio|graphics|downloads`, `POP_MENU_SEL`, `POP_MENU_OPEN`, `POP_MENU_FAKE`.

## [0.4.6] - 2026-11-20

### Added
- Setting `motion_smoothing` = `position` (default) / `off` (the original's 12 Hz motion), env `POP_SMOOTH`, in the settings menu
  ("Motion smoothing", applies at once).

### Fixed
- Figures no longer slide on turns, climbs and landings: the 60 Hz interpolation of kid, guard and shadow works on the drawn
  centre of the image (so the climb, hang and drop are smooth too) and is skipped on a room change, a mirror flip (turn) or a
  jump of the drawn centre over 40 px.
- docs/edge_cases.md 7: a runner passes a closed gate at the room edge because the room change comes before the gate's bump (verified
  original behaviour, not a bug).

## [0.4.5] - 2026-11-20

### Changed
- Gates in enhanced mode are built from the original images (VDUNGEON / VPALACE 250, 251, 252..259) as a voxel barrier across the
  corridor, at the same place and depth as the old procedural grille, with the bars rising by the original draw rule (checked
  against a 2D composite); the procedural grille stays as fallback. Debug hook `POP_SET=gate=N`.
- The sword lying on the floor (level 1 room 15, levels 12 and 13) is a voxel built from the original images (PRINCE.DAT 160 / 161,
  glint animated from the shared animation frame) instead of a grey bar; placed by the original draw rule (docs/texture_inventory.md 21).

## [0.4.4] - 2026-11-20

### Fixed
- Hanging, dropping and climbing kid (sprite and voxel figures): the hang position is set relative to the ledge direction, so the
  hands grip the slab's end (0..2 px inside it) in both mirrorings; the figure is shifted 5 px away from a ledge to its left and
  2 px away from a ledge to its right (`hang_dx`, ramped in at the grab and out at the drop / end of the climb). The hands lie
  deeper on the ledge top. Test on the real sprite frames, both mirrorings.

## [0.4.3] - 2026-11-20

### Fixed
- Tile textures follow the original's drawing rules (PRINCE.EXE draw routines read, docs/texture_inventory.md 19, 20): the
  dungeon wall face comes from the neighbours (right / left wall, rooms at the edge count as walls) and the 3 px strip is the
  wall's base; the seeded wall marks (joints, cracks) are drawn; the palace wall uses the real per-room brick pattern; the
  palace arcade uses the right pieces (capital, left / right ends, middle, frieze) so the arches join seamlessly.
- Floor slabs are as thick as in the original: the 3-row lip (0.095 units) instead of 0.3, about 3 times too thick before.

## [0.4.2] - 2026-11-18

### Fixed
- Voxel characters: hanging, grabbing and climbing poses stood at the same depth as walking, so hands, head and body sank
  into the ledge slab and the neighbouring wall. These poses now stand in front of the block faces, with an eased
  (frame-rate independent) depth change at grab and drop. The hands of a hanging kid lie over the top of the ledge; while
  climbing up the figure leans back row by row so it ends standing on the slab, never inside it (a test checks the real
  sprite meshes, frames 78-99 and 135-149).
- Debug hooks `POP_WARMUP=n` (hold the simulation for n steps) and a freeze at `POP_SHOT_TICK` for exact tick screenshots.

## [0.4.1] - 2026-11-18

### Fixed
- Two music cues were missing (all 51 play_sound call sites of PRINCE.EXE checked, docs/audio.md 10): the level 1 start music
  (cue 25, played from the crouch frame after the opening fall, only when no sound is playing) and cue 41 on uniting with
  the level 12 shadow.
- The default `synth` sound set played invented melodies (a melody check against the speaker tunes and the MIDI files showed
  none of them matched the game). Every set except `original` now plays the real melodies from the MIDI files through the
  AdLib driver / OPL2 emulation; `synth` keeps only its procedural effects. Waits then follow the MIDI length.

### Changed
- SoundFont music: the 18 AdLib programs are mapped to General MIDI from an analysis of the original files (docs/audio.md 6.6:
  channels, track roles, decoded OPL patches) instead of the identity guess. Still unverified by ear.

## [0.4.0] - 2026-11-18

### Added
- Music from the original MIDI files (M7, docs/audio.md 6.4, 7, 8). Sound set `adlib`: the effects of the original, the
  music through a re-implementation of the original AdLib driver (traced in PRINCE.EXE: instrument layout of PRINCE.DAT
  resource 1, F-number table, 9 round-robin voices without stealing, velocity formula, sysex transpose; controllers and
  pitch bend are ignored like in the original) feeding the MIT-licensed `oplon` OPL2 emulator. `pop_core::midi` (SMF
  reader with a tempo map) and `pop_core::adlib` (SMF -> timed OPL register writes) are headless and tested.
- Sound set `soundfont`: the same MIDI files through `rustysynth` and a downloaded SoundFont. Settings > SoundFonts
  downloads MuseScore_General (SF2, 216 MB) or FluidR3_GM (from Debian's package, 120 MB) after showing the licence
  notice (both MIT; the source's licence text is saved next to the font); you are responsible for respecting the
  licences of the sources. Nothing is bundled. Needs Windows' own curl.exe and tar.exe. The mapping of the 18 AdLib
  instruments to General MIDI is a guess (`music::GM_MAP`).
- Own mixer with a peak limiter (-1 dBFS): every sound is mixed in one audio source, so overlapping sounds can no longer
  clip. Waits (title, cutscenes, death counter, stairs) follow the length of the clip that actually plays, i.e. the MIDI
  length in `adlib` / `soundfont`.

### Changed
- The sound set list is `synth`, `original`, `adlib`, `soundfont`; settings.json gets `soundfont` (`musescore` or `fluidr3`).
- Sounds stay polyphonic (the original's one-sound rule is not applied).

### Known limits
- The audio output was not listened to during development (unit tests only); the SoundFont instrument map is unverified.
- The original sequencer's tempo-byte swap (up to 7 % faster) is not reproduced; nominal tempo is used.

## [0.3.0] - 2026-11-18

### Added
- Hall of fame as in the original (PRINCE.EXE read: load_hof 34D0, save_hof 347B, show_hof 33F8, end_sequence 2F2F;
  docs/cutscenes.md 4.10): 6 entries ranked by time left, shown as M:SS; a finish that qualifies asks for a name (up to
  24 characters; controller: A / Start keeps "PRINCE") on the ending pages; the attract loop shows the list after the
  credits when it has entries. Stored as JSON `PRINCE.HOF` next to the exe (atomic write, damaged files tolerated).
- Menus (menu.rs logic, menu_ui.rs drawing): Esc / Start on the title opens a menu (start, load, settings, controls,
  hall of fame, quit; the attract loop holds while it is open); pausing opens the pause menu (resume, restart level,
  save on levels 3-13, settings, controls, quit to title, quit game). The settings page cycles the settings.json values
  with left / right / Enter and saves at once; rendering, characters, room change, window, volume and mute apply live,
  the rest after a restart (marked). Keys: arrows / numpad, Enter, Esc / Backspace; controller d-pad, A, B / Start.
  P still pauses and resumes directly. Keys pressed in a menu no longer reach the kid after resuming.
- Screenshot hook `POP_MENU=main|settings|controls|hof|name|attract`.
- Torch flames, potion bubbles and the glint of a sword on the floor are simulated as in the original (docs/tiles.md
  8.2-8.4): on entering a room every torch, potion and sword gets a random start frame and animates once per tick while
  its room is drawn, drawing from the shared random generator in the original order. `Level::anim_frame` exposes the
  frame; the app's flames and bubbles now show it instead of a fixed per-tile phase.
- Level 13: entering rooms 23 and 16 starts the loose floors of the room above, staggered at random (check_fall_flo).
- Tests: the animation stream against a replayed prandom, trobs following the drawn room, level-13 falling floors, the
  exit-stairs timeline (frames 217..228, the kid gone from tick 13, level end on tick 23), level tail and used_rooms.
- Smooth room-to-room camera (setting `camera_scroll` `smooth` default / `flip` = the original's hard cut, env
  `POP_CAMERA`): a 0.3 s eased, frame-rate independent pan from the old room to the new one across the rebuilt
  neighbour set (camera.rs); a change in the middle of a pan continues from where the camera is.
- Upside-down view (invert potion): the picture is mirrored top to bottom like the original's playfield buffer flip
  (0C83:19BD / 0000:15A0, VERIFIED), in classic and enhanced; HUD upright, controls unchanged (display only, as in the
  original). Implemented by rendering into an image shown flipped, only while the flag is set.
- Torches and potions in the neighbouring rooms at the margins animate (a cosmetic per-tile phase; the sim animates the
  drawn room only, which keeps its original frames).
- Test hook `POP_SET="start=ROOM:TILE,door=N,upside=1"` (with `POP_LEVEL`) for screenshots of the level-8 mouse and the
  upside-down view.
- Tests: pan easing and room steps, re-origin continuity, camera_scroll parsing, neighbour animation, the upside-down
  state (toggle, no control swap, cleared on death).

### Fixed
- The floor row of a missing room above now runs the full width also in the diagonal places, so the ceiling over the
  previous room does not vanish when the room set is rebuilt around the next room.
- Load-time modifier conversion covers rooms 1..used_rooms like the original, including its pseudo room 25 (zeroes one
  door-link byte on level 6, no gameplay effect); rooms above used_rooms keep their raw modifiers.

### Changed
- New-game random seed 0x1234567A (the original seeds from the clock); with the extra animation draws this seed keeps
  the attract-mode demo ending with the kid's win in room 24.
- Docs: level tail bytes, resource 2015, start position and the exit door verified against PRINCE.EXE (docs/tiles.md
  1.5, 8.1); ROADMAP M1/M3/M4 status lines and the idle-timer note in docs/cutscenes.md brought up to date.
- Enhanced auto fallback judges the p95 frame time of each 300-frame window against 1.25 refresh periods (counted at
  no more than 60 Hz, so 20.8 ms on 60 Hz and faster monitors) instead of the average, and needs 3 slow windows in a row,
  so a 30 fps vsync stutter is caught while a single hitch is not. Warm-up and the focus check stay; the log line names
  avg, p95 and the limit.

## [0.2.1] - 2026-11-18

### Fixed
- Background light-tight: floor slabs, wall blocks, plates and loose floors now reach into the back wall (they ended
  0.4 units in front of it), so torch and sun light no longer leak through the seams and the floors no longer float in
  front of the wall in angled views. Torch shadow maps use a near plane of 0.02 (was 0.1, exactly the gap to the slab
  above, which let the light through it in a wedge).
- No more black margins: places around the shown room with no room (outside the level) get a back wall and the level
  edge as the original draws it (floors in the row above, walls elsewhere), instead of the clear colour.
- Background pieces of the original: the faint wall patterns 244 / 245 / 324 / 325 and the lattice window 326 are drawn
  on the back wall from the left neighbour's Empty / Floor modifier, as draw_tile_right does (original tiles only;
  `docs/texture_inventory.md` 18).

## [0.2.0] - 2026-11-18

### Added
- R3 ray-tracing trial (experimental, not in the default build): `cargo build --features solari` makes
  `render_mode = "rt_experimental"` / `POP_RENDER=rt_experimental` render with Bevy Solari (ray-traced sun and torch light,
  bounce light; torches as emissive proxies, sprites and other alpha-masked images not traced; `POP_RESTIR=1` for ReSTIR;
  F5 cycles classic / enhanced / rt; falls back to enhanced on GPUs without ray queries). No denoiser (DLSS-RR needs the
  NVIDIA SDK), so the image is noisy; go/no-go in `docs/r3_solari.md` (no-go for a shipped RT mode for now). Without the
  feature `rt_experimental` still runs classic.
- Voxel characters (V3): rounder bodies (the depth profile is soft-clamped instead of capped, so the torso domes while
  limbs stay round), smooth normals (`voxel_normals` `smooth` default / `flat`, env `POP_VOXEL_NORMALS`), an unlit option
  (`voxel_shading` `lit` default / `unlit`, env `POP_VOXEL_SHADING`), and a short red hit flash on the figure that lost
  hit points. The front view still matches the sprites exactly; sprite style is unchanged (pixel diff 0).

### Changed
- Cutscenes: the princess chamber uses the palace texture mapping of the levels with `tile_textures` original (palace wall
  faces, floors, lattice arches, voxel or flat columns 295 for the arcade, torch holders
  346, voxel torch flames driven by the cutscene's torch frames), honouring `column_style` and `object_style`. Procedural
  tiles, composition, camera and timing unchanged (`docs/texture_inventory.md` 17).

### Fixed
- Voxel characters: the step faces turned away from the torches no longer show as dark stripes (smooth normals).
- Cutscenes: no more gap left of the bed carpet. Checked against PV.DAT: the bed image is transparent at x 64..73 because
  the room picture's left front column (10 px, down to the screen bottom) shows through it; both front columns are now
  built from the original front column image PV 952 at the measured x (64, 245), in every tile/column style.
- Cutscenes (enhanced): the actors' and the hourglass's shadows now fall on the bed carpet. The carpet (PV 981) was drawn
  unlit, and unlit materials receive no shadows; enhanced mode now draws it with a lit, matte copy of its material
  (classic unchanged, pixel diff 0).

## [0.1.0] - 2026-11-18

First release: a Windows x64 2.5D remake of the VGA *Prince of Persia* (1990) that reuses the original game logic and
reads the user's own game files at run time.

### Simulation (`pop-core`, headless and deterministic)
- M1 data: level files, tile types, door links, guards, image/palette decoder, frame tables and sequences read from
  `PRINCE.EXE` (2026-11-14).
- M2/M3: room viewer, 60 Hz timer with the original logic-step cadence, kid sequence player, ground controller, faithful
  collision pipeline with test vectors, wall collisions, room changes, grab/hang/climb.
- M4: runtime tile state and the tick machine (gates, plates, loose floors, spikes, chompers, exits).
- M5: guards, sword fight, shadow, skeleton, hit points; combat review fixes (shared refractory, notice flags,
  shadow kill/unite, landing, order of fallout).
- Edge cases from the `PRINCE.EXE` analysis (`docs/edge_cases.md`): 5/6-step frame length, guard falls, dead-guard
  pointers, bump/chomp rules, potions, death counter and restart, level exits, mouse, mirror shadow, demo mode.
- Audit fixes: per-clear records, exit/time-out order, latch reset, level-13 clock, mirror reflection, feather ticks;
  tests for formerly unverified points (85 tests).

### App (`pop-app`, Bevy)
- M6 game flow: title, pause, HUD, save/load (`PRINCE.SAV`), level progression, cards.
- M7 audio: sound catalogue, synthesized effects, optional original-sound mode, sim-driven playback.
- M8 release: data discovery (`POP_DATA`), `settings.json` with per-field fallback and `.bad` backup, gamepad support,
  release profile, `tools/package.ps1`, CI, README/LICENSE/PROVENANCE.
- Input: keypad mapping for sword strikes, flash overlay, sword shown on pickup frames.
- Intro and cutscenes in the 2.5D style (princess chamber renderer, attract loop); delta clamp, missing clips keep
  their length, no stale frame after skip.
- Camera: the current room fills about 80% of the window, neighbours show at the margins.

### Rendering
- R1: `render_mode` and `character_style` settings, capability log.
- V1/V2: voxel characters extruded from the original sprites; rounded, colour-aware volume with AO and jitter; voxel
  cutscene actors and all character types; figures placed inside the block depth (F6 toggles sprite/voxel).
- R2: enhanced render mode (F5): torch and sun shadows, SSAO + SMAA, bloom, warmer ambient, automatic fallback to
  classic when too slow.
- Original tile textures on the 3D room pieces (`tile_textures`), second-pass background mapping for walls, palace
  walls, chompers, spikes, exits, torches, balconies and lattices.
- Raised pressure plates, gates turned 90 degrees about Y, textured wobbling loose floors, voxel potions and skeleton
  floor; potions verified against `PRINCE.EXE`.
- Voxel columns (`column_style`) and voxel objects (`object_style`): spikes, chompers, exit doors, torch holders and
  flames, debris.

### Performance
- FPS overlay (F9 / `POP_FPS=1`), benchmark mode (`POP_BENCH`, `POP_RES`, `POP_FX`) and `tools/bench.ps1`.
- Enhanced mode: point-light shadow views are batched into one command encoder per light, and world changes rebuild
  only the changed tiles (torch lights persist). Room 13 at 1080p: 4.7 ms -> 2.8 ms; moving-gate demo: 12.8 ms and
  growing -> 3.1 ms flat, no memory growth. Image output unchanged. Details: `docs/enhanced_perf.md`.

### Documentation
- Analyses: mechanics, collisions, tiles, combat, edge cases, audio, cutscenes, controls, texture inventory,
  render roadmap (ray tracing, voxel characters), enhanced-mode performance.

[0.3.0]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.3.0
[0.2.1]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.2.1
[0.2.0]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.2.0
[0.1.0]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.1.0
