# Prince of Persia x64

A Rust remake of the DOS Prince of Persia (1990) for Windows x64: a headless, tick-exact simulation (`crates/pop-core`)
and a Bevy 0.20 app (`crates/pop-app`) with procedural 2.5D rooms and the original sprites read from your own copy.

**You need your own copy of the game.** The original data is copyrighted and is not in this repository or in any
package. See [docs/PROVENANCE.md](docs/PROVENANCE.md).

## Game data

Put the files of your DOS installation (`LEVELS.DAT`, `PRINCE.EXE`, `KID.DAT`, `GUARD*.DAT`, `PRINCE.DAT`, `TITLE.DAT`,
sound `.DAT` files, ...) into a folder named `Original_data`. The app searches, in order:

1. `POP_DATA` (environment variable)
2. `Original_data` next to the exe
3. `Original_data` in the current folder
4. `"data_path"` in `settings.json`

If none has the data, the window lists the folders it searched (also on stderr / in `pop.log`).

## Build and run

```
cargo run -p pop-app                 # debug build (dependencies optimized)
cargo run --release -p pop-app       # release build (no console; log in pop.log next to the exe)
cargo test --workspace               # tests needing Original_data print "skipping" without it
```

Environment variables: `POP_DATA` (data folder), `POP_LEVEL` / `POP_ROOM` (skip the title), `POP_SOUND=original`
(the game's own sounds), `POP_PERF=1` (log frame time and room rebuilds every 300 frames, vsync off),
`POP_SHOT=file.png` (screenshot after `POP_SHOT_FRAME`, default 400 frames, then quit), `POP_SCRIPT` (scripted keys),
`POP_CUTSCENE=<level>|end|timeout|intro` (play that cutscene at once; `POP_REM_MIN` = minutes left for the hourglass).

## Controls

Arrow keys + Shift; Esc/P pause; Ctrl+A restart; Ctrl+G save; Ctrl+L load; M mute; - / = volume; F11 fullscreen;
F3 debug; F6 sprite / voxel characters. Controllers: d-pad/left stick, A = Shift, Start = pause, Back = restart. Full list and key rebinding:
[docs/controls.md](docs/controls.md).

Settings (window mode and size, volume, mute, sound mode, key bindings, data path, `render_mode`, `character_style`) live
in `settings.json` next to the exe, created on the first run. `render_mode`: `classic` (default; `enhanced` and
`rt_experimental` are reserved and fall back to classic for now), env `POP_RENDER`. `character_style`: `sprite` (default)
or `voxel` (3D voxel figures built at runtime from your own sprite files, nothing is written to disk), env `POP_CHARS`, F6.

## Package

```
powershell -ExecutionPolicy Bypass -File tools/package.ps1
```

Builds the release exe and creates `dist/PrinceOfPersia-x64/` (exe + README.txt) and `dist/PrinceOfPersia-x64.zip`.
It never copies `Original_data` or any game file.

## Docs

[ROADMAP.md](ROADMAP.md), `docs/` (mechanics, edge cases, collisions, combat, tiles, audio, controls, provenance).
Licence: see [LICENSE](LICENSE).
