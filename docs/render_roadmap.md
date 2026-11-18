# Render roadmap: ray-tracing mode (A) and voxel characters (B)

Status: R1, R2, V1 and V2 implemented (see section 9); the rest is plan. Written 2026-11-15 against Bevy 0.20.1 (released 2026-10-08) and wgpu 30.
Effort sizes assume one developer who knows this codebase and Bevy: S = up to 1 person-week (pw), M = 2-4 pw, L = 4-8 pw,
XL = more than 8 pw. Risk = chance that the estimate or the result is badly off.

## 0. Starting point (what the code does today)

- Rooms: procedural boxes (`room.rs`, one shared `Cuboid` mesh, `StandardMaterial` with roughness 0.9), torch flames are an
  emissive material plus a `PointLight` each (`room::torch_light`, `shadow_maps_enabled: false`), one `DirectionalLight`
  without shadows, `AmbientLight` on the camera (`main.rs::setup`). Forward rendering, no SSAO/SSR/bloom.
- Characters, swords, cutscene sprites: one unit `Rectangle` quad per entity, a cached `StandardMaterial` per (image,
  palette, tint) with `unlit: true`, `AlphaMode::Mask(0.5)`, nearest sampling (`kid.rs::material_for`, `cutscene.rs`).
  The quads therefore ignore all lights today. Placement follows docs/mechanics.md 7.3 (`world_pos`, `screen_x`):
  image bottom line and left/right edge from the frame table, mirrored by a negative x scale when facing right, position
  interpolated with `World::alpha()` between logic ticks, the image itself switches at tick rate (12 Hz, 10 Hz in fights).
- Performance: about 1.4 ms/frame uncapped in release (ROADMAP M8). Lots of headroom.
- Tests/CI: `cargo test --workspace` on GitHub runners without a GPU; `POP_SHOT`/`POP_SCRIPT` already give scripted
  screenshots for manual comparison.

Consequence for both goals: everything visual is in `pop-app`; the simulation, collision boxes and timing in `pop-core`
stay untouched. Both goals are "visuals only".

## 1. Goal A: optional ray-tracing mode

### 1.1 State of the technology (November 2026)

| Topic | Finding | Confidence |
|---|---|---|
| wgpu ray tracing | Ray *queries* (inline RT from any shader) behind `Features::EXPERIMENTAL_RAY_QUERY`; BLAS/TLAS API; "may have major bugs ... subject to breaking changes"; no partial AS updates (a TLAS is rebuilt after any BLAS change). RT *pipelines* (raygen/closest-hit) are in development (WGSL/SPIR-V work in v29/v30 and unreleased). | high (wgpu docs, changelog) |
| Backends | Vulkan: the established path. Metal: works since Bevy 0.20 (Solari runs on macOS). DX12: I found **no** source that confirms ray queries on wgpu's DX12 backend; treat as "unknown, assume Vulkan". | Vulkan high, DX12 low |
| Bevy Solari | `bevy_solari` (Cargo feature `bevy_solari`), "experimental", realtime ReSTIR + world-cache path tracer plus a non-realtime reference path tracer. Requires `EXPERIMENTAL_RAY_QUERY`, buffer/texture binding arrays, non-uniform indexing, partially bound arrays. Entities need `RaytracingMesh3d` + `MeshMaterial3d<StandardMaterial>`; the mesh must have `enable_raytracing`, exactly `{POSITION, NORMAL, UV_0, TANGENT}`, `TriangleList`, `Indices::U32`. | high (crate source v0.20.1) |
| Solari lights | `DirectionalLight`, **emissive meshes**, and (new in 0.20) `Atmosphere`/`EnvironmentMapLight`. **Point, spot and rect lights are not supported** ("hopefully in the near future"). ReSTIR is optional and off by default since 0.20. | high (0.20 release notes) |
| Solari materials | 0.17: diffuse only; 0.18: specular/reflections added. Alpha-masked or transparent materials: not documented; the official example forces `AlphaMode::Opaque` and zero transmission. | medium |
| Denoising | Only DLSS Ray Reconstruction (NVIDIA RTX, `dlss` feature, the NVIDIA DLSS SDK must be downloaded and set up at build time; 0.20 uses DLSS-RR 4.5). DLSS in Bevy "requires running via the Vulkan backend on Windows/Linux" (0.17 notes). **No built-in denoiser otherwise** (stated for macOS in 0.20; AMD/Intel on Windows are in the same position). AMD FSR Ray Regeneration exists but is not integrated in Bevy. | high |
| Maturity | The Solari author's own posts list open problems: light leaks, popping at world-cache LOD boundaries, BRDF not passing the furnace test, unhappy with the world cache. API changed every release (0.19 rework, 0.20 ReSTIR opt-in). docs.rs failed to build the 0.20.1 docs. | high |
| Raster alternatives built into Bevy | SSAO, SSR (deferred only, on-screen only), contact shadows (screen-space raymarch; seen in Bevy source, check that it is in 0.20 before planning on it), shadow maps for point/directional lights, bloom, volumetric fog. | medium |

### 1.2 Options

