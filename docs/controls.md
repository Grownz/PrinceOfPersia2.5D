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
| F6 | Characters: original sprites / voxel figures built from them (saved as `character_style`) |

The keypad is fixed like the original (its key handler `0000:1533` reads scancodes 0x47..0x50): keypad 1, 3 and 0 (End, PgDn, Ins)
do nothing. It works alongside the rebindable keys below.

With the debug text on (F3): Ctrl+arrows = neighbouring room, Ctrl+PageUp / Ctrl+PageDown = previous / next level (plain PageUp is up+right),
`,` / `.` = previous / next animation sequence (sequence viewer), Space = restart the sequence, F = flip, C = back to the
controlled kid at the level start. F7 = closer camera turned 35 degrees around the kid (inspect the voxel depth; also
`POP_DEBUG_CAM=1`; `POP_CAM_ANGLE="degrees[,distance]"` sets its angle and distance, default 34 degrees, 4 units).

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
  "character_style": "sprite"
}
```

`window`: `windowed` or `borderless` (F11 saves it). `sound`: `synth` (generated effects) or `original` (the game's own
sounds; the environment variable `POP_SOUND=original` overrides). Volume, mute and F11 are saved when changed.
`render_mode`: `classic` (default); `enhanced` and `rt_experimental` are reserved names, accepted but run as `classic`
with a log line until implemented (`POP_RENDER` overrides). `character_style`: `sprite` (default) or `voxel` (F6 toggles
and saves it, `POP_CHARS` overrides). Unknown values fall back to the default; the log names the GPU, the backend and
whether the adapter offers ray queries.
