# Kid mechanics (PRINCE.EXE v1.1), flat-ground subset

Source: disassembly of `Original_data/PRINCE.EXE` with `tools/disasm` (see its README).
Names follow SDLPoP where the code matches. Every claim below was checked against this binary.

- **VERIFIED**: read directly from the code or data at the address given.
- **INFERRED**: our interpretation (usually a name or a meaning), not proven by the code.

Address notation: `0000:6F90` is code segment 0000, offset 0x6F90. For segment 0000, **file offset = 0x890 + offset**
(for example `0000:6F90` is file 0x7820). `ds:189E` is a data-segment offset, **file = ds + 112832** (for example ds:189E is file 119134).
Game logic uses the Pascal calling convention (`ret far N`, callee pops its arguments, arguments are pushed left to right).

---

## 0. Corrections to `crates/pop-core/src/exe.rs` (found while doing this; nothing under crates/ was changed)

1. **The frame tables are off by one record.** VERIFIED at `0000:6E96` (load_frame): kid frame `f` is read from
   `ds:0FF4 + 5*f` (file 116916 + 5f). Frame 0 = `FF 00 00 00 00` (no image), frame 1 = `00 00 01 00 C4`.
   `exe.rs` reads from 116921, so `kid_frames()[i]` is **frame i+1**, and its last record (`00 FF 00 00 00`) is
   past the end of the table. `pop-app/src/kid.rs:95` indexes `frames[frame]`, so it shows the image of the next frame.
   The other tables have the same offset:
   - Cutscene table (charid 5/6): `ds:1578 + 5*f` (file 118328), but exe.rs starts at 118333 (frame 1).
   - Guard table (charid 2, and the shadow for frames 150..189): `ds:11C1 + 5*f`. Only frames 150.. are stored there,
     so file 118127 is **frame 150** and exe.rs `guard_frames()[i]` is frame 150+i. For a guard, frames 102..106 are
     moved +70, to 172..176 (`0000:6ECB`).
   - Image byte: 0-based (frame 15 has image 14). Flags is the 5th byte of the same record, not of the previous one.
2. **Opcode names** (dispatch table at `0000:70F6`, indexed by `op - 0xF1`; VERIFIED):

   | op | handler | effect |
   |---|---|---|
   | F1 | `0000:70B5` | `inc word [3D58]`. INFERRED: next_level, i.e. **end level** (it is used at the end of seq 70, the level-door exit) |
   | F2 | `0000:7049` | sound u8: 0 = sets guard-notice [43AE] only; 1 = sound 23 (footstep) + notice; 2 = sound 8 (bump) + notice; 3 = sound 18 (drink); 4 = level music (32 on level 4, 41 otherwise except levels 13 and 15) |
   | F3 | `0000:70BB` | get item u8 (only arg 1 calls `0000:81AB`) |
   | F4 | `0000:7040` | `[42E2] = -1`. INFERRED: **knock down** (exe.rs calls it EndLevel) |
   | F5 | `0000:7037` | `[42E2] = +1`. INFERRED: **knock up** |
   | F6 | `0000:70CC` | **no-op** in the interpreter. INFERRED: "die" marker |
   | F7 | `0000:6FD3` | jump if feather fall `[409E] != 0`, otherwise skip the 2-byte target |
   | F8 | `0000:701A` | fall_x = i8, fall_y = i8 |
   | F9 | `0000:700A` | action = u8 |
   | FA | `0000:6FB7` | y += i8 (byte add) |
   | FB | `0000:6FA1` | x = char_dx_forward(i8) |
   | FC | `0000:6FFE` | curr_row++ (`0000:85EF`), then start_chompers (`0000:950B`) |
   | FD | `0000:6FF8` | curr_row--, then start_chompers |
   | FE | `0000:6FC8` | direction = NOT direction |
   | FF | `0000:6FE8` | seq pointer = u16 |

3. **Frame flag bits** (VERIFIED, see section 3.4): low 5 bits = x weight (foot offset); **0x20 = thin** (collision box narrowed by 4 on each side);
   **0x40 = needs floor** (check_on_floor runs only then); **0x80 = x parity** (half step for drawing). exe.rs guessed 0x80 = thin.
4. **Sword byte**: the drawing image table = `2 + (sword >> 6)` (`0000:C131`). The low 6 bits are the sword index.

---

## 1. State and globals (VERIFIED addresses; names INFERRED from use)

`Char` = the working character, 16 bytes at `ds:3D62`. `loadkid` copies Kid (`ds:4396`) to Char (`0000:718B`, memcpy 16),
and `savekid` copies it back (`0000:71A0`). Guard = `ds:431C`, Opp = `ds:4EBE`.

| offset | field | evidence |
|---|---|---|
| 3D62 | frame | play_seq writes the frame byte here (`0000:70E8`) |
| 3D63 | x (u8) | dx op (`0000:6FB1`) |
| 3D64 | y (u8) | dy op (`0000:6FC1`) |
| 3D65 | direction: 0 = right, 0xFF = left | flip = NOT (`0000:6FCB`). char_dx_forward negates dx when it is <0 (`0000:6F52`) |
| 3D66 | curr_col | determine_col (`0000:6E92`) |
| 3D67 | curr_row | up/down ops |
| 3D68 | action | F9 op |
| 3D69 / 3D6A | fall_x / fall_y (i8) | F8 op, fall_accel |
| 3D6B | room | goto_other_room (`0000:3B92`) |
| 3D6C | repeat | safe_step (`0000:652E`) |
| 3D6D | charid (0 = kid, 1 = shadow, 2 = guard, ...) | load_frame (`0000:6EA9`) |
| 3D6E | sword (2 = drawn) | |
| 3D6F | alive (<0 = alive) | control (`0000:6089`) |
| 3D70 | curr_seq (ds pointer) | play_seq |

Current frame record (copied by load_frame): `ds:66CA` image, `66CB` sword, `66CC` dx, `66CD` dy, `66CE` flags.

