# Guards, sword fighting, special characters, hit points (PRINCE.EXE v1.1)

Round 3 of the disassembly notes (milestone 5). Conventions, address notation and the kid basics are in `docs/mechanics.md`
(sections 0-1: `0000:XXXX` = code, `ds:XXXX` = data, file = ds + 112832). Tile mechanics are in `docs/tiles.md`.

- **VERIFIED**: read from the code or data at the address given.
- **INFERRED**: our interpretation (a name, a meaning, a consequence), not proven by the code.

Function and variable names follow SDLPoP where the code matches it instruction for instruction (it does for everything below);
the names are INFERRED, the behaviour is VERIFIED. Tables were dumped with `pop-disasm ds`, sequences were decoded from
the sequence table `ds:189C` (word per seq number, seq 1 at `ds:189E`).

---

## 0. Character records and the swaps (VERIFIED)

Three 16-byte records with the `Char` layout of mechanics.md section 1:

| record | ds | frame | x | y | dir | col | row | action | fall_x | fall_y | room | repeat | charid | sword | alive | seq (word) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Char (working copy) | 3D62 | 3D62 | 3D63 | 3D64 | 3D65 | 3D66 | 3D67 | 3D68 | 3D69 | 3D6A | 3D6B | 3D6C | 3D6D | 3D6E | 3D6F | 3D70 |
| Kid | 4396 | 4396 | 4397 | 4398 | 4399 | 439A | 439B | 439C | | | 439F | | 43A1 | 43A2 | 43A3 | |
| Guard ("shad") | 431C | 431C | 431D | 431E | 431F | 4320 | 4321 | 4322 | 4323 | 4324 | 4325 | 4326 | 4327 | 4328 | 4329 | 432A |
| Opp (the other one) | 4EBE | 4EBE | 4EBF | 4EC0 | 4EC1 | 4EC2 | 4EC3 | 4EC4 | | | 4EC7 | | 4EC9 | 4ECA | 4ECB | 4ECC |

There is only **one** active guard. `Guard.direction == 0x56` means "no guard" (`0000:0F73`, `8388`).
Copies (`memcpy` far call `0C83:B20C`, 16 bytes):

| addr | name (INFERRED) | effect |
|---|---|---|
| `718B` | loadkid | Char = Kid |
| `71A0` | savekid | Kid = Char |
| `71B5` | loadshad | Char = Guard |
| `71CA` | saveshad | Guard = Char |
| `71DF` | loadkid_and_opp | Char = Kid, Opp = Guard |
| `71F8` | savekid_and_opp | Kid = Char, Guard = Opp |
| `7211` | loadshad_and_opp | Char = Guard, Opp = Kid |
| `722A` | saveshad_and_opp | Guard = Char, Kid = Opp |

charid (VERIFIED by use): 0 kid, 1 shadow, 2 guard (also fat guard and Jaffar, only the images differ), 4 skeleton,
5/6 cutscene characters, 0x18 = 24 mouse. Frame table per charid, load_frame `0000:6E96`: 0 and 24 -> kid table; 1 -> guard
table for frames 150..189, kid table otherwise; 2 and 4 -> guard table, frames 102..106 read as 172..176; 5/6 -> cutscene table.

---

## 1. Tick order with a guard (VERIFIED)

play_frame `0000:090A` (one game tick):

1. `965B`, `85F4` (mobs, trobs; tiles.md), **check_skel** `4513` (section 7.2), **check_can_guard_see_kid** `51D1` (2.3).
2. **play_kid_frame** `0ED3` (mechanics.md 3.1; returns non-zero on a level change -> the rest is skipped).
3. **play_guard_frame** `0F73` (below).
4. If resurrect_time `[3D76]` == 0: **check_sword_hurting** `440E`, then **check_sword_hurt** `43C8` (section 5).
5. **check_sword_vs_sword** `1386`: Kid.frame == 167 or Guard.frame == 167 -> **sound 10**.
6. **do_delta_hp** `12A6` (section 8).
7. exit_room `3ABB` (mechanics.md 10, guard part in 2.4), **check_the_end** `0FE8` (room entry, 2.4), **check_guard_fallout** `395D` (2.5).
8. Level special cases (section 7), show_time `C170` (section 9), expired `31F4` when level < 13 and rem_min == 0.

The hit-point deltas `[612E]` (kid) and `[4C76]` (guard) are cleared at the start of every frame (`0000:4D7F`), set by
take_hp during the tick and applied once in step 6.

**Frame length during fights** (`0000:4D6A`): before each frame `[336E]` = **6 if Kid.sword == 2, else 5**; the frame ends by
waiting until `[336E]` reaches 0 (`4DC5`). The decrement is done by the timer interrupt (not traced). So while the kid
holds his sword drawn, each tick lasts 6/5 as long (INFERRED: 12 Hz -> 10 Hz real time). The game logic per tick is unchanged.

### play_guard_frame `0000:0F73`
If Guard.direction != 0x56: loadshad_and_opp, load_fram_det_col `6E80`, check_killed_shadow `84B1`, **play_guard** `7ACD`.
Then, only if Char.room == drawn_room `[40DE]`: play_seq `6F90`, and only if **44 <= Char.x < 211**: fall_accel, fall_speed,
load_frame_to_obj, load_fram_det_col, set_char_collision, **check_guard_bumped** `5C6B`, check_action `7309`, check_press `7E2E`,
check_spike_below `7EE1`, check_spiked `7394`, **check_chomped_guard** `5D31`. Finally saveshad `71CA`.
So a guard in another room keeps thinking (autocontrol runs) but does not animate or move. It never leaves the room by itself.

### play_guard `0000:7ACD`
- charid 24 (mouse): autocontrol_opponent `3E6A` only.
- Otherwise: if alive < 0 and guardhp_curr `[5AA8]` == 0: alive = 0 and **on_guard_killed** `833C`. Then, if it is dead (alive >= 0,
  also when it just died) and charid == 1: clear_char `8388` (a dead shadow vanishes). Then **autocontrol_opponent** `3E6A` and
  **control** `607B`.

control `607B` (mechanics.md 2.2) for an opponent: dead (alive >= 0) on frame 15/166/158/171 -> **seq 71 (dying)**; action 4/5 ->
release_arrows only; **sword == 2 -> control_with_sword `6AD3`** (section 4); else **charid >= 2 -> control_guard_inactive `857B`**.

---

## 2. Guard setup and room changes

### 2.1 Level tail guard fields (VERIFIED)

