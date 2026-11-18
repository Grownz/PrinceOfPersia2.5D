# Controls

## Keyboard

| Key | Action |
|---|---|
| Arrow keys | Move the kid (left / right / up = jump, climb / down = crouch, step down) |
| Numeric keypad | 8 up, 4 left, 6 right, 2 and 5 down, **7 up+left, 9 up+right** (running jump, strike); NumLock does not matter |
| Home / PageUp | Same as keypad 7 / 9 (same scancode in the original) |
| Shift | Action: grab a ledge, careful step, drink, strike with the sword; after death, hold to restart |
| Enter | After death: restart the level |
| Esc / P | Pause / resume |
| Ctrl+A | Restart the level |
| Ctrl+G | Save the game (levels 3 to 13) |
| Ctrl+L | Load the saved game (title screen and demo only) |
| Any key | Title screen: start a new game; demo: start the game |
| M | Mute / unmute |
| - / = (numpad - / +) | Master volume down / up (10 % steps) |
| F11 | Windowed / borderless fullscreen |
| F3 | Debug text and debug keys |
| F5 | Rendering: classic / enhanced (torch and sun shadows, SSAO, bloom; saved as `render_mode`) |
| F6 | Characters: original sprites / voxel figures built from them (saved as `character_style`) |
| F9 | Frame-time overlay (top right): FPS, avg / min / max / 1% low over 120 frames, mode, resolution, mesh and light counts, GPU pass times when started with `POP_FPS=1` |

The keypad is fixed like the original (its key handler `0000:1533` reads scancodes 0x47..0x50): keypad 1, 3 and 0 (End, PgDn, Ins)
do nothing. It works alongside the rebindable keys below.

With the debug text on (F3): Ctrl+arrows = neighbouring room, Ctrl+PageUp / Ctrl+PageDown = previous / next level (plain PageUp is up+right),
`,` / `.` = previous / next animation sequence (sequence viewer), Space = restart the sequence, F = flip, C = back to the
controlled kid at the level start. F7 = closer camera turned 35 degrees around the kid (inspect the voxel depth; also
`POP_DEBUG_CAM=1`; `POP_CAM_ANGLE="degrees[,distance]"` sets its angle and distance, default 34 degrees, 4 units).

Performance (docs/enhanced_perf.md): `POP_FPS=1` starts with the F9 overlay on and per-pass GPU timing (wgpu timestamp
queries; F9 alone shows CPU frame times only). `POP_BENCH=<frames>` runs a benchmark (vsync off, 500 warm-up frames,
`POP_BENCH_WARMUP`), logs one `BENCH ...` line and quits; `POP_RES=WxH` or `full` sets the window; `POP_FX` switches
enhanced features off for cost measurements (`no_torch_shadows`, `no_sun_shadow`, `no_ssao`, `no_bloom`, `no_smaa`,
`no_hdr`, `no_ambient`, `lights=N`, `point_map=N`, `dir_map=N`, `ssao=low|medium|high|ultra`, `filter=hw|gauss|temporal`,
`rooms=1`, `casters=boxes|none`). `tools/bench.ps1` runs a matrix and prints a table.

The kid's five keys (left, right, up, down, shift) can be rebound in `settings.json` under `"keys"`, each as a list of
Bevy `KeyCode` names, for example `"left": ["ArrowLeft", "KeyA"]`. Unknown names are ignored with a warning; an empty
list falls back to the default.

## Controller

Any controller supported by gilrs (XInput pads, PlayStation, Switch Pro and others) works alongside the keyboard; inputs
from both are combined.

| Button | Action |
|---|---|
| D-pad / left stick | Arrow keys (stick threshold: half deflection) |
| A / Cross (South), X / Square (West) | Shift (action) |
| Start / Options | Pause / resume |
| Back / Select / Share | Restart the level (Ctrl+A), while playing |
| Any button | Title: start a new game; demo: start the game; skip a cutscene |

Save, load, volume, mute and fullscreen stay on the keyboard.

## settings.json

Created next to the exe on the first run; unknown keys are ignored, missing ones take their defaults.

```json
{
  "window": "windowed",
  "width": 1280,
  "height": 720,
  "volume": 0.7,
  "muted": false,
  "sound": "synth",
  "keys": { "left": ["ArrowLeft"], "right": ["ArrowRight"], "up": ["ArrowUp"], "down": ["ArrowDown"], "shift": ["ShiftLeft", "ShiftRight"] },
  "data_path": null,
  "render_mode": "classic",
  "character_style": "sprite",
  "shadow_quality": "high",
  "ssao": true,
  "voxel_normals": "smooth",
  "voxel_shading": "lit",
  "camera_scroll": "smooth"
}
```

`camera_scroll`: `smooth` (default: on a room change the camera pans to the new room in 0.3 s, eased, frame rate
independent, the neighbouring rooms visible at the margins) or `flip` (the original's hard cut to the new room); env
`POP_CAMERA=smooth|flip` overrides. Level changes, restarts and jumps to a room that is not a neighbour always cut.
Upside-down (the invert potion; in the shipped levels raw potion modifier 4 sits in level 9 rooms 7 and 10, INFERRED kind 4): the picture is mirrored top to bottom as in the original; the hit points and
texts stay upright and the controls are not swapped (original: display only, docs/edge_cases.md 5.1).

`window`: `windowed` or `borderless` (F11 saves it). `sound`: `synth` (generated effects) or `original` (the game's own
sounds; the environment variable `POP_SOUND=original` overrides). Volume, mute and F11 are saved when changed.
`render_mode`: `classic` (default, the original look), `enhanced` (shadow maps from the nearest torches and a soft sun
light, screen-space ambient occlusion, bloom on the flames, a brighter fill light; any GPU, F5 toggles and saves it) or
`rt_experimental` (reserved, accepted but run as `classic` with a log line); `POP_RENDER` overrides. Enhanced only:
`shadow_quality` `off`, `low` (2 torch shadows, small maps) or `high` (default, 4 torch shadows), `ssao` `true` (default;
turns MSAA off and uses SMAA) or `false`. If enhanced is too slow after its warm-up (p95 frame time of 300-frame
windows over 1.25 refresh periods, at most 60 Hz counted, i.e. 20.8 ms; three windows in a row, window focused), it falls back to classic for the session with a log line (not saved). `character_style`: `sprite` (default) or `voxel` (F6 toggles
and saves it, `POP_CHARS` overrides). Voxel characters only: `voxel_normals` `smooth` (default: normals from the voxel
surface, rounded shading) or `flat` (one normal per cube face, the blocky look), env `POP_VOXEL_NORMALS`; `voxel_shading`
`lit` (default: torches, sun and ambient shade the figures, they cast shadows in enhanced) or `unlit` (the palette colours
with the baked occlusion only, closest to the sprites), env `POP_VOXEL_SHADING`; both are read at startup. Unknown values fall back to the default; the log names the GPU, the backend and
whether the adapter offers ray queries.
