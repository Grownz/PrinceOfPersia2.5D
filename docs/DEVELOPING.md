# Developing and configuring

Technical notes moved out of the README: building, the environment variables for debugging and screenshots, and every
`settings.json` key.

## Layout

- `crates/pop-core`: headless, tick-exact simulation of the original game logic and the readers for the original file formats
  (no graphics, no audio). Tests that need the original data print "skipping" without it.
- `crates/pop-app`: the Bevy 0.20 app (rendering, voxel characters, audio, menus).
- `tools/`: `package.ps1` (release zip), `bench.ps1` (benchmark matrix), `disasm` (PRINCE.EXE disassembly helper),
  `sound_inventory` (lists the sound resources).
- `docs/`: mechanics, edge cases, collisions, combat, tiles, audio, controls, cutscenes, performance, provenance.
  [ROADMAP.md](../ROADMAP.md) has the plan.

## Build and run

```
cargo run -p pop-app                 # debug build (dependencies optimized)
cargo run --release -p pop-app       # release build (no console; log in pop.log next to the exe)
cargo test --workspace               # tests needing Original_data print "skipping" without it
tools\test_without_data.ps1          # the same without Original_data, as GitHub CI runs it (tools\package.ps1 does this first)
```

Bevy features are listed explicitly in `crates/pop-app/Cargo.toml` (`default-features = false`). If a Bevy feature is missing at
runtime (e.g. invisible text, wrong tonemapping), add it. Release uses fat LTO and `panic = "abort"` (the panic hook still writes `pop.log`).

Package: `powershell -ExecutionPolicy Bypass -File tools/package.ps1` builds the release exe and creates
`dist/PrinceOfPersia-x64/` (exe + README.txt) and `dist/PrinceOfPersia-x64.zip`. It never copies `Original_data` or any game
file. Release notes come from `CHANGELOG.md` (`tools/release_notes.ps1 v0.5.0 notes.md`). `-SkipTests` only builds and packs
(`package.ps1` runs the CI test run `tools/test_without_data.ps1` next to the build otherwise).

Hygiene: `tools/package.ps1` ends with `tools/hygiene.ps1` (`-SkipHygiene` to skip; not on CI): it removes git worktrees under `.claude/worktrees` that are
clean, unlocked, fully merged into HEAD and unused for 24 h (with their merged branch), and the incremental caches `target/*/incremental` (a debug build
recreates them; `-KeepCaches` keeps them). `-DryRun` only lists. Uncommitted or unmerged work is never touched; `target/debug/deps` is not cleaned (use `cargo clean`).

Release: bump the versions and `CHANGELOG.md`, run `tools/package.ps1`, commit and push `main`, then publish with the local zip:
`gh release create vX.Y.Z --target <commit> --title vX.Y.Z --notes-file notes.md dist/PrinceOfPersia-x64.zip` (this creates the tag).
`.github/workflows/release.yml` runs on the tag, sees the zip on the release and stops after a few seconds; without a zip it builds one
(`package.ps1 -SkipTests`, the tests ran in CI) with the dependencies from the cache that `release-cache.yml` keeps on `main`. Pushing the
tag before the release exists makes the workflow build and upload its own zip (about 10 minutes with the cache, 30 without).

## Checking the levels against the original

* `cargo test -p pop-core level_` (needs `Original_data`): `crates/pop-core/src/level_audit.rs` holds the invariants every
  regular level keeps (`hard`: links in range, guards and start valid, buttons hit gates or exits, every level has an exit),
  a printed report of oddities the shipped data itself has (`level_audit_report -- --nocapture`: one-way room links, rooms no
  link leads to), and golden hashes per level room in `level_golden.txt` (hashes only). A deliberate parser change updates them
  with `POP_BLESS=1 cargo test -p pop-core level_golden`. The hashes catch changes, not a mistake that was already there.
* `tools/level_ram/level_ram.py` compares the parsed levels with the original's RAM: it starts `PRINCE.EXE megahit N` in
  DOSBox 0.74-3, reads the emulated RAM of the DOSBox process (the level array sits at a fixed offset of the 16 MB RAM block,
  found once by a scan of level 1), freezes the process (`NtSuspendProcess`) as soon as the load-time conversion is done and
  before the first tick, and compares every tile type and every modifier (gates, buttons, loose floors, torches, swords, potions,
  spikes, exit doors, chompers, walls), the room links and the start position. Wall drawing bits are recomputed from the
  original's rule (`0000:B6DE`; `ds:3035` is 5 in all levels). Run `cargo run -p pop-core --example level_dump -- Original_data <dir>`
  first, then `python tools/level_ram/level_ram.py <Original_data> <dir> [levels]`.
  Accepted as the game's own start-up work (docs/tiles.md): torch flame frames 0..8, the potion bubble frame (low bits), the
  first exit door of the start room at 43 (`start_level_door`, or still 0 a moment earlier), and on level 13 the random fall
  delays of the floors above rooms 23/16 (`check_fall_flo`). Result: levels 1..14 match, two runs in a row.
