# Tile mechanics (PRINCE.EXE v1.1): trobs, mobs, gates, buttons, loose floors, spikes, chompers

Source: disassembly of `Original_data/PRINCE.EXE` with `tools/disasm`, plus a tally of `Original_data/LEVELS.DAT`.
Conventions as in `docs/mechanics.md` (addresses, VERIFIED / INFERRED, Pascal argument order: arguments are pushed
left to right, so the **last** argument is at `[bp+6]`). Names follow SDLPoP where the code matches; every claim below
was checked against this binary. "tick" = one call of play_frame (`0000:090A`).

Notation for state: `mod` = the tile's runtime modifier byte (bg byte), `type` = the trob's type byte.
All modifier arithmetic is **unsigned 8-bit** unless marked.

---

## 1. Runtime level state

### 1.1 Memory layout (VERIFIED)
- load_level `0000:0E97` reads resource `2000 + level`, **2305 bytes, raw**, to `ds:50C2` (the level struct), then calls
  alter_mods_allrm `0000:B5FF`. Called from the level init at `0000:4948`, which right after sets drawn_room `[40DE]`,
  mobs_count `[40CC]` and trobs_count `[50B8]` to 0 (`0000:4959..4961`) and leveldoor_open `[40DC]` = 0 (`0000:4979`).
- get_room_address(room) `0000:BAD4`: curr_room `[5AA6]` = room; curr_room_tiles `[66C8]` = `50A4 + 30*room`
  (fg of room 1 = ds:50C2), curr_room_modif `[60C6]` = `5374 + 30*room` (bg of room 1 = ds:5392). Room 0 leaves the pointers unchanged.
- doorlinks1 = ds:5662 (`[4C98]`, set at `0000:024F`), doorlinks2 = ds:5762 (`[4C9A]`), room links ds:5862 (`585E + 4*room`),
  used_rooms = ds:58C2 (level byte 2048).
- get_curr_tile(tilepos) `0000:9639`: curr_modifier `[4366]` = modif[tilepos]; curr_tile `[4E0B]` = fg[tilepos] & 0x1F.

### 1.2 Load-time conversion, load_alter_mod `0000:B656` (VERIFIED)
alter_mods_allrm runs it for every tile of rooms 1..used_rooms (`0000:B606..B62A`). Only the **bg byte** changes:

| tile | raw bg (data) | runtime mod |
|---|---|---|
| 4 gate | `== 1` | **188** (open) `0000:B697` |
| 4 gate | anything else (2, 0, 188) | **0** (closed) `0000:B6A0` |
| 11 loose floor | any | **0** |
| 10 potion | kind k | **k << 5** (kind in bits 5..7, bubble frame in bits 0..4) `0000:B6AB`. Level 15: if (room, tilepos) matches copy-protection entry `[4394]` (tables ds:00D6 rooms / ds:00F2 tiles) -> **0xC0** (kind 6) |
| 20 wall | b | `(b << 7)` then OR 1 / 2 / 3 for wall neighbours (drawing only) |
| everything else | unchanged | buttons keep the door-link index, spikes/chompers/exit doors keep 0 |

Shipped data (rooms 1..min(used_rooms,24) of levels 0..15; VERIFIED by tally): gate raw = 1 (16x) or 2 (78x);
the values 0 and 188 occur only in unused rooms. Spikes, chompers, loose floors, exit doors (16/17), skeleton,
sword: raw 0 everywhere. Potions: kind 1 (38x), 2 (7x), 3 (1x), 4 (2x), 5 (16x). Raise buttons: link index 0..20,
drop buttons 0..15. **No tile 5 (stuck button) and no tile 13 (mirror) in the data.** Button chains: max 4 entries;
targets are gates (15->4: 115, 6->4: 34) and exit doors (15->16: 15). The doorlinks2 timer bits of every referenced entry are 0.

### 1.3 The 3 flag bits of the fg byte (VERIFIED)
- Every read of curr_room_tiles in segment 0000 masks with 0x1F (22 references to `[66C8]`), **except one**:
  make_loose_fall `0000:94DB` `test [tile], 20h`: **bit 0x20 set on a loose floor = it never falls** (but it still shakes).
- In the shipped data only bit 0x20 occurs (flag field 0 or 1; tile 31 bytes 0xFF are filler). It is set on many tiles
  (walls, floors, pillars, gates, ...) but on **no** loose floor, so the "stuck loose floor" never happens in the
  shipped levels. Bits 0x40/0x80 are never read. INFERRED: bit 0x20 elsewhere is an editor/drawing leftover with no gameplay effect.
- The game **writes whole bytes** when it changes a tile type (clearing the flags): 0 `94BB`, 1 `3D9D/4572/75AA/7DF7/92FE`,
  5 `931C`, 0x0D `8C89`, 0x0E `9833/988D`, 0x1E `987E`, 0 `4A4D`.

