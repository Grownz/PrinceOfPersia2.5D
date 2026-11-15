# Edge cases left by the M5 port (PRINCE.EXE v1.1)

Round 4 of the disassembly notes. Conventions as in `docs/mechanics.md` sections 0-1 and `docs/combat.md`:
`0000:XXXX` = code (file = 0x890 + XXXX), `ds:XXXX` = data (file = ds + 112832), sequences from the table `ds:189C`
(word per seq number). Char/Kid/Guard/Opp record offsets are in combat.md section 0.

- **VERIFIED**: read from the code or data at the address given.
- **INFERRED**: our interpretation (a name, a meaning, a consequence, or arithmetic derived from verified code).
- **UNVERIFIED**: not checked; do not port as fact.

Names follow SDLPoP where the code matches it. Rust references point at the current port (`crates/pop-core/src`).

Corrections to earlier docs found in this round:

| doc | claim | correction (VERIFIED) |
|---|---|---|
| combat.md 4.5 | seq 82/83 contents "fall_x 0/0/1" | right, but the dx/dy differ per seq (section 1.3 below) |
| combat.md 5.3 | gate case `x_bump[tile_col - (curr_tile2 != gate) + 5]` | the `- 1` branch is dead code; x always uses the gate's own column (1.4) |
| combat.md 7.1 | level 5 move list "(0 forward) (14 nothing) ..." | the list starts with (0 nothing) (1 forward); full table in 5.4 |
| combat.md 7.3 | mouse counter runs while "drawn_room 16" | the test is Char.room `[3D6B]` == 16 (2.x of this doc, 4.2) |
| combat.md 11 | frame-length timer rate not traced | timer interrupt runs at **60 Hz** (5.2) |

---

## 1. Guards against walls, chompers, ledges

### 1.1 check_guard_bumped `0000:5C6B` (VERIFIED)

Called once per guard tick from play_guard_frame (combat.md 1), after set_char_collision and before check_action, only while the
guard is in the drawn room and `44 <= x < 211`.

```
if [4CAE] != 0: return                 ; 5C71. Written only at 497C (= 0 at every level start): always passes
if Char.action != 1: return            ; 5C7B  (standing/running on the ground; en garde is action 1)
if Char.alive >= 0: return             ; 5C85  (dead)
if Char.sword < 2: return              ; 5C8F  (sword drawn only)
t = get_tile_at_char()                 ; 7449, sets curr_tile2 / tile_col / curr_room
if t == 20 (wall) or curr_tile2 == 7 (doortop) or (curr_tile2 == 4 and can_bump_into_gate()): goto hit
if Char.direction < 0 (facing left): return                          ; 5CB8
t = get_tile(curr_room, tile_col - 1, tile_row)                      ; 5CBF, NOTE: `dec [4370]` before the push
if t == 7 or (curr_tile2 == 4 and can_bump_into_gate()): goto hit    ; a wall (20) one column left does NOT count
return
hit:
load_frame_to_obj (C110); set_char_collision (745F)
if !is_obstacle(): return              ; 5726 on curr_tile2; on success sets coll_tile_left_x (collisions.md 4.2)
dx = dist_from_wall_behind(curr_tile2) ; 5E23
if dx >= 0 or dx <= -13: return
x = char_dx_forward(-dx)               ; moves the guard forward by |dx|, out of the wall behind it
seqtbl(65); play_seq; load_fram_det_col (6E80)
```

dist_from_wall_behind(tile) `0000:5E23` (`ret far 2`):
```
wt = wall_type(tile)                   ; 7D0B: 4/7/12 -> 1, 13 -> 2, 18 -> 3, 20 -> 4, else 0
if wt == 0: return 99
facing right: return wall_dist_from_right[wt] + char_x_left_coll - coll_tile_left_x - 13
facing left:  return wall_dist_from_left[wt]  + coll_tile_left_x - char_x_right_coll
```
Tables (VERIFIED, signed bytes): wall_dist_from_left `ds:24CE` = {0, 10, 0, -1, 0}, wall_dist_from_right `ds:24D4` = {0, 0, 10, 13, 0}
(index wt 0..4). x_bump `ds:24BA` = {-12, 2, 16, 30, 44, 58, 72, 86, 100, 114, 128, 142, 156, 170, 184, 198, 212, 226, 240, 254};
`ds:24BF[col]` = x_bump[col + 5].

Seq 65 (`ds:1A89`): act5, f160, f157, then the en-garde loop (combat.md 4.5). So the guard is shoved out of the wall and shown in its
"step back" pose for two ticks.

Notes:
- The guard's own column is tested for wall/doortop/gate whatever its facing; the column to the left only when facing right, and
  there only doortop or gate (not a plain wall). INFERRED: the left-hand case catches a guard that backed into a doortop/gate that is
  drawn in the neighbouring column.
- Unlike the kid's check_bumped there are no flag arrays and no room tags: is_obstacle uses the tile from the last get_tile.

Test vectors (inputs given explicitly; the box values come from the frame images, not derived here):

| case | dir | tiles | coll_tile_left_x | box | dist | result |
|---|---|---|---|---|---|---|
| A | left | own col 3 = wall 20 (wt 4) | xpos(100)+7 = 107 | right_coll = 112 | 0 + 107 - 112 = -5 | x -= 5 (forward 5), seq 65 |
| B | right | own col 3 floor, col 2 = doortop 7 (wt 1) | 86+7 = 93 | left_coll = 100 | 0 + 100 - 93 - 13 = -6 | x += 6, seq 65 |
| C | right | col 2 = wall 20 | - | - | - | nothing (wall only counts in the own column) |
| D | left | own col = wall, right_coll = 125 | 107 | | -18 | nothing (<= -13) |
| E | any | guard sword 0 (inactive) | | | | nothing |

### 1.2 check_chomped_guard `0000:5D31` (VERIFIED)