Controls (signed bytes): `[4650]` control_x, `[465A]` control_y, `[4E04]` control_shift (raw);
`[42E8]` control_forward, `[40C8]` control_backward, `[4362]` control_up, `[40DA]` control_down, `[4644]` control_shift2 (edge-triggered, see 2.1).
Others: `[66CF]` edge_type, `[4339]` curr_tile2 (last tile read), `[432D]` curr_tilepos, `[34EA]` grab_timer,
`[409E]` feather fall, `[34E2]` have_sword, `[2970]` keyboard mode, `[0FB2]` current_level.

Action values (behaviour VERIFIED in check_action `0000:7309`; names INFERRED): 0 stand, 1 run/jump (ground),
2 hang/climb, 3 in midair, 4 freefall, 5 bumped, 6 hang straight, 7 turn.

---

## 2. Input and decision logic

### 2.1 Reading the keys (VERIFIED)

- `0000:1533` (keyboard). The key-state array is indexed by scancode: up = 0x48, 7/Home = 0x47 (up+left), 9/PgUp = 0x49 (up+right),
  down = 0x50, keypad 5 = 0x4C (down), left = 0x4B, right = 0x4D. The int 9 handler `0C83:00DE` stores `scancode & 0x7F` and drops
  the E0 prefix, so keypad 8/4/6/2 are the arrows, and the grey Home / PgUp keys act like keypad 7 / 9. Keypad 1/End (0x4F), 3/PgDn (0x51)
  and 0/Ins (0x52) are never read. Shift comes from the BIOS flags, so NumLock and the keypad's fake Shift codes do not matter.
  - control_y = -1 for up/Home/PgUp, else +1 for keypad-5/down.
  - control_x = -1 for left/Home, else +1 for right/PgUp. So left beats right and up beats down.
  - control_shift -= (BIOS `0040:0017` & 3), the left/right Shift bits. It is negative while Shift is held.
- `0000:14C0` (do_paused) clears x, y and shift every tick before reading them. `0000:107C` is the joystick version.
- `0000:7BF7` read_user_control turns the raw axes into edge-triggered flags (one per flag):
  `if (fwd >= 0) fwd = (control_x == -1) ? (fwd == 0 ? -1 : fwd) : 0`. The pairs are backward with control_x == +1,
  up with control_y == -1, down with control_y == +1, and shift2 with control_shift < 0.
  States: **-1 = held (fires)**, **0 = released**, **1 = consumed** (stays 1 until the key is let go).
- `0000:7B48` release_arrows sets forward, backward, up and down to 0 and returns 1. Callers store the 1 in the flag they just used
  (for example `control_forward = release_arrows()`), so that action does not fire again until its key is released.
- `0000:7B08` user_control: if the kid faces right (direction >= 0), flip_control_x (`0000:7B24`: control_x = -control_x,
  swap forward and backward), then control(), then flip back. **So inside control(), control_x == -1 means "forward" and forward/backward are relative to facing.**
  The flags are kept between ticks in `rest_ctrl_1`/`save_ctrl_1` (`0000:7B79`/`0000:7B5A`).
- Order inside control_kid (`0000:7A19`) when current_level != 0: rest_ctrl_1, do_paused, read_user_control, user_control, save_ctrl_1.
  Level 0 is the demo: `0000:7A91` and then control().

### 2.2 control() dispatcher, `0000:607B` (VERIFIED)

If alive >= 0 (dead) and frame is 15/166/158/171: seq 71. Otherwise (alive) it checks these in order:

| condition | handler |
|---|---|
| action 5 or 4 | `0000:7B48` release_arrows |
| sword == 2 | `0000:6AD3` control_with_sword |
| charid >= 2 | `0000:857B` guard |
| frame 15, or 50..52 | `0000:61C4` **control_standing** |
| frame 48 | `0000:63FB` **control_turning** |
| frame < 4 | `0000:666B` **control_startrun** |
| frame 67..69 | `0000:667E` control_jumpup |
| frame < 15 (4..14) | `0000:64B5` **control_running** |
| frame 87..99 | `0000:67D4` control_hanging |
| frame 109 | `0000:6153` **control_crouched** |

### 2.3 control_standing, `0000:61C4` (VERIFIED)

1. If shift2 is held and shift is held, and check_get_item (`0000:655C`) returns true: return.
2. If not the kid, and down and forward are held: draw_sword (`0000:6A89`).
3. If have_sword: guard and sword checks (offguard `[432E]`, can_guard_see_kid `[5A8A]` >= 2, char_opp_dist (`0000:85A6`) in -10..89, ...). This can lead to draw_sword or back_pressed. Combat comes in a later pass.
4. **Shift held**: backward → back_pressed; else up → up_pressed; else down → down_pressed; else control_x == -1 **and** forward held → **safe_step**.
5. **Shift not held**: forward held → (keyboard mode and up held) ? standing_jump : forward_pressed;
   else backward → back_pressed; else up → (keyboard mode and forward held) ? standing_jump : up_pressed;
   else down → down_pressed; else control_x == -1 (forward still held but already consumed) → forward_pressed.

Handlers:
- `0000:647B` **forward_pressed**: `d = get_edge_distance()`. If edge_type == 1 (wall), curr_tile2 != 18 (chomper) and d < 8:
  safe_step if forward is held, otherwise nothing. Else **seq 1 (start run)**.
- `0000:642A` **back_pressed**: `control_backward = release_arrows()`. With have_sword, a guard in view (>= 2), char_opp_dist <= 0
  and distance_to_edge_weight >= 2: sword = 2, seq 89 (turn and draw). Otherwise **seq 5 (turn)**.
- `0000:6514` **safe_step**: shift2 = forward = 1. `d = get_edge_distance()`. If d != 0: repeat = 1, **seq 28+d** (d = 1..13 gives seq 29..41; d = 11, plain floor, gives seq 39, the full step).
  Else if edge_type != 1 and repeat != 0: repeat = 0, **seq 44 (step onto the edge)**. Else **seq 39**.