### 1.4 Door links (VERIFIED)
For index i: `room = ((dl1[i] & 0x60) >> 5) + ((dl2[i] & 0xE0) >> 3)` (`0000:921E`), `tilepos = dl1[i] & 0x1F`
(`0000:91EA`), **last entry when dl1[i] & 0x80** (`0000:9201` returns "has next" = bit 7 clear).
**dl2[i] & 0x1F is the button timer** of that link (get `0000:91AE`, set `0000:91C5` keeps the top 3 bits).
The loop condition at `0000:903B` compares the zero-extended byte with -1 and is therefore always true: only bit 7 ends a chain.

### 1.5 Level tail, used_rooms and resource 2015 (VERIFIED by xref of every address ds:58C2..59C4)
Level struct at ds:50C2 (level offset o = ds - 0x50C2):

| offset | content | read by |
|---|---|---|
| 2048 (ds:58C2) | used_rooms | only alter_mods_allrm `B622` (loop `cmp [bp-2],used; ja`: rooms **1..=used_rooms**) |
| 2049..2072, 2073..2096 | room x / room y per room (editor map, 255 = unused) | nothing |
| 2097..2111 | fill | nothing |
| 2112..2114 | start room, tile, dir | do_startpos `4A4F/4A59/4A83` (mechanics.md 8) |
| 2115..2117 | fill (0) | nothing |
| 2118 (ds:5908) | fill | only as the 1-based base of guard_tile: follow_guard `3A98/3AA3` and `39CC` write `[room + 5908h] = 0xFF`; with room 0 that would hit this unused byte |
| 2119.. 2263 | guard tile, dir, x, seq lo, skill, seq hi, colour (24 each) | enter_guard `381E..38EF`, leave_guard `3A26..3A73` (combat.md) |
| 2287..2304 | fill | nothing (ds:59BB at `3068` is a hall-of-fame copy whose index never reaches the level) |