The level is loaded at `ds:50C2` (so level byte n is at `ds:50C2 + n`; start room 2112 = `ds:5902`). Per room r (1..24), index r-1:

| file offset | ds | field | read by |
|---|---|---|---|
| 2119 | 5909 | guards_tile (0..29, >= 30 = no guard) | enter_guard `3819` |
| 2143 | 5921 | guards_dir (0 = right, 255 = left), copied to Char.direction **unchanged** | `3863` |
| 2167 | 5939 | guards_x | `384E` |
| 2191 | 5951 | guards_seq_lo | `38D7` |
| 2215 | 5969 | guards_skill (>= 12 -> 3) | `38EF`, `38F8` |
| 2239 | 5981 | guards_seq_hi (0 = start fresh) | `38A8` |
| 2263 | 5999 | guards_color (palette slice, 0 = default) | `387E` |
| 2287 | | 18 more bytes, not read here | |

**pos_guards `0000:518B`** runs at every level load (`play_level 4949`): for each room with tile < 30,
`guards_x = x_bump[tile % 10 + 5] + 14 = 72 + 14*col` and `guards_seq_hi = 0`. So the x and seq bytes stored in LEVELS.DAT
are ignored (they are 0/72/114/255 junk in the shipped files); they only carry state while the level is played (2.4).

Shipped levels (decoded with these offsets; colour 255 on level 14 is never used because level 14 loads no guard images):

| level | room: col,row dir skill colour |
|---|---|
| 0 (demo) | 4: 2,0 R s5 c1 |
| 1 | 3: 7,1 L s0 c2; 21: 6,0 L s0 c2 |
| 2 | 4: 0,1 R s1 c1; 7: 1,1 R s2 c3; 11: 3,1 R s1 c1; 15: 0,1 R s1 c1; 24: 0,1 R s3 c6 |
| 3 | none (the skeleton is created by check_skel) |
| 4 | 2: 4,1 R s4 c7; 18: 5,0 L s3 c3; 22: 8,0 L s3 c6; 24: 5,1 L s4 c7 |
| 5 | 8: 5,0 R s3 c6; 9: 5,1 L s3 c6; 12: 2,1 R s4 c7; 16: 2,1 R s4 c4 |
| 6 | 6: 2,1 R s5 c0 (fat guard) |
| 7 | 2: 4,0 L s1 c1; 10: 2,0 L s3 c6; 22: 6,2 L s2 c3 |
| 8 | 5: 5,0 R s7 c5; 12: 1,0 L s2 c3; 22: 7,2 L s3 c4; 24: 6,0 L s3 c4 |
| 9 | 2: 7,0 L s3 c6; 13: 7,2 L s1 c1; 17: 0,1 R s3 c5; 20: 3,0 L s1 c1 |
| 10 | 2: 4,2 L s3 c6; 5: 2,0 R s4 c7; 7: 6,0 R s3 c6; 8: 2,2 L s4 c3; 14: 7,0 L s3 c4 |
| 11 | 15: 7,0 L s4 c5; 16: 7,0 L s3 c1; 22: 8,0 R s4 c5 |
| 12 | none (the shadow is created by check_shadow) |
| 13 | 1: 7,0 L s9 c0 (Jaffar) |
| 14 | 11, 15, 24 (colour 255; not reachable, INFERRED) |

### 2.2 Per-level guard type and hit points (VERIFIED)

- **tbl_guard_type** `ds:02F0` (word per level 0..15) = 0,0,0,2,0,0,1,0,0,0,0,0,4,3,-1,-1. Used at level load `0000:0DBC`:
  -1 -> no guard images; otherwise chtab 5 is loaded from the file named by `ds:02D6[type]`: 0 `GUARD.DAT`, 1 `FAT.DAT`,
  2 `SKEL.DAT`, 3 `VIZIER.DAT`, 4 `SHADOW.DAT`. For type 0 also a palette file: `GUARD1.DAT` (`ds:02BF`) when the level is a
  palace level (`ds:01CE[level]` != 0: levels 4, 5, 6, 10, 11, 14), else `GUARD2.DAT` (`ds:02CA`).
- **Colour** (enter_guard `386A`): curr_guard_color `[59C4]` = guards_color only when the graphics mode `[3035]` == 5 (VGA) and
  tbl_guard_type[level] == 0, else 0. When the guard is drawn (`0000:4CF3`) with colour c != 0, 16 palette entries are set from the
  palette data `[4E24]` at offset `48*(c-1)` (`9519` call; INFERRED: 16 RGB triples, one slice per colour 1..7).
- **Hit points**, get_guard_hp `0000:3738`: `guardhp_curr [5AA8] = guardhp_max [4C96] = guardhp_delta [4C76]
  = extrastrength[skill] + tbl_guard_hp[level]`.
  - extrastrength `ds:0ED6` (word per skill 0..11) = 0,0,0,0,**1**,0,0,0,0,0,0,0.
  - tbl_guard_hp `ds:0EEE` (byte per level 0..15) = 4,3,3,3,3,4,5,4,4,5,5,5,4,6,0,0.
  - So: level 1 guards 3, level 2: 3, level 4 skill 4: 4, Jaffar (13, skill 9): 6. Shadow (do_init_shad) and skeleton: fixed (7.1, 7.2).

### 2.3 Placing a guard: enter_guard `0000:3806` (VERIFIED)

Called from check_shadow `3758` (end of check_the_end, i.e. on every room entry) when no special character applies.
For room `d = drawn_room`, `i = d - 1`, if guards_tile[i] < 30:
1. Char.room = d; curr_row = tile / 10; **y = y_land[row + 1] = 55 + 63*row** (`ds:22B2`); **x = guards_x[i]** (= 72 + 14*col unless
   changed during play); curr_col = get_tile_div_mod_m7(x); direction = guards_dir[i].
2. colour (2.2). charid = **4 on level 3**, otherwise 2.
3. guards_seq_hi[i] == 0: skeleton: sword = 2, **seq 63** (act1, knock-down op, then the en-garde loop `ds:19E6`);
   others: sword = 0, **seq 77** (frame 166, stand inactive, forever). Otherwise curr_seq = seq_lo + 256*seq_hi (a saved pointer).
4. play_seq once. guard_skill `[43A6]` = guards_skill[i] (>= 12 -> 3).
5. Frame 185, 177 or 178 (dead, spiked, chomped): alive = 1, draw_guard_hp(0, hp), guardhp_curr = 0.
   Otherwise alive = -1, justblocked `[350A]` = 0, guard_refrac `[436E]` = 0, is_guard_notice `[43AE]` = 0, get_guard_hp.