- `0000:6691` **standing_jump**: control_forward = control_up = 1, **seq 3**.
- `0000:62EE` up_pressed: if the tile at, behind or in front of the kid is 16 (level door left), level start room `[5902]` != drawn room `[40DE]`
  and the door is open `[40DC]`: go_up_leveldoor (`0000:63E0`: x = door x + 10, face left, seq 70). Else control_x == -1 → standing_jump.
  Else check_jump_up (`0000:66A2`, later pass).
- `0000:6333` **down_pressed**: control_down = 1.
  - If the tile in front is not floor and distance_to_edge_weight < 3: x = forward(5), re-determine col. No sequence starts: the kid just shuffles to the edge.
  - Else, if the tile behind is not floor and distance >= 8, through_tile = that tile, can_grab (`0000:7C9D`) is true, and (facing right, or the tile is not a gate, or the gate is open (modifier>>2 >= 6)):
    x = forward(distance - 9), **seq 68 (climb down)**.
  - Else crouch (`0000:6419`: **seq 50**, control_down = release_arrows()).

### 2.4 Other ground states (VERIFIED)

- `0000:63FB` **control_turning** (frame 48): if shift is not held, control_x == -1 and control_y >= 0: **seq 43 (run straight out of the turn)**.
- `0000:666B` **control_startrun** (frames 1..3): if control_y == -1 and control_x == -1: standing_jump (seq 3).
- `0000:667E` control_jumpup (frames 67..69): if control_x == -1 or forward is held: standing_jump.
- `0000:64B5` **control_running** (frames 4..14):
  - control_x == 0 and frame is 7 or 11: forward = release, **seq 13 (stop)**. The kid can only stop on those two frames.
  - else control_x > 0 (backward): backward = release, **seq 6 (run turn)**.
  - else control_y == -1 and up held: **running_jump** (`0000:6963`).
  - else down held: down = 1, **seq 26 (crouch while running)**.
- `0000:6963` **running_jump**: only if frame >= 7, otherwise nothing happens. For the next 2 columns ahead of `get_tile_div_mod_m7(x + forward(4))`
  (direction step from `ds:22A2` = {-1, +1} for left/right): if a tile is spike (2) or not floor,
  `adj = distance_to_edge(x+fwd4) + 14*i - 14`. If adj < -8 or adj >= 2 (unsigned compare): return if adj < 128, otherwise adj = -3.
  Then x = forward(adj + 4) and stop looking. Then control_up = release, **seq 4**.
- `0000:6153` **control_crouched** (frame 109): a special case for level-1 music (`[4CE8]`), then: shift2 held → check_get_item;
  control_y != +1 (down released) → **seq 49 (stand up)**; else forward held → forward = 1, **seq 79 (crouch hop)**.

### 2.5 Sequences on flat ground (VERIFIED from the seq table and the bytecode)

| seq | ds / file | started by | contents (frames; dx = x op before the frame) | ends with |
|---|---|---|---|---|
| 1 start run | 1987 / 119367 | forward_pressed | act1, f1-f4, then f5-f14 (run cycle) | loops f7..f14 at ds:1995 |
| 84 run | 1982 / 119362 | (entry point into the loop) | act1, jmp 1995 | run loop |
| 2 stand | 19B4 / 119412 | end of most ground seqs | act0, f15 | loops |
| 5 turn | 1B4D / 119821 | back_pressed | act7, **flip**, dx+6, f45..f52 (f48 = turn frame) | seq 2 |
| 43 run after turn | 1B67 / 119847 | control_turning | act1, dx-1, jmp ds:1989 (f1) | start-run path |
| 13 stop run | 1D63 / 120355 | control_running | act1, f53-f56, f49-f52 | seq 2 |
| 6 run turn | 1B6E / 119854 | control_running | act1, f53..f65, dx-14, **flip** | jmp ds:19A9 (run loop at f13) |
| 3 standing jump | 1AA7 / 119655 | standing_jump | act1, f16..f33, F4 at landing | seq 2 |
| 4 running jump | 1AE1 / 119713 | running_jump | act1, f34..f44, F4 at landing | run loop (ds:1995) |
| 26 crouch while running | 1B18 / 119768 | control_running | act1, f107, f108, f109 x3 | crouch loop ds:1B2A |
| 50 crouch | 1FBB / 120955 | crouch() | act1, f107, f108, f109 | loops f109 (ds:1FC3) |
| 49 stand up | 1FC7 / 120967 | control_crouched | act5, f110..f119 | seq 2 |
| 79 crouch hop | 1B2F / 119791 | control_crouched | act1, f110-f112, f108, f109 | crouch loop |
| 29..41 safe step d | e.g. 39 at 1EEE / 120750 | safe_step | act1, f121..f132 with dx summing to d | seq 2 |
| 44 step onto edge | 1E6D / 120621 | safe_step | f121..f126, f86, F4, f116..f119 | seq 2 |
| 68 climb down | 1D00 / 120256 | down_pressed | act1, f148..f141, dx-5 dy+63 **down**, act3, ... act2 | hang (ds:1CB8) |

Sums of the dx ops (VERIFIED arithmetic; the meaning "distance travelled" is INFERRED, because frame dx and weight also move the foot):
one run cycle f7..f14 = 5+1+2+4+5+2+3+4 = **26 units per 8 ticks**; seq 3 = 39; seq 4 = 67; seq 39 = 11 (the d of safe step = the dx sum, for example seq 29 = 1, seq 41 = 13).

---

## 3. Per-tick physics

### 3.1 Tick order, play_kid_frame `0000:0ED4` (VERIFIED)

loadkid_and_opp (`71DF`), load_fram_det_col (`6E80`), `84B1`, **play_kid** (`792A` → control_kid `7A19` → control), then, if Char.room != 0:
**play_seq** (`6F90`), **fall_accel** (`72B8`), **fall_speed** (`72EA`), load_frame_to_obj (`C110`), load_fram_det_col (`6E80`),
set_char_collision (`745F`), bump_into_opponent (`5122`), check_collisions (`53AC`), check_bumped (`5645`), check_gate_push (`5BDB`),
**check_action** (`7309`), check_press (`7E2E`), check_spike_below (`7EE1`), and if `[3D76]` == 0: check_spiked (`7394`) and check_chomped_kid (`5B13`).
Then check_knock (`4F7A`) and savekid (`71A0`).
So **input is handled before play_seq in the same tick**: a new sequence shows its first frame in the tick the key was read.

