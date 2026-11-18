# Enhanced render mode: performance

Written 2026-11-18 against Bevy 0.20.1 / wgpu 30. Everything marked **MEASURED** comes from the benchmark mode below
on this machine; **ESTIMATED** numbers are extrapolations or come from the source/literature and say so.

Machine: AMD Ryzen 7 7700X (8 cores / 16 threads), NVIDIA GeForce RTX 3090, Vulkan, driver 621.24
(Windows 32.0.16.2124), Windows 11 Pro 26300, primary monitor 2560x1440 at 60 Hz. Release build (thin LTO).

## 1. Tools

- **F9 / `POP_FPS=1`**: overlay in the top-right corner, above the HUD (spawned on first use, refreshed every 10
  frames, nothing runs while it is off). It shows the smoothed FPS and frame time, avg / min / max / 1% low over the last
  120 frames, the render mode, the physical resolution, visible / total mesh entities, the point-light count, the active
  shadow casters (point + sun), and the GPU time per pass. GPU pass times need `RenderDiagnosticsPlugin`, which can only
  be added at startup, so they are there only when the app was started with `POP_FPS=1` (or `POP_BENCH`). The wgpu device
  already has `TIMESTAMP_QUERY` when the adapter offers it (Bevy enables every adapter feature with its default
  `WgpuSettingsPriority::Functionality`); on an adapter without it the overlay shows CPU frame times only. Measured
  overhead of the GPU timing: 0.4 ms/frame in enhanced (4.89 -> 5.28 ms, 1080p), so `POP_GPU_TIMES=0` turns it off for
  pure frame-time runs.