* `tools/room_shots/all_rooms.py` takes a screenshot of every non-empty room of levels 1..14 from the original in DOSBox
  (`dosbox_shot.py` captures the window with `PrintWindow`, no focus or keys needed; the start room in a copy of `LEVELS.DAT`
  is set to the room, start tile = a floor tile, so rooms without floor are skipped). Output `tools/room_shots/out/L<level>_R<room>.png`
  at 320x200 (git-ignored, it is game imagery): the reference for a room-by-room picture comparison. The engine draws rooms as a
  3D scene, so a whole-room pixel comparison needs a flat 2D reference renderer first; that does not exist yet. Instead
  `learn.py` (with `tile_export`, `tiles_lib.py`, `calib.py`) finds where the original draws each image of each tile type; the result
  and the differences to the port are in [level_render_check.md](level_render_check.md). `render2d.py` / `compare2d.py` are a 2D model of
  the room drawing built on that (plus `learn_pieces.py`, `learn_textures.py`, `palette_learn.py`, `which_room.py`; the order and the
  results are in the document) and report how many screenshot pixels it explains per level and which rooms stay below 95 %.
  DOSBox leaves empty `stdout.txt` / `stderr.txt` in the working directory; delete them.

## Tick-trace comparison with the original (kid)

`tools/tick_trace` runs the original in DOSBox 0.74 tick by tick and replays the same keys in the sim. The original's data segment is
read from the DOSBox process (`ramlib.py`, same offset as `tools/level_ram`): the 60 Hz timer is `ds:3374` (a logic tick every 5
counts), the kid record `ds:4396`, the guard record `ds:431C`, the edge-triggered control flags `ds:42E8 / 40C8 / 4362 / 40DA / 4644`,
the prandom seed `ds:40C4`. Keys are injected by writing the key table (`ds:28A0 + scancode`; Shift through the BIOS flag at physical
`0040:0017`, MemBase = block + 0x20) at tick-aligned moments, so no focus or real key is needed (DOSBox's joystick is switched off:
a connected gamepad otherwise steers the kid).

* `record.py <Original_data> <level> "R:6,-:4,RU:1" out.json`: the kid stands (frame 15, action 0) after the level's start, then the
  script (segments of `KEYS:TICKS`, keys from `L R U D S`, `-` none) runs; every counter's kid / guard / seed / flags is stored.