### 3.2 Sequence step, play_seq `0000:6F90` (VERIFIED)

It reads bytes from curr_seq. Bytes 0xF1..0xFF are executed as ops (table in section 0) and the loop continues. The first byte below 0xF1 becomes
Char.frame and the function returns. So **exactly one frame per tick**, and all ops before that frame (dx, dy, flip, action, sound, ...) take effect in the same tick, before the frame is shown.
- dx: `x = (u8)(x + (direction < 0 ? -d : d))` (char_dx_forward `0000:6F4F`). x wraps as a byte.
- dy: `y = (u8)(y + d)`, not mirrored.
- The frame record's own dx/dy are **not** added to Char.x/y. They only offset the drawn position and the foot position (3.4).

### 3.3 Gravity (VERIFIED)

- fall_accel `0000:72B8`: only when action == 4. Normal fall: `fall_y += 3`, capped at **33**. Feather fall: `fall_y += 1`, capped at **4**.
- fall_speed `0000:72EA`: **every tick** `y += fall_y` (byte). When action == 4 also `x = forward(fall_x)` and re-determine col.
- Falls start from seq data: seq 7/18/19/21 set `fall(x,15)` then go to seq 12 (act4, f106 loop). Starting fall_y = 15. fall_x = 1/2/3 by fall type (seq 7: 1, 18: 2, 21: 3).
- do_fall `0000:5E98`: when fall_y >= 31 the first time, sound 1 (scream). If `y < y_land[row+1]`: check_grab. Else: tile at char is wall → in_wall;
  floor → land (`0000:5EF8`); otherwise row++ (falls into the next row).
- land `0000:5EF8`: y = y_land[row+1]. fall_y < 22: soft landing, seq 17 (sound 17). fall_y < 33: seq 20 (sound 16, take_hp(1); death if it returns 1).
  fall_y >= 33: take_hp(100), seq 22. Landing on spikes (tile 2) → `0000:601F`. Then play_seq and fall_y = 0.

### 3.4 Frame flags, foot position and floor checks (VERIFIED)

- dx_weight `0000:6F31`: **foot x = char_dx_forward(frame.dx - (frame.flags & 0x1F))**.
- determine_col `0000:6E89`: curr_col = get_tile_div_mod_m7(foot x) (see 4).
- check_action `0000:7309`: action 6 → nothing. Action 5 → check_on_floor only on frame 109. Action 4 → do_fall.
  Action 3 → check_grab (`7755`) on frames 102..105. Action 2 → nothing. Actions 0, 1, 7 → check_on_floor.
- check_on_floor `0000:7551`: **only if flags & 0x40**. Tile at (room, curr_col, curr_row): wall (20) → in_wall (`7809`, pushes x out of the wall).
  Not floor → start_fall (`75F5`), except for a level-12 special floor.
- start_fall `0000:75F5`: sword = 0, row++, start_chompers. The sequence depends on the frame the kid was on:
  frame 9 → seq 7, 13 → 19, 26 → 18, 44 → 21, 81..85 → 19 with x += 5, 150..179 (sword) → 81/82/83, any other frame → 7.
  Then play_seq and re-determine col. If now inside a wall → in_wall. If the tile in front is a wall: x -= 1, but frame 44 with distance < 6 starts seq 104 instead.
- set_char_collision `0000:745F`: box from the image width and height. Left x = obj_x/2 + 58 (minus half the width when facing right);
  right = left + half the width. **flags & 0x20 → left += 4, right -= 4** (thin). Top row and bottom row come from y_to_row_mod4. Columns are clamped to 0..9.
- load_frame_to_obj `0000:C110`: obj_x = `2*char_dx_forward(frame.dx) - 116`, plus 1 when `(flags ^ direction)` has bit 7 clear (**0x80 = parity**).
  obj_y = y + frame.dy. Screen x = obj_x * 320 / 280 (`0000:C2CA`).

### 3.5 Edge and wall distance (VERIFIED)

- distance_to_edge(x) `0000:78D0`: tile_mod of get_tile_div_mod_m7(x). Facing right → 13 - mod. That is the distance from the foot to the tile edge in front (0..13).
  distance_to_edge_weight `0000:78C6` = distance_to_edge(foot x).
- get_edge_distance `0000:5A24` sets edge_type `[66CF]` and returns the distance:
  - Tile at the kid is wall-like (wall_type `0000:7D0B`: gate 4 / 7 / doortop 12 → 1, mirror 13 → 2, chomper 18 → 3, wall 20 → 4):
    d = dist_from_wall_forward (`0000:5DB8`). If d >= 0: d < 14 → edge_type 1 (wall), otherwise edge_type 2 with d = 11.
  - Else the tile in front: doortop (12) while facing right, or loose floor (11) → edge_type 0, d = distance_to_edge_weight.
    Wall-like → as above. Tiles 6, 22, 10 → d = distance_to_edge_weight, edge_type 0 if d != 0, otherwise 2 with d = 11.
    Floor → edge_type 2, d = 11. Not floor → edge_type 0, d = distance_to_edge_weight.
  - Wall offsets: `ds:24CE` wall_dist_from_left = {0,10,0,-1,0,0}, `ds:24D4` wall_dist_from_right = {0,0,10,13,0,0}, indexed by wall_type.
- Bumping (`0000:5645` check_bumped, `0000:57F0` bumped): x is pushed back by the collision delta. Then a bump sequence:
  seq 47 (normal), 46 (hard, for frames 24, 25, 40..42 and 102..106), 45 (bump-fall, via `0000:588C`, x -= 4), 64/65 with the sword.
  The collision deltas come from check_collisions `0000:53AC` (now VERIFIED, see section 9).

---

## 4. Tiles and coordinates (VERIFIED)