6. fall_x = fall_y = 0, **action = 1**, saveshad.

### 2.4 Room change: guard leaves, follows, or stays (VERIFIED)

exit_room `0000:3ABB`, after the kid changed room (leave dir in `[4E36]`: 0 left, 1 right, 2 up, 3 down):
- If there is a guard, it is alive (<0) and its sword is drawn (2), and the kid's new room has no guard (guards_tile >= 30) or a dead
  one (seq_hi != 0): the guard **follows** if (left) Guard.x < 91, (right) Guard.x >= 165, (up) Guard.curr_row < 0, (down) curr_row >= 3.
- Otherwise the guard is **left behind** (leave_guard).

**leave_guard `39E9`** (not for charid 1 or 24): stores into the guard's room slot: tile = get_tilepos(0, row) = 10*row
(the column is dropped, x is kept), colour, x, direction, skill; **alive guard: seq_hi = 0** (it comes back fresh: sword sheathed,
**full hit points** again via get_guard_hp); dead guard: seq_lo/hi = curr_seq (it comes back lying dead). Then Guard.direction = 0x56,
draw_guard_hp(0, hp), guardhp_curr = 0.

**follow_guard `3A92`**: guards_tile of the kid's room and of the guard's room = 0xFF, then goto_other_room(Guard, leave dir)
(`3B7A`: room link, x +-140, y +-189). The active Guard record keeps its state (HP, skill, sequence). enter_guard in the new room then
finds tile 0xFF and does nothing. (A dead guard lying in that room is lost.)

check_the_end `0FE8` (on the tick drawn_room changes): load room links `9CFD`, level 14 room 5 -> ending `2F2F`, redraw,
anim_tile_modif `0B3D`, start_chompers `950B`, check_fall_flo `1034` (level 13 rooms 23/16, tiles.md), **check_shadow `3758`**.
check_shadow first sets **offguard `[432E]` = 0**, then: level 12 shadow / level 6 shadow / level 5 shadow (section 7) or enter_guard.

### 2.5 Falling out, check_guard_fallout `0000:395D` (VERIFIED)
Only when a guard exists and Guard.y >= 211 (fell below the room):
- shadow: only if action == 4: clear_char.
- skeleton: Guard.room = down link of its room; if that is **room 3**: x = 133, row = 1, direction right, alive = -1, leave_guard
  (it will stand in room 3 next time). Otherwise like a guard:
- on_guard_killed, guards_tile[drawn_room-1] = 0xFF, direction = 0x56, draw_guard_hp(0, hp), guardhp_curr = 0.

clear_char `8388`: direction = 0x56, alive = 0, action = 0, draw_guard_hp(0, hp), guardhp_curr = 0.

---

## 3. Guard AI: autocontrol_opponent `0000:3E6A` (VERIFIED)

Every opponent tick, before control(). It first clears all control flags (move_0_nothing `3DFA`), so the flags only hold for this
tick. Then (charid 0 means the kid in the demo, section 9):
- decrement (when > 0) **justblocked `[350A]`**, **kid_sword_strike `[40D8]`**, **guard_refrac `[436E]`**;
- charid 24 -> mouse `3EDF`; 4 -> skeleton `3F44` (sword = 2, then autocontrol_guard); 1 -> shadow `3F11` (per level, 7.1);
  level 13 -> Jaffar `3F4E` (= autocontrol_guard); else **autocontrol_guard `3F58`**: sword < 2 -> inactive `3F6A`, else active `3FD0`.

Move helpers (they write the same control flags the kid uses; inside control() forward/backward are relative to facing):

| addr | name | flags set |
|---|---|---|
| `3DFA` | move_0_nothing | all 8 flags = 0 |
| `3E15` | move_1_forward | control_x = -1, forward = -1 |
| `3E1E` | move_2_backward | backward = -1, control_x = +1 |
| `3E29` | move_3_up | control_y = -1, up = -1 |
| `3E32` | move_4_down | down = -1, control_y = +1 |
| `3E3D` | move_up_back | up = -1 + move_2 |
| `3E47` | move_down_back | down = -1 + move_2 |
| `3E51` | move_down_forw | down = -1 + move_1 |
| `3E5B` | move_6_shift | shift = -1, shift2 = -1 |
| `3E64` | move_7 | shift = 0 |

### 3.1 char_opp_dist `0000:85A6`
Opp.room != Char.room -> **999**. Else `d = Opp.x - Char.x`, negated if Char faces left; if d >= 0 and the two face different
directions, d += 13. Positive = the opponent is in front.