- **`POP_BENCH=<frames>`**: vsync off (`PresentMode::AutoNoVsync` = Immediate on this driver), `POP_BENCH_WARMUP`
  frames of warm-up (default 500, pipelines compile asynchronously), then records `<frames>` frame times and logs one line,
  then quits:
  `BENCH render=Enhanced chars=Sprite res=1920x1080 fx= frames=1000 avg_ms=5.276 p50=.. p95=.. p99=.. max=.. fps=..
  unfocused_frames=0 gpu_ms=3.092 gpu=[auxiliary_views=2.068 main_opaque_pass_3d=0.346 ssao=0.188 ...]`
  (`gpu=` = Bevy's GPU pass spans averaged over the last 120 frames). `POP_RES=WxH` sets the window size, `POP_RES=full`
  borderless fullscreen on the current monitor. Tried and working: 1280x720, 1920x1080, 2560x1440 and 3840x2160 (a 4K
  window larger than the 1440p monitor is created at full size; the BENCH line reports the physical size), `full` =
  2560x1440 here.
- **Background throttling**: a window started from a shell came up focused in every run here (`unfocused_frames=0`
  in all ~150 runs). The earlier 16.67 ms cap appears when the window loses focus (another window clicked) - the
  BENCH line counts unfocused frames so a throttled run is visible. What works: start it and do not touch the desktop
  until it quits.
- **`tools/bench.ps1`**: builds the release exe, runs a matrix (default: level 1 room 13, classic vs enhanced at the four
  resolutions plus enhanced with each feature off at 1080p; `-Quick` = 720p only; `-Configs @(@{...})` for your own), reads
  each BENCH line from `target/release/pop.log` and prints a table (`-Csv file` saves it).
- **`POP_FX`** (comma separated, enhanced only, for measurements; defaults change nothing): `no_torch_shadows`,
  `no_sun_shadow`, `no_ssao`, `no_bloom`, `no_smaa`, `no_hdr` (implies no bloom), `no_ambient` (classic fill light),
  `no_indirect` (`NoIndirectDrawing` on the camera, inherited by its shadow views), `lights=N` shadowed torches,
  `point_map=N` / `dir_map=N` shadow map sizes, `ssao=low|medium|high|ultra`, `filter=hw|gauss|temporal`, `rooms=1` (no
  neighbouring rooms, also in classic), `casters=boxes|none` (voxel room meshes / all room meshes stop casting; characters
  still cast), `core3d_shadows` (light shadow views back on Bevy's per-view `Core3d` schedule, i.e. without the fix in 5.2,
  for A/B runs). `POP_GPU_ALL=1` adds the nested GPU spans to the BENCH line. `<feature>=off` works as well as `no_<feature>`. Render-resolution scaling was skipped: Bevy 0.20's
  `MainPassResolutionOverride` is a render-world component meant for DLSS and does not upscale by itself; the resolution
  sweep below answers the same question (the mode is CPU-bound up to 1440p).

Classic is unchanged: `POP_SHOT` diff against the build before this work = 0 pixels on level 1 room 13; on level 2
room 1 (voxel characters) 7 pixels in one run and 0 in the repeat, and the new build differs from itself by the same 7
pixels between runs (timing), so the change adds nothing.

## 2. Measurements (MEASURED)

Frame time = average over 1000 frames after 500 warm-up, GPU timing on unless noted (adds ~0.4 ms in enhanced).

### 2.1 Classic vs enhanced, by resolution and scene (sprite characters)

| scene | mode | 720p | 1080p | 1440p | 4K | GPU ms 1080p |
|---|---|---|---|---|---|---|
| L1 room 13 | classic | 1.30 | 1.36 | 1.32 | 1.89 | 0.54 |
| L1 room 13 | enhanced | 5.36 | 5.32 | 5.48 | 6.09 | 3.13 |
| L3 room 6 (4 torches) | classic | 1.35 | 1.32 | 1.34 | 2.31 | 0.58 |
| L3 room 6 | enhanced | 5.25 | 5.43 | 5.33 | 6.37 | 3.35 |
| L5 room 13 (palace, 6 columns) | classic | 1.39 | 1.37 | 1.33 | 1.80 | 0.48 |
| L5 room 13 | enhanced | 5.24 | 5.46 | 5.47 | 6.37 | 3.21 |
| demo (level 0, guard fight) | classic | 1.39 | 1.41 | 1.41 | 1.51 | 0.34 |
| demo | enhanced | **11.19** (p95 16.8) | **11.19** (p95 17.0) | 10.71 | 10.87 | 4.62 |
| cutscene 2 | classic | 1.42 | 1.39 | 1.47 | 1.43 | 0.28 |
| cutscene 2 | enhanced | 3.62 | 3.60 | 3.57 | 4.26 | 1.90 |

Voxel characters (1080p): classic 1.31 / 1.32 / 1.34 / 1.40 / 1.40, enhanced 5.24 / 5.33 / 5.45 / 9.71 / 3.68 for the
same five scenes - no measurable cost over sprites (the demo number varies with the leak in 2.3).

Reading: enhanced costs ~4 ms more than classic and **does not scale with resolution** up to 1440p (5.36 -> 5.48 ms), while
the GPU is busy only ~3 ms of a 5.3 ms frame. The mode is **CPU-bound**; at 4K it starts to become GPU-bound (GPU 5.5 ms
of 6.1).

### 2.2 Cost per feature, level 1 room 13, 1080p (enhanced = 5.28 ms, classic 1.22 ms)

| change | frame ms | delta ms | GPU ms (shadow views) |
|---|---|---|---|
| none (enhanced high) | 5.28 | - | 3.09 (2.07) |
| `no_torch_shadows` | 1.91 | **-3.37 (-64%)** | 0.98 (0) |
| `lights=2` | 3.41 | -1.87 | 2.04 (1.04) |
| `lights=1` | 2.56 | -2.72 | 1.39 (0.46) |
| `no_sun_shadow` | 5.09 | -0.18 | 3.16 |
| `no_ssao` | 5.06 | -0.22 | 2.99 |
| `ssao=low` | 5.28 | 0.00 | 3.13 |
| `no_bloom` | 5.03 | -0.25 | 3.10 |
| `no_smaa` | 5.15 | -0.13 | 3.03 |
| `no_hdr` (and no bloom) | 4.89 | -0.39 | 3.02 |
| `no_ambient` | 5.35 | 0 (noise) | 3.14 |
| `point_map=512` / `2048` | 5.36 / 5.23 | 0 (noise) | 3.14 / 3.25 |
| `dir_map=1024` | 5.11 | -0.17 (noise level) | 3.15 |
| `filter=hw` | 5.19 | -0.09 | 3.09 |
| `rooms=1` | 4.18 | -1.10 | 2.38 (1.48) |
| `casters=boxes` | 4.90 | -0.38 | 3.03 (1.95) |
| `casters=none` | 4.07 | -1.21 | 2.40 (1.34) |
| `no_indirect` (GPU timing off: 5.01 -> 4.67) | | -0.34 | |

Cumulative: no torch + no sun shadows 1.70; + no SSAO 1.51; + no HDR/bloom 1.30; + no SMAA, classic ambient 1.30 (=
classic 1.22 + the 0.08 ms of the timing plugin). Run-to-run noise is about +-0.15 ms.

At 4K (enhanced 6.13 ms, GPU 5.56): `no_torch_shadows` -2.61, `no_ssao` -0.63, `no_smaa` -0.47, `no_bloom` -0.30,
`ssao=low` -0.22, `no_sun_shadow` -0.19, `point_map=512` -0.06. GPU at 4K: shadow views 2.13, main pass 1.46, SSAO 0.79,
SMAA ~0.5, bloom 0.26.

Candidate presets (1080p / 4K, GPU timing off): `lights=2,casters=boxes,ssao=low` 3.06 / 3.92 ms; `lights=0,ssao=low`
(sun shadow only) 1.59 / 3.23 ms; high 5.01 / 5.18 ms.

### 2.3 The demo: room rebuilds at 12 Hz and a leak with shadowed point lights (fixed, 5.3)

The demo runs at 11 ms average with p95 17 ms, and **gets slower the longer it runs** (`POP_PERF=1`, 720p, 300-frame
windows): 6.1, 7.0, 8.6, 12.0, 15.0, 18.7 ms. Causes found:

1. **The whole 9-room set is despawned and respawned on every logic tick in which a gate or door moves** (or a loose
   floor falls): `room::rebuild_on_change` watches `world.version`, which `sim.rs` bumps on every `set_mod` / trob
   update. The demo has a moving gate: ~12 rebuilds per second (5 per 300 frames in classic at 1.4 ms/frame). Classic
   absorbs this (1.4 ms flat); in enhanced each rebuild also respawns the torch `PointLight` entities, so the 4 shadow
   lights are new entities with new cube-shadow views every tick.
2. **Frame time and memory grow with every rebuild while point-light shadows are on**: private memory +8 to +15 MB/s
   (1.22 -> 1.44 GB in 12 s; classic flat at 688 MB), frame time ~+0.05 ms per rebuild with 4 shadowed torches. With
   `lights=0` (sun shadow only) or no shadows it stays flat at 2.0 / 1.9 ms; with `lights=1` memory still grows but frame
   time almost not (3.0 -> 3.1 ms). A static room (level 1 room 13, 0 rebuilds) is flat at 5.1-5.2 ms over 3000 frames.
   `no_indirect` does not stop it. Bevy prunes its per-view phases, caches and work-item buffers by live view
   (`light.rs` 2125, `gpu_preprocessing.rs` 1841), so the growing resource was not identified; it sits somewhere in the
   per-shadow-view resources of respawned point lights (Bevy or wgpu). A Chrome trace of the demo (built with
   `bevy/trace_chrome`) shows the render thread waiting in `submit_pending_command_buffers` for the finishing of the
   per-shadow-view command buffers (`early_gpu_preprocess`, `allocate_uniforms`, `unpack_bins`,
   `early_prepass_build_indirect_parameters`, `shared_shadow_pass`, ~18 views/frame, 136 command buffers per submit):
   each finish took ~1.2 ms in the demo against ~0.02 ms in the static room.

This is the most likely reason for "VERY SLOW": with vsync on a 60 Hz monitor, every frame over 16.7 ms is shown for
33 ms, so a demo or any room with a moving gate drops to 30 fps after a few seconds and keeps getting worse, while the
fallback (2.4) never fires at 60 Hz (threshold 20.8 ms).

### 2.4 Where the time goes (static room, 1080p)

- **Shadow views dominate**: 4 torches x 6 cube faces + 1 sun cascade = 25 extra views. Each torch costs ~0.85-0.95 ms
  of frame time (5.28 -> 3.41 -> 2.56 -> 1.91 for 4 / 2 / 1 / 0), of which only ~0.5 ms is GPU (shadow-view GPU 2.07 ms for
  4). The rest is CPU per view: Bevy records several compute passes plus a shadow pass per view, each in its own command
  encoder (trace above), plus visibility, specialisation and batching per view. Shadow-map **resolution costs nothing**
  measurable (512 vs 2048), the filter almost nothing.
- **Casters matter less than views**: removing all room geometry from the shadow passes saves 1.2 ms, the voxel objects
  alone 0.4 ms; only the current room (`rooms=1`) saves 1.1 ms (fewer casters in light range plus fewer main-pass
  meshes). The scene has only ~355 mesh entities (overlay), so entity count is not the CPU problem; views are.
- **Post-processing is cheap at 1080p**: SSAO 0.19 ms GPU (0.09 at 720p, 0.32 at 1440p, 0.77 at 4K), SMAA 0.2, bloom 0.1,
  HDR + bloom together 0.4 ms frame time. They matter only at 4K (SSAO + SMAA + bloom ~1.4 ms).
- **Presentation** is not a factor with vsync off (frame time = CPU time with Immediate present).

## 3. Levers (research, before section 5)

Sources: the Bevy 0.20.1 source in the cargo registry (paths below), measurements above, and:
[Qt Quick 3D shadow mapping](https://doc.qt.io/qt-6/quick3d-shadow-mapping.html) ("a point light ... renders its
shadowmap into a cubemap which means that it does six render passes. This can be quite computationally expensive"),
[three.js forum on point-light cost](https://discourse.threejs.org/t/point-lights-and-performance-revisited/49316),
[Bevy SSAO quality levels](https://doc.qu1x.dev/bevy_trackball/bevy_pbr/enum.ScreenSpaceAmbientOcclusionQualityLevel.html),
[Bevy batching / GPU preprocessing source](https://docs.rs/crate/bevy_render/0.18.0/source/src/batching/mod.rs),
[Unity HDRP AO notes on resolution cost](https://docs.unity3d.com/Packages/com.unity.render-pipelines.high-definition@14.0/manual/Override-Ambient-Occlusion).
Web sources give no Bevy-specific millisecond numbers; all numbers below are from this machine.

| # | lever | what it does here | gain | effort | look risk |
|---|---|---|---|---|---|
| A | **Persistent torch lights** (spawn the `PointLight`s outside `RoomEntity`, or keep a pool of 4 shadow lights moved to the nearest torches; despawn only on a room change) | stops new shadow views per tick, stops the leak | demo 11-19 ms (growing) -> ~5 ms (ESTIMATED from the static rooms); no growth | S-M (2-4 h) | none |
| B | **Incremental room updates** (update the gate / plate / loose-floor entity instead of rebuilding 9 rooms on `world.version`) | removes 12 Hz full respawns in every mode | same as A for enhanced, plus ~0 in classic (MEASURED: classic flat) | M (1 day) | none |
| C | **Fewer shadowed torches** (`lights`: 4 -> 2 / 1) | 12 / 18 fewer views | -1.9 / -2.7 ms (MEASURED, -35% / -52%) | S (settings exist) | medium: fewer torch shadows |
| D | **Sun shadow only** (`lights=0`) | 1 view instead of 25 | -3.4 ms (MEASURED, 1.6-1.9 ms total) | S | high: no torch shadows; the clear character shadows come from the sun and stay |
| E | **Trim shadow casters** (`NotShadowCaster` on voxel objects, back-wall boxes, neighbour rooms; or a smaller shadow `range` so neighbour rooms fall out of the light sphere: `check_point_light_mesh_visibility` culls by `range`, bevy_light/src/lib.rs 517) | fewer draws per view | -0.4 (voxel objects) to -1.2 ms (all room geometry) (MEASURED) | S-M | low-medium |
| F | **Cheaper torch shadow geometry**: one `SpotLight` facing into the room instead of a cube (1 view instead of 6; the torches hang on the back wall, the half behind is wall) | 6 -> 1 views per torch | ~-0.7 ms per torch (ESTIMATED from 0.85-0.95 ms per 6-view torch) | M (1 day incl. tuning) | medium: cone edge, no light/shadow behind the torch |
| G | **Static mesh merging** (one mesh per material per room) | fewer batches in 26 views | -0.3 to -1 ms (ESTIMATED: casters=none -1.2 ms is the ceiling for shadow passes) | L (2-3 days, animated pieces stay separate) | none |
| H | **HDR + bloom off** | no HDR target, no bloom chain | -0.4 ms (MEASURED 1080p), -0.3 ms bloom at 4K | S | medium: no flame glow |
| I | **SSAO off / low** | no prepass + SSAO compute | -0.22 / 0 ms 1080p, -0.63 / -0.22 ms 4K (MEASURED) | S | low-medium |
| J | **SMAA off** (or FXAA) | no AA pass | -0.13 ms 1080p, -0.47 ms 4K (MEASURED) | S | medium: jaggies (MSAA is off with SSAO) |
| K | **`NoIndirectDrawing`** on the camera | CPU-built draws in all views | -0.34 ms (MEASURED, image identical) | S | none |
| L | Shadow map size 512 / filter `Hardware2x2` | | 0 / -0.09 ms (MEASURED) | S | low |
| M | Greedy meshing of the voxel models | fewer vertices in shadow + main passes | < 0.2 ms (ESTIMATED: all voxel casters out of the shadow passes = -0.12 ms GPU) | M | none |
| N | Render-resolution scaling | | ~0 below 4K (MEASURED: CPU-bound), up to ~1 ms at 4K (ESTIMATED from the GPU passes) | M | blur |
| O | Shadow re-picking every frame | the nearest 4 are only re-picked when the camera moves (room change); no thrash measured in static rooms | 0 | - | - |
| P | `PresentMode` | vsync stays the default; with vsync, missing 16.7 ms halves the rate, so the goal is p95 < 16.7 ms, not avg | - | - | - |
| Q | GPU mesh preprocessing off (`PbrPlugin { use_gpu_instance_buffer_builder: false }`) | tried: 4.9 -> 2.4 ms, but **the 3D scene rendered black** (only the HUD), so the number is void and the experiment was removed | - | - | broken in 0.20.1 here |

## 4. Ranked by value (gain / effort)

| rank | option | gain | effort | basis |
|---|---|---|---|---|
| 1 | A: persistent torch lights | demo/gates 11-19 ms -> ~5 ms, ends the leak | S-M | MEASURED cause, ESTIMATED result |
| 2 | C: 2 shadowed torches by default (high = 4 stays) | -1.9 ms (-35%) | S | MEASURED |
| 3 | K + E (voxel objects not casting) | -0.34 and -0.38 ms | S | MEASURED |
| 4 | Fallback on p95 and refresh period, and to a lower preset first (4.4) | avoids the 30 fps vsync cliff | S | - |
| 5 | B: incremental room updates | removes 12 Hz respawns everywhere | M | MEASURED cause |
| 6 | F: spot lights for torches | ~-0.7 ms per torch | M | ESTIMATED |
| 7 | H / I / J at 4K only | -1.4 ms at 4K | S | MEASURED |
| 8 | G: merged room meshes | -0.3 to -1 ms | L | ESTIMATED |
| 9 | M, N, L | < 0.2 ms each below 4K | S-M | MEASURED / ESTIMATED |

### 4.1 Recommended plan (top 5)

1. Keep torch `PointLight`s alive across `rebuild_on_change` (spawn them per room change only, or a pool of N shadow
   lights moved onto the nearest torches); re-run the demo bench and `POP_PERF` to confirm a flat line.
2. Default `shadow_quality` medium = 2 torch shadows (new step between low and high), sun shadow on.
3. `NotShadowCaster` on the voxel room objects (spikes, chompers, exit doors, debris: their shadows fall mostly
   into the void or onto the wall right behind them) and `NoIndirectDrawing` on the enhanced camera.
4. Change the fallback (4.4).
5. Incremental tile updates for gates / plates / loose floors (helps classic too and makes rebuilds rare).

### 4.2 Quality ladder (expected 1080p frame time on this machine; a mid-range GPU will be slower at 1440p/4K)

| preset | settings | expected ms (1080p / 4K) |
|---|---|---|
| low | sun shadow only (0 torch shadows), SSAO low, bloom on, SMAA | 1.6 / 3.2 (MEASURED `lights=0,ssao=low`) |
| medium | 2 torch shadows, voxel objects not casting, SSAO low, bloom, SMAA | 3.1 / 3.9 (MEASURED `lights=2,casters=boxes,ssao=low`) |
| high | 4 torch shadows, SSAO medium, bloom, SMAA (today's enhanced) | 5.0 / 5.2 (MEASURED) |

All three after fix 1; without it any room with a moving gate grows past 16.7 ms within seconds in medium and high.

### 4.3 Not measured

- A mid-range or integrated GPU (only the RTX 3090; the AMD iGPU in this machine was not selected by wgpu).
- GPU time inside the shadow passes split per light (Bevy has one span for all shadow views, `auxiliary_views`), and
  CPU time per render system outside a trace build.
- The exact leaking resource (2.3), mesh merging, spot-light torches and greedy meshing (estimates only).
- Pipeline statistics (`PIPELINE_STATISTICS_QUERY`) and DX12.

### 4.4 Auto fallback

The current rule (300-frame average over max(12 ms, 1.25 refresh periods), once, to classic) misses the real failure:
the average can stay under 20.8 ms at 60 Hz while p95 is over 16.7 ms, which with vsync means 30 fps. Suggested: judge the
**p95** of the window against **one refresh period** (minus ~10% headroom) while vsync is on (12 ms with vsync off), keep
the focus check and the warm-up, and step down one preset (high -> medium -> low) before switching to classic, logging
each step. The overlay's 1% low shows the same number to the user.

## 5. Root cause and quality-neutral fixes (2026-11-18, second pass)

Constraint: no visual change at all (same lights, same shadows, same presets). Everything below is **MEASURED** on the
machine above, 1080p, release build, unless marked.

### 5.1 Root cause (evidence)

A Chrome trace (`cargo build --release --features bevy/trace_chrome`, level 1 room 13, 200 frames; trace spans inflate
times about 2x) showed the frame is the **render thread**: the main thread spends most of its frame waiting in
`RenderExtractApp` for it. Render thread 9.9 ms/frame (trace build), of which:

- `camera_driver` **5.1 ms**: Bevy 0.20 marks each point-light cube face as a `RootNonCameraView`
  (`bevy_pbr/src/render/light.rs` 2239) and `camera_driver` (`bevy_core_pipeline/src/schedule.rs` 136) runs the **whole
  `Core3d` camera schedule** for each of them: 24 runs/frame for 4 torches, 0.19-0.26 ms each, ~1390 multithreaded-executor
  spans per frame. Of the ~45 systems in `Core3d` only five do anything for a shadow view (`allocate_uniforms`,
  `unpack_bins`, `early_gpu_preprocess`, `early_prepass_build_indirect_parameters`, `shared_shadow_pass`; the real work of
  each is 0.01 ms); the rest are spawned on the executor and skip on parameter validation. The camera itself costs 0.35 ms.
  The directional (sun) shadow does not have this cost: it is drawn inside the camera's schedule (`per_view_shadow_pass`).
- `submit_pending_command_buffers` **2.15 ms**: each of the five systems records into its own command encoder, so a frame
  submits **136 command buffers** (`queue_submit count=136`, 1.43 ms) after finishing them in parallel (0.5 ms).
- Entity count, mesh merging, culling and specialisation are not the problem: `check_point_light_mesh_visibility` 0.13 ms,
  shadow queue/batching < 0.3 ms together, 355 mesh entities. GPU time was 3.1 ms of a 5.3 ms frame.

The demo's growth (2.3) has a separate cause: every gate tick despawned and respawned all 9 rooms, torch lights included, so
the 4 shadowed lights were new entities with new shadow views ~12 times a second.

### 5.2 Changes

1. **`enhanced::light_shadow_views`** (render app, `RenderGraph` schedule, `RenderGraphSystems::Render`, before
   `camera_driver`): calls exactly those five Bevy systems itself, per view in `Core3d`'s order and with its run condition
   (`any_match_filter`), through `SystemState`s, sharing **one command encoder per light** (6 faces), then removes
   `RootNonCameraView` from the views for this frame so `camera_driver` skips them (`prepare_lights` puts it back next
   frame). The GPU receives the same commands in the same order. Views with `OcclusionCulling` (never set here) are left to
   `Core3d`, since the late passes are not called. `POP_FX=core3d_shadows` turns it off for A/B runs.
   One pitfall found on the way: ordered only `.before(camera_driver)`, the system could run ahead of
   `update_sparse_buffers` (`RenderGraphSystems::Begin`) and drew changed meshes from last frame's buffers (68 pixels off in
   the demo, 0 in the static room); `in_set(RenderGraphSystems::Render)` fixed it.
2. **Incremental room updates** (`room::rebuild_on_change`): a level or room change still rebuilds the 9 rooms; a world
   change (gate, plate, loose floor, door, restart) respawns only the tiles whose `(kind, modifier, plate lift)` changed
   (loose floors without the modifier: they animate themselves) plus the falling slabs. Torch lights and everything else
   stay. Entities carry `TileOf(room, index)`; a room shown twice is handled. Test `tile_key_changes_only_with_the_look`.

Steps on room 13 (avg ms, GPU timing off, 2000-3000 frames): baseline 4.66-4.74; one slim single-threaded schedule per view
instead of `Core3d` 3.64-3.70; all views in one system with one encoder 3.41-3.44; one encoder per light 2.74-2.80 (one
encoder for everything was finished alone, ~1 ms in the trace; per view 2.82-2.97, per half light 2.73-2.74, all within
noise of per light). Final trace: `camera_driver` 0.35 ms, `light_shadow_views` 0.18 ms, `queue_submit count=17`.

### 5.3 Results: Classic / Enhanced baseline (git HEAD 890c254) / Optimized, 1920x1080

GPU timing off (`POP_GPU_TIMES=0`), 500 warm-up frames, average of 2 runs, window focused in every run
(`unfocused_frames=0`). Memory = process private bytes sampled every 0.5 s.

| scene | config | avg ms | p95 | p99 | fps | memory |
|---|---|---|---|---|---|---|
| L1 room 13 | Classic | 1.19 | 1.39 | 1.55 | 839 | ~945 MB, flat |
| L1 room 13 | Enhanced baseline | 4.66 | 5.21 | 5.52 | 215 | ~1.22 GB, flat |
| L1 room 13 | **Optimized** | **2.78** | **3.14** | **3.41** | **360** | ~1.13 GB, flat (10000-frame run: +0.2 MB/s) |
| demo (level 0, gate) | Classic | 1.25 | 1.44 | 1.58 | 801 | ~955 MB, flat |
| demo | Enhanced baseline | 12.8 (1500 frames) | 20.8 | 23.1 | 79 | +15 to +21 MB/s, peak 1.58 GB |
| demo | **Optimized** | **3.11** (3000 frames) | **3.56** | **3.92** | **322** | +0.1 MB/s, peak 1.04 GB |

- Demo baseline: 1500 frames, because in a 3000-frame run the watchdog switches it to classic (BENCH `render=Classic`,
  10 ms average). Per 300 frames (`POP_PERF`) it climbs 5.4, 6.2, 8.0, 11.3, 14.5, 18.4 ms; optimized stays at 3.0-3.3 ms
  over 3300 frames. Classic was measured with both builds: 1.17 / 1.19 ms (room 13), 1.25 / 1.25 ms (demo).
- Enhanced vs classic went from +3.5 ms to +1.6 ms in room 13. With GPU timing on: 3.46 ms frame, GPU 3.02 ms
  (`light_shadow_views` 1.98 ms, was `auxiliary_views` 2.06 ms): the GPU work is unchanged and the mode is now close to
  GPU-bound at 1080p on this machine.

### 5.4 Image identity (`POP_SHOT`, 1920x1080, diff over all pixels)

| shot | baseline vs optimized | baseline vs baseline (repeat) |
|---|---|---|
| room 13 classic | 0 pixels | - |
| room 13 enhanced | 0-2 pixels, max delta 1 | 3 pixels, max delta 1 |
| demo enhanced, tick 200 | 6-9 pixels, max delta 1 | 0 |
| demo classic, tick 400 | 2 pixels, max delta 1 | - |
| demo classic, tick 200 | 8 pixels up to delta 131 (6x2 px on the kid's hand edge) + 2 at delta 1 | 2 pixels, max delta 1 |

The classic tick-200 difference stays when the optimized build is forced to do full rebuilds again, so it does not come
from the incremental update; it sits on the interpolated kid sprite and is most likely the frame's interpolation phase
(start-up timing differs between the builds). Not proven. A demo shot at tick 400 cannot compare enhanced: the baseline
has fallen back to classic by then.

### 5.5 Not done, not measured, risks

- Not adopted: `NoIndirectDrawing` (-0.15 ms after the fix, CPU-built draws instead of GPU-driven), fewer lights / casters /
  presets (change the look), static mesh merging (the trace shows no per-entity cost worth it), Bevy source patches.
- What is left: on the GPU the shadow work is still 24 views x 5 small passes with barriers (~2 ms, mostly per-pass
  overhead, the depth draws themselves are tiny). Batching all views into one dispatch per stage needs changes inside
  `bevy_pbr`. Caching static shadow maps is not possible without Bevy changes either (the maps are re-rendered every frame).
- `light_shadow_views` depends on Bevy 0.20 internals: the five system signatures, `RootNonCameraView`, `camera_driver`
  and the `RenderGraphSystems` order. A Bevy update must re-check it (compile errors for signatures; `core3d_shadows` A/B
  plus a `POP_SHOT` diff for behaviour).
- The leak with respawned shadowed lights (2.3) is avoided, not fixed: a room change still respawns the lights (rare).
- Only this machine (RTX 3090, Vulkan), only 1080p, only room 13 and the demo.