| # | Option | What it gives this game | Effort | Risk |
|---|---|---|---|---|
| A0 | **Enhanced raster** (no RT): torch point-light shadow maps (cube maps, 2-4 torches per room), SSAO, contact shadows, bloom on flames, light volumetric fog, lit characters (see B or a lit sprite material) | soft-ish torch shadows, contact darkening in corners, glow; runs on every GPU | S-M (1-2 pw) | low |
| A1 | **Bevy Solari** as an opt-in mode (Vulkan, RT hardware) | real GI: torch light bouncing off floors/walls, soft shadows from flame area lights, glossy floor reflections | spike M (2-3 pw), production L (5-8 pw incl. A0 work it shares) | high: experimental, API churn per Bevy release, no point lights, NVIDIA-only denoiser |
| A2 | **Own hybrid ray-query pass** (compute shader with wgpu ray queries over our own BLAS/TLAS: RT shadows + RT AO only, own temporal denoiser, composited onto the raster image) | crisp-to-soft shadows and AO, keeps point lights and the current materials | L (5-8 pw) | high: bypasses Bevy's renderer internals (render graph code changes each release), own denoiser |
| A3 | **SDF / grid ray marching** (software, any GPU): the level is a 10x3 tile grid of boxes per room, so an exact SDF or a coarse 3D occupancy texture of the visible rooms is cheap to build; march it in a fullscreen pass for soft torch shadows and AO | soft shadows/AO that look RT-like, no RT hardware, no denoiser needed for shadows | M-L (3-5 pw) | medium: custom render-graph node; characters must be added as simple proxies |
| A4 | **Own path tracer / voxel cone tracing** in compute shaders, or raw Vulkan (ash) next to Bevy | full control | XL (10-20+ pw) | very high, duplicates Solari; raw Vulkan beside wgpu means a second device/swapchain or unsafe interop |