- **Pseudo room 25** (VERIFIED `BAD4`, `B656`): get_room_address has no upper bound, so with used_rooms = 25 (levels 0, 2,
  4..9, 11..13) load_alter_mod also runs on room 25: its "tiles" are ds:5392 = room 1's **already altered modifiers**, its
  "modifiers" ds:5662 = **doorlinks1[0..30]**. In the shipped data only level 6 is hit: room 1 tile 11 (the drop button,
  link 11) has modifier 11 = "loose floor" -> doorlinks1[11] = 0, and tile 17 modifier 10 = "potion" -> doorlinks1[17] <<= 5
  (index 17 is unused). Link 11's chain becomes {room 0 tile 0 (bit 7 clear), room 1 tile 12}: get_room_address(0) keeps the
  previous pointers (the button's room 1), whose tile 0 is a pillar, so trigger_1 returns -1; entry 12 is the same gate the
  data's entry 11 named. No gameplay change; the port reproduces the bytes (`Level::parse`, test `level_tail_and_used_rooms`).
- Rooms above used_rooms keep raw modifiers (level 10 room 22, level 14 rooms 8..24, level 15 rooms 10..24; all unreachable).
- **Resource 2015** (level 15, the copy-protection level) is 2304 bytes, load_level asks for 0x901 = 2305 (`0EB8`): the missing
  byte is fill 2304, never read. Rooms 1 and 9..24, their links ({255, 0, 255, 0}), room x/y and the guard tiles of rooms 9..24
  hold the editor's 0/255 fill pattern; the reachable rooms (3..8 from start room 6) are clean. The port parses it like any level
  (it creates inert guard slots for the filler rooms 9, 11, ...; unreachable).

---

## 2. Per-tick structure

### 2.1 Order of calls (VERIFIED)
Level loop `0000:4D64`: hitp_delta = 0, **timers** `4FA9`, **play_frame** `090A`. play_frame:

1. **do_mobs** `965B` (falling loose floors)
2. **process_trobs** `85F4` (all tile animations)
3. check_skel `4513`, check_can_guard_see_kid `51D1`
4. **play_kid_frame** `0ED4` (input, seq, physics; at its end check_gate_push, check_action, **check_press**,
   **check_spike_below**, then if resurrect_time `[3D76]` == 0 **check_spiked**, **check_chomped_kid**, then **check_knock**; see mechanics.md 3.1)
5. play_guard_frame `0F73`: only if Guard.direction != 0x56 and Guard.room == drawn_room; seq, and if 44 <= x < 211:
   physics, check_action, **check_press, check_spike_below, check_spiked** for the guard, `5D31`.
6. sword checks, `1386`, `12A6`, exit_room `3ABB`, **check_the_end** `0FE8` (room entry), `395D`, ...

Consequences for a port:
- Anything the kid triggers in tick N (button, loose floor, spike, knock) is added to the trob list in tick N and
  **animates for the first time in tick N+1** (process_trobs runs before the kid).
- A mob created by process_trobs in tick N (loose floor falls) **moves first in tick N+1** (do_mobs runs before process_trobs).
- process_trobs re-reads trobs_count every iteration (`8645`), do_mobs uses the count saved at the start (`9662`).

### 2.2 Trobs (moving tiles) (VERIFIED)
- Array `ds:43C2`, 3 bytes each: `{tilepos, room, type}`; count `[50B8]`; max 30 (`add_trob` `9052` calls the fatal error
  `0C83:0C4C` with "Trobs Overflow" (ds:27D7) when count >= 30). Current trob copy = `ds:4DFA` (tilepos), `4DFB` (room), `4DFC` (type).
- **add_trob(room, tilepos, type)** `9052`: find_trob `90C2` (match tilepos and room). Not found: append. **Found: only
  the type of the existing entry is replaced.**
- **process_trobs** `85F4`: for i in 0..count: copy trobs[i] to trob, animate_tile `86A3`, write trob.type back.
  If any type < 0, compact the list (keep type >= 0, order kept).
- **animate_tile** `86A3`: get_room_address(trob.room), get_curr_tile(trob.tilepos), then dispatch on the
  **current tile type** (jump table `cs:870B`, 31 entries); afterwards `modif[tilepos] = curr_modifier` (`8749`).

| tile | handler |
|---|---|
| 0 empty | animate_empty `938B`: type = -1 |
| 2 spikes | animate_spike `8AC7` |
| 4 gate | animate_door `8B16` |
| 6 drop button, 15 raise button | animate_button `9332` |
| 10 potion | animate_potion `89DD` |
| 11 loose floor | animate_loose `9395` |
| 16 exit door (left) | animate_leveldoor `8BE5` |
| 18 chomper | animate_chomper `8A3C` |
| 19 torch, 30 torch with debris | animate_torch `89C3` |
| 22 sword | animate_sword `8A19` |
| anything else (5 stuck button, 14 debris, ...) | type = -1 (`8704`) |

So a trob whose tile changed type is dropped or re-dispatched automatically (a loose floor that fell is empty -> dropped).

### 2.3 Mobs (falling loose floors) (VERIFIED)
- Array `ds:4BF2`, 6 bytes: `{xh, y, room, speed (i8), type, row}`; count `[40CC]`; max 14 ("Mobs Overflow", `9608`).
  Current mob = `ds:4CEA..4CEF`, index `[43C0]`. `xh = col*4` (col = xh >> 2).
- **do_mobs** `965B`: n = count at start; for each: copy, move_mob `9707`, check_loose_fall_on_kid `9B89`, copy back.
  Then remove every mob with **speed == -1** (`96C9`).
- move_mob `9707`: if type == 0: move_loose `971E`. Then if speed <= 0: speed++ (signed).

---

## 3. Gates (tile 4)

### 3.1 State encoding (VERIFIED)
`mod` = opening height, **0 = closed, 188 = fully open**; `mod >> 2` (0..47) is what the kid code uses (mechanics.md 9.2/11):
blocks while `(mod >> 2) + 6 < char_height` (can_bump_into_gate `59FF`). **238** = "fully open, counting down", **255** = open forever.
Trob types: **0 = closing slowly, 1 = opening, 2 = opening for good, 3..8 = closing fast (type = speed index).**
Tables: door_delta `ds:27D4` = {-1, +4, +4} (index type 0..2); gate_close_speeds `ds:27C6` = {0,0,0, 20,40,60,80,100,120}.

### 3.2 animate_door `8B16` (VERIFIED), every tick while the trob exists
```
if type < 0: return
if type >= 3:                                   # fast close
    if type < 8: type += 1
    if mod < close_speed[type]: mod = 0; type = -1; play_sound(6)   # 'sub; jb' = unsigned borrow
    else: mod -= close_speed[type]
elif mod == 255: gate_stop()                    # 8BD7: type = -1; door_sound_if_visible(7)
else:
    mod += door_delta[type]                     # byte add
    if type == 0:                               # slow close
        if mod == 0: gate_stop()
        elif mod < 188 and (mod & 3) == 3: door_sound_if_visible(4)
    else:                                       # opening
        if mod < 188: if (mod & 7) == 0: play_sound(5)
        elif type < 2: mod = 238; type = 0; play_sound(7)       # 8BB7
        else: mod = 255; gate_stop()                            # 8BC6
```
door_sound_if_visible `9C61`: plays when (level 3 and trob.room == 2), or trob.room == room_left `[4DFE]` and column 9,
or trob.room == drawn_room and column != 9.

### 3.3 Timing (derived from 3.2, VERIFIED arithmetic)
- **Open from closed** (type 1, mod 0): +4/tick -> mod 4, 8, ..., 184, **188 at tick 47** -> immediately 238, type 0, sound 7.
  Sound 5 at mod 8, 16, ..., 184 (every 2nd tick). (mod is a multiple of 4 because the trigger rounds it, 3.4.)
- **Stay open, then slow close** (type 0): -1/tick. 238 -> 188 takes 50 ticks (no sound), then 187 -> 0 takes 188 ticks
  with sound 4 at every mod with `(mod & 3) == 3` (every 4th tick, only if visible); at 0: type -1, sound 7 if visible.
  So one press opens the gate for 47 + 238 = **285 ticks** in total; it blocks the kid again once mod < about 4*(h-6).
- **Fast close** from 188 (type 3): tick 1 type 4 -> 148, tick 2 type 5 -> 88, tick 3 type 6 -> 8, tick 4 type 7 -> 0, sound 6.
  From smaller values it is shorter. While a drop button is held the type is reset to 3 every tick (3.4), so the gate then
  closes at a constant 40/tick.
- **Permanent open** (type 2): as opening, at >= 188: mod 255, trob removed, sound 7 if visible.

### 3.4 Triggering (VERIFIED)
**trigger_button(playsound, button_type, modifier)** `924B` (uses the room/tilepos of the last get_tile: `[4368]`, `[432D]`):
```
get_curr_tile(curr_tilepos)
if button_type == 0: button_type = curr_tile          # 0 = "the button itself"
if modifier == -1: modifier = curr_modifier            # the button's link index
t = dl2[modifier] & 0x1F
if t != 0x1F:                                          # 0x1F = jammed link (never in the data)
    dl2 timer = 5
    if t < 2:                                          # button was up: start its press animation
        add_trob(curr_room, curr_tilepos, 1); is_guard_notice [43AE] = 1
        if playsound: play_sound(3)
    do_trigger_list(modifier, button_type)             # 8FDD, EVERY call, also while held
```
**do_trigger_list(index, button_type)** `8FDD`: for each chain entry: get_room_address(room); `tt = fg[tilepos] & 0x1F`;
`r = trigger_1(tt, room, tilepos, button_type)` `8F91`; if r >= 0: add_trob(room, tilepos, r).
trigger_1: tt == 4 -> trigger_gate; tt == 16 -> (mod != 0 ? -1 : **1**); otherwise -1.

**trigger_gate(room, tilepos, button_type)** `8F24`:

| button_type | gate mod | effect | returns (new trob type) |
|---|---|---|---|
| 15 raise | 255 | none | -1 |
| 15 raise | >= 188 | mod = 238 (restart the open countdown) | -1 |
| 15 raise | < 188 | mod = (mod + 3) & 0xFC | **1** open |
| 14 (debris: permanent) | < 188 | none | **2** open for good |
| 14 | >= 188 | mod = 255 | -1 |
| other (6 drop, from a stuck button) | != 0 | none | **3** close fast |
| other | 0 | none | -1 |

Callers of trigger_button: check_press `7EBB` (kid or guard on a button), died_on_button `92D1`, loose_land `9847`,
level 1 start (`4AD0`: get_tile(5, 2, 0), playsound 0), open potion (`8289`: get_tile(8, 0, 0)), jaffar exit (`3D37`: when
leveldoor_open == 2, get_tile(24, 0, 0)).

### 3.5 Gates and the kid (VERIFIED)
- **check_gate_push** `5BDB` (kid tick, after check_bumped): only if action == 7, frame == 15, or frame 108..110.
  get_tile_at_char; orig_col = tile_col; if that tile is a gate, or the tile one column to the left (tile_col--) is a gate, and
  `curr_row_coll_flags[tile_col] & prev_coll_flags[tile_col]` (`59CE`, `5CDE`) == 0xFF and can_bump_into_gate:
  bumped_sound (`5999`: is_guard_notice = 1, sound 8) and **x += 5 - 10*(orig_col <= tile_col)** (gate in the kid's own
  column: x -= 5; gate one column left: x += 5). There is no gate crushing; a closing gate pushes the kid 5 units per tick.
- Collision and edges: gate = wall_type 1; obstacle only while can_bump_into_gate (mechanics.md 3.5, 9.2, 9.4).
- Climb into a gate from below / climb down past a gate: mechanics.md 11 (seq 73, `(mod >> 2) < 6`) and 2.3 (`>= 6`).

---

## 4. Buttons (tiles 5, 6, 15)

### 4.1 Pressing, check_press `7E2E` (VERIFIED; runs for the kid and for a guard in the drawn room)
```
if 87 <= frame < 100 or 135 <= frame < 141:  get_tile_above_char()      # hanging / climbing: the grabbed tile
elif action in (7, 5) or action < 2:
    if frame == 79 and get_tile_above_char() == 11: make_loose_fall(1)  # then falls through to the test below
    else:
        if not (frame.flags & 0x40): return                             # only frames that need floor
        get_tile_at_char()                                              # tile under the foot column
else: return
if curr_tile2 in (15, 6):
    if Char.alive < 0: trigger_button(1, 0, -1)
    else: died_on_button()
elif curr_tile2 == 11: is_guard_notice = 1; make_loose_fall(1)
```
Midair actions (3, 4) and action 2/6 never press. Kid frames < 150 **without** flag 0x40 (VERIFIED from ds:0FF4):
20-25 (standing jump in the air), 39-43 (running jump in the air), 47, 48, 56-66, 74-80, 86-101, 120, 135-140, so a
jump over a plate does not press it; the take-off and landing frames do.
- Holding a plate re-runs trigger_button every tick: timer back to 5 and the whole chain re-triggered (opener: gate kept at 238
  or reopened; closer: type reset to 3; exit door: no effect once mod != 0).

### 4.2 Button animation, animate_button `9332` (VERIFIED)
`t = timer(mod) - 1; timer(mod) = t; if t < 2: type = -1`. The pressed image is drawn while **timer > 1**
(`0000:A03F`/`A061`: drop button drawn as tile 5, raise button drawn as floor). After the last press the plate stays down
for ticks with timer 4, 3, 2 and pops up when the timer reaches 1 (4th tick). The timer is per **link index**, so all buttons
sharing a link index share it.

### 4.3 died_on_button `92D1` (dead kid/guard on a button) (VERIFIED)
`bt = get_curr_tile(curr_tilepos)`; if it is 15: tile = 1 (floor), mod = 0, bt = 14 (permanent open); else tile = **5 (stuck
button)**. Then trigger_button(1, bt, old mod). Tile 5 has no trob handler and check_press ignores it, so a stuck closer stays
pressed and inert.

---

## 5. Loose floors (tile 11)

### 5.1 State (VERIFIED)
mod 0 = idle; **1..10 = about to fall** (counts up); **0x80..0x83 = shaking only** (bit 7). Falls when it reaches 11.

### 5.2 Triggers (VERIFIED)
- **make_loose_fall(m)** `94CD`: if `!(fg & 0x20)` and `(i8)mod <= 0`: mod = m; add_trob(curr_room, curr_tilepos, 0).
  Called with m = 1 from check_press (standing on it with a floor frame, hanging from it, frame 79 under it), and on
  level 13 from check_fall_flo `1034` (see 5.6).
- **loose_make_shake** `95AC`: if mod == 0 and level != 13: mod = 0x80; add_trob(curr_room, curr_tilepos, 1).
  Called by **do_knock(room, row)** `95D8` for every column 0..9 of that row with tile 11.
- do_knock callers: **check_knock** `4F7A` (kid tick end): knock `[42E2]` != 0 -> do_knock(Char.room, Char.curr_row - (knock > 0)),
  knock = 0. Seq op F4 sets knock = -1 (landings: shakes the kid's own row), F5 sets +1 (jump against the ceiling: the row above).
  And move_loose when a falling floor crashes (do_knock on the crash row).

### 5.3 animate_loose `9395` (VERIFIED)
```
if type < 0: (redraw) return
mod += 1
if mod & 0x80:                                  # shaking
    if level == 13: return                      # 93BB: no stop, no sound (mod still written back)
    if mod >= 0x84: mod = 0; type = -1
    loose_shake(mod == 0)
elif mod >= 11:                                 # fall now
    mod = remove_loose(room, tilepos)           # 94B0: tile = 0 (empty); mod = ds:01CE[level] (empty-tile pattern)
    type = -1
    add_mob(xh = (tilepos % 10) * 4, y = y_loose_land[row + 1], room, speed = 0, type = 0, row = tilepos / 10)
else: loose_shake(0)
```
y_loose_land `ds:2298` (words) = {2, 65, 128, 191, 254}.
**loose_shake(force)** `944D`: if force or loose_sound[mod & 0x7F] (`ds:2748` = {0,1,1,1,0,1,0,0,1,0,0,0}):
`do s = prandom(2) + 20 while s == last_loose_sound [60C2]`; if sound flags `[31C1]` bit 0: prandom(1000) (sample rate) and
last_loose_sound = s; play_sound(s).
Timing: pressed in tick N (mod 1) -> mods 2..10 in ticks N+1..N+9 (sounds at mods 2, 3, 5, 8) -> **falls in tick N+10**.
Knock shake: mods 0x81, 0x82, 0x83, then 0x84 -> 0 and removed: 4 ticks, a shake sound in each.

### 5.4 move_loose `971E` (VERIFIED), y_something `ds:228E` (words) = {-1, 62, 125, 188, 251}
```
if speed < 0: return
if speed < 29: speed += 3                       # 3, 6, ..., 27, 30 (max 30)
y += speed                                      # byte
if room == 0: if y >= 210: speed = -2; return
if y >= 226 or y < y_something[row + 1]: return
t = get_tile(room, col, row)
if t == 11: loose_fall()                        # 98C3, knock the next loose floor down too
if t in (0, 11): mob_down_a_row(); return       # 997F: row++; if row >= 3: y -= 192, row = 0, room = link_down[room]
play_sound(2)                                   # crash
do_knock(room, row)
y = y_something[row + 1]; speed = -2            # removed in this tick (speed -> -1 in move_mob)
loose_land()
```
First tick after creation: y = y_loose_land[row+1] + 3 is already past the threshold of its own (now empty) tile, so the
mob moves to the next row at once. **loose_fall** `98C3`: the second floor's tile is removed (remove_loose), the current mob's
speed is halved (`sar`), and a copy with y += 6 and row + 1 is added as a new mob (it moves from the next tick).

### 5.5 loose_land `97E0` (VERIFIED)
Tile under the crash point (room, col, row):
- 15 raise button: tile = 0x0E, trigger_button(1, 14, -1) (linked gates open for good), then becomes debris.
- 6 drop button: trigger_button(1, 0, -1) (close fast), then tile = 0x0E.
- 1 floor, 2 spikes, 10 potion: tile = **0x0E debris**. 19 torch, 30: tile = **0x1E torch with debris**.
- anything else (e.g. 14 debris, 5, 3 pillar): no change. The modifier is not touched.

### 5.6 Falling on the kid, check_loose_fall_on_kid `9B89` / fell_on_your_head `9BCB` (VERIFIED)
Every mob tick: if mob.room == Kid.room, mob col == Kid.curr_col, mob.y < Kid.y and Kid.y - 30 < mob.y:
only if (level == 13 or frame < 5 or frame >= 15) and (action < 2 or action == 7):
y = y_land[row+1]; if take_hp(1) (died): **seq 22**, and frame 177 -> x = forward(-12);
else if frame != 109: if the tile behind is 0 then x = forward(-2); **seq 52**.
(Running frames 5..14 are immune except on level 13.) Mobs never hurt guards.
Level 13 special, check_fall_flo `1034` (room entry): in rooms 23 and 16, for tilepos 22..27 of the room above:
make_loose_fall(-(prandom(255) & 15)): those floors count up through 0xF1..0xFF (no stop on level 13), wrap to 0 and
fall 11 ticks later, staggered by the random start.

Sounds: 20/21/22 shake, 2 crash.

---

## 6. Spikes (tile 2)

### 6.1 Cycle, animate_spike `8AC7` (VERIFIED)
```
if type < 0 or mod == 255: return                      # 255 = stuck out (a body on it); the trob stays in the list forever
if mod & 0x80: mod -= 1; if (mod & 0x7F) == 0: mod = 6 else return   # hold, no redraw
else: mod += 1; if mod == 5: mod = 0x8F elif mod == 9: mod = 0; type = -1
```
Ticks after the start: 1, 2, 3, 4 (rising), **0x8F at tick 5**, 0x8E..0x81 (ticks 6..19), tick 20 0x80 -> 6, 7, 8,
tick 23 -> 0, removed. Total 23 ticks; out (>= 0x81) for ticks 5..19.
**start_anim_spike(room, tilepos)** `8EDB`: if `(i8)mod <= 0`: mod == 0 -> add_trob(room, tilepos, 1) and **sound 49**;
mod != 255 (so 0x80..0xFE) -> mod = 0x8F (keeps them out while something is above). Retracting 6..8 is not restarted.

### 6.2 Triggering, check_spike_below `7EE1` (VERIFIED; kid tick and guard tick)
For col from col_m7(char_x_left `[3D5C]`) to col_m7(char_x_right `[3D50]`) (nothing if the right column < 0): row = Char.curr_row;
loop: get_tile(Char.room, col, row) == 2 -> start_anim_spike; else if the tile is not floor and curr_room != 0 and
curr_room == Char.room: row++ and repeat (it looks down through empty tiles of the same room). Uses the image box, not the coll box.

### 6.3 Harm and death (VERIFIED)
is_spike_harmful `9B4E` (on curr_tilepos): mod 0 or 255 -> 0; `(i8)mod < 0` (out) -> 1; mod >= 5 (retracting) -> 0; 1..4 -> 2.
- **check_spiked** `7394` (tile at Char's col/row is 2): spiked if (harmful >= 2 and frame in 7..14 or 34..39) or
  (frame == 43 or 26, and harmful != 0). So running into rising spikes kills; landing a jump (26 standing, 43 running) on
  rising or out spikes kills; walking/standing does not.
- **land** `5EF8`: if the tile at char is 2, or (alive, distance_to_edge_weight >= 12 and the tile behind is 2): harmful != 0 -> spiked.
- **spiked** `601F`: mod = **255**; y = y_land[row+1]; x = x_bump[tile_col+5] + 10, then x = char_dx_forward(8); fall_y = 0; **sound 48**; take_hp(100); **seq 51**; play_seq.

---

## 7. Chompers (tile 18)

### 7.1 Cycle, animate_chomper `8A3C` (VERIFIED)
mod = blood bit 0x80 | frame (1..15).
```
if type >= 0:
    blood = mod & 0x80; f = (mod & 0x7F) + 1; if f > 15: f = 1
    mod = blood | f; if f == 2: sound 47
    if (trob.room != drawn_room or tilepos/10 != Kid.curr_row `[439B]` or (Kid.alive `[43A3]` >= 0 and blood == 0)) and f >= 6:
        type = -1
```
Frame 2 is the closed (deadly) frame; the cycle is 15 ticks and runs while the kid is in that room and row.

### 7.2 start_chompers `950B` (VERIFIED)
Called by check_the_end (room entry), land `5F45`, start_fall `760D`, seq ops FC/FD (row change) `7004`.
If Char.curr_row < 3: for col 0..9 of Char.room, row curr_row: tile 18 with (mod & 0x7F) == 0 or >= 6 ->
start_anim_chomper(room, tilepos, timing | (mod & 0x80)) `8E9F` (sets mod and add_trob(.., 1) only if the old full mod is 0 or >= 6),
then timing = next_chomper_timing(timing) `9592`: `t -= 3; if t < 6: t += 10` (unsigned). timing starts at 15, so the
chompers of a row start at 15, 12, 9, 6, 13, 10, ... (15 wraps to frame 1 on the first tick).

### 7.3 Death, check_chomped_kid `5B13` (VERIFIED; kid only, when resurrect_time == 0)
For col 0..9: if curr_row_coll_flags[col] `[59CE]` == 0xFF (the coll box overlaps the chomper's wall extent on both sides, see
mechanics.md 9.1; chomper wall_type 3 = {left -1, right 13}) and get_tile(curr_row_coll_room[col] `[43B2]`, col, curr_row) == 18 and
(mod & 0x7F) == 2: **chomped** `5B67`: mod |= 0x80; if frame != 178 and Char.room == curr_room:
x = x_bump[tile_col+5] + 7, then x = char_dx_forward(7 - (direction == 0)) (6 facing right, 7 facing left), y = y_land[row+1],
take_hp(100), **sound 46**, **seq 54**, play_seq.

---

## 8. Other tiles

### 8.1 Exit door (16 left half; 17 is drawing only) (VERIFIED)
mod 0 = closed, **43 = open**. Opened by a raise button linked to it (trigger_1 returns 1 if mod == 0).
animate_leveldoor `8BE5`:
- type 1/2 (opening): mod += 1; if mod >= 43: type = -1, stop_sounds (`0C83:717F`), and if leveldoor_open `[40DC]` is 0 or 2:
  leveldoor_open = 1, and on level 4 the tile (room 4, col 4, row 0) becomes **0x0D mirror**. Else: **sound 15** each tick.
  So opening takes 43 ticks, and the kid can enter (up_pressed needs leveldoor_open != 0, mechanics.md 2.3) only after that.
- type >= 3 (closing): type += 1; mod -= table[type] with `ds:27CC` indexed by type -> 5, 17, 99, 0 for types 4..7
  (unsigned borrow -> mod = 0, type -1, **sound 14**); at type 4 also sound 15 if `[31C1] & 1` and `[00B8] == 0` (INFERRED: digi sound on).
- Level start: the **first** tile 16 in Kid.room gets start_level_door `936A`: mod = 43, add_trob(room, tilepos, **3**):
  43 -> 38 -> 21 -> 0 in 3 ticks, sound 14.
- Entering (VERIFIED `62EE`, `63E0`, seq 70 at ds:20F1, play_level_2 `4DA2`): Up with tile 16 at, behind (`78A2`) or in
  front of (`7848`) the kid, in that order, start room `[5902]` != drawn room and leveldoor_open != 0 (set only when a door's
  mod reached 43, so "fully open" is global, not per door): x = x_bump[tile_col + 5] + 10, direction left, seq 70. Seq 70:
  action 5, dx -5, dy -1, footstep, frames **217..228** (ticks 1..12, the climb, music F2 4 with frame 224 on tick 8), then
  frame **0 = image 255** (the kid is not drawn) for ticks 13..22 with a footstep every 3rd tick, **op F1 on tick 23**, then
  `jmp` to a frame-0 loop (F1 runs once). The frame loop returns in that frame if no sound plays, else keeps running full
  frames (invisible kid) until the sounds end. Port: identical (test `exit_stairs_timeline`); pop-core ends on tick 23, the
  app holds the exit while the music plays.
- Level end: seq 70 (go_up_leveldoor) ends with op F1 (next level, mechanics.md section 0). leveldoor_open is also set to 2 at `836C`
  (INFERRED: Jaffar killed; then jaffar exit triggers room 24 via trigger_button) and used as a counter on level 8 room 16 (timers `4FF8`).

### 8.2 Potions (10) (VERIFIED)
mod = kind << 5 | bubble frame. Room entry (anim_tile_modif `0B3D`): start_anim_potion `8E3F`: frame = prandom(6) + 1, trob.
animate_potion: only while trob.room == drawn_room (is_trob_in_drawn_room `875A` otherwise sets type -1): frame + 1, 8 -> 1.
Drinking: crouched (frame 109) with Shift on a potion: get_item `65AA` -> do_pickup(mod >> 5) `7DD2` (tile = 1, mod = 0),
**seq 78**; on level 15 the copy-protection entry is cleared. Pick-up frame op F3 1 -> proc_get_object `81AB` (kid only):

| kind | effect |
|---|---|
| 1 heal | if hitp_curr `[4CD6]` != hitp_max `[6130]`: stop_sounds, **sound 33**, hitp_delta `[612E]` = +1, flash |
| 2 life | stop_sounds, **sound 30**, flash, add_life `1217` |
| 3 feather | set_feather_fall `162B` |
| 4 invert | toggle_upside `161C` |
| 5 hurt | stop_sounds, **sound 13**, hitp_delta = -1; **level 15: -((hitp_max + 1) >> 1)** |
| 6 open (copy protection, level 15) | get_tile(8, 0, 0); trigger_button(0, 0, -1) |

### 8.3 Sword (22) (VERIFIED)
Pick-up: crouched + Shift: do_pickup(-1), **seq 91**; proc_get_object: have_sword `[34E2]` = -1, **sound 37**, flash.
Trob (drawn room only): start mod = prandom(255) & 0x1F; each tick mod -= 1, at 0: (prandom(255) & 0x3F) + 40 (glint timer).
Level 12: leaving room 18 to the right turns (room 15, col 1, row 0) into floor (`3D74`).

### 8.4 Torches (19, 30) (VERIFIED, animation only)
Start mod = prandom(8); each tick (drawn room only) `8CC5`: r = prandom(255); if r == mod or r >= 9: r = (mod + 1) % 9.
The flame image is PRINCE.DAT 151 + mod, drawn only if mod < 9 (`A5E1`).

### 8.4a Animation start and drawing, ported (VERIFIED)
- check_the_end `0FE8` (room entry: next_room != 0 and != drawn_room; also at level start `4B92`) sets drawn_room, then
  anim_tile_modif `0B3D` for tilepos 0..29 of the drawn room: potion `8E3F` (mod = kind | prandom(6) + 1), torch 19/30 `8E16`
  (prandom(8)), sword `8E74` (prandom(255) & 0x1F), each with add_trob(type 1); then start_chompers, check_fall_flo, check_shadow.
- animate_torch `89C3` and animate_sword `8A19` have **no type < 0 test**; animate_potion `89DD` has one. All three call
  is_trob_in_drawn_room `875A` (trob.room != drawn_room -> type -1), so leaving a room drops its trobs one tick later and the
  modifiers stay frozen until the room is entered again.
- Drawing: bubbles image ds:27BE[mod & 0x0F] (`A8AE`, frames 1..7 -> 166..172, 0 -> none); sword image 10 + (mod == 1)
  (`A910`): the glint shows for exactly one tick per countdown (start 0..31, then 40..103 ticks).
- Port: `World::check_the_end`, `animate_tile` (the modifiers change without a renderer `version` bump), `Level::anim_frame`;
  tests `torch_potion_sword_animate_from_the_shared_prandom`, `anim_trobs_follow_the_drawn_room`, `level13_falling_floors_on_room_entry`.
  The app's flames and bubbles read `anim_frame`; the floor sword (a placeholder bar) shows no glint yet.

### 8.5 Mirror (13), skeleton (21), debris (14), doortops (VERIFIED where cited)
- Mirror: placed on level 4 by the exit door (8.1). Jumping through it: mechanics.md 9.2 (is_obstacle sets its mod to 0x56).
- Skeleton: check_skel `4513` (level 3, no guard present, drawn room 1, exit door open, Kid col 2 or 3): tile (room 1, col 5, row 1)
  == 21 -> tile = 1 and a skeleton guard is created (rest not traced).
- Debris 14 and torch-with-debris 30 are plain floor (tile_is_floor), no trob. Doortops 7/12: walls (wall_type 1), no trob.
- Level 12 hidden floor: check_on_floor `7576` turns the missing tile into floor (`75AA`) when united_with_shadow < 0, row 0, room 2 or room 13 col >= 6.

---

## 9. Sound ids (for the audio milestone)

| id | event | where |
|---|---|---|
| 2 | loose floor crashes | `97AD` |
| 3 | button pressed (first press only) | `92B8` |
| 4 | gate closing slowly (every 4th unit, if visible) | `8B94` |
| 5 | gate opening (every 8 units) | `8BAC` |
| 6 | gate fast close finished | `8B57` |
| 7 | gate fully open (always) / gate stopped closed or permanent (if visible) | `8BC1` / `8BDC` |
| 8 | pushed by a gate (bumped_sound) | `599F` |
| 13 / 30 / 33 | hurt / life / heal potion | `8297` / `8249` / `8223` |
| 14 | exit door closed | `8C1D` |
| 15 | exit door sliding | `8C93` |
| 20, 21, 22 | loose floor shaking (random, never the same twice when digi) | `9467` |
| 37 | sword picked up | `81CC` |
| 46 | chomped | `5BC2` |
| 47 | chomper closes (frame 2) | `8A79` |
| 48 | spiked | `605A` |
| 49 | spikes start | `8F02` |

---

## 10. Determinism notes (VERIFIED)
- prandom(max) `0C83:0412`: `seed = seed * 0x343FD + 0x269EC3` (32-bit, `[40C4]`), returns `(seed >> 16) % (max + 1)`;
  the first call seeds from the clock (`0C83:B416`).
- The **same generator** is used by the tile code: torch (every tick per torch in the drawn room), potion and sword trobs,
  start_anim_* at room entry, loose_shake (prandom(2) until different, plus prandom(1000) when digi sound is on), level 13
  falling floors. To reproduce the original's random sequence exactly (guards also use it), the port must make the same
  calls in the same order, including those for the visual-only tiles. **Done** (8.4a): pop-core makes the torch, potion,
  sword and check_fall_flo calls in the original order. Not consumed: the loose-shake prandom(1000) (digi sample rate, only
  with digital sound on). The seed: the original seeds from the clock on its first call (`0C83:0418`, flag ds:2956 never
  reset), the port uses `sim::SEED`; 1763 (wall pattern) saves and restores the seed, so it is stream-neutral.

Background note: for drawing, the modifier of an Empty (1..3) or Floor tile picks the background piece that
draw_tile_right draws in the next column (`0000:A332`, docs/texture_inventory.md 18).

## 11. Not verified / open
- Whether the guard's `5D31` is a chomper check for guards (not traced).
- `[00B8]` in the exit-door closing sound condition, and the meaning of leveldoor_open == 2 (both INFERRED).
- The full skeleton wake-up and the shadow / Jaffar level events that call trigger_button (only the trigger part was read).
- The drawing of gates, buttons and spikes from mod (only the pressed-button test was read).
