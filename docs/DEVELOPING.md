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
```

Bevy features are listed explicitly in `crates/pop-app/Cargo.toml` (`default-features = false`). If a Bevy feature is missing at
runtime (e.g. invisible text, wrong tonemapping), add it. Release uses fat LTO and `panic = "abort"` (the panic hook still writes `pop.log`).

Package: `powershell -ExecutionPolicy Bypass -File tools/package.ps1` builds the release exe and creates
`dist/PrinceOfPersia-x64/` (exe + README.txt) and `dist/PrinceOfPersia-x64.zip`. It never copies `Original_data` or any game
file. Release notes come from `CHANGELOG.md` (`tools/release_notes.ps1 v0.5.0 notes.md`).

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
`POP_MENU=main|settings|video|audio|graphics|downloads|controls|hof|name|attract` (open a menu page or a hall-of-fame screen at
once; the pause menu with `POP_LEVEL`; `POP_MENU_SEL=<row>`, `POP_MENU_OPEN=1` opens its drop-down,
`POP_MENU_FAKE=downloading:45|installed|confirm|delete|unpack` shows made-up SoundFont states without a download),
`POP_CUTSCENE=<level>|end|timeout|intro` (play that cutscene at once; `POP_REM_MIN` = minutes left for the hourglass),
`POP_PALETTE_TTL=<seconds>` (how long an unused tile palette stays cached, default 300), `POP_FPS=1` (frame-time overlay with GPU pass times; F9 toggles it), `POP_BENCH=<frames>` (benchmark: vsync off, warm-up, one
`BENCH` log line, quit; with `POP_RES=WxH|full` and the `POP_FX` cost toggles; `tools/bench.ps1` runs a matrix,
[enhanced_perf.md](enhanced_perf.md)).

## settings.json

Created on the first run next to the exe; unknown keys are ignored and missing ones take their defaults. Most keys are in the
in-game Settings menu (Esc), see [controls.md](controls.md).

- Window mode and size, `volume`, `muted`, key bindings, `data_path`.
- `sound`: `synth` (procedural effects, the original melodies from the MIDI files through the AdLib driver), `original` (the
  game's own digitized effects and PC-speaker tunes), `adlib` (original effects and music through the AdLib driver and an OPL2
  emulation) or `soundfont` (music through a downloaded SoundFont, `soundfont` = `musescore` or `fluidr3`); env `POP_SOUND`.
- `render_mode`: `classic` (default) or `enhanced` (torch and sun shadows, ambient occlusion, bloom; any GPU; F5; `shadow_quality`
  `off|low|high` and `ssao` `true|false` and `sun_brightness` `dim|low|medium|high` tune it); `rt_experimental` is reserved and falls back to classic; env `POP_RENDER`.
- `character_style`: `sprite` (default) or `voxel` (3D voxel figures built at runtime from your own sprite files, nothing is
  written to disk); env `POP_CHARS`, F6. `voxel_normals` `smooth|flat` (`POP_VOXEL_NORMALS`), `voxel_shading` `lit|unlit`
  (`POP_VOXEL_SHADING`).
- `tile_textures`: `original` (default: the tile images of your VDUNGEON.DAT / VPALACE.DAT on the room pieces, decoded at runtime;
  [texture_inventory.md](texture_inventory.md) 13) or `procedural` (plain coloured boxes); env `POP_TILES`. With `original`,
  the skeleton floor, the sword and the potions are voxel models built at runtime from their images (bubbles glow in the
  potion's colour).
- `column_style` `voxel|flat` (pillars and door posts, `POP_COLUMNS`) and `object_style` `voxel|flat` (spikes, chompers, gates,
  exit doors, torch flames and debris, `POP_OBJECTS`); both only with `tile_textures = original`.
- `camera_scroll`: `smooth` (default: a short eased pan to the next room) or `flip` (the original's hard cut); env `POP_CAMERA`.
- `motion_smoothing`: `position` (default: kid and guard drawn between the last two logic frames, 60 Hz motion, one logic frame
  / 83 ms of display delay) or `off` (the original's 12 Hz motion); env `POP_SMOOTH`.