Called at the very end of the guard's physics in play_guard_frame (after check_spiked), drawn room and `44 <= x < 211` only.
No resurrect-time test (the kid's check_chomped_kid has one).
```
get_tile_at_char()
if check_chomped_here(): return                        ; 5D58
get_tile(curr_room, tile_col + 1, tile_row)            ; `inc [4370]`: the column to the RIGHT, whatever the facing
check_chomped_here()

check_chomped_here():                                  ; 5D58
  if curr_tile2 != 18: return 0
  if (curr_room_modif[curr_tilepos] & 0x7F) != 2: return 0     ; the jaws-closed frame
  coll_tile_left_x = ds:24BF[tile_col] + 7                     ; no xpos_in_drawn_room correction here
  if left_wall_x(curr_room, tile_col, tile_row) >= char_x_right_coll: return 0     ; 55CE
  if right_wall_x(curr_room, tile_col, tile_row) <= char_x_left_coll: return 0     ; 5607
  chomped(); return 1                                          ; 5B67
```
chomped `0000:5B67` (shared with the kid, collisions.md 5.7):
```
curr_room_modif[curr_tilepos] |= 0x80                 ; blood on the chomper
if Char.frame == 178: return                          ; already chomped
if Char.room != curr_room: return
x = ds:24BF[tile_col] + 7
x = char_dx_forward(direction == right ? 6 : 7)
y = y_land[row + 1]                                   ; ds:22B2 + 2*row (word table y_land at ds:22B0 = -8, 55, 118, 181, 244)
take_hp(100); play_sound(46); seqtbl(54); play_seq
```
Seq 54 (`ds:20DD`): act1, f178, F6, f178, then f178 forever (loop at `ds:20E1`, resting seq pointer **0x20E2**).
With take_hp(100) the guard's HP reaches 0 in do_delta_hp; on the next tick play_guard sets alive = 0 and calls on_guard_killed
(sound 37). control() does not start seq 71 because frame 178 is not in its dying list (15/166/158/171), so the body stays on f178.

Test vector: guard in room 5, col 4, row 1, facing right, chomper at col 4 with modifier 2 and overlapping box:
x = 114 + 7 = 121 -> forward(6) = **127**, y = **118**, seq 54, sound 46, chomper modifier |= 0x80. Facing left: x = 121 - 7 = **114**.

Where guards meet spikes: check_spike_below `7EE1` and check_spiked `7394` run for the guard too (combat.md 1, tiles.md 6); a spiked
guard ends on f177 (seq 51, resting pointer **0x20DA**). Landing on spikes after a fall is in land `5F52..5F7B` (tiles.md 6.3).

### 1.3 Guards falling off a ledge: start_fall `0000:75F5` (VERIFIED)

Reached from check_action/check_on_floor when the character has no floor (collisions.md 5.4/5.5). For frames **150..179** (sword frames):
```
; common prefix (all characters): f0 = frame; sword = 0; curr_row++ (85EF); start_chompers; [40D6] = f0
if 150 <= f0 < 180:
  if Char.charid == 2:                                  ; 7698: guards (and Jaffar / fat guard), NOT skeleton (4) or shadow (1)
      if curr_row == 3 and curr_col == 10: clear_char(); return     ; 76A6, skips the play_seq/in_wall tail entirely
      if fall_x < 0:                                    ; 76B4, signed
          seq = 82
          if facing left and distance_to_edge_weight() <= 7: x = forward(-5)
      else:                                             ; fall_x >= 0, INCLUDING 0
          droppedout [3D74] = 0; seq = 83
  else:                                                 ; kid, shadow, skeleton
      droppedout = 1
      if facing left and distance_to_edge_weight() <= 7: x = forward(-5)
      seq = 81
seqtbl(seq); play_seq; load_fram_det_col; then the wall tail of start_fall (collisions.md 5.5)
```
fall_x is set by the sword sequences (combat.md 4.5): advance (56/86) = +1, retreat (57) = -1, guard strike (58) = -1,
hit (74) = -1, others leave the previous value. Typical: a guard **backing off** a ledge falls with seq 82, one **advancing** (or never
moved, fall_x 0) with seq 83.

The three "pushed off" sequences (VERIFIED bytes; dx is applied with char_dx_forward, i.e. relative to facing):

| seq | addr | ops | net dx | then |
|---|---|---|---|---|
| 81 | `ds:1B99` | act3, dy-1 f102, dx-2 dy+6 f103, dx-2 dy+9 f104, dx-1 dy+12 f105, dx-3, fall(0, 15) | -8 | seq 12 |
| 82 | `ds:1BB5` | act3, dy-1 dx-2 f102, dx-3 dy+6 f103, dx-3 dy+9 f104, dx-2 dy+12 f105, dx-3, fall(0, 15) | -13 | seq 12 |
| 83 | `ds:1BD3` | act3, dx+1 dy-1 f102, dx+2 dy+6 f103, dx-1 dy+9 f104, dy+12 f105, dx-2, fall(1, 15) | 0 | seq 12 |
| 12 | `ds:1D5D` | act4, f106 forever (freefall) | | |

For guards (charid 2/4) frames 102..106 are drawn from the guard table as 172..176 (combat.md 0, load_frame `6E96`).

Fall arithmetic (INFERRED from the verified order play_seq -> fall_accel `72B8` -> fall_speed `72EA` -> check_action in
play_guard_frame): fall_accel only acts in action 4: fall_y += 3, max 33 (feather: += 1, max 4). fall_speed always does y += fall_y and,
in action 4, x = forward(fall_x). Guard starting on row 0 (y 55), seq 82, landing target y_land[2] = 118:

| tick | frame | action | fall_y after accel | y after the tick |
|---|---|---|---|---|
| T0 (start_fall) | 102 | 3 | 0 | 54 |
| T1 | 103 | 3 | 0 | 60 |
| T2 | 104 | 3 | 0 | 69 |
| T3 | 105 | 3 | 0 | 81 |
| T4 | 106 (seq 12; F8 set fall_y 15) | 4 | 18 | 99 |
| T5 | 106 | 4 | 21 | 120 >= 118: land, fall_y 21 |

land `5EF8` outcome by fall_y for non-kids (VERIFIED `5F48..6016`):
- dead at landing (alive >= 0) or fall_y >= 33: take_hp(100), sound 0, **seq 22** (fatal, ends on f185).
- fall_y < 22: charid >= 2 or sword == 2 -> sword = 2, **seq 63** (lands en garde, combat.md 4.5); kid: seq 17 + sound 17 + notice.
- 22 <= fall_y < 33: **charid 2 -> fatal like above**; charid 1 -> same as a short fall; others (kid, skeleton 4): take_hp(1), sound 16,
  notice, seq 20 (dead -> fatal path).
So a guard survives a one-row drop (fall_y 21), dies from two rows (fall_y 30 at T8), and a guard on row 2 falls out of the room
(y >= 211) and is removed by check_guard_fallout (combat.md 2.5). The skeleton survives two rows with 1 damage (it takes no
sword damage but does take fall damage; INFERRED, take_hp(1) on 3 HP).

### 1.4 Death placement in hurt_by_sword `0000:42C1` (VERIFIED, exact)

```
if alive >= 0: return
if sword != 2: take_hp(100); seqtbl(85); goto placement          ; 42D8
if charid == 4: seqtbl(74); goto set_y                           ; 4336: skeleton, no damage
if !take_hp(1): seqtbl(74); goto set_y                           ; survives
; killed while en garde:
placement:                                                       ; 42EA
if get_tile_behind() != 0 (78A2) or (d = distance_to_edge_weight()) < 4:     ; 78C6
    seqtbl(85)                                                   ; 4303 (stabbed, then seq 71)
    if charid != 0 and direction < 0:                            ; non-kid facing left
        if curr_tile2 == 4 or get_tile_at_char() == 4:           ; behind tile is a gate, else own tile
            x = ds:24BF[tile_col] + 7                            ; tile_col of THAT gate (the -1 branch at 4331 is unreachable)
            x = char_dx_forward(10)                              ; i.e. x -= 10
set_y:
    y = y_land[row + 1]; fall_y = 0
else:                                                            ; 4388: empty tile behind and d >= 4
    x = char_dx_forward(d - 20); load_fram_det_col; curr_row++ (85EF); seqtbl(81)
sound(charid == 0 ? 13 : 12); play_seq
```
`curr_tile2` at 431A is the tile behind (from 78A2) when that one was non-empty, otherwise whatever distance_to_edge_weight read last
(INFERRED: the tile in front/at the character). Both branches into 432A guarantee curr_tile2 == 4, so ax = 0 and
**x = x_bump[gate_col + 5] + 7 - 10**. Note the guard and the kid both use **seq 81** when pushed off (seq 82/83 come only from start_fall).

Test vectors (guard facing left, killed en garde, room has no neighbours involved):

| own col | behind (col+1) | result |
|---|---|---|
| 5 floor | 6 gate | x = 142 + 7 - 10 = **139**, seq 85 |
| 5 gate | 6 floor (non-empty) | behind != 0 -> in place; curr_tile2 = floor -> own tile gate -> x = 128 + 7 - 10 = **125** |
| 5 floor | 6 floor | in place, x unchanged, seq 85 |
| 5 floor | 6 empty, d = 9 | x = forward(-11) (x += 11 facing left), row + 1, seq 81 |

Quirk (INFERRED): tile_col is the normalised column, so a gate found in the neighbouring room (behind = column 10 -> col 0 of the
right room) puts the guard at x = 58 + 7 - 10 = 55 in its own room coordinates.

### Implementation notes (section 1)

- `kid.rs` `guard_tick` (around line 486): add `check_guard_bumped` between `set_char_collision` and `check_action`, and
  `check_chomped_guard` after `check_spiked`, exactly in the play_guard_frame order (combat.md 1). Both are guard-only; the kid keeps
  `check_bumped` / `check_chomped_kid` (kid.rs:1081 / 1190). Reuse `chomped` from the kid code (it already handles `frame == 178`
  and the `room != curr_room` test) but call it without the resurrect test.
- `kid.rs:1404`: replace the guess. Test `charid == 2` (not `>= 2`: the skeleton takes the kid branch, seq 81 + droppedout = 1),
  `fall_x < 0 -> 82` (with the facing-left edge nudge), else `83` and `droppedout = 0`; the `row == 3 && col == 10 -> clear_char`
  early return skips play_seq. fall_x == 0 currently gives 82 in the port; the original gives 83.
- land: check the `22 <= fall_y < 33` branch kills charid 2 outright (port: verify in `land`).
- `kid/combat.rs:537-544` (death placement): add the "tile behind is the gate" test before the own-tile test and use the gate's
  column; keep `forward(10)`.
- Tests to add in `sim/combat.rs`: vectors 1.1 A/B/C, 1.2, the 1.3 tick table (y and frame per tick, landing in seq 63), 1.4 rows.

---

## 2. Dead guards across room changes (VERIFIED)

### 2.1 Saving: leave_guard `0000:39E9`
```
if Guard.direction == 0x56 or charid == 1 (shadow) or charid == 24 (mouse): return
i = Guard.room - 1
guards_tile[i]  (ds:5909) = get_tilepos(0, Guard.curr_row) = 10*row     ; column dropped, x kept
guards_color[i] (ds:5999) = curr_guard_color [59C4]
guards_x[i]     (ds:5939) = Guard.x
guards_dir[i]   (ds:5921) = Guard.direction
guards_skill[i] (ds:5969) = guard_skill [43A6]
if Guard.alive < 0: guards_seq_hi[i] (ds:5981) = 0                       ; alive: comes back fresh
else: guards_seq_lo[i] (ds:5951) = low byte of Guard.curr_seq [432A]
      guards_seq_hi[i]           = high byte [432B]                      ; dead: the raw sequence POINTER
Guard.direction = 0x56; draw_guard_hp(0, guardhp_curr); guardhp_curr = 0
```
The sword state, HP, fall values, frame and y are not stored.

### 2.2 Restoring: enter_guard `0000:3806` (combat.md 2.3, the relevant part)
```
... y = y_land[row + 1]; x = guards_x[i]; col from x; direction = guards_dir[i]; charid = 4 on level 3 else 2
if guards_seq_hi[i] == 0: fresh (seq 63 skeleton / seq 77 guard)
else: Char.curr_seq = guards_seq_lo[i] + 256*guards_seq_hi[i]           ; 38D4: pointer restored as-is, Char.sword NOT set
play_seq                                                                 ; exactly ONE step
guard_skill = guards_skill[i] (>= 12 -> 3)
if frame in {185, 177, 178}: alive = 1; draw_guard_hp(0, hp); guardhp_curr = 0     ; 390B..3931
else: alive = -1; justblocked = guard_refrac = is_guard_notice = 0; get_guard_hp (full HP)
fall_x = fall_y = 0; action = 1; saveshad
```
Resting pointers (the byte after the shown frame, i.e. where play_seq stopped; VERIFIED from the bytes):

| death | seq | loop | resting curr_seq | one play_seq from there |
|---|---|---|---|---|
| sword / fall (dying) | 71 (`ds:20C2`) | `ds:20CE` f185, `FF 20CE` | **0x20CF** | FF -> 0x20CE -> f185, pointer 0x20CF |
| spiked | 51 (`ds:20D2`) | `ds:20D9` f177, `FF 20D9` | **0x20DA** | f177 |
| chomped | 54 (`ds:20DD`) | `ds:20E1` f178, `FF 20E1` | **0x20E2** | f178 |
| fatal landing | 22 (`ds:20B0`) | `ds:20B9` f185, `FF 20B9` | **0x20BA** | f185 |

So a guard that has finished dying reappears on the same frame with no animation. Seq 71 = act1, F6, f179, f180, f181, f182, dx+1 f183,
dx-4 f185, loop; seq 85 = act5 then seq 71; seq 22 = act5, F4, dy-2, dx+3, f185, F6, loop.

Quirk (INFERRED from the code above): if the kid leaves while the body is still in seq 71 before f185 (the 6 ticks after the kill),
the stored pointer is mid-sequence. On re-entry play_seq shows f180..f183, the frame test fails, and the guard is set **alive with full
HP**, finishes the dying animation and then lies on f185 with alive = -1 (control() no longer treats it as dead). Its sword field is
whatever the Guard record last held. Rare; port it only if it is free.

### Implementation notes (section 2)

- `GuardSlot.dead: Option<usize>` (sim/combat.rs:10-20) stores a seq number and `enter_guard` replays it 24 times
  (sim/combat.rs:152-162). Replace with the saved animation position: store `(seq, pc)` of the guard's `Kid` record at leave time
  (`seq`, `pc`; plus `cur` if the op list is not re-derivable from `seq`), restore it, call `play_seq` **once**, then decide alive/dead by
  the frame (185/177/178) as in 2.2. This also covers seq 22 and any mid-death pointer for free.
- The pointer mapping original -> port: `seq` = the sequence that owns the bytes (71, 51, 54, 22), `pc` = index of the `Jmp` op that
  follows the resting frame. A unit test can check that one `play_seq` from there yields the table's frame.
- leave_guard must store the guard's current x (death placement may have moved it) and `tile = 10*row`.
- Do not store/restore the sword for dead guards (the original does not); set alive from the frame test.

---

## 3. Death and restart of the kid

### 3.1 Death sequences (VERIFIED)

| cause | code | kid ends on | is_dead frame |
|---|---|---|---|
| sword (en garde, last HP, or any hit without sword) | hurt_by_sword 1.4 | seq 85 -> 71, or 81 off a ledge | 185 |
| spikes | check_spiked `7394` | seq 51 | 177 |
| chomper | chomped `5B67` | seq 54 | 178 |
| fatal landing / dead at landing | land `5FF4` | seq 22 | 185 |
| falling out of the level (Kid.room 0) | fell_out `7900`: take_hp(100), alive = 0, `C335(1)`, **frame = 185** directly | 185 |
| potion / loose floor / other HP loss to 0 | control_kid `7A1F`: alive = 0; control() starts seq 71 on frames 15/166/158/171 | 185 |
| level 12 shadow killed | check_killed_shadow (5.5) | take_hp(100) path above | |

is_dead `0000:82ED`: frame 177, 178 or 185.

### 3.2 Death counter, play_kid `0000:792A` (VERIFIED)

Kid.alive is a counter after death (-1 = alive).
```
fell_out(); control_kid()                 ; control_kid: alive < 0 and hitp_curr == 0 -> alive = 0
if alive < 0 or !is_dead(): return
if resurrect_time [3D76] != 0: (cheat) stop sounds, hitp_delta = hitp_max, seq 2, x += 8, set_start_pos; ...
if sound playing (0C83:720D) and [4360] != 5: return          ; wait (INFERRED: [4360] == 5 is a no-sound mode)
is_show_time [435C] = 0
switch alive:
  0..5: alive++
  6:    if sound on [31C2] and level not 0/15: death music (8309): Guard.charid == 1 -> sound 32,
        holding_sword [5A8C] -> 28, else 24;   alive++
  7:    if sound playing: return (wait for the music to end)
        if rem_min == 0: expired()  (31F4, 3.5)
        level 0 or 15: text timer = 36 (no text);  else: text timer = 288, show "Press Button to Continue" (ds:09A6)
        alive++
  >= 8: nothing
```
Timeline, sword death with all sounds instantly finished (INFERRED, real sounds add their length): kill at tick T (f179) -> f185 at T+5
-> alive 1..6 at T+5..T+10 -> music at T+11 -> text at T+12 (or after the music ends).

### 3.3 "Press Button to Continue" and the restart (VERIFIED)

- Key handler `0000:054D..05E8`: if rem_min != 0 and Kid.alive > 6 and (Shift held [4E04] or the key is Enter 0x0D): the key becomes
  Ctrl+A (1). Ctrl+A (any time, not level 15): stop sounds, restart flag `[4310]` = 1.
- play_kid_frame returns 1 when `[4310]` is set (`0F01`), so the rest of the tick is skipped; play_level_2 `4D90` clears the flag and
  returns current_level; play_level `48C0` restarts the same level.
- Text timer `[60C8]` (kind in `[4C94]`), per frame in `0AAE..0B39`: when it reaches 1 and the kind is 288 or 36 -> `[42F8]` = 0 and
  start_game `0358` (back to the title / attract loop; INFERRED from `03A0`: `[42F8]` == 0 -> `181D`). So **no key within 288 ticks
  (24 s) returns to the title**; on level 0/15 after 36 ticks. During the last 72 ticks of a 288 timer the text blinks with period 12:
  visible while `timer % 12 <= 3`, erased otherwise, re-drawn (plus a call to `0C83:7198`, UNVERIFIED: a beep) on `== 3`.
- Level start shows "Level N" for 24 ticks (`C270`: levels 1..13 and not seamless; **level 13 is shown as 12**), which also
  overwrites the text timer, so the 288 countdown does not survive a restart.

### 3.4 What a restart resets and what persists (VERIFIED)

play_level(level) `48C0` on every (re)start: load_level `0E97` (LEVELS.DAT resource 2000+level, so **all tiles are reset**),
pos_guards `518B` (guards fresh), and clears drawn_room, leveldoor_open, flash_time, united_with_shadow, guardhp_curr, hitp_delta,
demo indices, `[4CAE]`, Guard.charid = 2, Guard.direction = 0x56. Graphics and the cutscene are skipped when level == current_level.
Then do_startpos `4A0F`, **have_sword = (level != 1)** (`49A6`), start.

| state | on restart (same level) | on next level | new game (`4893`, only when `[42F4]` == 0) |
|---|---|---|---|
| hitp_max, hitp_curr | = hitp_beg_lev (`4AA9`) (level 0: 4) | = hitp_beg_lev = hitp_max at level end (`4DD7`) | hitp_beg_lev = 3 |
| hitp_beg_lev | unchanged (except level-3 checkpoint `3D6D`: = hitp_max) | = hitp_max | 3 |
| level 12 -> 13 seamless (`[59C8]`) | | HP **not** reset (`4A8B`) | |
| rem_min / rem_tick | **unchanged** (time keeps its value; it does not run while dead, combat.md 8) | unchanged | 60 / 719 |
| have_sword | level != 1 | level != 1 | |
| checkpoint `[4CAC]` | kept (level 3: start room 2, tile 6, start_dir 0xFF -> direction NOT 0xFF = right, then seq 5's flip: **facing left** (corrected, `4A24`/`4A86`), tile (7, 4, 0) set to 0) | cleared (`4DDD`) | cleared (`4884`) |
| upside-down `[4FD0]`, feather `[409E]` | cleared by set_start_pos `4B17/4B1A` | cleared | cleared |

Consequence: a life potion drunk on a level is lost if the kid dies there (unless the level-3 checkpoint was passed after it).
Upside-down is also cleared as soon as the kid is dead (`0EE7`: upside and alive >= 0 -> upside = 0, redraw).

Next level (play_level_2 `4DA2`): when next_level `[3D58]` != current_level, the frame loop **keeps running** until no sound plays,
then stop sounds, hitp_beg_lev = hitp_max, checkpoint = 0, return next_level.

### 3.5 Time out: expired `0000:31F4` (VERIFIED)
Called from play_frame when level < 13 and rem_min == 0 (`09C9`), and from play_kid at alive 7. Unless `[00B2]` != 0 (INFERRED: demo
build flag): plays the cutscene `0000:2A1B` (sound 36, INFERRED: the princess's time is up) and returns to the title (`0358`).

### 3.6 Screen flashes (VERIFIED)

Frame loop `0000:4D64`: before play_frame `[336E]` = frame length and the HP deltas are cleared; after it: flash_if_hurt `5374`, draw
`09DD`, remove_flash_if_hurt `5392`, wait.
```
flash_if_hurt:   if flash_time [438E] != 0: flash(flash_color [4ED2]) elif hitp_delta [612E] < 0: flash(12)
remove_flash:    if flash_time != 0: flash_time--, unflash() elif hitp_delta < 0: unflash()
```
flash(c) `2E60` on VGA (`[3035]` == 5): `0C83:198F(c, 0)` sets **palette entry 0** (the background black) to the EGA colour c from
`ds:2BBA` (6-bit RGB triples, read by `0C83:198F`; all 16 entries are the standard EGA palette, VERIFIED by a pop-app test); unflash
sets entry 0 back to colour 0. Only the background changes colour; sprites/tiles keep theirs.
A flash with flash_time N lasts **N frames**; the red hurt flash lasts the one frame in which the kid's HP delta was negative (only the
kid, never guard damage). Table:

| colour c | RGB (0..63) | frames | trigger | code |
|---|---|---|---|---|
| 12 bright red | 63,21,21 | 1 | kid hitp_delta < 0 this tick (and no other flash active) | `5388` |
| 4 red | 42,0,0 | 2 | heal potion | `8232` |
| 4 red | 42,0,0 | 4 | life potion | `8252` |
| 2 green | 0,42,0 | 3 | feather potion | `1631` |
| 14 yellow | 63,63,21 | 8 | sword picked up | `81D5` |
| 15 white | 63,63,63 | 18 | Jaffar killed / united with the level-12 shadow | `835A` / `480E` |
| 15 white | 63,63,63 | 5 | level-12 shadow killed (check_killed_shadow) | `84D7` |
| 15 white | | 5 | inside a cutscene (`2CA0`, via the cutscene player `2540`) | |
| 4 | | 2 / 4 | cheat keys heal/life (`0886`, `08A3`; INFERRED: debug keys) | |

Other graphics modes use other routines (`95BE`, `95F3`, `03FF`); not needed.

### Implementation notes (section 3)

- The port's `Kid.alive` is a bool; the original needs an `i8`-like counter (`-1` alive, `0..8` after death). Keep the bool for logic
  and add `death_ticks: u8` in `World` if that is less invasive; the counting rules are in 3.2 and depend on "sound playing", which the
  headless core does not model: treat sounds as instantly finished in `pop-core` (deterministic) and let `pop-app` optionally hold
  the counter while a death sound plays (UNVERIFIED sound lengths).
- New `World` state: `rem_min`, `rem_tick` (combat.md 8), `hitp_beg_lev`, `checkpoint`, `text_timer` + kind, `flash: (colour, time)`,
  `restart_requested`, `next_level`. Put the cross-level part (`rem_min`, `rem_tick`, `hitp_beg_lev`, `checkpoint`, level number)
  in a small `Progress` struct; restart = rebuild `World` for the same level from `Progress` (the original reloads the level fully).
- `pop-core` exposes `flash` as data (colour index + remaining frames); `pop-app` maps it to a background tint plus a translucent full-window overlay of the same EGA colour (alpha 0.4) for that many frames: the opaque 3D room hides most of the background, so palette entry 0 alone would not be seen.
- `have_sword` (kid.rs:156, sim.rs:98 sets it for level >= 2): the original rule is `level != 1` (level 0, the demo, has the sword).
- Restart input: Shift or Enter only when `alive > 6` (and rem_min != 0); Ctrl+A any time except level 15.

---

## 4. Level transitions, cutscenes, level-specific events

### 4.1 What the game loop must provide (VERIFIED)

| hook | when | effect |
|---|---|---|
| next level via op F1 | end of seq 70 (exit stairs) | next_level++ (mechanics.md 0) |
| level 6 fall | leave_room returns -2: level 6, Char.room 1 (`3D22`) | play_frame `0981`: Kid.y = -1 (`[4398]`), stop sounds, next_level++ |
| level 12 room 23 | Kid.room == 23 (`09AA`) | next_level++, stop sounds, seamless `[59C8]` = 1 (no HP reset, no "Level" text) |
| level 0 room 24 | Kid.room == 24 (`095B`) | demo over: start_game -> title |
| level 14 room 5 | check_the_end (`100A`) | ending `2F2F`: cutscene `2960` (sound 26), then `0C89` (UNVERIFIED: end screen) |
| time out | 3.5 | cutscene `2A1B`, title |
| level change | play_level `48C0`, level != current | cutscene from **tbl_cutscenes `ds:0FB4`** (far pointer per level), then load graphics |

tbl_cutscenes (VERIFIED, offsets in segment 0000): level **2: `2B04`**, **4: `2AD0`**, **6: `2B04`**, **8: `2891`**, **9: `28FB`**,
**12: `2A71`**, all others none. So a cutscene plays before levels 2, 4, 6, 8, 9 and 12 (never on a restart). Launcher
`322D(1, ptr, level > 2)` (third argument meaning UNVERIFIED). Contents (INFERRED from the calls: sound id, then
`2540(n)` = play n cutscene ticks, `2724`/`273D` = start a princess/vizier sequence):

| cutscene | sound | ticks (sum of 2540 args) | notes |
|---|---|---|---|
| `2B04` (before 2 and 6) | 27 | 26 | princess waiting |
| `2AD0` (before 4) | 27 | 26 | same shape, different setup `27D5` |
| `2891` (before 8) | 35 | 20 + 20 + 20 | princess and the mouse (`[4396]` = 0 at the end) |
| `28FB` (before 9) | 35 | 5 + 9 + 58 | |
| `2A71` (before 12) | 40 | 2 + 24, then `2B04` | |
| `2A1B` (time out) | 36 | 2 + 100 | |
| `2960` (ending) | 26 | 8 + 5 + 2 + 39 + 9 + 41 | |
The cutscene scripts themselves are presentation (PV.DAT, cutscene frame table); not analysed further (UNVERIFIED beyond the table).

### 4.2 Level 8 mouse (VERIFIED)

timers `4FA9` (every frame, before play_frame): level 8, **Char.room `[3D6B]` == 16** (the working copy; in practice the kid's room,
INFERRED: room 16 has no guard so Char last held the kid), leveldoor_open != 0: leveldoor_open += 1; at exactly **150**: do_mouse.
The counter starts at 1 when the exit door finishes opening (tiles.md 8.1), so the mouse appears **149 frames** after the door opened
while Char.room is 16 (counting pauses elsewhere), and the counter keeps running past 150 harmlessly.

do_mouse `5332`: loadkid (Char = Kid copy, so sword/fall values come from the kid), charid = 24, x = 200, row 0, y = 55, alive = -1,
direction left, guardhp_curr = 1, seq 105, play_seq, saveshad.

| seq | addr | ops |
|---|---|---|
| 105 | `ds:2259` | act1, then loop: f186, dx+5 f186, dx+3 f187, dx+4 (net 12 per 3 ticks, forward = left) |
| 107 | `ds:226F` | act0, f186 x3, f188 x8, flip, dx+8, jump into the seq-105 loop at `ds:225B` (action stays 0) |

autocontrol_mouse `3EDF` (from play_guard, no control()): nothing if direction == 0x56; action 0 and x >= 200 -> clear_char;
action != 0 and x < 166 -> seq 107 + play_seq. So: runs left from 200, stops below 166 for 11 ticks, turns, runs right, vanishes at 200.
Frames 186..188 come from the kid table (charid 24). The mouse is never seen by can_guard_see_kid and has no HP bar.

### 4.3 Level 4 mirror and its shadow (VERIFIED)

- The mirror tile appears when the exit door finishes opening (tiles.md 8.1). is_obstacle marks the jump-through (`[42E0]` = -1)
  during the kid's running jump (frames 39..43, facing left).
- check_mirror `5010` runs in the **drawing pass** (`09DD -> B7C0 -> BF52`), not in play_frame: if `[42E0]` == -1 ->
  jump_through_mirror `5082`; else, kid on a mirror tile -> draw the reflection. The draw pass skips this call on frames that do a full
  redraw (`[4C46]` set by a room change or upside toggle, `0A23`), INFERRED harmless.
- jump_through_mirror: Char = Kid (loadkid `718B` copies the whole record), mirror image check_mirror_image `50D3`:
  x = 2*(x_bump[col+5] + 10) - x in byte arithmetic with **col = Char.curr_col `[3D66]`, i.e. the kid's stored column** (not the
  mirror's; VERIFIED), direction flipped, `[42E0]` = 0, **charid = 1**,
  sound 45, saveshad (the shadow continues the kid's running-jump sequence, mirrored), guardhp_curr = guardhp_max = hitp_max,
  **hitp_curr = 1**, both HP bars redrawn.
- autocontrol level 4 `46F4`: Char.room 4: x < 80 -> clear_char, else move_1_forward (runs away). It is never fought.
- Level 4 leave room 11 to the left on row 0 with the door open (`3DC7`): sound 25, leveldoor_open = 0x4D (music, once).

### 4.4 Level 13 Jaffar, level 0 demo (VERIFIED)
- Level 13 start: kid seq **84** (run) (`4ADF`); level 1: seq 7 + button (5, 2, 0); all others seq 5.
- Jaffar flow: combat.md 7.4 (meet_Jaffar `3DA2`, on_guard_killed `8353`, Jaffar_exit `3D37`); nothing new.
- Demo: control_kid on level 0 (`7A3D`) calls `7A91` then control() **without reading the keyboard**; any key during the demo
  (`0C83:0E3F` != 0) sets `[42F8]` = 1 and calls start_game (the real game starts); key 0x0C -> `1E6F` (INFERRED: load saved
  game) and start_game only if it returns non-zero. `7A91`:
  after the demo guard died (checkpoint): forward + control_x = -1 every tick (runs right into room 24 -> title);
  while Kid.sword != 0: guard_skill = 10, autocontrol_opponent with Char = Kid (the guard AI drives the kid), guard_skill = 11;
  else do_auto_moves(`ds:0F36`). on_guard_killed level 0 resets demo_time and the move index.

Demo move list `ds:0F36` (time, move), moves as in 5.4:
(0,0) (1,1) (13,0) (30,1) (37,5) (47,0) (48,1) (65,0) (73,2) (75,0) (99,2) (100,0) (115,5) (128,6) (136,3) (157,7) (158,0) (159,1)
(171,4) (177,0) (178,1) (188,0) (193,1) (205,0) (233,-1 end).
On level 0 the control flags are **not** cleared between ticks (no keyboard read, no move_0 except move 0), so each move holds until
the next entry; on the level-5 shadow they are cleared every tick by autocontrol_opponent and re-applied, same effect.

### Implementation notes (section 4)

- `Op::EndLevel` (kid.rs:931) is a no-op: make it `next_level += 1` and let `World` report `LevelExit` to the app; the app waits for
  sounds (optional), plays the cutscene for the new level from the table above, then builds the new `World` from `Progress`.
- Add the level-6 (`leave_room` -2) and level-12 (room 23, seamless) cases next to `on_room_left` (sim.rs:223).
- Mouse: a `charid 24` guard record; reuse the guard slot machinery (`clear_char`, play_guard_frame) with `autocontrol_mouse` only.
  The counter lives in `leveldoor_open` exactly as in the original (it doubles as the door state on level 8).
- Mirror: run `check_mirror` once per logic tick **after** the tick (the original does it in the per-frame draw, i.e. after
  play_frame); create the shadow by copying the kid with the mirror formula and `charid = 1`.
- Demo: level 0 world with a `demo` flag: no input, `do_auto_moves(DEMO_MOVES)` / guard AI as above; end at Kid.room 24 or on any
  key; the app returns to the title.

---

## 5. Potions, frame length, the two shadow rules

### 5.1 Potions (VERIFIED, refines tiles.md 8.2)

Drinking: crouched + Shift on tile 10 -> get_item -> do_pickup(mod >> 5) stores the kind in pickup_obj_type `[50BE]` (sword: -1),
tile -> floor, seq 78; the pick-up frame runs op F3 1 -> proc_get_object `81AB` (kid only, `charid == 0`):

| `[50BE]` | potion | effect | sound | flash |
|---|---|---|---|---|
| 0 | empty flask | nothing | (drink sound 18 from the seq only) | |
| 1 | heal (small red) | only if hitp_curr != hitp_max: stop sounds, **hitp_delta = +1** (assignment) | 33 | red 4, 2 frames |
| 2 | life (big red) | stop sounds, **add_life `1217`**: hitp_max = min(hitp_max + 1, 10), hitp_delta = hitp_max - hitp_curr (full heal) | 30 | red 4, 4 frames |
| 3 | feather (green) | set_feather_fall `162B`: feather `[409E]` = 1, stop sounds | 39 | green 2, 3 frames |
| 4 | invert (blue) | toggle_upside `161C`: `[4FD0]` = NOT `[4FD0]`, full redraw `[4C46]` = 1 | | |
| 5 | hurt (blue small) | stop sounds, hitp_delta = -1; **level 15: -((hitp_max + 1) >> 1)** | 13 | red hurt flash (12, 1 frame) |
| 6 | open (level 15) | get_tile(8, 0, 0); trigger_button(0, 0, -1) | | |
| -1 | sword | have_sword = -1 | 37 | yellow 14, 8 frames |

- Feather: fall_accel (`72BF`) in action 4: fall_y += 1, max **4** (normal += 3, max 33); seq op F7 branches on it. It ends in timers
  `4FD0` on the first frame where **no sound is playing** (i.e. when sound 39 ends, or immediately if sound is off: INFERRED quirk).
  With the 18.61 s speaker tune and 5/60 s frames that is frame T0+224 (223 * 5/60 = 18.58 s still plays): the port's
  `FEATHER_TICKS` = 224 (was 225, SDLPoP's 18.75 s); a sound that interrupts the tune would end it earlier (UNVERIFIED).
  Cleared by set_start_pos.
- Upside-down is **display only**: every reader of `[4FD0]` is drawing/text code (`0A65`, `0A93`, `15A6`, `4D1F`, `4D3A`) plus the
  resets; the controls are not swapped. Cleared when the kid dies (`0EE7`) and at every (re)start.
- The HP change is applied by do_delta_hp at the end of the tick with clamping (combat.md 6); a heal on the tick the kid is also hurt is
  overwritten by take_hp (both assign the delta; whichever runs last wins).

### 5.2 Frame length 5 vs 6 (VERIFIED)

- Game timer: `0000:0055` installs the ISR `0C83:8E9C` via `0C83:8D84` with rate **60** (`mov ax,3Ch`); the PIT divisor is
  1193181 / 60 (`ds:3388` = 0x1234DD, divided in `0C83:8C86`). The ISR decrements the four words `[336C] [336E] [3370] [3372]` while
  non-zero, then chains the BIOS tick by a fraction accumulator.
- Frame loop `4D6A`: `[336E]` = **6 if Kid.sword == 2, else 5**; play_frame; draw; `while [336E] != 0: wait` (`4DC5`).
- So a frame lasts 5/60 s = **12 fps** normally and 6/60 s = **10 fps** while the kid's sword is drawn. The decision uses Kid.sword at
  the **start** of the frame, i.e. the state left by the previous tick. The level-start wait (`4BD4`) uses 5. Cutscenes use the same
  timer through `[336C]` (`2549`).

Implementation notes: `World::step` (sim.rs:140, `SUBSTEPS = 5`, `STEP_HZ = 60`): make the substep count per logic frame dynamic:
at the moment a new logic frame starts, `frame_len = if kid.sword == 2 { 6 } else { 5 }`; run the tick on the last substep of that
frame as now; `alpha()` divides by `frame_len`. Game logic per tick does not change, so all replay tests stay valid; add one test that
counts 60 Hz steps over a fight (e.g. 30 ticks with the sword drawn = 180 steps).

### 5.3 check_killed_shadow `0000:84B1` (VERIFIED; was unverified in the port)

Called in **both** ticks: play_kid_frame `0EDF` (Char = Kid, Opp = Guard) and play_guard_frame `0F86` (Char = Guard, Opp = Kid), right
after load_fram_det_col and before the character's own logic.
```
if level != 12: return
if (Char.charid | Opp.charid) != 1: return          ; exactly kid (0) + shadow (1)
if Char.alive >= 0 or Opp.alive < 0: return         ; Char alive and Opp dead
flash white (15) for 5 frames; take_hp(100)         ; on Char: kid -> hitp_delta, shadow -> guardhp_delta
```
- Shadow dies (HP 0 -> alive = 0 in its next play_guard) -> the following kid tick kills the kid: **killing the shadow kills the kid**.
- Kid dies of anything on level 12 while the shadow exists (fall, spikes, sword) -> the next shadow tick kills the shadow.
- After uniting the rule is off: the unite code (`4805..4831`) does add_life, flash 18, united = 42, **Char.charid = 0, savekid
  (Kid = the shadow's record!), clear_char**, and play_guard_frame's final saveshad writes a Guard with charid 0 -> OR == 0.
  So at the unite tick the kid takes over the shadow's x/frame/sequence (INFERRED: they are within 10 units).
- do_delta_hp (`12A6`): level 12 and Opp.charid == 1 and guardhp_delta != 0 -> hitp_delta = guardhp_delta (overwrites, does not add).

Implementation notes: the port has it inline in sim.rs:187-192 plus a copy in do_delta_hp (sim/combat.rs:466-472). Make it one
function called at the two original points (start of the kid tick with kid/guard, start of the guard tick with guard/kid) so the
one-tick delay is exact, and check the unite path (sim/combat.rs:384) copies the shadow record into the kid before clearing it.

### 5.4 do_auto_moves `0000:4633` and the level-5 shadow (VERIFIED; was unverified in the port)

```
do_auto_moves(table):                    ; table of (time: i16, move: i16), `ret far 2`
  if demo_time [4E12] >= 254: return
  demo_time += 1
  i = demo_index [464E]
  if table[i].time <= demo_time: demo_index += 1        ; use entry i
  else: i = demo_index - 1                              ; keep the previous entry
  m = table[i].move
  m == -1: [4E34] = 0 (never set elsewhere: no-op); 0 move_0_nothing; 1 forward; 2 back; 3 up; 4 down;
  5 up + forward; 6 shift (move_6); 7 shift off (move_7); anything else (e.g. -2): nothing
```
The index advances by at most one per call, so two entries due on the same tick are applied on consecutive ticks.

Level 5 shadow, autocontrol `470E` (after autocontrol_opponent cleared all flags):
```
if Char.room != 24: return
if demo_time == 0:
    if gate (24, col 1, row 0) modifier < 80: return      ; waits; x is not checked while waiting
    demo_index = 0
do_auto_moves(ds:0F16)
if Char.x < 15: clear_char
```
Move list `ds:0F16` (VERIFIED): **(0, 0) (1, 1) (14, 0) (18, 6) (29, 7) (45, 2) (49, 1) (255, -2)**.

Test vector (demo_time / index before the call -> move applied this tick):

| call | demo_time | index used | move |
|---|---|---|---|
| 1 (gate >= 80) | 1 | 0 | nothing |
| 2 | 2 | 1 | forward |
| 3..13 | 3..13 | 1 | forward |
| 14 | 14 | 2 | nothing |
| 15..17 | | 2 | nothing |
| 18 | 18 | 3 | shift |
| 19..28 | | 3 | shift |
| 29 | 29 | 4 | shift off |
| 45 | 45 | 5 | back |
| 49 | 49 | 6 | forward, held until x < 15 (clear_char) or demo_time 254 |

Creation (check_shadow `37D7`): level 5, entering room 24 while tile (24, 3, 0) is a potion (10): do_init_shad(init_shad_5, seq 2);
if the potion is gone, nothing (and no enter_guard). do_init_shad resets demo_time but **not** demo_index (the autocontrol does it when
the gate opens). Level 6 (`37A7`): every entry of room 1 re-creates the shadow (sound 25 + leveldoor_open = 0x4D only the first time).
Level 12 (`3765`): only while united == 0, entering room 15 with the sword tile (15, 1, 0) no longer a sword: shadow_initialized = 0,
do_init_shad(init_shad_12, seq 7) (every entry); with the sword still there nothing happens (no guard either).

Implementation notes: SHADOW5_MOVES in sim/combat.rs:45 must become the 8-entry table above with the index rule; `ShadowState`
needs `demo_index` beside `demo_time` (both reset as described). Share `do_auto_moves` with the demo (4.4).

---

## 6. Implementation order and risk

| # | item | sections | effort | risk | why this position |
|---|---|---|---|---|---|
| 1 | Frame length 5/6 at 60 Hz | 5.2 | S | low | isolated in `World::step`, no logic change, visible in every fight |
| 2 | start_fall guard branch (82/83/81, droppedout, clear_char) + land fatal rule for charid 2 | 1.3 | S | low | replaces a guess with exact code; vectors given |
| 3 | Dead-guard save/restore by (seq, pc) + one play_seq | 2 | S | low | removes the 24x replay; resting pointers known |
| 4 | Death placement gate case | 1.4 | S | low | small branch fix |
| 5 | check_guard_bumped / check_chomped_guard | 1.1, 1.2 | M | medium | box values (char_x_*_coll) must match the kid pipeline; needs the per-tick order in guard_tick |
| 6 | check_killed_shadow at the two call points, unite copy, do_auto_moves + level-5 table | 5.3, 5.4 | S | low | both verified now |
| 7 | Potions (proc_get_object, feather, upside flag) | 5.1 | M | low (logic) / medium (feather ends with a sound) | feather duration depends on sound length; pick a fixed tick count in core (UNVERIFIED value) or let the app report "sound ended" |
| 8 | Death counter, restart, Progress struct, time persistence | 3.2-3.4 | M | medium | touches World construction and the app loop; sound-wait rules need a policy |
| 9 | Flashes as data + app tint | 3.6 | S | low | presentation only |
| 10 | Level transitions: EndLevel op, level 6/12 exits, next level flow | 4.1 | M | medium | app-level state machine |
| 11 | Mouse (level 8) | 4.2 | S | low | self-contained |
| 12 | Level-4 mirror shadow | 4.3 | M | medium | runs in the draw pass in the original; ordering relative to the next tick matters |
| 13 | Demo level 0 | 4.4 | M | medium | needs the guard-AI-drives-kid path and the title loop |
| 14 | Cutscenes | 4.1 | L | high | scripts not analysed (UNVERIFIED), mostly art/presentation |
| 15 | Mid-death leave quirk (guard revived with full HP) | 2.2 | S | low value | optional fidelity |

UNVERIFIED in this round: sound lengths (they gate the death counter, the next-level exit and the feather), the meaning of `[4360]`
== 5, `0C83:7198` in the blinking text, the cutscene script contents, `0C89` after the ending, the third argument of `322D`.