- **x**: unsigned byte, **14 units per column**, 10 columns per room. Column c starts at x = 58 + 14c (`ds:24BA` x_bump = -12, 2, 16, 30, 44, 58, 72, ...; index = col + 5).
  - get_tile_div_mod `0000:712C`: lookup tables `ds:22BA` (col) and `ds:23BA` (mod), equivalent to **col = floor((x - 58) / 14), mod = (x - 58) mod 14** for x = 0..255.
  - get_tile_div_mod_m7 `0000:711A` is the same with x - 7. Note: x < 7 makes a negative index, which reads the bytes before the table (an original quirk).
- **Room change** `0000:3B7A` goto_other_room(dir): room = link[dir] (also when the link is 0), then left (0): x += 140; right (1): x -= 140;
  up (2): y += 189; down (3): y -= 189. For up **and** down the row is recomputed with y_to_row_mod4 (`0000:3BC9`, `0000:3BE3`;
  round 1 said only up). Returns the opposite direction. Room links: `ds:585E + 4*room` = left, right, up, down. Trigger rules: section 10.
- **y**: unsigned byte, **63 per row**, 3 rows per room (189). y_land `ds:22B0` = {-8, 55, 118, 181, 244}. A kid standing on row r has y = y_land[r+1] = **55 + 63r** (land `0000:5F0C`).
  - y_to_row_mod4 `0000:716F` = ((y + 60) / 63) % 4 - 1.
  - `0000:7142` = the largest i in -1..3 with y_land[i+1] <= y.
  - Other row tables nearby: `ds:2290` {62, 125, 188, 251}, `ds:2298` {2, 65, 128, 191, 254}, `ds:22A6` {-60, 3, 66, 129, 192} (not traced).
- **get_tile(room, col, row)** `0000:6D42`: find_room_of_tile (`0000:6D99`) moves col<0 / col>=10 / row<0 / row>=3 into the neighbour room (repeats as needed).
  tilepos = `ds:2288`[row] (0, 10, 20) + col. tile = room_tiles[tilepos] & 0x1F. **Room 0 (outside the level) gives tile 20 (wall).**
- Helpers: get_tile_at_char `0000:7449` (curr_col); in front `0000:7848` (col + dir_front, dir_front at `ds:22A2` = {-1, +1});
  two ahead `0000:786C`; behind `0000:78A2` (dir_behind at `ds:22A4` = {+1, -1}).
- tile_is_floor `0000:7364`: false for 0 (empty), 9, 12 (doortop), 20 (wall), 26..29 (lattices). True for everything else.

---

## 5. Entry points for later passes (found, not analysed)

check_jump_up `0000:66A2`, jump_up_or_grab `66E7`, grab_up_no_floor_behind `6733` (seq 16), jump_up `6752` (seq 14/28), grab_up_with_floor_behind `6914` (seq 8/24),
control_hanging `67D4` (seq 25), climb up `684B` (seq 10/73), hang release `68B2` (seq 23/11), check_grab `7755` (seq 15, grab_timer = 12), can_grab `7C9D`,
do_fall `5E98`, land `5EF8`, spiked `601F` (seq 51), start_fall `75F5`, in_wall `7809`, check_collisions `53AC`, check_bumped `5645`, bumped `57F0`,
check_gate_push `5BDB`, check_press `7E2E`, check_spike_below `7EE1`, check_spiked `7394`, check_chomped_kid `5B13`, check_knock `4F7A`,
bump_into_opponent `5122`, control_with_sword `6AD3`, draw_sword `6A89`, guard control `857B`, get_item `65AA`, take_hp `73F9`, play_sound `12F0`,
start_chompers `950B`, release_arrows `7B48`, room change `3B7A`, seq setters `5E72` (Char) / `5E85` (Opp).

Tile mechanics (trobs, mobs, gates, buttons, loose floors, spikes, chompers, exit door, potions; check_press, check_spike_below,
check_spiked, spiked, check_chomped_kid, check_gate_push, check_knock, start_chompers, trigger_button) are in `docs/tiles.md`.
Guards, sword fighting, shadow/skeleton/Jaffar/mouse, hit points, death and level flow (control_with_sword, guard control `857B`,
bump_into_opponent, take_hp, the level-tail guard fields 2119..2286) are in `docs/combat.md`.

## 6. Not verified

- The 12 Hz tick rate (taken from the brief, not from the timer code).
- Whether `84B1` and `71DF` do more than check_killed_shadow and loading Kid/Opp.
- The joystick thresholds.

---

# Round 2

## 7. Facing and drawing (VERIFIED unless marked)

### 7.1 Which way the images face
- **Every KID.DAT image faces LEFT** as stored. Checked visually: resource 415 (image 14 = frame 15, standing, 12x41 px) and 410
  (image 9, running) decoded with `crates/pop-core/examples/dump_image.rs`: face, nose and chest are on the left side.
- **Char.direction: 0 = right, 0xFF (any negative) = left** (char_dx_forward `0000:6F4F` negates dx when direction < 0).
- **The engine mirrors the image when direction == 0 (facing right)**: add_midtable `0000:AE6C`:
  `if (obj_direction == 0 && chtab_flip_ok[chtab]) blit += 0x80`. chtab_flip_ok = `ds:259C` = {1,0,1,1,1,1,0,0,0,0,0,1,...}
  (chtab 0 sword, 2/3/4 kid, 5 guard can flip; chtab 1 cannot). Facing left = image drawn as stored.

### 7.2 Pipeline
1. load_frame_to_obj `0000:C110` (Char = the character being drawn; draw_kid `0000:BFB8` = loadkid + this, draw_guard `0000:BFEC`):
   - obj_direction `[4DF6]` = Char.direction; obj_id `[42E4]` = frame.image; obj_chtab `[4CD8]` = **2 + (frame.sword >> 6)**.
     The guard frame table has 0xC0 in its sword bytes (`ds:14AF`: `0C CD 02 01 00`), so guards get chtab 5. The kid gets 2.
   - **obj_x** `[4642]` (word, 280-wide space) = `2 * char_dx_forward(frame.dx) - 116` = `2*(x + (dir==right ? dx : -dx)) - 116`,
     then **+1 if bit 7 of (frame.flags XOR direction) is 0**, i.e. +1 when (facing right and flags&0x80 == 0) or (facing left and flags&0x80 != 0).
   - **obj_y** `[4651]` (byte) = `(u8)(y + frame.dy)`: the **bottom** line of the image.