### 3.2 Inactive guard (sword sheathed), `3F6A`
Nothing if the kid is dead (Kid.alive >= 0). `d = char_opp_dist()`.
- If Opp.row == Char.row and d is in -8..-1: go to the "see" test.
- Else, if is_guard_notice `[43AE]` (set by the kid's noises, op F2 and bumps/landings/buttons): clear it; d >= 0 -> "see" test;
  d < -4 -> **move_4_down** (turn around); else nothing. Without a notice: d < 0 -> nothing, else "see" test.
- "see" test: can_guard_see_kid `[5A8A]` != 0 and (level != 13 or guard_notice_timer `[3D72]` == 0) -> **move_down_forw** (draw).

control_guard_inactive `857B`: on frame 166 with down held: forward held -> **draw_sword** (guard: sword = 2, **seq 90** = the en-garde loop);
else control_down = 1, **seq 80** (act5, flip, dx +18, then seq 87 = frame 166). (The kid-side control_standing also draws for a
non-kid with down + forward held, `61E3`.)

### 3.1b can_guard_see_kid, check_can_guard_see_kid `0000:51D1` (once per tick, before the kid)
0 unless: Guard.charid != 24; (charid != 1 or level 12); Kid.frame != 0 and not 219..228 (climbing the exit stairs); a guard exists;
both alive; same room and same row. Then 2, and with `L = x_bump[Kid.col+5] + 7`, `R = x_bump[Guard.col+5] + 7` (swapped so L <= R):
if the tile at L (kid's room/row, get_tile_div_mod_m7) is a chomper, L += 14; if the tile at R is a gate, R -= 14. For each column
from L to R (step 14): **wall 20, doortop 7 or 12 -> 0 (stop)**; loose floor 11, chomper 18, a gate with modifier < 112, or any non-floor
tile -> 1 (seen but not reachable). Values: 0 not seen, 1 seen but blocked, 2 seen and reachable.

### 3.3 Active guard (sword drawn), `3FD0`
Nothing if the guard's frame is 166 or < 150, or can_guard_see_kid == 1.
- can_guard_see_kid == 0: droppedout `[3D74]` -> guard_follows_kid_down `40EC`; else (not a skeleton) **move_down_back** (put the sword away).
- can_guard_see_kid == 2: `d = char_opp_dist()`, of = Opp.frame.
  1. d >= 12 and of in 102..117 and Opp.action == 5: nothing (INFERRED: the kid is bumped/landing).
  2. d < 35: if (Char.sword < 2 and d < 8) or d < 12: same facing -> **move_2_backward**, else **move_1_forward**
     (INFERRED: close the gap / turn round). Else **autocontrol_guard_kid_in_sight(d)** `4187`.
  3. d >= 35: only if guard_refrac == 0: if the two face each other and of in 7..14 (running) with d < 40, or of in 34..43 (running jump) with
     d < 50: **move_6_shift** (strike); if facing and of is in one of those ranges but farther: nothing. Otherwise
     **autocontrol_guard_kid_far** `40BF`: tile in front or two in front is floor -> move_1_forward, else move_2_backward.
- kid_in_sight `4187`: Opp.sword == 2 -> **kid_armed(d)** `41B5`; else if guard_refrac == 0: d < 29 -> move_6_shift, else move_1_forward.
- **kid_armed** `41B5`: d < 10 or d >= 29 -> **guard_advance**. Else **guard_block**, then if guard_refrac == 0: d < 12 or d >= 29 ->
  guard_advance, else **guard_strike**.
- guard_follows_kid_down `40EC`: nothing if Opp.action is 2 or 6 (hanging). Tile in front is wall-like, or it is not floor and (the
  tile below it is spikes, or it is loose floor, wall-like or still not floor, or Char.row + 1 != Opp.row) -> droppedout = 0,
  move_2_backward. Otherwise move_1_forward (walks off the edge after the kid).

### 3.4 The skill tables (VERIFIED, word per skill 0..11; test is `table[skill] > prandom(255)`, so P = value/256)

| ds | name | used in | skill 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0E46 | strikeprob | guard_strike | 75 | 100 | 75 | 75 | 75 | 50 | 100 | 220 | 0 | 60 | 40 | 60 |
| 0E5E | restrikeprob | guard_strike (own frame 150/161) | 0 | 0 | 0 | 5 | 5 | 175 | 20 | 10 | 0 | 255 | 255 | 150 |
| 0E76 | blockprob | guard_block | 0 | 150 | 150 | 200 | 200 | 255 | 200 | 250 | 0 | 255 | 255 | 255 |
| 0E8E | impblockprob | guard_block when justblocked | 0 | 75 | 75 | 100 | 100 | 145 | 100 | 250 | 0 | 145 | 255 | 175 |
| 0EA6 | advprob | guard_advance | 255 | 200 | 200 | 200 | 255 | 255 | 200 | 0 | 0 | 255 | 100 | 100 |
| 0EBE | refractimer | guard_refrac after a hit (5.3) | 20 | 20 | 20 | 20 | 10 | 10 | 10 | 10 | 0 | 10 | 0 | 0 |
| 0ED6 | extrastrength | extra HP (2.2) | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

prandom = `0C83:0412` (tiles.md 10): `seed = seed*0x343FD + 0x269EC3`, returns `(seed >> 16) % 256` here.
- **guard_advance `41E9`**: if skill == 0 or kid_sword_strike == 0: one prandom; advprob > r -> move_1_forward.
- **guard_block `4211`**: only if Opp.frame is 152, 153 or 162 (kid's strike frames): one prandom; justblocked ? impblockprob : blockprob
  > r -> move_3_up (parry, 4.4).
- **guard_strike `4267`**: nothing if Opp.frame is 169 or 151. One prandom; own frame 161 or 150 (just parried) ? restrikeprob :
  strikeprob > r -> move_6_shift (strike, 4.3).
- So per tick a fighting guard makes 0, 1 or 2 prandom calls, in the order block, then advance or strike.
- Skill 8 never advances, blocks or strikes (all 0). Skills 10 and 11 are used for the kid in the demo (section 9).

Timers (all decremented once per opponent tick in 3E85..3EA6, while > 0):
- **kid_sword_strike `[40D8]`** = 15 when the kid presses strike (`6B82`); while > 0, guards of skill >= 1 do not advance.
- **justblocked `[350A]`** = 4 when the guard's strike was parried (`44DD`).
- **guard_refrac `[436E]`** = refractimer[skill] after the guard is hurt (`43EF`), = 9 when the kid sheathes in front of it (`6BC8`).
  While > 0: no advance/strike in kid_armed, no shift/forward in kid_in_sight, nothing in the far case.
- **guard_notice_timer `[3D72]`** = 28 when meeting Jaffar (7.4); Jaffar does not draw before it is 0.

---

## 4. Sword control (kid and guards share it)

### 4.1 Drawing, draw_sword `0000:6A89` (VERIFIED)
shift2 = forward = release_arrows(). Kid: **sound 19**, offguard = 0, **seq 55**. Shadow: seq 55. Other opponents: **seq 90**.
sword = 2. Seq 55 = act1, dx+2 f207, f208, dx+2 f209, dx+2 f210, dx+3, then the en-garde loop `ds:19E6`: act1, F2 0 (notice), f158, f170,
then f171 forever. Seq 89 (turn and draw, from back_pressed) = act7, flip, dx+6 f45, dx+1 f46, then seq 55.

Kid, control_standing `61FF` (have_sword `[34E2]` != 0): if offguard and Shift not held -> normal controls without the sword test.
Else if can_guard_see_kid >= 2 and d = char_opp_dist() in -10..89: holding_sword `[5A8C]` = 1; d in -6..-1 -> **back_pressed**
(turn and draw, seq 89); d in -10..-7 or 0..89 -> **draw_sword**, except against the shadow while it is in midair (action 3) or
landing (frames 107..117): offguard = 0 and normal controls. If can_guard_see_kid < 2: offguard = 0.
So the kid **draws automatically** when a reachable guard is within 90 in front (or 10 behind), unless he sheathed in front of it
(offguard) and Shift is not held.

### 4.2 control_with_sword `0000:6AD3` (VERIFIED)
Only when action < 2. If the tile at the character is loose floor or can_guard_see_kid >= 2: `d = char_opp_dist()`;
0 <= d < 90 -> **swordfight**; d < -4 -> **seq 60** (turn with sword: act5, flip, dx+5, then seq 57); -4 <= d < 0 -> swordfight.
Otherwise (no opponent in view, or d >= 90 incl. 999): kid alive -> holding_sword = 0; guards (charid >= 2) -> swordfight;
kid/shadow on frame 171 -> sword = 0, **seq 92** (put the sword away; the kid sheathes by himself when no guard is in view).

### 4.3 swordfight `0000:6B47` (VERIFIED)
1. Frame 161 (parried, see 5.1) and shift2 not held -> **seq 57** (step back) and return.
2. shift2 held: kid -> kid_sword_strike = 15; **sword_strike `6C1C`**: own frame 157, 158, 170, 171 or 165 -> seq 75 (kid) / **seq 58** (others);
   frame 150 or 161 -> **seq 66** (riposte after a parry); else nothing. If it started: shift2 = 1 and return.
3. down held and frame 158/170/171: down = 1, sword = 0; kid: **offguard = 1, guard_refrac = 9, holding_sword = 0, seq 93** (fast sheathe);
   shadow: seq 92; others: **seq 87** (frame 166).
4. Else up held -> **parry `6C7B`**; else forward -> **forward_with_sword `6A4F`**; else backward -> **back_with_sword `6A21`**.

- forward_with_sword: frame 158/170/171 -> forward = 1, **seq 86** (kid) / **seq 56** (others).
- back_with_sword: frame 158/170/171 -> backward = 1, **seq 57**.

### 4.4 parry `0000:6C7B` (VERIFIED)
Default seq 62. If own frame is 158, 170, 171, 168 or 165:
- not the kid and char_opp_dist() >= 32 -> back_with_sword instead.
- kid: Opp.frame 168 -> nothing; Opp.frame 151, 152, 162 -> parry; 153 -> parry **and play_seq immediately** (the kid advances two frames
  this tick); any other -> parry.
- guard: parry only when Opp.frame == 152.
Own frame 167 (blocked) -> **seq 61**. Other frames: nothing. On a parry: up = 1, seqtbl(seq).

### 4.5 The sword sequences (VERIFIED bytes; frame roles from the code that tests them)

| seq | used for | contents (dx before the frame) | then |
|---|---|---|---|
| 55 | draw | act1 +2 f207 f208 +2 f209 +2 f210 +3 | en garde `19E6` |
| `19E6` | en garde | act1 snd0 f158 f170 f171 | f171 loop |
| 56 | guard advance | act1 fall_x=1, +2 f163, +4 f164, f165 | en garde |
| 86 | kid advance | act1 fall_x=1, +6 f164, f165 | en garde |
| 57 | retreat | act1 fall_x=-1, -3 f160, -2 f157 | en garde |
| 75 | kid strike | act1 f151, f152, f153, f154, act5 f155, act1 f156, f157 | en garde |
| 58 | guard strike | act1 fall_x=-1, f168, then seq 75 from f151 | en garde |
| 66 | riposte | f162, f152, f153, f154, act5 f155, act1 f156 f157 | en garde |
| 62 | parry | f169, f150 | en garde |
| 61 | parry after being blocked | f159, f160, f150 | en garde |
| 69 | attack was parried | act1 f167, act5 f155, act1 f156 f157 | en garde |
| 74 | hit by sword (survives) | act5 fall_x=-1, f172, -1 dy+1 f173, -1 f174, -1 dy+2, -2 dy+1, -5 dy-4, act1 f156 f157 | en garde |
| 60 | turn with sword | act5 flip +5 | seq 57 |
| 63 | skeleton/stand active | act1 knock-down op | en garde |
| 64 / 65 | bumped with sword (mechanics 9.3) | act5 -8 / act5 f160 f157 | en garde |
| 76 | (not referenced here) | act1 fall_x=-1 f155 f156 f157 f158 | seq 57 |
| 77 / 87 | guard inactive | (87: act1) f166 | loop |
| 80 | inactive guard turns | act5 flip +18 | seq 87 |
| 90 | guard draws | | en garde |
| 92 | kid sheathes | act1 -5 f233..f240 f133 f133 f134 f134 f134 f48 +1 f49 -2 act5 f50 act1 f51 f52 | seq 2 |
| 93 | kid sheathes fast | act1 -5 f234 f236 f238 f240 f134 -1 | seq 2 |
| 85 | stabbed to death | act5 | seq 71 |
| 71 | dying | act1 F6 f179 f180 f181 f182 +1 f183 -4 f185 | f185 loop |
| 81 / 82 / 83 | pushed off a ledge (kid / guard back / guard forward) | act3 falls, fall_x 0/0/1, fall_y 15 | seq 12 (freefall) |
| 88 | skeleton wakes | act5 +10 f177 f177 -7 dy-2 f178 +5 dy+2 f166 -1 | en garde |

Frame roles (INFERRED names, VERIFIED tests): 150 parry pose (can parry), 151-154 strike (153 = the parryable instant, **154 = the hit
frame**), 155-157 recover, 158/170/171 en garde, 161 "was parried" (set from outside), 162 riposte, 163-165 advance, 166 inactive
(sheathed), 167 blocked (sound 10), 168 guard wind-up, 169 parry start, 172-174 hit, 177 spiked / skeleton heap, 178 chomped,
179-185 dying, 185 dead.

The en-garde frames 158/170/171 are the only ones from which advance, retreat, sheathe and (with 157/165) strike start. Movement per
action: advance +6 (kid +6), retreat -5, hit -10. fall_x set by these seqs is what start_fall reads when a fighter steps off a ledge.

---

## 5. Hits, parries and damage

### 5.1 check_sword_hurting `0000:440E` / check_hurting `444A` (VERIFIED)
Skipped when Kid.frame is 0 or 219..228. Runs check_hurting twice: first with Char = Guard, Opp = Kid (loadshad_and_opp ...
saveshad_and_opp), then Char = Kid, Opp = Guard (loadkid_and_opp ... savekid_and_opp).

check_hurting (Char = attacker): needs sword == 2, Char.row == Opp.row, own frame 153 or 154. `d = char_opp_dist()`.
- If 0 <= d < 29 and Opp.frame is 161 or 150 (parrying): **parried**: Opp.frame = 161; if the attacker is not the kid, justblocked = 4;
  attacker **seq 69** and play_seq now.
- Else, if own frame == 154: min = (Opp.sword < 2 ? 8 : 12); min <= d < 29 -> **Opp.action = 99** (marked as hurt).
- If own frame 154, Opp.frame != 161 and Opp.action != 99: **sound 11** (sword swish, a miss).

Worked timing (derived from the order kid tick -> guard tick -> check_hurting; the kid's control reads the guard frame of the
previous tick): a guard strike shows f168, f151, f152, f153, f154 on ticks T0..T4.
- Kid presses up at T2 (sees 151): kid f169 at T2, f150 at T3 = guard f153 -> parried at T3.
- At T3 (sees 152): kid f150 at T4 = guard f154 -> parried. At T4 (sees 153): the extra play_seq shows f169 and then f150 in the same
  tick -> parried. At T1 (sees 168): nothing happens. Earlier (any other guard frame): the parry is over before T3/T4 -> hit at T4.
- A guard parries only when, in its tick, the kid shows f152 (guard_block passes on 152/153/162 but parry needs 152): guard f169 with
  the kid's f152, guard f150 with the kid's f153 -> parried.

### 5.2 check_sword_hurt `0000:43C8` (VERIFIED)
If Guard.action == 99: if Kid.action == 99 too, Kid.action = 1 (**the kid wins a double hit**); loadshad, hurt_by_sword, saveshad,
**guard_refrac = refractimer[skill]**. Else if Kid.action == 99: loadkid, hurt_by_sword, savekid.

### 5.3 hurt_by_sword `0000:42C1` (VERIFIED)
Nothing if dead. Then:
- **sword != 2** (not en garde, e.g. a sheathed guard or a kid without the sword): **take_hp(100)** (dead) -> death placement below.
- **sword == 2**: skeleton: no damage; others: take_hp(1), and if that kills -> death placement. Survivors: **seq 74**, y = y_land[row+1],
  fall_y = 0.
- Death placement: if the tile behind is not empty (tile != 0) or distance_to_edge_weight < 4: **seq 85** (stabbed, then dying), and for a
  non-kid facing left on or next to a gate: x = x_bump[tile_col - (curr_tile2 != gate) + 5] + 7, then x = forward(10); y = y_land[row+1],
  fall_y = 0. Else (empty tile behind and >= 4 from the edge): x = forward(d - 20), row += 1, **seq 81** (pushed off the ledge, falls).
- **Sound 13** if the kid was hurt, **12** for anyone else; then play_seq.

### 5.4 bump_into_opponent `0000:5122` (VERIFIED, kid tick)
can_guard_see_kid >= 2, Char.sword == 0, Opp.sword != 0, Opp.action < 2, facing each other, |char_opp_dist| <= 15:
y = y_land[row+1], fall_y = 0, **seq 47** (bump back), play_seq. (A kid without the sword cannot walk through an armed guard.)

### 5.5 check_guard_bumped `0000:5C6B` (VERIFIED, guard tick)
Only if `[4CAE]` == 0 (cleared at level start, never set elsewhere), action 1, alive, sword == 2, and the tile at the guard is a wall,
doortop 7 or a bumpable gate, or (facing right) the tile one column left is 7 or a bumpable gate: load_frame_to_obj,
set_char_collision; if is_obstacle: `dx = dist_from_wall_behind(tile)` (`5E23`); if -13 < dx < 0: x = forward(-dx), **seq 65**, play_seq.
check_chomped_guard `5D31`: tile at the guard or the next column is a chomper with (modifier & 0x7F) == 2 and the box overlaps -> chomped `5B67`.

### 5.6 Guard death and level state (VERIFIED)
- HP reaches 0 via do_delta_hp; on the **next** tick play_guard sets alive = 0 and calls **on_guard_killed `833C`**:
  - level 0 (demo): checkpoint `[4CAC]` = 1, demo_time/index = 0 (the demo kid then just runs, section 9).
  - level 13 (Jaffar): flash white (`[4ED2]` = 15) for 18 ticks (`[438E]`), is_show_time `[435C]` = 1, **leveldoor_open `[40DC]` = 2**,
    **sound 43**.
  - other levels, not the shadow: **sound 37**.
- The body plays seq 85 -> 71 and stays on frame 185. Nothing else changes (no gates); a dead body on a button keeps it pressed
  (tiles.md 4.3). Re-entering the room shows it dead (2.4).
- check_killed_shadow `84B1` (both ticks, level 12): Char or Opp is the shadow (charid OR == 1), Char alive and Opp dead -> flash white
  5 ticks and **take_hp(100)**: killing the shadow kills the kid (and vice versa).

---

## 6. Hit points (VERIFIED)

| var | meaning |
|---|---|
| `[4CD6]` hitp_curr, `[6130]` hitp_max, `[612E]` hitp_delta | kid |
| `[5AA8]` guardhp_curr, `[4C96]` guardhp_max, `[4C76]` guardhp_delta | guard |
| `[4646]` hitp_beg_lev | HP at level start |

- **take_hp(n) `0000:73F9`**: kid (Char.charid == 0): n >= hitp_curr -> delta = -hitp_curr, returns 1 (dead); else delta = -n, returns 0.
  Others: the same on guardhp. It **sets** the delta (does not add).
- **do_delta_hp `12A6`**: level 12 and Opp.charid == 1 and guardhp_delta != 0 -> hitp_delta = guardhp_delta (damage to the shadow also
  hurts the kid; at this point Opp = Guard after check_sword_hurting). Then hitp_curr = clamp(hitp_curr + hitp_delta, 0, hitp_max) and
  guardhp_curr = clamp(guardhp_curr + guardhp_delta, 0, guardhp_max).
- Death: control_kid `7A1F`: alive < 0 and hitp_curr == 0 -> alive = 0. (Guard: play_guard, 1.)
- **add_life `1217`** (life potion, uniting with the shadow): hitp_max = min(hitp_max + 1, **10**), hitp_delta = hitp_max - hitp_curr (full heal).
- Level start (do_startpos `4A92`): hitp_curr = hitp_max = hitp_beg_lev (level 0: 4). New game (`4893`): hitp_beg_lev = **3**,
  rem_min = 60, rem_tick = 719. Level completed (`4DD7`) or level-3 checkpoint (`3D6D`): hitp_beg_lev = hitp_max.
- Jumping through the level-4 mirror (`5082`): a mirrored copy of the kid (check_mirror_image `50D3`: x = 2*(x_bump[col+5]+10) - x,
  direction flipped) is saved as the shadow (charid 1); guardhp_max = guardhp_curr = hitp_max, **hitp_curr = 1**, sound 45.
- Potions and the sword pick-up: tiles.md 8.2/8.3.

Display (data only; drawn by `1236` each frame):
- Kid, draw_kid_hp(curr, max) `1115`: slots i in curr..max-1 empty, i < curr full; **x = 7*i, y = 194** (bottom left). Images from the
  table at `[437A]` (+366 full, +36A empty).
- Guard, draw_guard_hp(curr, max) `1184`: same, **x = 314 - 7*i**, y = 194 (bottom right), images at `[4380]`. Not drawn for the
  skeleton, the mouse, or the shadow except on level 12.
- Kid on 1 HP (not level 15) and guard on 1 HP blink: full or empty first slot by rem_tick `[50C0]` bit 0.
- Flash: flash_if_hurt `5374`: flash_time `[438E]` > 0 -> flash with `[4ED2]`; else hitp_delta < 0 -> **red (12)** for that frame
  (`2E60`, removed by `5392`). Colours used: 15 white (Jaffar 18, unite 18, shadow killed 5), 14 yellow (sword 8), 4 (potions 2/4).

---

## 7. Special characters and per-level events

### 7.1 The shadow (charid 1)
do_init_shad(src, seq) `36F4`: Char = 7 bytes of src (frame, x, y, dir, col, row, action), seqtbl(seq), charid = 1,
demo_time `[4E12]` = 0, guard_skill = 3, guardhp_* = 4, saveshad. Tables (VERIFIED `ds:0EFE`):

| table | frame | x | y | dir | col | row | seq |
|---|---|---|---|---|---|---|---|
| init_shad_6 `0EFE` | 15 | 81 | 118 | 0 (right) | 0 | 1 | 2 (stand) |
| init_shad_5 `0F06` | 15 | 55 | 55 | 0 | -1 | 0 | 2 |
| init_shad_12 `0F0E` | 15 | 81 | 232 (above the room) | 0 | 0 | 0 | 7 (fall) |

Per level (autocontrol_shadow `3F11`):
- **Level 4** `46F4` (created by the mirror jump `5082`, 9.2 of mechanics): in room 4: x < 80 -> clear_char, else move_1_forward.
- **Level 5** `470E` (check_shadow: room 24 entered while the potion (24, col 3, row 0) is still there): in room 24, while demo_time == 0
  it waits until the gate (24, col 1, row 0) modifier >= 80, then plays the move list `ds:0F16` with do_auto_moves `4633`
  (time, move): (0 forward) (14 nothing) (18 shift) (29 shift off) (45 back) (49 forward) (255 end); x < 15 -> clear_char.
  do_auto_moves: demo_time += 1 (max 254), moves 0..7 = nothing, forward, back, up, down, up+forward, shift, shift off.
- **Level 6** `4758` (check_shadow: room 1, sets leveldoor_open = 0x4D with **sound 25** once): in room 1, when Kid.frame == 43 (running
  jump) and Kid.x < 128: move_6_shift + move_1_forward. (The shadow is not seen by guards: can_guard_see_kid needs level 12.)
- **Level 12** `4776` (check_shadow: room 15, not yet united, sword tile (15, 1, 0) already taken):
  in room 15 while shadow_initialized `[4330]` == 0: Opp.x >= 150 -> do_init_shad(init_shad_12, 7) again (it keeps re-dropping until the
  kid is left of x 150), else shadow_initialized = 1. Then: shadow sword drawn -> (offguard == 0 or guard_refrac == 0) ?
  autocontrol_guard_active : move_4_down. Shadow sword sheathed: if Opp.sword >= 2 or offguard == 0: (can_guard_see_kid < 2 or
  d >= 90) -> (d < 0 ? move_2_backward : nothing), else on frame 15 move_down_forw (draws). Else (kid sheathed in front of it):
  **d < 10 -> unite**: flash white 18, add_life, united_with_shadow `[4FCE]` = 42, Char.charid = 0, savekid, clear_char;
  else if can_guard_see_kid == 2 and Opp.frame in 3..14 or 127..132 (running or stepping): move_1_forward.
  Notes: d is uninitialised when can_guard_see_kid < 2 (original bug, INFERRED harmless); united_with_shadow counts down in timers
  `4FA9` to 0 and then to -1 (the -1 state enables the level-12 hidden floor, tiles.md 8.5). Leaving room 23 on level 12 = next level
  (`09AA`). Shadow damage is mirrored onto the kid (6) and killing it kills the kid (5.6).

### 7.2 Skeleton (level 3), check_skel `0000:4513` (VERIFIED)
Each tick: level 3, no guard present, drawn_room 1, leveldoor_open != 0, Kid.curr_col 2 or 3, tile (room 1, col 5, row 1) == 21:
tile -> 1 (floor), redraw; Char: room 1, row 1, y = 118, col 5, **x = 142** (x_bump[10] + 14), facing left, **seq 88** (wake up), play_seq,
**sound 44**, guard_skill = 2, alive = -1, guardhp_curr = guardhp_max = 3, fall 0, guard_refrac = 0, is_guard_notice = 0, sword = 2,
charid = 4, saveshad. The skeleton takes no sword damage (5.3) and never sheathes (3.3); it can only be pushed off: falling out into
room 3 parks it there standing (2.5), elsewhere it dies. Any guard entering on level 3 is a skeleton (2.3).

### 7.3 Mouse (level 8), do_mouse `0000:5332`
timers `4FEA`: level 8, drawn_room 16, leveldoor_open != 0: leveldoor_open += 1 each tick; at **150**: Char = Kid, charid = 24, x = 200,
row 0, y = 55, alive = -1, facing left, guardhp_curr = 1, **seq 105**, play_seq, saveshad. autocontrol_mouse `3EDF`: action 0 and
x >= 200 -> clear_char; action != 0 and x < 166 -> **seq 107** (turns back). (INFERRED: the mouse runs in, stops, and returns.)

### 7.4 Jaffar (level 13) and the exit door
- Kid starts with seq 84 (run). Leaving room 3 to the right with leveldoor_open == 0 (meet_Jaffar `3DA2`): **sound 29**,
  guard_notice_timer = 28.
- Jaffar is guard charid 2 with skill 9 (6 HP), vizier images; same AI except the draw delay (3.2). The timer stops after he dies
  (show_time `C189`).
- on_guard_killed sets **leveldoor_open = 2** (5.6). Jaffar_exit `3D37`: on leaving any room to the left while leveldoor_open == 2:
  get_tile(24, 0, 0), trigger_button(0, 0, -1) (opens the exit door through room 24's button; tiles.md 8.1: when the door finishes
  opening and leveldoor_open is 0 or 2 it becomes 1).
- check_fall_flo `1034`: entering rooms 23 or 16 on level 13 makes floors (tilepos 22..27 of room `[4CCE]`) start falling with a random delay (tiles.md 5).

### 7.5 Event table

| level | trigger | code | effect |
|---|---|---|---|
| 0 | demo, Kid.room == 24 | `095B` | end of the demo |
| 1 | level start | mechanics 8 | seq 7 fall, button (5, 2, 0) |
| 3 | kid near the skeleton | `4513` | skeleton wakes (7.2) |
| 3 | leave room 7 to the left | `3D59` | checkpoint `[4CAC]` = 1, hitp_beg_lev = hitp_max |
| 3 | restart with checkpoint | mechanics 8 | start room 2 |
| 4 | exit door opened | tiles 8.1 | mirror tile in room 4 |
| 4 | leave room 11 to the left, row 0, door open | `3DC7` | sound 25, leveldoor_open = 0x4D |
| 4 | jump through the mirror | `5010`/`5082` | shadow created, kid HP = 1 |
| 5 | enter room 24, potion there | `37D7` | shadow drinks the potion (7.1) |
| 6 | enter room 1 | `37A7` | shadow (7.1), sound 25 once |
| 6 | fall out of room 1 | `3D22`, `0981` | next level (Kid.y = -1) |
| 7 | level start in room 17 | mechanics 8 | kid starts above, goto_other_room(down) |
| 8 | room 16, exit door open | `4FEA` | mouse after 150 ticks (7.3) |
| 12 | enter room 15 | `3765` | shadow fight (7.1) |
| 12 | leave room 18 to the right | `3D74` | sword tile (15, 1, 0) -> floor |
| 12 | Kid.room == 23 | `09A3` | next level, seamless `[59C8]` = 1 |
| 13 | leave room 3 to the right | `3DA2` | Jaffar music 29, notice timer 28 |
| 13 | Jaffar killed | `8353` | leveldoor_open = 2, sound 43 |
| 13 | leave a room to the left after that | `3D37` | button in room 24 -> exit door |
| 14 | enter room 5 | `100A` | ending (`2F2F`, not traced) |

---

## 8. Death and level flow (VERIFIED unless marked)

- fell_out `7900` (start of play_kid): alive and Kid.room == 0 -> take_hp(100), alive = 0, frame = 185.
- play_kid `792A` after control_kid, once dead (alive >= 0) and is_dead (`82ED`: frame 177, 178 or 185): resurrect cheat `[3D76]` aside,
  it waits while a sound plays, then counts alive up by 1 per tick: at **6** death music (**sound 32** shadow opponent, **28** if
  holding_sword, else **24**; `8309`, not on level 0/15 and only with sound on); at **7** (when silent): rem_min == 0 -> expired;
  else "Press Button to Continue" text for 288 ticks (level 0/15: 36).
- Restart: key handler `0554`: rem_min != 0, Kid.alive > 6 and (Shift or Enter) -> the same action as Ctrl+A (`05D2`, not on level 15):
  stop sounds, restart flag `[4310]` = 1; play_level_2 `4D90` then returns current_level and play_level `48C0` restarts it
  (do_startpos with hitp_beg_lev).
- Next level: op F1 (end of seq 70, the exit stairs) or the level 6/12 cases increment next_level `[3D58]`; play_level_2 returns it when
  it differs from current_level (`4DA2`), after hitp_beg_lev = hitp_max and checkpoint = 0.
- **Time** (show_time `C170`): only while the kid is alive, rem_min `[50BA]` != 0, level < 13 (level 13 until Jaffar dies), not 15:
  rem_tick `[50C0]` -= 1; at 0: rem_tick = 719, rem_min -= 1 (**720 ticks per minute**; INFERRED: 60 s at 12 ticks/s, which agrees with
  the 12 Hz assumption of mechanics.md 6), show the time when rem_min < 5 or rem_min % 5 == 0; in the last minute every 12 ticks.
  rem_min == 0 -> expired `31F4` (game over, not traced).
- timers `4FA9` (each frame): united_with_shadow, guard_notice_timer, resurrect_time count down; feather fall ends when its sound ends;
  the level-8 mouse counter.

---

## 9. Demo (level 0) (VERIFIED)
control_kid on level 0 calls `7A91`: after the demo guard is killed (checkpoint) the kid runs forward; while the kid's sword is drawn,
guard_skill = 10, **autocontrol_opponent for the kid** (the guard AI drives the kid, charid 0 skips the timers), then guard_skill = 11;
otherwise the move list `ds:0F36` via do_auto_moves.

---

## 10. Sound ids (combat and characters)

| id | event | where |
|---|---|---|
| 10 | swords clash (frame 167) | `1394` |
| 11 | sword swish, a miss | `4506` |
| 12 / 13 | guard / kid hurt by the sword | `43B6` / `43B1` |
| 19 | kid draws the sword | `6AA6` |
| 24 / 28 / 32 | death music: normal / holding the sword / shadow | `832B` / `8324` / `8316` |
| 25 | level 6 shadow, level 4 mirror music | `37BF`, `3DEA` |
| 29 | meet Jaffar | `3DB7` |
| 37 | guard killed (also sword picked up) | `837E` |
| 43 | Jaffar killed | `8372` |
| 44 | skeleton wakes | `45F6` |
| 45 | jump through the mirror | `509B` |
| F2 0 | en-garde loop: only sets is_guard_notice | seq `19E6` |

## 11. Open / not verified
- The timer interrupt that decrements `[336E]` (frame length 5 or 6 units) and its rate.
- `0000:8422` (draws a hurt splash next to a character; VERIFIED offsets only: kid chtab 2 image 218, guard chtab 5 image 1) - when it
  is called was not traced. The palette call `9519` argument meaning. draw images in 6.
- The ending (`2F2F`), expired (`31F4`), the level-14 guard entries, the cutscene triggers (princess/vizier, PV.DAT) beyond the entry
  points listed here.
- The exact behaviour of a shadow left behind on a room change (leave_guard skips charid 1, so its record stays active, INFERRED).