Notes on "via Vulkan": in this stack the realistic Vulkan route is wgpu's Vulkan backend (A1/A2). Writing raw Vulkan is A4
and not recommended. On Windows, wgpu can be forced to Vulkan (`WGPU_BACKEND=vulkan` or Bevy's `WgpuSettings::backends`).

### 1.3 Specific problems this game creates for Solari

1. **Torches are point lights.** Solari ignores them. The flame meshes are already emissive (`emissive: 8, 3, 0.5`), so in
   RT mode the flame becomes the light source (emissive mesh lights are Solari's strong case, and they give soft shadows
   for free). Needs tuning of emissive strength and maybe a slightly larger invisible emissive proxy per torch. Without
   ReSTIR, many small emitters give noisier shadows: test with `SolariLighting::restir` on.
2. **Sprite quads are alpha-masked and unlit.** If they get `RaytracingMesh3d` and Solari treats them as opaque, every
   character casts a rectangular shadow. Open question (no source): what Solari does with alpha-masked `StandardMaterial`
   and with meshes that have no `RaytracingMesh3d` (are they lit from the G-buffer but invisible to rays?). Goal B solves
   this cleanly: voxel characters are opaque geometry.
3. **Mesh format.** `Cuboid`/`Rectangle` meshes need tangents generated and U32 indices; trivial, done once at startup.
4. **Denoiser.** Without DLSS-RR the realtime output is noisy. Realistic target for the RT mode: NVIDIA RTX on Vulkan.
   The DLSS SDK is a build-time dependency (download, licence terms to check for redistribution of its DLLs in the zip).
5. **Things that must keep working:** the background flash (`World::flash`, UI overlay, unaffected), fades in cutscenes,
   the cutscene chamber renderer, HUD, F3 debug text, room rebuilds on scroll (BLAS for the shared cube mesh is built
   once; TLAS rebuild per frame is fine for a few hundred instances).

### 1.4 Recommendation for A

Do **A0 first, then A1 as an opt-in spike**, both behind a render abstraction (R1). Reasons: A0 delivers most of the
visible gain (shadows from torches, AO, glow) on all hardware at low risk and is also the fallback that RT mode needs
anyway. A1 is the only RT path where someone else maintains the hard parts (BLAS/TLAS, ReSTIR, DLSS-RR); its scene is
tiny, so performance is not the issue, maturity is. A2 and A4 are not worth it for this project. A3 is the fallback plan
if Solari stays unusable (no point lights, no denoiser on AMD/Intel) after the spike.

Verdict for A: **moderate effort for a decent "enhanced" look (S-M), large effort for a robust RT mode (L), with high
platform risk**: realistically "Windows + Vulkan + RT-capable GPU, good quality only with NVIDIA DLSS-RR".

## 2. Goal B: voxel characters

### 2.1 Constraints from the original data

- The original is image-based: each frame is one hand-drawn bitmap (kid 219 images, guard 34, fat guard 34, skeleton 28,
  vizier 34, shadow = kid images tinted, princess/vizier/mouse in PV.DAT ~102, mouse also uses kid images 130..132).
  Kid frames ~12-20 px wide, ~40 px tall in a 320x200 space. **No limb positions, no depth, one view only.**
- A visual hull needs several views; from one view it is impossible. Any depth is invented: by rule, by hand or by a model.
- Colours are palette indices: guard colours are palette swaps (PRINCE.DAT resource 10), palace/dungeon palettes, shadow
  tint. A voxel pipeline should keep the palette index per voxel so these stay cheap.
- Animation: one image per logic tick (12 Hz, 6-step frames in fights), position interpolated at 60 Hz. Swapping a voxel
  mesh per tick is the faithful equivalent of swapping a sprite.

### 2.2 Options

| # | Option | Description | Look | Effort | Risk |
|---|---|---|---|---|---|
| B1 | **Extruded sprites** | per image: every opaque pixel becomes a column of voxels; thickness from a depth heuristic (constant N, or a distance transform from the silhouette edge for a rounded "pillow" body, capped); front and back faces get the pixel colour; normals from the voxel faces (or smoothed from the depth field for a softer look) | "thick pixel art": exact from the front, cardboard-like from steep angles; our camera is near-frontal (slight top-down tilt), so it holds up well | M (2-4 pw) incl. all characters | low |
| B2 | **Hand-built voxel rig** | 10-14 voxel parts per character (head, torso, arms, legs, sword hand), posed per original frame; frame -> pose mapping authored in a pose editor that overlays the sprite | real 3D, can rotate/light properly, could interpolate between poses | XL (15-30 pw; ~370 poses for kid + guard + cutscene tables, 7 characters) | high: manual work, poses must still match the original timing/silhouettes exactly or the game "feels" wrong; 2D pose estimation on 40 px pixel art is not reliable |
| B3 | **AI depth estimation** (offline tool) | upscale each sprite, run a monocular depth model (e.g. Depth Anything V2 Small, Apache-2.0; Base/Large are CC-BY-NC), quantise the depth to voxel slabs, mirror for the back | more relief than B1 where it works | M-L (3-5 pw) | high: models are trained on photos, not 15x40 px pixel art; depth flickers between frames; adds an ML runtime (ONNX) or a separate tool the user must run |
| B4 | **Hybrid** | B1 as the runtime baseline + small authored depth rules (per palette index and per image region: e.g. "far arm/leg -1 layer, sword arm +1") for the frames that matter, optionally seeded by B3 as a hint | B1 quality plus layered limbs | B1 + M (2-4 pw authoring tool and rules) | medium |

Published tools confirm B1 as the standard technique (Kenney Shape and PixtoMesh: per-pixel depth on pixel art;
Voxelorama: per-layer depth; several Unity/GameMaker sprite-to-voxel generators). A 2025 TU Delft study of 2D-pixel-art to
voxel reconstruction scored gradient-based depth estimation (0.61) below silhouette/carving/hybrid methods (0.71-0.75),
but those need more than one view; it supports keeping B3 as a hint, not the base.

### 2.3 Rendering in Bevy

- **Meshing:** per image a small grid (about 20 x 45 x 8 voxels). Plain face culling (emit a quad only where the
  neighbour is empty) is enough; greedy meshing (`block-mesh` crate, `greedy_quads`, or `binary-greedy-meshing`) cuts the
  triangle count further but is optional at this size. All ~480 images mesh in well under a second at load; build lazily
  and cache like `material_for` does today.
- **Colour:** keep the palette index per voxel. Either one `StandardMaterial` per palette variant with a 16x1 palette
  texture and UVs pointing at the index (works with Solari, which requires `StandardMaterial`), or a small
  `ExtendedMaterial` doing the palette lookup, which also covers the shadow tint and hit flash without remeshing.
- **Mirroring:** the existing negative x scale; check that back-face culling/winding is correct with a negative
  determinant in Bevy 0.20, otherwise build mirrored meshes (cheap) or keep `cull_mode: None`.
- **Anchoring:** unchanged formula from docs/mechanics.md 7.3; the voxel mesh origin is the image's bottom-left pixel,
  centred in depth on today's sprite plane (z 0.6, sword 0.65). Collision boxes, hit detection, positions all stay in
  `pop-core`.
- **Animation timing:** mesh swap at tick rate, position interpolation with `World::alpha()` exactly as now. No pose
  interpolation in B1 (it would alter the original look; B2 could offer it as an option).
- **Sword:** voxelise PRINCE.DAT images 701+ the same way; placement from the sword table as now.
- **Lighting and shadows:** voxel characters can be lit (`unlit: false`), cast shadow maps (A0) and appear in Solari (A1)
  without the alpha-mask problem. Keep an "unlit" switch for players who want the original colours.
- **GPU-raymarched voxel volumes** (3D textures traced in a shader) are possible but unnecessary for objects this small;
  meshes are simpler and work with every renderer path.

### 2.4 Recommendation for B

**B1 first, then B4** if the cardboard look is not enough. B2 only as a separate, later decision (it is a content
project, not a code project). B3 at most as an optional offline hint generator.

Verdict for B: **B1 is a medium task (2-4 pw) with low risk; true articulated 3D characters (B2) are very large (XL) and
mostly manual authoring work.**

## 3. Architecture changes

1. **Render abstraction (`pop-app`):** a `RenderMode` resource (`Classic` = today, `Enhanced` = A0, `RayTraced` = A1) and a
   `CharacterStyle` (`Sprite`, `Voxel`). Character drawing keeps one function computing position/flip/image
   (`draw_char`), with the "visual" behind it swappable (quad + material vs voxel mesh). Room building gets one place that
   applies per-mode extras (tangents/`RaytracingMesh3d`, emissive torch proxies vs point lights).
2. **Settings:** `settings.json` gains `"render": "classic|enhanced|rt"` and `"characters": "sprite|voxel"`; unknown or
   unsupported values fall back to `classic` (same strict style as the save parser). Optional `POP_RENDER` env override
   for tests.
3. **Feature flags:** Cargo features in `pop-app`: `rt` (enables `bevy/bevy_solari`), `dlss` (enables `bevy/dlss`, needs the
   SDK; never default). Default build unchanged.
4. **Capability detection and fallback:** wgpu features must be requested at device creation, and requesting an
   unsupported feature fails. So before building the Bevy app: create a wgpu instance (Vulkan), enumerate adapters,
   check that `SolariPlugins::required_wgpu_features()` is a subset of the adapter's features; only then add the Solari
   plugins and request the features (force the Vulkan backend for this mode). Otherwise log the reason and run `enhanced`.
   Exact Bevy 0.20 hook for this is to be confirmed in the spike.
5. **CI:** keep the default job; add `cargo build -p pop-app --features rt` (compile check only; GitHub runners have no
   RT GPU). Voxeliser unit tests run on CPU. GPU screenshot tests stay local or on a self-hosted GPU runner. No DLSS in CI.

## 4. Phased roadmap

### Ray tracing (A)

| Milestone | Content | Effort | Acceptance criteria | Verification |
|---|---|---|---|---|
| **R1** Renderer abstraction + settings | `RenderMode`, settings key, env override, `rt` feature compiling, capability probe stub logging adapter name/backend/RT features | S (1 pw) | `classic` is pixel-identical to today; unsupported mode falls back with a log line | `POP_SHOT` before/after diff = 0 on 3 fixed scenes (level 1 start, a torch room, a cutscene) |
| **R2** Enhanced raster (A0) | torch point-light shadows, SSAO, contact shadows if present in 0.20, bloom, optional fog; lit character material option | S-M (1-2 pw) | frame time < 4 ms at 1080p on a mid-range GPU; no shadow acne on tiles; HUD/flash/fades unchanged | screenshot set reviewed; `POP_PERF=1` numbers logged in the doc |
| **R3** Solari spike (time-boxed 2 weeks) | `rt` mode: tangents + U32 indices + `RaytracingMesh3d` for rooms, emissive torch proxies, characters left out or opaque; DLSS-RR if available | M (2-3 pw) | runs on one RTX GPU at 60 fps 1080p; go/no-go written down: noise level without DLSS, sprite handling, light leaks | side-by-side screenshots vs Solari's reference path tracer (`PathtracingPlugin`) |
| **R4** RT mode production | capability probe + fallback, settings UI/key, cutscene chamber in RT, characters in RT (needs V2 or an answer to the alpha-mask question), DLSS feature build + packaging notes | L (3-5 pw) | starts on a non-RT GPU without error (falls back); RT GPU: stable image, no regressions in flow/cutscenes | manual test matrix (NVIDIA RTX, AMD RDNA2+, Intel Arc, iGPU without RT) |
| **R5** (only if R3 is no-go) SDF/grid ray-marched shadows + AO (A3) | own fullscreen pass over the room occupancy grid | M-L (3-5 pw) | soft torch shadows on any GPU, < 2 ms extra | screenshots + perf |

### Voxel characters (B)

| Milestone | Content | Effort | Acceptance criteria | Verification |
|---|---|---|---|---|
| **V1** Voxel pipeline prototype | voxeliser (image + palette -> voxel grid -> culled mesh) for the kid's stand/run/jump frames, constant thickness, `CharacterStyle::Voxel` switch | S-M (1-2 pw) | from the front camera the silhouette matches the sprite; anchoring/mirroring identical to the sprite in the hang-anchor case | CPU unit test: orthographic projection of the voxel grid == sprite alpha mask and palette indices, for every kid image (skips without `Original_data`); screenshot sprite vs voxel |
| **V2** All characters | guard (+colours, palace/dungeon), fat guard, skeleton, vizier/Jaffar, shadow tint, princess/mouse (PV.DAT), swords, cutscene sprites, hit flash, lazy mesh cache | M (2 pw) | every character in every level and cutscene drawn as voxels; no missing image (fallback to sprite + warning) | projection test over all DATs; play-through screenshots per level; `POP_PERF` |
| **V3** Depth heuristics + lighting | distance-transform rounding, smoothed normals option, lit material, shadow casting (A0), Solari compatibility (A1) | M (2 pw) | reviewed look at the default camera and at a 30 degree side angle; frame time budget held | screenshot review; projection test still passes (depth must not change the front view) |
| **V4** (optional) Hybrid depth rules | small authoring format (per image region / palette index depth offsets), viewer tool, rules for key frames (run, fight, climb) | M (2-4 pw) | limbs visibly layered in key poses; rules file contains no pixel data | rules parser test; screenshots |
| **V5** (separate decision) Rigged voxel characters (B2) | part models, pose editor, frame -> pose table | XL | to be specified only if chosen | |

Suggested order: R1 -> V1 -> R2 -> V2 -> R3 spike -> V3 -> R4 or R5 -> V4. V1/V2 come early because voxel characters remove
the alpha-mask question from the RT work and benefit A0 shadows too.

## 5. Legal and asset provenance

- Voxel meshes are derived from the copyrighted sprites. Build them **at runtime from the user's own `Original_data`**
  (like the sprites today) and keep them in memory or in a cache under the user's settings directory. Never commit or ship
  converted meshes, depth maps, or AI outputs; `tools/package.ps1` must keep excluding them.
- V4 rules files should hold only coordinates/offsets keyed by image number, no pixel data; even so, keep them clearly
  documented in docs/PROVENANCE.md. (This is an engineering precaution, not legal advice.)
- Third-party licences: Depth Anything V2 Small is Apache-2.0, larger checkpoints are non-commercial (CC-BY-NC-4.0).
  The NVIDIA DLSS SDK has its own licence: check the redistribution terms before shipping its DLLs in the zip.

## 6. Open questions and decisions for you

1. Which hardware must the RT mode support? "NVIDIA RTX only (with DLSS-RR)" makes A1 feasible; "any RT GPU with good
   quality" currently has no denoiser in Bevy.
2. Is a noticeably different look acceptable (lit characters, bounced torch light), or must `classic` stay the default?
3. Voxel style: "thick pixel art" (B1) acceptable, or is articulated 3D (B2, XL authoring) the real goal?
4. Should character pose interpolation between original frames ever be offered (deviates from the original 12 Hz look)?
5. Is a self-hosted GPU runner for screenshot tests wanted, or is local manual verification enough?
6. Track Bevy releases for Solari (point lights, non-DLSS denoiser) before starting R4?

## 7. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Solari API/behaviour changes every Bevy release (0.19 rework, 0.20 ReSTIR opt-in) | rework on each upgrade | keep RT behind a feature and one module; time-boxed spike |
| No point lights in Solari | torches need emissive proxies, look differs from other modes | emissive flames already exist; tune per mode |
| Denoiser NVIDIA-only | noisy RT on AMD/Intel | ship RT as "experimental, NVIDIA recommended"; fall back to `enhanced` |
| wgpu RT on DX12 unconfirmed | RT only via Vulkan on Windows | force Vulkan in RT mode; probe at start |
| Alpha-masked sprites in RT (undocumented) | rectangular shadows | voxel characters (V2) or exclude sprites from rays |
| Voxel look disappoints from side angles | wasted work | V1 prototype on a few frames first; camera is near-frontal |
| Front view of voxels drifts from the original sprite | fidelity loss | projection unit test as a hard gate |
| Bevy negative-scale winding with lit meshes | missing faces when mirrored | test in V1; mirrored meshes as fallback |
| No GPU in CI | rendering regressions slip through | compile check in CI, CPU projection tests, local screenshot diffs |

## 8. Sources

- Bevy 0.20 release notes (Solari on Metal, supported lights, ReSTIR opt-in, DLSS-RR 4.5): https://bevy.org/news/bevy-0-20/
- Bevy 0.18 release notes (Solari specular): https://bevy.org/news/bevy-0-18/
- Bevy 0.17 release notes (Solari introduced, diffuse only, DLSS-RR only denoiser, DLSS needs Vulkan): https://bevy.org/news/bevy-0-17/
- Solari in Bevy 0.19, author's blog (DLSS-RR guide buffers, world cache limits, open problems): https://jms55.github.io/posts/2026-04-12-solari-bevy-0-19
- bevy_solari source v0.20.1, required wgpu features and plugin docs: https://raw.githubusercontent.com/bevyengine/bevy/v0.20.1/crates/bevy_solari/src/lib.rs
- bevy_solari source v0.20.1, `RaytracingMesh3d` mesh requirements: https://raw.githubusercontent.com/bevyengine/bevy/v0.20.1/crates/bevy_solari/src/scene/types.rs
- docs.rs bevy_solari (0.20.1 docs build failed, 0.18.1 last good): https://docs.rs/crate/bevy_solari/latest
- Bevy Solari example (forces opaque materials): https://docs.rs/crate/bevy/0.18.1/source/examples/3d/solari.rs
- Bevy DLSS release note: https://cocalc.com/github/bevyengine/bevy/blob/main/release-content/release-notes/dlss.md
- wgpu ray tracing extension docs (experimental, ray queries only, no AS updates): https://wgpu.rs/doc/wgpu/documentation/extensions/ray_tracing/index.html
- wgpu changelog (v29/v30 RT entries, no DX12 RT entries found in the part read): https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md
- wgpu ray queries PR (merged 2024-11-09): https://github.com/gfx-rs/wgpu/pull/6291
- W3C GPU for the Web WG 2026 activity (rayQuery in WGSL still a proposal): https://www.w3.org/groups/wg/gpu/activity/2026/
- Bevy SSR / SSAO / contact shadows sources: https://docs.rs/bevy/0.17.1/src/ssr/ssr.rs.html , https://doc.qu1x.dev/bevy_trackball/src/bevy_pbr/contact_shadows.rs.html
- AMD FSR Ray Regeneration (denoiser, not in Bevy): https://gpuopen.com/manuals/fsr_sdk/techniques/denoising/
- Kenney Shape (per-pixel depth extrusion): https://80.lv/articles/kenney-shape-tool-for-generating-extruded-3d-models-from-2d-pixel-art-is-now-free
- PixtoMesh: https://dacuza.itch.io/pixtomesh ; Voxelorama: https://orama-interactive.itch.io/pixelorama/devlog/389910/announcing-voxelorama
- TU Delft, "How can we convert 2D pixel art into a 3D voxel representation" (2025): https://repository.tudelft.nl/record/uuid:c617a480-53fe-4e27-bfda-008cc718cee1 , poster: https://cse3000-research-project.github.io/static/f7343b3b2c49afb4e6560b7a966df9db/poster.pdf
- Depth Anything V2 (model licences): https://github.com/DepthAnything/Depth-Anything-V2
- block-mesh (greedy meshing): https://github.com/bonsairobo/block-mesh-rs ; binary-greedy-meshing: https://docs.rs/crate/binary-greedy-meshing/0.5.1/source/README.md

Where sources were thin: DX12 ray-query support in wgpu, Solari's handling of alpha-masked materials and of meshes without
`RaytracingMesh3d`, Solari hardware requirements beyond the wgpu feature list, and any performance figures for a scene
like ours. These are left as spike questions (R3) rather than stated as facts. The 0.20.1 API docs on docs.rs did not build,
so crate details come from the tagged source.

## 9. Status of R1, R2, V1 and V2 (2026-11-15)

**R1 done (minus the `rt` Cargo feature, which waits for the R3 spike).** `settings.json` keys `render_mode`
(`classic` default, `enhanced` / `rt_experimental` accepted and run as classic with a log line) and `character_style`
(`sprite` default, `voxel`), parsed per field like the other keys (bad values fall back alone); env `POP_RENDER` /
`POP_CHARS` override (`settings::resolve_render`, unit tests). Startup logs the adapter (`RenderAdapterInfo`: name,
backend, device type) and whether its features include `EXPERIMENTAL_RAY_QUERY`; nothing extra is requested from the
device, so startup cannot fail on it (RTX 3090 / Vulkan: "exposed by the adapter"). F6 toggles sprite/voxel at runtime and
saves it; no render-mode key yet (only classic exists). Classic is pixel-identical to the build before R1: `POP_SHOT`
diff = 0 pixels against a HEAD build on level 1 start, level 2 room 1 (torches) and the level-2 cutscene (the cutscene
itself differs between two runs of the same build, timing).

**R2 done: `render_mode = "enhanced"`** (`enhanced.rs`; F5 toggles classic / enhanced at runtime and saves it, `POP_RENDER=enhanced`).
Classic stays the default and adds nothing at startup: `POP_SHOT` diff against the build before R2 = 0 pixels on level 1
start, level 2 room 1 and level 3 start, sprite and voxel style (a first run differed by 12 / 1 pixels; the repeat and the
base build against itself gave 0, i.e. run-to-run timing, not the change). New settings, parsed per field like the rest:
`shadow_quality` `off|low|high` (default high) and `ssao` (default true). What enhanced turns on (all Bevy 0.20.1, checked in
the registry source):

- **Torch shadows** (point-light cube maps): only the 4 torches nearest the view (2 on low) cast shadows, re-picked every
  frame; the rest stay shadowless (6 passes per shadowed light). The flame boxes and the cutscene flame sprites are
  `NotShadowCaster` (the earlier attempt's flames shadowed their own light). In enhanced the light moves from z -0.2 to
  -0.6, 0.3 in front of the back wall's face: at -0.2 the exit/door boxes next to the torches stood beside the light and threw
  big shadows over the wall glow. Biases: depth 0.2, normal 2.5 (defaults 0.08 / 0.6 still hatched the voxel creases;
  1.2 still left diagonal hatching in the robe fold and a checker at the feet, 2.5 removed it with no visible peter-panning).
  Torch intensity x1.4 in enhanced (shadows and the wall-near position take light away). Maps 1024 (512 low); filter
  Gaussian (Hardware2x2 on low: harder, jagged edges). The original has no torch flicker and none was added.
- **Sun shadows**: the existing fill `DirectionalLight` casts shadows (one cascade, 1-30 units, map 2048 / 1024 low, normal
  bias 2.5 against blotches on stacked wall boxes), illuminance 4000 -> 3300. This is where the clear character shadows
  come from: characters stand at z 0.6, in front of the floor slabs (front edge z 0.5), so torch shadows (lights behind
  them) fall into the void; the sun, from the front-left above, puts the kid's/guards' shadows on the back wall and doors
  and a soft patch on the floor at the feet.
- **SSAO** (`ScreenSpaceAmbientOcclusion`, Medium): requires depth + normal prepass and does nothing with MSAA on (Bevy logs
  an error and skips it), so with SSAO the camera gets `Msaa::Off` + SMAA (High). TAA rejected: needs motion vectors and
  ghosts on sprites that change image at 12 Hz. SSAO is skipped (logged) where the adapter has fewer than 5 storage
  textures per shader stage (Bevy disables its plugin there). Unlit sprite quads ignore it (SSAO only scales the lit
  ambient term); no halo around sprites was visible.
- **Bloom** (`Bloom::NATURAL`, intensity 0.12) with `Hdr`: only the emissive flame boxes (8, 3, 0.5) glow. Tonemapping stays
  Bevy's default TonyMcMapface, which classic already uses, so colours stay close.
- **Ambient**: (0.78, 0.76, 0.86) at 220 instead of the blue (0.6, 0.65, 0.9) at 150, so voxel faces turned away from the
  torches no longer sink into blue-black. No extra rim light needed.
- **Dropped**: contact shadows (`ContactShadows` exists in 0.20.1 but the sun shadow already grounds the figures and the
  torch side cannot help, see above), volumetric/distance fog (no benefit in these closed rooms), TAA (above).
- **Fallback**: after a 600-frame warm-up, a 300-frame window averaging over max(12 ms, 1.25 refresh periods of the slowest
  monitor), counted only while the window is focused, switches to classic for the session (logged, not saved, fires once).
  The refresh floor came from testing: Windows throttled the unfocused test window to exactly 16.67 ms even with vsync off,
  and a plain 12 ms rule fell back on it. Startup logs which features were enabled.

Screenshots reviewed (scratchpad, enhanced vs classic, both character styles): level 1 start, level 2 room 1 (torch room),
demo-level guard fight, cutscene 2, plus a debug-camera view of the voxel kid. No striping or acne after the bias change,
no black rooms (enhanced is somewhat darker away from the torches and in sun-shadowed wall bands; the slab fronts are a bit
darker than classic), soft kid/guard shadows on wall and doors, torch halos tighter but brighter and still the warm focus.
Weak spots: kid/guard wall shadows are low-contrast on the dark back-wall material; slight light blotches on the side faces
of stacked wall boxes. Perf (release, RTX 3090 / Vulkan, demo level, `POP_PERF=1`, 1280x720): classic ~1.9-2.4 ms/frame,
enhanced ~2.6 ms/frame steady (voxel; first windows 6.8-7.8 ms while pipelines compile); after some seconds every run was
capped at 16.67 ms by the window being in the background, so longer runs could not be measured. The 4 ms acceptance target
is met on this GPU; a mid-range GPU is not measured.

**V1 done for all in-game characters** (kid, shadow, guard with colour palettes and palace/dungeon palettes, fat guard,
skeleton, vizier, both swords; cutscene actors still sprites, see V2). `voxel.rs`: `VoxelModel { w, h, d, voxels }`
(palette index per voxel) built from the decoded original image at first use and cached per (kind, image, shadow,
palette, colour, mirrored) next to `material_for`'s cache; nothing is written to disk. Depth: *inflated* extrusion, a
column is 2k + 1 voxels thick, k = 4-neighbour distance to the silhouette edge capped at 3 (edges 3, inner body up to 7),
centred on the sprite plane z = 0.6. Chosen over constant thickness, which reads as a cardboard cut-out from the tilted camera (2.2 B1);
the rounded profile gives head and torso volume and keeps thin limbs thin, and the front view is unchanged by
construction. Meshing: per-face culling (a quad per filled/empty boundary), flat normals, linear vertex colours, `U32`
indices; lit `StandardMaterial` (roughness 0.9), so torches and ambient shade the cubes. Placement: the same `world_pos`
/ interpolation / left-or-right edge rule as the sprite; the mesh origin is the image's bottom-left pixel, one voxel = one
sprite pixel (1/32 x 2/63 x 1/32 world units). Mirroring uses a separately built mirrored mesh (no negative scale, so
winding and normals stay correct). The voxel sword sits at z = 0.6 + 5/32 so it stays in front of the body as in the
original. An image without a voxel mesh falls back to the sprite quad.

Tests: front projection == original image (silhouette and palette indices), stored and mirrored, for all 219 kid images,
all guard-type images that decode (GUARD/FAT/SKEL/VIZIER/SHADOW 751+) and the sword images; small-image tests for the
depth profile and mirroring; every mesh triangle winds counter-clockwise around its normal. Build time: all 219 kid images
in both facings voxelised + meshed in ~81 ms (release, includes the projection checks), i.e. ~0.2 ms per image, built lazily.

Screenshots (`POP_CHARS=voxel`, `POP_SHOT_FRAME` 300-2900): kid standing (level 1 start), running right (mirrored),
demo-level sword fight against the palace guard, plus `POP_DEBUG_CAM=1` / F7 views. From the game camera the figures
match the sprites; from the side camera they read as chunky cube figures with per-face shading and warm torch light, close
to the "Prince of Persia Voxelized" reference, without cracks or floating parts. Perf (release, RTX 3090, demo level,
`POP_PERF=1`, steady state): sprite ~1.45 ms/frame, voxel ~1.35-1.4 ms/frame (no measurable cost).

Shadows: tried shadow maps on the torch point lights in voxel style. Not kept: the torch/flame boxes sit at the light
and shadow it, the lights are at the character plane (z 0.6), the room got much darker and the voxel steps showed
self-shadow stripes. Needs NotShadowCaster on torch meshes, light placement and bias tuning: belongs to R2 (done there, see above).

**V2 done: better volume, all characters, cutscene actors.** The V1 profile (2k + 1 voxels, k = 4-neighbour edge
distance) looked like a thick cut-out from the side. `voxel.rs` now builds a *union of balls*: every pixel is a ball of
radius = its chamfer distance to the silhouette edge (minus 0.5, capped at 4 px) and a column's front height is the highest
ball over it, so heads come out sphere-like, limbs as cylinders as thick as they are wide, hands/feet/sword thin, and the
torso a rounded slab. Colour-aware on top: pixels are classed by colour (neutral cloth, dark, skin, 60-degree hue
sectors) and each class region adds its own small rounded bump (radius cap 1.5 px, weight 0.6), so arm over torso, sash,
hair and face separate; darker shades of a region (below 0.8 of its brightest luminance: cloth folds) sit 0.6 px deeper.
The back is a flatter copy (0.6 of the profile, capped at 2 px). Classes come from the untinted palette (the shadow's dark
tint would merge them). Voxel grid: `plane` = the sprite-plane layer, depth = deepest back + 1 + highest front (about 8
layers for the kid). Mesh: still per-face culling with flat normals (no greedy meshing, which per-vertex AO would defeat;
1393 faces per kid image on average); vertex colours carry baked AO (side/side/corner occluders: 1.0 / 0.8 / 0.66 /
0.54, quad split along the symmetric diagonal), rear layers darker and greyer by their distance to the column's front
(-8 % and +7 % grey per pixel, floor 0.55), and a +-6 % per-voxel brightness jitter. Front projection is unchanged by
construction. Build: 219 kid images in both facings voxelised and meshed in ~118 ms (release, with the projection checks),
~0.27 ms per image, lazily.

Fine detail (2 x 2 voxels per pixel) was tried and dropped: ~5500 faces per image (4x), ~1.2 ms per image, and from the
debug camera it read as contour bands rather than chunky cubes, further from the reference; no `voxel_detail` setting.

Cutscenes: with character_style voxel the actors (princess PV 800/900 sets, Jaffar PV 850, kid and mouse from KID.DAT)
are voxel meshes from the same pipeline (cache per file, image, mirrored), anchored like the sprites (same rectangle, mesh
origin at its bottom-left). Depth: 0.04 behind the sprite depth so the back clears the bed carpet sprite (z 0.52); the
hourglass moves from z 0.78 to 0.84 in voxel style so the figures stay behind it as with sprites. Flames, hourglass, sand
stream and the bed stay sprites (set dressing). The voxel material is created on first use, so sprite style keeps its
exact asset set. In-game, the mouse (kid-table frames 186..188) and the sword pickup frames already use the kid path.

Screenshots (scratchpad, `POP_CHARS=voxel`, game camera and `POP_DEBUG_CAM=1` with the new `POP_CAM_ANGLE=deg,dist`):
kid standing / jumping up / running, dungeon guard (level 2) and palace guard (level 5) fights, fat guard (level 6), skeleton
(level 3, needed the exit door open), Jaffar (level 13), the shadow (level 5, potion room), the mouse (level 8, door counter
forced by a temporary hook, removed), cutscenes 2, 8, 9, intro, timeout and ending. Not seen on screen: the level-6 and
level-12 shadows, the sword-raised pickup pose (covered by the projection test only). Sprite style: 0 differing pixels
against the previous build on level 1 start (4 runs), level 2 room 1 and level 3 start, and the intro cutscene. Perf
(release, RTX 3090, demo level, `POP_PERF=1`): sprite 1.41-1.57 ms/frame, voxel 1.35-1.41 ms/frame.

**R3 done (2026-11-18): Solari trial, verdict no-go for R4 for now** (`docs/r3_solari.md`). Prototype behind the
`solari` cargo feature (`solari.rs`): started with `render_mode = rt_experimental` it adds `SolariPlugins`, traces opaque
lit meshes (rooms, voxel columns/objects/characters) via Solari-layout mesh copies, replaces each torch point light with a
ray-traced-only emissive box, scales unlit/emissive materials for Solari's exposure, F5 cycles classic / enhanced / rt;
falls back to enhanced when the adapter lacks the features. Default build unchanged. Level 1 room 13 at 1080p on the RTX
3090: Solari 1.7 ms (ReSTIR off) / 2.8 ms (ReSTIR on) vs enhanced 2.95 ms and classic 1.24 ms. Shadows and bounce light
are a real gain, but without a denoiser the image is very noisy (fine grain with ReSTIR), unlit corners go black (no
ambient in Solari), alpha-masked sprites cannot be traced; DLSS-RR could not be built (no DLSS SDK / libclang here).
Re-check when DLSS-RR can be tested or Bevy ships a non-DLSS denoiser. R5 is not needed for the shadow look (enhanced).

Open for V3: hit flash, an unlit option, smoothed normals, softer side lighting on the back terraces (the -x/+x step faces
away from the torches go very dark under the blue ambient), a turned camera for cutscenes, a 30-degree review of key poses.
