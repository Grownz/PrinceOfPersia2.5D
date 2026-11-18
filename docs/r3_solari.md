# R3: Bevy Solari trial and go/no-go (2026-11-18)

Time-boxed spike from `docs/render_roadmap.md` (R3). Question: is Bevy 0.20.1's ray-traced lighting (`bevy_solari`) worth
turning into a shipped render mode (R4)? Everything here was run on one machine: RTX 3090, NVIDIA driver 621.24, Vulkan,
Windows 11, release build, 1920x1080.

**Verdict: no-go for R4 now.** The prototype stays in the repo behind the `solari` cargo feature (off by default, not
in CI's default build). Solari runs, is cheap on this GPU and adds real ray-traced shadows and bounce light, but without a
denoiser the image is not shippable, and the only denoiser Bevy has (DLSS Ray Reconstruction) is NVIDIA-RTX-only and
could not be built here. Details and next steps in 6.

## 1. What Solari is in Bevy 0.20.1 (from the source)

Read in the registry copy of `bevy_solari-0.20.1`, `bevy_anti_alias-0.20.1` (dlss), `bevy_pbr-0.20.1`, `bevy_render-0.20.1`,
`dlss_wgpu-6.0.0` and `bevy-0.20.1/examples/3d/solari.rs`.

- **Plugins**: `SolariPlugins` = `RaytracingScenePlugin` (BLAS/TLAS, bindless resources) + `SolariLightingPlugin`
  (`lib.rs`). `PathtracingPlugin` is a separate reference path tracer (not added by default).
- **wgpu features** (`SolariPlugins::required_wgpu_features`): `EXPERIMENTAL_RAY_QUERY`, `BUFFER_BINDING_ARRAY`,
  `TEXTURE_BINDING_ARRAY`, `SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING`, `PARTIALLY_BOUND_BINDING_ARRAY`.
  Both plugins check the device in `finish()` and skip themselves with a warning when any is missing, so startup does not
  fail. Bevy's default `WgpuSettingsPriority::Functionality` already requests every adapter feature
  (`bevy_render renderer/mod.rs` 319-340), so nothing extra must be requested. (Correction to R1's log line "exposed by
  the adapter (not enabled)": with this default the feature *is* enabled on the device.)
- **Side effect of adding the plugins**: `SolariLightingPlugin::build` inserts `DefaultOpaqueRendererMethod::deferred()`
  unconditionally (`realtime/mod.rs` 62): every StandardMaterial in the app switches to the deferred path, also in
  classic/enhanced. That is why the prototype only adds Solari when the game starts in `rt_experimental`.
- **Camera**: `SolariLighting` requires `Hdr`, `DeferredPrepass`, `DepthPrepass`, `MotionVectorPrepass`; the camera must
  have `Msaa::Off` and `CameraMainTextureUsages` with `STORAGE_BINDING`. Extraction adds `SkipDeferredLighting`: Bevy's
  deferred lighting pass does not run, Solari writes the lit image from the G-buffer before the main opaque pass.
  Settings: `restir` (default false in 0.20), sample counts, bounces (3), world cache tuning.
- **Geometry**: `RaytracingMesh3d(Handle<Mesh>)` + `MeshMaterial3d<StandardMaterial>` (required). The mesh must have
  exactly `{POSITION, NORMAL, UV_0, TANGENT}`, `TriangleList`, `Indices::U32` and `enable_raytracing` (default true);
  anything else is silently skipped (`scene/blas.rs` `is_mesh_raytracing_compatible`). Only `StandardMaterial`.
- **Materials seen by rays**: base colour (+ texture), emissive (+ texture), metallic/roughness/reflectance, normal map
  (`scene/bindings.wesl` `resolve_material`). **No alpha**: rays use `RAY_FLAG_NONE` / `TERMINATE_ON_FIRST_HIT` with no
  any-hit or alpha test, so an alpha-masked quad occludes as a full rectangle. **No vertex colours** for rays.
- **Primary visibility comes from the raster G-buffer** (`realtime/gbuffer_utils.wesl`): what the camera sees directly
  keeps vertex colours and alpha masks; only secondary rays (shadows, bounces) use the simplified ray-traced scene.
- **Lights**: emissive meshes (max 65535 triangles per emissive mesh, 65535 lights) and `DirectionalLight`s
  (`scene/binder/lights.rs`); one environment map. **No point or spot lights, no `AmbientLight`.**
- **Units**: Solari multiplies emissive and the G-buffer's unlit colour by the camera exposure (`no_restir.wesl`
  line 25), raster does not (`emissive_exposure_weight` defaults to 0). Default exposure (EV100 9.7) = 1/1040.
- **Denoising**: none in Solari itself. DLSS Ray Reconstruction via `bevy/dlss`: NVIDIA RTX only, Windows/Linux Vulkan only
  (`bevy_anti_alias/src/dlss/mod.rs` 5). Building needs the NVIDIA DLSS SDK (`DLSS_SDK` env var), the Vulkan SDK
  (`VULKAN_SDK`) and libclang for bindgen (`dlss_wgpu-6.0.0/build.rs` 12-14, 58), and a `DlssProjectId` resource. The 0.20
  release note for Metal says the same: no denoising outside DLSS (`_release-content/release-notes/solari_metal.md`).

## 2. What was built (`crates/pop-app/src/solari.rs`, feature `solari`)

- `Cargo.toml`: `solari = ["bevy/bevy_solari"]`. No `dlss` feature: it cannot be compiled here (3). `Cargo.lock` gains the
  optional `bevy_solari` / `dlss_wgpu` / `bindgen` entries (lock files list optional deps; the default build does not
  compile them).
- `RenderMode::Rt` exists only with the feature; `rt_experimental` resolves to it there and still to classic (with the old
  log line) without it. `RenderMode::key()` gives the settings value.
- `main.rs`: `solari::plugin` is added only when the game starts in `rt_experimental`. A startup check falls back to
  enhanced with a warning (and the missing features) when the device lacks Solari's features; otherwise `RtAvailable` is
  inserted and **F5 cycles classic -> enhanced -> rt_experimental**. A session started in classic/enhanced cannot reach Rt
  (restart needed: the plugins change the renderer for the whole app).
- On entering Rt: camera gets `SolariLighting` (`POP_RESTIR=1` turns ReSTIR on), storage-binding main texture, MSAA off,
  bloom; SSAO/SMAA/normal prepass go; sun shadow maps off, sun at the classic 4000 lux. `enhanced::apply` ignores Rt and
  redoes classic/enhanced when F5 leaves it (Rt's camera components are removed first).
- **Meshes**: every new entity with an opaque, lit, non-emissive StandardMaterial gets a `RaytracingMesh3d` with a cached
  copy of its mesh in Solari's layout (zero UVs if absent, a tangent perpendicular to the normal, U32 indices; vertex colours
  dropped). Mesh swaps (character animation frames) follow via `Changed<Mesh3d>`. Traced: room boxes and textured
  pieces, voxel columns, voxel objects (doors, spikes, chompers, debris, holders), voxel characters.
- **Not traced** (shown and lit through the G-buffer, but rays pass through): alpha-masked materials (sprite characters,
  cut-out tile images, cutscene sprites) and unlit/emissive ones (voxel flames, flame boxes, potion bubbles). Logged per
  rebuild, e.g. level 1 start: `45 meshes traced, not traced: 9 alpha-masked/blended, 7 unlit/emissive`, then
  `285 traced, 9 unlit/emissive` for the 9-room build.
- **Unlit and emissive brightness**: while in Rt, unlit base colours and emissive colours are multiplied by 1/exposure
  (1040) so sprites, flames and bubbles look as in classic; the originals are restored when leaving Rt.
- **Torches**: Solari ignores the point lights, so each torch `PointLight` gets a child box (0.2 x 0.3 x 0.2, ray-traced
  only, no raster mesh) at the flame, emissive in the torch colour with radiance = 250 000 lm / (pi x area) (the classic
  light's flux from a Lambertian box), removed when leaving Rt.
- Tests (feature build): `rt_copy_has_solari_layout` (the copy matches `is_mesh_raytracing_compatible` from a U16 mesh
  with vertex colours; tangents unit and perpendicular; line lists rejected), `only_opaque_lit_non_emissive_materials_trace`,
  and the render-mode resolution test with the feature.
- Checks: default build `cargo test --workspace` pop-core 85, pop-app 57 passed, no build warnings; `--features solari`
  pop-app 59 passed, no clippy warnings in the changed files. Classic in the default build vs a HEAD (24d0935) build,
  `POP_SHOT` 1080p: level 2 room 1 and level 3 start (voxel) 0 pixels; level 1 room 13 2 pixels at delta 1 next to the
  torches, but the new build against itself also gives 2 pixels (delta 1) between two runs, so that is run-to-run timing.
- Not built: the `PathtracingPlugin` reference comparison from the R3 verification column, DLSS-RR, cutscene tuning.

## 3. What could not be built

DLSS-RR: `DLSS_SDK` is not set and no DLSS SDK is installed on this machine, `clang`/libclang is not on the PATH
(bindgen in `dlss_wgpu` needs it); `VULKAN_SDK` is set. `dlss_wgpu`'s build script panics without `DLSS_SDK`, so a `dlss`
feature would break any build that enables it. Not tried further (getting the SDK needs an NVIDIA developer download and
licence acceptance, which is the user's call). So every Solari number and image below is **without a denoiser**.

## 4. Measurements (level 1 room 13, 1920x1080, `POP_BENCH=1500`, 500 warm-up frames, GPU timing off)

`unfocused_frames=0` in every run. RAM = process private bytes; VRAM = `nvidia-smi` memory used on the whole GPU (peak
during the run minus ~2770 MB used by the desktop after exit).

| config | build | avg ms | p95 | p99 | fps | RAM | VRAM (app) |
|---|---|---|---|---|---|---|---|
| classic | default | 1.24-1.26 | 1.50-1.58 | 1.71-1.79 | ~800 | 945 MB | ~525 MB |
| enhanced (optimized, current) | default | 2.95-2.96 | 3.43-3.44 | 3.71-3.78 | ~338 | ~1.11 GB | ~520 MB |
| Solari, ReSTIR off (0.20 default) | solari | 1.69-1.72 | 2.08-2.13 | 2.74-2.77 | ~588 | ~1.14 GB | ~595 MB |
| Solari, ReSTIR on (`POP_RESTIR=1`) | solari | 2.77 | 3.34-3.39 | 3.64-3.68 | ~361 | ~1.45 GB (peak 1.5) | ~850 MB |
| classic in the solari build (plugins not added) | solari | 1.24 | 1.48 | 1.66 | 810 | 987 MB | - |

GPU pass times (600 frames, `POP_GPU_TIMES=1 POP_GPU_ALL=1`): Solari without ReSTIR 1.82 ms GPU total: lighting 0.48,
`blas_build` 0.59 (unexpected in a static room; not investigated, possibly compaction/rebuild work Solari schedules per
frame), world cache 0.13, TLAS 0.12, deferred prepass 0.11, bloom 0.12. With ReSTIR 2.88 ms: lighting 1.74, blas 0.33.
Enhanced for comparison: 3.09 ms GPU, of which point-light shadow views 1.96.

Read: on an RTX 3090 Solari at 1080p costs about what enhanced costs (ReSTIR) or less (no ReSTIR), far inside the R3
target of 60 fps. A DLSS-RR pass would add to this (not measured). Mid-range and non-NVIDIA RT GPUs are not measured.

## 5. Image quality vs enhanced

Screenshots (1080p, frame 600; scratchpad of the session, not committed: they contain the original game's art):
`r13_classic.png`, `r13_enhanced.png`, `r13_solari.png`, `r13_solari_restir.png`, `start_voxel_enhanced.png`,
`start_voxel_solari.png`, `start_voxel_solari_restir.png`, `start_sprite_solari.png`. Reproduce with
`POP_RENDER=rt_experimental POP_LEVEL=1 [POP_ROOM=13] [POP_CHARS=voxel] [POP_RESTIR=1] POP_SHOT=... POP_RES=1920x1080`
on a `--features solari` build.

- **Noise**: without ReSTIR the whole image is covered in heavy per-pixel salt-and-pepper noise, also on the sun-lit
  front wall, with bright fireflies in dark doorways. Not playable as is. With ReSTIR the noise drops to a fine, even grain
  (like strong film grain) that is still clearly visible on flat walls and around the torches, and the torch holders and
  thin bars sparkle. Temporal behaviour (crawling grain, ghosting on moving characters) was only seen in single frames, not
  judged in motion.
- **Shadows**: the clear gain. Ray-traced sun shadows are sharp and contact-correct: the voxel kid's shadow on the back
  wall, the gate bars, the column and wall step edges; no acne, no bias tuning, no shadow-map resolution limit. Torch light
  is occluded correctly by the columns next to the torches. Enhanced needed bias work and 24 shadow views for less.
- **GI / ambient**: there is bounce light (the wall faces next to the torch light pick up warm light, the sun's bounce
  reaches the alcoves), but Solari ignores `AmbientLight` and there is no sky: every space the sun and the torches do not
  reach goes nearly black (under the slabs, the corridor right of the start room). Enhanced's ambient fill keeps them
  readable. Would need an environment map or emissive fill, i.e. more tuning.
- **Flame lighting**: the proxy boxes give soft, warm pools on the back wall around each torch, close to enhanced in
  brightness (the flux formula landed in the right range without tuning); the halo is noisier and the flames themselves
  read as flatter orange shapes (unlit voxel colours through the G-buffer's rgb9e5 emissive) than in enhanced.
- **Voxel characters**: lit by sun, torches and bounce like the room and cast correct shadows; but they pick up the same
  grain, which on a ~60 px tall figure eats detail (face, sash). Rays see them without vertex colours (base colour white),
  so their bounce light is grey.
- **Sprite characters**: shown exactly as in classic (unlit, scaled for exposure), no shadow and no lighting (alpha
  mask: not traced). Same for cut-out tile images.
- **Light leaks**: none seen in these two rooms (thick boxes). The ceiling band above the room is dark brown instead of
  black (bounce off the slab undersides).

## 6. Go/no-go

| criterion | finding | verdict |
|---|---|---|
| visual gain | real: exact shadows of everything traced, bounce light, torches as area lights; enhanced already has the main look (torch + sun shadows, SSAO, bloom) | moderate |
| image stability | unusable without ReSTIR, grainy with it; DLSS-RR (the intended denoiser) not testable here | **blocker** |
| cost | 1.7 ms (no ReSTIR) / 2.8 ms (ReSTIR) vs 2.95 ms enhanced, 1.24 ms classic at 1080p on RTX 3090; +75 / +330 MB VRAM, +0.35 GB RAM with ReSTIR | fine on this GPU |
| hardware | needs ray queries + bindless (RTX 20+, RX 6000+, Arc; Vulkan here, DX12 not tried); a denoised image needs NVIDIA RTX + DLSS SDK in the build; the game otherwise runs on any GPU | narrow |
| stability | ran every time; no crash, no validation error in the log; fallback to enhanced is coded but not exercised (no non-RT GPU here) | ok for a prototype |
| maintenance | Solari changed shape in 0.17, 0.18, 0.19 and 0.20 (ReSTIR now opt-in, Metal support); `solari.rs` is ~250 lines in one module behind a feature, but it relies on internals such as the unconditional deferred switch and the exposure handling | medium risk |
| remaining effort for R4 | DLSS feature + SDK packaging/licence, sky/ambient fill, flame look, emissive vertex colours, alpha-masked sprites (or voxel only), cutscene chamber, F5 from any mode (needs Bevy to not force deferred), test matrix on AMD/Intel | L (3-5 pw), as estimated |
| risk | a mode that looks noisy on every non-NVIDIA GPU next to a clean enhanced mode | high |

**Recommendation: no-go for R4 now; keep the prototype parked.** Reasons in order: no shippable image without DLSS-RR;
DLSS limits the mode to NVIDIA RTX plus an SDK in the build and its redistribution terms; enhanced already delivers most of
the visible gain on every GPU at the same cost on this machine. R5 (SDF ray-marched shadows) is not needed for the
shadow look either: enhanced has torch and sun shadows; only start it if soft torch shadows on low-end GPUs become a goal.

What would turn it into a go (cheap re-check, 1-2 days each):
1. Install the DLSS SDK (user's decision: NVIDIA developer download and licence), add `dlss = ["solari", "bevy/dlss"]`,
   insert a `DlssProjectId` before `DefaultPlugins` and `Dlss::<DlssRayReconstructionFeature>` on the camera when
   `DlssRayReconstructionSupported` exists (pattern in `examples/3d/solari.rs`), then compare screenshots and motion.
   If DLSS-RR gives a clean, stable image, an "NVIDIA RTX only, experimental" mode is defensible.
2. A Bevy release with a non-DLSS denoiser (the 0.20 notes name MetalFX Ray Reconstruction and Open Image Denoise as
   candidates) or point-light support: re-run this trial; the module should port with small changes.
3. Before either: give Rt an environment light or fill so unlit corners do not go black, and run the comparison against
   `PathtracingPlugin` that R3's verification asked for (not done here).

Not verified: DLSS-RR, ReSTIR in motion (ghosting), DX12, any other GPU, the fallback path on a GPU without the features,
cutscenes and moving gates in Rt, long-run memory in Rt (runs were 2000 frames), the cause of the per-frame `blas_build`
time.