* `compare.py out.json`: runs the ignored test `world_tests::trace_replay` (the sim is run to its first stand frame, its prandom state
  set to the original's after the stand tick, then the same keys) and compares frame, x, y, direction, row, action, room, fall,
  alive, sword and, with `GUARD`, the guard per tick. `run.py <level> <seed>` = seeded random script + record + compare,
  `batch.py <levels> <seeds>`, `run_guard.py <level> <guard> <seed>` (the kid starts in the guard's room, patched `LEVELS.DAT`),
  `show.py out.json <from> <to>` side by side with control flags. `POP_TRACE_ALIGN=ticks` uses the observed tick instants and keys
  (for a run with a slow tick: a room change can take longer than 5 counts in the emulation).
* Result: 38 of 39 runs (levels 1..6 and 8..14, three seeds of 150 ticks each: runs, jumps, safe steps, crouching, falls, room
  changes, picking up potions) match the original tick by tick. Known limits: level 7 starts with the kid falling to his death (no
  stand frame); level 13's loose floors get random delays at the level start from the original's clock seed (one run differs); the
  original restarts the level after the kid's death, the sim does not (the comparison stops there); guard fights: the first fight
  (level 2 room 4) diverges at tick 15 (the original's guard draws two more random numbers than the sim's), not resolved.

## Where the game data is searched

1. `POP_DATA` (environment variable)
2. `Original_data` next to the exe
3. `Original_data` in the current folder
4. `"data_path"` in `settings.json`

If none has the data, the window lists the folders it searched (also on stderr / in `pop.log`).

## Environment variables

`POP_DATA` (data folder), `POP_LEVEL` / `POP_ROOM` (skip the title), `POP_SOUND=synth|original|adlib|soundfont` (sound set),
`POP_PERF=1` (log frame time and room rebuilds every 300 frames, vsync off), `POP_SHOT=file.png` (screenshot after
`POP_SHOT_FRAME`, default 400 frames, or at logic tick `POP_SHOT_TICK`, then quit), `POP_SCRIPT` (scripted keys),
`POP_MENU=main|settings|video|audio|graphics|downloads|controls|controller|hof|name|attract` (open a menu page or a hall-of-fame screen at
once; the pause menu with `POP_LEVEL`; `POP_MENU_SEL=<row>`, `POP_MENU_OPEN=1` opens its drop-down,
`POP_MENU_FAKE=downloading:45|installed|confirm|delete|unpack` shows made-up SoundFont states without a download),
`POP_CUTSCENE=<level>|end|timeout|intro` (play that cutscene at once; `POP_REM_MIN` = minutes left for the hourglass),
`POP_SKIP_UPDATE=1` (no update check), `POP_UPDATE_AS=0.5.0` (pretend to be that version: offers the newest release, for testing), `POP_PAD=xbox|ps` (force the pad drawing on the Controls page, for screenshots), `POP_PALETTE_TTL=<seconds>` (how long an unused tile palette stays cached, default 300), `POP_FPS=1` (frame-time overlay with GPU pass times; F9 toggles it), `POP_BENCH=<frames>` (benchmark: vsync off, warm-up, one
`BENCH` log line, quit; with `POP_RES=WxH|full` and the `POP_FX` cost toggles; `tools/bench.ps1` runs a matrix,
[enhanced_perf.md](enhanced_perf.md)).

## settings.json

Created on the first run next to the exe; unknown keys are ignored and missing ones take their defaults. Most keys are in the
in-game Settings menu (Esc), see [controls.md](controls.md).

- `external_terminal` (`false` by default): a console window with the log (Settings > Video > External Terminal, applies at once).
- `update_check` (`true` by default): ask GitHub for a newer release at every start (released `PrinceOfPersia.exe` only), see `update.rs`.
- Window mode and size, `volume`, `muted`, key bindings, `data_path`.
- `sound`: `synth` (procedural effects, the original melodies from the MIDI files through the AdLib driver), `original` (the
  game's own digitized effects and PC-speaker tunes), `adlib` (original effects and music through the AdLib driver and an OPL2
  emulation) or `soundfont` (music through a downloaded SoundFont, `soundfont` = `musescore` or `fluidr3`); env `POP_SOUND`.
- `render_mode`: `classic` or `enhanced` (default; torch and sun shadows, ambient occlusion, bloom; any GPU; F5; `shadow_quality`
  `off|low|high` and `ssao` `true|false` and `sun_brightness` `dim|low|medium|high`, default `low`, tune it); `rt_experimental` is reserved and falls back to classic; env `POP_RENDER`.
- `character_style`: `sprite` or `voxel` (default; 3D voxel figures built at runtime from your own sprite files, nothing is
  written to disk); env `POP_CHARS`, F6. `voxel_normals` `smooth|flat` (`POP_VOXEL_NORMALS`), `voxel_shading` `lit|unlit`
  (`POP_VOXEL_SHADING`).
- `tile_textures`: `original` (default: the tile images of your VDUNGEON.DAT / VPALACE.DAT on the room pieces, decoded at runtime;
  [texture_inventory.md](texture_inventory.md) 13) or `procedural` (plain coloured boxes); env `POP_TILES`. With `original`,
  the skeleton floor, the sword and the potions are voxel models built at runtime from their images (bubbles glow in the
  potion's colour).
- `tile_relief` (off; Settings > Graphics, `POP_RELIEF=1`, `relief.rs`, needs `tile_textures = original`): the tile colour images enlarged 4x with
  MMPX on their palette indices (palette and single pixels kept, linear sampling) plus a normal map (palette brightness of the indices scaled 2x,
  blur, Sobel, `RELIEF_STRENGTH` in `tiles.rs`) lit by the torches. Measured in the dev build (atlas included): tile textures 9 MB RGBA and 37 ms to load
  without, 156 MB and 1.3 s with (no mip maps); frame time on a palace level 12.6 ms plain, 13.5 ms with. [tile_filters.md](tile_filters.md) has the filter comparison.
- `POP_MIPS=<levels>` (test only, default 1 = no mip maps, at most 4): mip levels of the enlarged tile textures, built on the CPU. The tile
  images live in an atlas per tile set (`atlas.rs`, [tile_filters.md](tile_filters.md)); to add a texture call `texture(w, h, indices,
  palette, opaque)` in `TileSet::load` as for the others, the atlas grows by itself and the gutters keep neighbours out.
- `POP_UPSCALE` (test only, ignored with `tile_relief`): `nearest` (default), `epx`, `mmpx` or `sharp` enlarge the tile colour images 4x and
  sample them linearly.
- `column_style` `voxel|flat` (pillars and door posts, `POP_COLUMNS`) and `object_style` `voxel|flat` (spikes, chompers, gates,
  exit doors, torch flames and debris, `POP_OBJECTS`); both only with `tile_textures = original`.
- `camera_scroll`: `smooth` (default: a short eased pan to the next room) or `flip` (the original's hard cut); env `POP_CAMERA`.
- `motion_smoothing`: `position` (default: kid and guard drawn between the last two logic frames, 60 Hz motion, one logic frame
  / 83 ms of display delay) or `off` (the original's 12 Hz motion); env `POP_SMOOTH`.
- `quick_input` (on by default, Settings > Video): `World::tick_now` lets a tick start after 2 of its 5 steps when a direction key arrives while the kid stands
  (frame 15, action 0, no sword); 3-tick cooldown. Measured in the sim: a key changes the pose on the tick that reads it (`input_latency_in_ticks`,
  ignored test), so the wait for the tick is the delay it removes. `low_latency_present` (off, restart): `AutoNoVsync` and `desired_maximum_frame_latency` 1.