2. draw_kid order (`0000:BFB8`): loadkid, load_fram_det_col, load_frame_to_obj, `0000:812E` (**if the tile at the char is 5, Char.y += 1**,
   on the temporary copy), set_char_collision, ... It runs after obj_y was computed and set_char_collision reads obj_y, so it does not move
   the image or the box; what reads it later (`4DEA`, `4EB9`, `7F79`) was not traced.
3. add_objtable `0000:C050` stores obj_x as xh = obj_x div 8, xl = obj_x mod 8 (`0000:7258`, floor division, exact for negatives) and obj_y.
4. draw_objtable_item (`0000:BEF0` reload, kid case `0000:BDD0`) calls add_midtable(chtab, id+1, xh, xl, obj_y, blit, 1) `0000:AE02`:
   - entry.y = **obj_y - image_height + 1** (top line; `0000:AE61`, image header word 0 = height).
   - flip bit as in 7.1.
5. draw_mid `0000:B0D4`: x = xh*8 + xl (= obj_x); if chtab != 0, **x = obj_x * 320 / 280** (`0000:C2CA`, long multiply then divide);
   **if flipped, x -= image_width** (`0000:B22D`, header word 1 = width). The image is then mirrored in place if its cached state
   differs (`0000:B24F`, bit 15 of header word 2). y is passed on unchanged (INFERRED: screen line, no scaling was seen; the blitter `0000:B343` was not read).

### 7.3 Formula (kid, guard, shadow: chtab != 0)
```
dxs   = (direction == 0) ? frame.dx : -frame.dx
obj_x = 2*(Char.x + dxs) - 116 + ((((frame.flags ^ direction) & 0x80) == 0) ? 1 : 0)
sx    = trunc(obj_x * 320 / 280)
left  = (direction == 0) ? sx - w : sx          right edge = left + w (exclusive)
bottom= (u8)(Char.y + frame.dy)                 top = bottom - h + 1
```
Facing left: sx is the **left** edge, image as stored. Facing right: sx is the **right** edge (exclusive), image mirrored.
Example: Char.x = 58 + 14c, dx 0, no parity -> obj_x = 28c, sx = 32c = the left edge of column c on screen (32 px per column).
- **Sword** (`0000:84EC`, drawn when Char.sword != 0 or frames 229..237, and frame.sword & 0x3F != 0): table `ds:1726`, 3 bytes per entry
  {image, dx, dy}. obj_x = screen_x(obj_x of the body) + (direction < 0 ? -dx : dx) (`0000:6F70`), obj_y += dy, chtab 0.
  chtab 0 is **not** rescaled in draw_mid, so the sword dx is in screen pixels. The same flip rule applies.
- Shadow (charid 1) uses the kid images with chtab 2 (same formula; it only differs in blit mode). INFERRED from load_frame_to_obj having no charid test.

### 7.4 Collision box (set_char_collision `0000:745F`)
`half = (w + 1) / 2` (pixels), `char_x_left = obj_x/2 + 58`, minus half when facing right; `char_x_right = left + half`.
Coll copies `[3D60]` left_coll / `[4336]` right_coll get `+4 / -4` when flags & 0x20. char_height `[4656]` = image height. So the box uses
the image width in pixels against x units (a quirk), and its side matches the drawing (the image extends right of obj_x when facing left).
Globals: `[3D5C]` char_x_left, `[3D50]` char_x_right, `[3D60]` char_x_left_coll, `[4336]` char_x_right_coll, `[4ED6]` top y, `[5ADA]` top row, `[34E4]` bottom row.

## 8. Level start (do_startpos `0000:4A0F`, set_start_pos `0000:4AF6`; VERIFIED)

Level file bytes at 2112 are copied to `[5902]` start room, `[5903]` start tile (0..29), `[5904]` start_dir.
- Char.room = start room; col = tile % 10, row = tile / 10.
- **Char.x = x_bump[col + 5] + 14 = 72 + 14*col** (`ds:24BF` = x_bump + 5).
- **Char.direction = NOT start_dir** (`0000:4A86`). Then the first sequence:
  - **level 1: seq 7 (fall)**, after get_tile(room 5, col 2, row 0) and trigger_button(0, 0, -1) (`0000:924B`). Level 1 data: start
    room 1, tile 0, dir 255 -> direction = 0, **facing right**, standing on row 0 over an empty tile. **Falling is the original behaviour**:
    seq 7 shows f102..f105 with dy +3/+6/+9/+12 (action 3), then fall(1,15) and seq 12. Worked through: y 55 -> 58 (first frame shown at
    start) -> 64 -> 73 -> 85; then action 4: fall_y 18, y 103, row becomes 1 (tile empty); next tick fall_y 21, y 124 >= 118 -> lands on
    row 1 col 0 (tile 19 torch = floor) with fall_y 21 < 22 -> **seq 17, soft landing into a crouch**, no damage.
  - level 13: seq 84 (run).
  - all other levels: **seq 5 (turn)**. Its flip op restores start_dir, so the kid ends facing start_dir. The dx ops of seq 5 add +7
    forward in the new direction: standing x = 72 + 14*col + 7*(start_dir == 0 ? +1 : -1). Start tiles point at the right half of the
    entry door (tile 17), for example level 2 room 5 tile 13.
  - level 3 with checkpoint flag `[4CAC]`: start room 2, tile 6, dir 0xFF, and the tile at (room 7, col 4, row 0) is cleared.
- set_start_pos: y = y_land[row + 1]; alive = -1; charid 0; zeroes knock, feather, fall_x, fall_y, sword, offguard and others; then
  **play_seq once** (the first frame is shown at level start). Level 7 in room 17: goto_other_room(3) (the kid starts above and drops in). Then savekid.
- HP: `[4646]` (hitp_beg_lev) on normal levels, 4 on level 0.
- The 4 bytes after start_dir (2115..2118) are not read here; they were not traced.

