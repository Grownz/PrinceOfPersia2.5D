# Prince of Persia x64

A Rust remake of the DOS Prince of Persia (1990) for Windows x64: a headless, tick-exact simulation (`crates/pop-core`)
and a Bevy 0.20 app (`crates/pop-app`) with procedural 2.5D rooms and the original sprites read from your own copy.

![Screenshot: the prince fights a guard in the enhanced render mode with voxel characters](docs/images/screenshot.png)

**You need your own copy of the game.** The original data is copyrighted and is not in this repository or in any
package. See [docs/PROVENANCE.md](docs/PROVENANCE.md). The VGA version this remake is based on is available at
the Internet Archive: <https://archive.org/details/PERSIA_VGA> (check the rights situation for your country).

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
`POP_SHOT=file.png` (screenshot after `POP_SHOT_FRAME`, default 400 frames, or at logic tick `POP_SHOT_TICK`, then quit), `POP_SCRIPT` (scripted keys),
`POP_MENU=main|settings|controls|hof|name|attract` (open a menu page or a hall-of-fame screen at once; the pause menu with `POP_LEVEL`), `POP_CUTSCENE=<level>|end|timeout|intro` (play that cutscene at once; `POP_REM_MIN` = minutes left for the hourglass), `POP_FPS=1` (frame-time overlay with GPU pass times; F9 toggles it), `POP_BENCH=<frames>` (benchmark: vsync off, warm-up, one `BENCH` log line, quit; with `POP_RES=WxH|full` and the `POP_FX` cost toggles; `tools/bench.ps1` runs a matrix, docs/enhanced_perf.md).

## Controls

Arrow keys + Shift; Esc pause menu (resume, restart, save, settings, controls, quit) and title menu; P pause; Ctrl+A restart; Ctrl+G save; Ctrl+L load; M mute; - / = volume; F11 fullscreen;
F3 debug; F5 classic / enhanced rendering; F6 sprite / voxel characters; F9 frame-time overlay. Controllers: d-pad/left stick, A = Shift, Start = pause, Back = restart. Full list and key rebinding:
[docs/controls.md](docs/controls.md).

Settings (window mode and size, volume, mute, sound mode, key bindings, data path, `render_mode`, `character_style`,
`shadow_quality`, `ssao`, `tile_textures`) live in `settings.json` next to the exe, created on the first run. `render_mode`: `classic`
(default) or `enhanced` (torch and sun shadows, ambient occlusion, bloom; any GPU; F5; `shadow_quality` `off|low|high` and
`ssao` `true|false` tune it), `rt_experimental` is reserved and falls back to classic; env `POP_RENDER`. `character_style`: `sprite` (default)
or `voxel` (3D voxel figures built at runtime from your own sprite files, nothing is written to disk), env `POP_CHARS`, F6. `tile_textures`: `original` (default: the tile images of your VDUNGEON.DAT / VPALACE.DAT on the room pieces, decoded at runtime; walls, chompers, spikes, exit doors, torches, balconies and lattices included, docs/texture_inventory.md 13) or `procedural` (plain coloured boxes), env `POP_TILES`. With `original`, the skeleton floor and the potions are voxel models built at runtime from the skeleton tile images and the PRINCE.DAT bottle and bubble images (independent of `character_style`; there is no separate item setting), bubbles glow in the potion's colour; `procedural` keeps simple boxes. `column_style`: `voxel` (default with `original`: pillars, big pillars and door posts as round voxel columns revolved from their images, the dungeon pillar as a square stone block, docs/texture_inventory.md 14) or `flat` (the image on a box), env `POP_COLUMNS`. `object_style`: `voxel` (default with `original`: spikes, chompers, exit doors, torch flames and debris as (animated) voxel objects, docs/texture_inventory.md 15) or `flat` (the flat images and boxes), env `POP_OBJECTS`. Voxel characters: `voxel_normals` `smooth` (default, rounded shading) or `flat` (per cube face), env `POP_VOXEL_NORMALS`; `voxel_shading` `lit` (default) or `unlit` (palette colours only, closest to the sprites), env `POP_VOXEL_SHADING`. `camera_scroll`: `smooth` (default: a short eased pan to the next room, neighbours visible at the margins) or `flip` (the original's hard cut), env `POP_CAMERA`.

## Package

```
powershell -ExecutionPolicy Bypass -File tools/package.ps1
```

Builds the release exe and creates `dist/PrinceOfPersia-x64/` (exe + README.txt) and `dist/PrinceOfPersia-x64.zip`.
It never copies `Original_data` or any game file.

## Docs

[ROADMAP.md](ROADMAP.md), `docs/` (mechanics, edge cases, collisions, combat, tiles, audio, controls, provenance).
Licence: see [LICENSE](LICENSE).
