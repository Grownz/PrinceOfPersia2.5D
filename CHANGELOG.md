# Changelog

All notable changes. Format: [Keep a Changelog](https://keepachangelog.com/); versions follow `Cargo.toml`.
The original game data is never part of the repository or a release; see `docs/PROVENANCE.md`.

## Unreleased

### Changed
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

[0.2.1]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.2.1
[0.2.0]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.2.0
[0.1.0]: https://github.com/Grownz/PrinceOfPersia2.5D/releases/tag/v0.1.0