## 9. Collisions and walls (VERIFIED)

### 9.1 check_collisions `0000:53AC`
Skipped for action 7 (turn). Otherwise:
- move_coll_to_prev `0000:5487`: copies last tick's row data to prev arrays (`[4C4E]` room, `[5CDE]` flags), picking the current, above or
  below row data to match the change of curr_row, then clears the row arrays to -1.
- Checked columns: left = col_m7(char_x_left_coll) - 1, right = min(col_m7(char_x_right_coll) + 2, 11).
- get_row_collision_data `0000:552D` for rows curr, curr+1, curr-1. Per column: `tile_left = x_bump[col+5] + 7` (= 65 + 14col, adjusted
  by ±140 when the tile is in a side room, `0000:57AD`).
  - left_wall_x = wall_dist_from_left[wt] + tile_left (`0000:55CE`), or 0xFF if wall_type 0.
  - right_wall_x = tile_left - wall_dist_from_right[wt] + 13 (`0000:5607`), or 0 if wall_type 0.
  - flags = (left_wall_x < char_x_right_coll ? 0x0F : 0) | (right_wall_x > char_x_left_coll ? 0xF0 : 0).
- For column 9 down to 0 with the same room as last tick: low nibble went 0 -> non-zero: bump_col_left_of_wall `[436D]` = col;
  high nibble went 0 -> non-zero: bump_col_right_of_wall `[43AC]` = col. So a bump fires only on the tick the box **starts** overlapping.

### 9.2 check_bumped `0000:5645`
Skipped when action is 2 or 6 or frame is 135..148 (climbing). If bump_col_left_of_wall >= 0 (`56B2`): only when facing right or sword == 2,
and is_obstacle at that column (current row): **bumped(left_wall_x - char_x_right_coll, -1)**. Else if bump_col_right_of_wall >= 0 (`567A`):
facing left or sword == 2, is_obstacle: **bumped(right_wall_x - char_x_left_coll, 0)**.

is_obstacle `0000:5726` (on curr_tile2): potion 10 -> no; gate 4 -> **can_bump_into_gate** (`0000:59FF`: `(modifier >> 2) + 6 < char_height`);
chomper 18 -> only if modifier == 2; mirror 13 -> when kid (charid 0), frame 39..43 and facing left: the tile becomes 0x56 and `[42E0]` = -1
(jump through the mirror), not an obstacle. Every other wall_type tile -> obstacle.

### 9.3 bumped `0000:57F0`
Only if alive and frame != 177. x += delta. Pushing left (dir < 0): if curr_tile2 is wall, look one column left; pushing right: if it is
doortop 12, doortop-with-floor 7 or wall, look one column right. Then if that tile is floor -> bumped_floor `58C8`, else bumped_fall `588C`.
- **bumped_fall**: x = forward(-4); if action 4: fall_x = 0, else **seq 45** (bump into wall and fall). Sound 8.
- **bumped_floor**: if sword != 2 and y_land[row+1] - y >= 15 (unsigned): bumped_fall. Else y = y_land[row+1]; if fall_y >= 22: x = forward(-5)
  and nothing else; else fall_y = 0 and (alive): sword: same direction -> seq 65 and x = forward(1), otherwise seq 64; no sword:
  **frame 24, 25, 40..42 or 102..106 -> seq 46 (hard bump)**, else **seq 47 (soft bump: dx-4, f50..f52, stand)**. Then play_seq and sound 8.
- Running into a wall: the run frames push the box past the wall face -> seq 47 with x pushed back to the face. A running jump that hits a
  wall high (more than 14 above the floor line) -> seq 45 and a fall. Walking: forward_pressed does a safe step instead of a run when the
  wall is within 8 (section 2.3), so a walk normally does not bump. A closed gate counts while (modifier>>2)+6 < image height.

### 9.4 dist_from_wall_forward `0000:5DB8` (used by get_edge_distance)
Argument = tile type (tile_col `[4370]` is the column of the last get_tile). Gate that can_bump_into_gate says no -> -1. wall_type 0 -> -1.
`tile_left = x_bump[tile_col+5] + 7`. Facing left: `wall_dist_from_right[wt] + char_x_left_coll - tile_left - 13`;
facing right: `wall_dist_from_left[wt] + tile_left - char_x_right_coll`. **It uses the collision box, not the foot.**

### 9.5 in_wall `0000:7809`
`d = distance_to_edge_weight`; if d >= 8 or the tile in front is wall 20: d = 6 - d, else d += 4. x = forward(d), redetermine column,
re-read the tile at the char. Called from check_on_floor (tile at char is wall; afterwards the normal floor test runs on the new tile),
from do_fall (landing column is a wall) and from start_fall.

## 10. Room change (VERIFIED)

exit_room `0000:3ABB` is called from the main loop (`0000:0948`), after the kid's tick. If exit_room_timer `[5CE8]` != 0 it only decrements it.
Else loadkid, load_frame_to_obj, set_char_collision, then leave_room `0000:3BF8`:
1. **Up**: action not 5, 4 or 3, and y as a signed byte in -16..9 (y <= 9 or y >= 240).
2. Else **down**: y >= 211 (unsigned; so a falling kid with y 241..255 also goes down).
3. Else no change for frames 135..149 (climb), 110..119 (stand up), 150..162 and 166..168 (sword), or action 7.
4. Else facing left: char_x_left <= 54 -> **left**; char_x_left >= 198 -> **right**.
   Facing right: if the tile at (col 9, curr_row) is not 7/12 (doortop) and char_x_right >= 201 -> **right**; else char_x_right <= 57 -> **left**.
   These use the image box from 7.4 (not the coll copies, not the foot).
5. Side effects before the move: left: mirror music / level-3 checkpoint / Jaffar exit; right: sword disappears / meet Jaffar (`3D74`, `3DA2`);
   down on level 6 in room 1 returns -2 (falls to the next level) without moving.
Then goto_other_room(dir), savekid, next_room `[4E06]` = Char.room. goto_other_room is also called by the guard version `0000:3A92`
and by set_start_pos (level 7).

## 11. Jump up, grab, hang, climb (VERIFIED)

Helpers: get_tile_above_char `0000:7D4D` (col, row-1), get_tile_behind_above `7D68`, get_tile_front_above `7D91`; through_tile `[4C4C]`.
back_delta_x `7DBA` = (facing left ? d : -d).

- **can_grab** `0000:7C9D` (target = curr_tile2, through = through_tile): false if through is wall 20, or through is doortop 12 and facing right,
  or through **is floor**, or target is loose floor 11 with modifier != 0, or target is 7 and facing left, or target is not floor. Else true.
- **check_jump_up** `0000:66A2` (from up_pressed): control_up = release. through = tile above; target = tile front-above:
  can_grab -> grab_up_with_floor_behind. Else through = tile behind-above, target = tile above: can_grab -> jump_up_or_grab, else jump_up.
- **jump_up_or_grab** `66E7`: d = distance_to_edge_weight. d < 6 -> jump_up. Else tile behind not floor -> grab_up_no_floor_behind.
  Else x = forward(d - 14), redetermine column, grab_up_with_floor_behind.
- **grab_up_no_floor_behind** `6733`: x = forward(distance_to_edge_weight - 10), **seq 16**.
- **jump_up** `6752`: control_up = release. d = get_edge_distance; if d < 4 and edge_type == 1: x = forward(d - 3).
  Tile at (col = get_tile_div_mod(dx_weight - 6), row - 1): if it is not wall and not floor -> **seq 28** (jump up, nothing to grab),
  else **seq 14** (jump up and bump the ceiling, knock up, then seq 11).
- **grab_up_with_floor_behind** `6914`: d = distance_to_edge_weight. If d < 4 and get_edge_distance < 4 and edge_type != 1:
  x = forward(d), **seq 8**. Else x = forward(d - 4), **seq 24**. Both end in seq 9 (hang, action 2).
- **check_grab** `0000:7755` (action 3, frames 102..105, and do_fall while y < y_land[row+1]): needs **raw Shift held** `[4E04] < 0`,
  fall_y < 32, alive, and y_land[row+1] <= y + 25. Then x = forward(-8), redetermine column; if can_grab (through = above, target = front-above)
  fails, restore x. Else x = forward(distance_to_edge_weight), y = y_land[row+1], fall_y = 0, **seq 15**, play_seq, **grab_timer = 12**, sound 9.
  grab_timer is decremented once per tick in control_kid (`0000:7A39`).
- **control_hanging** `0000:67D4` (frames 87..99): dead -> hang_fall. grab_timer == 0 and control_y == -1 (up) -> can_climb_up.
  Else raw Shift held: if action != 6 and (tile at char is wall, or facing left and it is 7/12): sound 8 if grab_timer == 0, **seq 25**
  (hang against the wall, action 6). Otherwise keep hanging while the tile above is floor, else hang_fall. Shift released -> hang_fall.
- **can_climb_up** `684B`: control_up = shift2 = release. Tile above: (mirror or chomper and facing right) or (gate, facing left, (modifier>>2) < 6)
  -> **seq 73** (climb up into a closed gate and drop back, dx-7, then seq 11); else **seq 10** (climb up: up op, ends standing, seq 2).
- **hang_fall** `68B2`: control_down = release. Tile behind and tile at char both not floor -> **seq 23** (drop, fall(0,12), seq 12).
  Else if the tile at char is wall, or facing left and it is 7/12: x = forward(-7). Then **seq 11** (drop and land standing).
- Seq 9 (hang) swings f91..f99 and back; if nothing happens it ends in seq 11 (drop). Climb down is seq 68 (section 2.3, down_pressed),
  which ends inside seq 9 (`ds:1CB8`).

## 12. Errors found in crates/ (round 2; not fixed here)

1. `pop-app/src/kid.rs` render: the mesh is drawn with scale.x = -w when `facing_left` and extends **left** of the anchor; the original
   draws facing-left images **unmirrored, extending right** of obj_x, and mirrors only facing right, extending left (7.3). Unless the
   camera or the mesh UVs flip x, both the mirroring and the anchor side are reversed. The 0x80 parity (+1 in obj_x) is not applied.
2. `pop-core/src/kid.rs` `spawn`: x should be 72 + 14*col (not 75 + 14*col), direction = NOT start_dir, and the first sequence is seq 5
   (levels other than 1 and 13), seq 7 on level 1 (so the fall seen in level 1 is correct, but it should be seq 7 and land in seq 17), seq 84 on level 13.
3. `dist_from_wall`: uses the foot and `58 + 14col`; the original uses the collision box edges and `65 + 14col` (9.4).
4. `check_on_floor` / `do_fall` / `start_fall`: a wall tile must call in_wall (9.5), not count as floor or be skipped.
5. `check_room_change`: the trigger is leave_room (section 10: box edges 54/198/201/57, y <= 9 or >= 240 for up unless action 3/4/5,
   y >= 211 for down, skipped for climb/stand-up/turn frames), and the original moves even when the link is 0.

**Status (port):** items 2-5 above are fixed in crates/ (spawn, box-based dist_from_wall, in_wall, leave_room). Item 1 (drawing) was fixed earlier. Two readings remain INFERRED in `pop-core/src/kid.rs`: the column lookup after a bump (9.3) and the climb-down condition (2.3, can_grab target).

**Rendering note (port):** logical column c spans x = 65 + 14c .. 79 + 14c (col_m7 = x - 7 in determine_col, tile_left = x_bump + 7 in the collision code), i.e. screen px 32c + 16 .. 32c + 48 - half a tile right of the 7.3 example `sx = 32c`. pop-app draws tile column c at world x = c .. c + 1, so sprites are shifted by -16 px (-0.5 world units) to keep the figure on the tile the physics uses.
Hanging is mirror symmetric to within 2 px: the hang frame 91 hands are 4..6 px over the ledge facing left and 1..3 px facing right
(the core pairs x <-> 283 - x around a column, plus the 8/7 truncation). That is the original's own rounding, not an anchor error
(pop-app test `hang_anchor_matches_the_original_formula`).

