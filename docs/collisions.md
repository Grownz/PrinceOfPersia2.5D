# Kid collision pipeline, instruction-exact (PRINCE.EXE v1.1)

Source: disassembly of `Original_data/PRINCE.EXE` with `tools/disasm` (`pop-disasm dis 0000:XXXX`).
Conventions as in `docs/mechanics.md`: `0000:XXXX` = code segment 0000, `[XXXX]` = data-segment word/byte,
Pascal calling convention (arguments pushed left to right, so the **first** argument is at the highest `[bp+N]`).
Every statement below was read from the instructions at the address given (**VERIFIED**) unless it is in section 8
("Unresolved"). Names follow SDLPoP where the code matches.

Arithmetic notation: `u8()` = wrap to a byte, `s8()` = sign-extend a byte, `/` on signed values = truncation toward 0
(8086 `cwd; sub ax,dx; sar ax,1` and `idiv`), `cmp ... jl/jg` = signed 16-bit compare, `jb/jae` = unsigned.

---

## 1. One kid tick, `play_kid_frame` 0000:0ED4

### 1.1 Exact call order (0000:0ED4..0F72)

| # | addr | call | what it does to collision state |
|---|---|---|---|
| 1 | 0ED5 | `71DF` loadkid_and_opp | Char = Kid (memcpy 16 bytes `ds:4396 -> ds:3D62`), Opp = Guard |
| 2 | 0EDA | `6E80` load_fram_det_col | `rec = frame[Char.frame]` (copied to `ds:66CA..66CE`), `curr_col = col_m7(dx_weight())` -- **the frame shown last tick** |
| 3 | 0EDF | `84B1` | (shadow/killed check, no collision state) |
| 4 | 0EE4 | `792A` play_kid -> control_kid -> control() | input. get_edge_distance (5A24) **recomputes** curr_col, obj_*, and the whole box (set_char_collision) for the *current* (pre-play_seq) frame and x |
| - | 0F01/0F0D | `if [4310] != 0 return 1`; `if Char.room == 0 goto 0F6B` (skip everything up to savekid) | |
| 5 | 0F16 | `6F90` play_seq | ops (dx/dy/flip/action/up/down...) then **Char.frame = next frame** |
| 6 | 0F1B | `72B8` fall_accel | action 4 only |
| 7 | 0F20 | `72EA` fall_speed | `y = u8(y + fall_y)` always; action 4: `x = u8(char_dx_forward(fall_x))` and load_fram_det_col |
| 8 | 0F25 | `C110` load_frame_to_obj | reloads `rec` for the **new** frame; obj_x / obj_y / obj_id / obj_chtab from the **new frame and the x after fall_speed** |
| 9 | 0F2A | `6E80` load_fram_det_col | rec (new frame) and `curr_col = col_m7(dx_weight())` |
| 10 | 0F2F | `745F` set_char_collision | box from obj_* and rec (section 2) |
| 11 | 0F34 | `5122` bump_into_opponent | guard contact: may set y, fall_y = 0, seq 47 + play_seq. **Box not recomputed** |
| 12 | 0F39 | `53AC` check_collisions | section 3 |
| 13 | 0F3E | `5645` check_bumped | section 4; called unconditionally here |
| 14 | 0F43 | `5BDB` check_gate_push | section 5.6 |
| 15 | 0F48 | `7309` check_action | check_on_floor / do_fall / check_grab (section 5) |
| 16 | 0F4D/0F52 | `7E2E` check_press, `7EE1` check_spike_below | (tiles.md; check_spike_below reads char_x_left/right) |
| 17 | 0F55..0F63 | if `[3D76]` == 0: `7394` check_spiked, `5B13` check_chomped_kid | check_chomped_kid reads the flag arrays (5.7) |
| 18 | 0F68 | `4F7A` check_knock | |
| 19 | 0F6D | `71A0` savekid | Kid = Char |

Later in the same frame (play_frame `090A`, see tiles.md 2.1): play_guard_frame (`0F73`, which calls set_char_collision
for the **guard** at `0FC1`, overwriting the box globals), sword checks, then **exit_room `3ABB`** (section 5.8), which
recomputes the kid's box (loadkid, `C110`, `745F` at `3AD6..3AE0`) before leave_room. Drawing (`BFB8` draw_kid, `BFEC`
draw_guard) calls set_char_collision again (`BFCD`, `C007`). **Consequence:** the box globals are only valid for the kid
between `0F2F` and the end of play_kid_frame, and inside get_edge_distance / exit_room which recompute them. The flag
arrays (1.3) are touched by nothing but the kid's check_collisions and the level init.

### 1.2 Box and position globals (all VERIFIED by xref)

| global | size | written by | read by (logic) |
|---|---|---|---|
| `[4642]` obj_x | word, signed | C110 | 745F |
| `[4651]` obj_y | byte | C110 | 745F |
| `[42E4]` obj_id (image index) / `[4CD8]` obj_chtab | byte | C110 | 745F |
| `[4DF6]` obj_direction | byte | C110 (= Char.direction) | C110 parity test |
| `ds:66CA..66CE` rec = {image, sword, dx, dy, flags} | 5 bytes | 6E96 load_frame (called by C110 and 6E80) | dx_weight, C110, 745F (thin bit), check_on_floor (`7552`) |
| `[605A]` char_width_half | word | 745F | 745F |
| `[4656]` char_height | word | 745F | can_bump_into_gate `5A15`, 745F |
| `[3D5C]` char_x_left | word | 745F | leave_room `3CAC/3CBB`, check_spike_below `7F04` |
| `[3D50]` char_x_right | word | 745F | leave_room `3CEC/3CF4`, check_spike_below `7EE7` |
| `[3D60]` char_x_left_coll | word | 745F | check_collisions `53F7`, get_row_collision_data `555B`, check_bumped `56A5`, dist_from_wall_forward `5E00`, guard helpers `5DA5`, `5E4E` |
| `[4336]` char_x_right_coll | word | 745F | check_collisions `53D5/53E9`, get_row_collision_data `55BB`, check_bumped `56DD`, dist_from_wall_forward `5E19`, guard helpers `5D8F`, `5E67` |
| `[4ED6]` char_top_y, `[5ADA]` char_top_row, `[34E4]` char_bottom_row, `[60C4]` char_col_left, `[43B0]` char_col_right | word | 745F | drawing only (`4DEA`, `4EB9`, `7F79`, `813B`, all called from BFB8/BFEC) |
| `[3D66]` Char.curr_col | byte | 6E89 determine_col (via 6E80, and at the start of 5A24) | get_tile_at_char `7449`, tile_front `7848`, behind `78A2`, get_edge_distance |
| `[3D67]` Char.curr_row | byte, signed | play_seq up/down ops, start_fall, do_fall (`85EF`), goto_other_room | everything |
| `[4368]` curr_room, `[4370]` tile_col, `[4374]` tile_row | word | get_tile `6D42` (normalised by find_room_of_tile `6D99`); also bumped `5853/585F` | bumped, check_bumped, check_gate_push, chomped |
| `[4339]` curr_tile2, `[432D]` curr_tilepos | byte | get_tile (tilepos only when the room is not 0) | is_obstacle, bumped, can_bump_into_gate, ... |
| `[4C4A]` coll_tile_left_x | word | get_row_collision_data `554A/558A`, is_obstacle `57A5`, dist_from_wall_forward `5DDE`, guard `5D7F` | left_wall_x `55F8`, right_wall_x `5633`, `5E04..5E63` |
| `[66CF]` edge_type | byte | get_edge_distance | forward_pressed, safe_step, jump_up |

get_tile(room, col, row) `6D42`: stores room/col/row into `[4368]/[4370]/[4374]`, then find_room_of_tile `6D99`
repeatedly: `col < 0 -> col += 10, room = link_left(room)`; `col >= 10 -> col -= 10, link_right`; `row < 0 -> row += 3,
link_up`; `row >= 3 -> row -= 3, link_down`; a room of 0 stays 0 (but col/row are still normalised). If the final room is 0:
`curr_tile2 = 20` (wall) and **curr_tilepos / curr_room_modif are not updated**. Otherwise `get_room_address(room)`,
`curr_tilepos = 10*row + col` (`ds:2288` table), `curr_tile2 = fg & 0x1F`.

### 1.3 The flag arrays (VERIFIED: 5487, 552D, 53AC, 59A9)

Eight 10-byte arrays, **indexed by the normalised tile column 0..9 (`[4370]` after get_tile), not by the raw column**:

| array | flags | rooms | content |
|---|---|---|---|
| current row (curr_row) | `ds:59CE` curr_row_coll_flags | `ds:43B2` curr_row_coll_room | this tick |
| row below (curr_row + 1) | `ds:4C8A` below_row_coll_flags | `ds:3D46` below_row_coll_room | this tick |
| row above (curr_row - 1) | `ds:4E28` above_row_coll_flags | `ds:4312` above_row_coll_room | this tick |
| previous tick, matched to curr_row | `ds:5CDE` prev_coll_flags | `ds:4C4E` prev_coll_room | copied by move_coll_to_prev |

Plus `[409A]` coll_row (byte), `[4302]` prev_coll_row (byte), `[436D]` bump_col_left_of_wall (byte, signed),
`[43AC]` bump_col_right_of_wall (byte, signed), `[40CA]` left_checked_col (byte), `[4C48]` right_checked_col (byte).

- A **room byte of -1 (0xFF) means "not filled this tick"**. The room byte is the room the tile was actually read from,
  so a raw column -1 is stored at index 9 with the **left neighbour's** room id, raw column 10 at index 0 with the
  **right neighbour's** id, raw column 11 at index 1, etc. Rows -1 / 3 likewise store the room above / below.
  If two raw columns map to the same index, the later one (higher raw column) wins.
- Room id 0 (outside the level) is a valid stored value (its tile reads as wall 20).
- Initialisation: only `0000:59A9`, called once from the level init (`0000:4951`): fills `4C4E`, `43B2`, `3D46`, `4312`
  with 0xFF (10 bytes each) and sets `[4302] = 0xFF`. **Flag arrays are never cleared** (they keep their bytes).
  **Nothing resets the arrays on a room change** (xref: no other writer).
- Writers: rooms/flags of the three row arrays: get_row_collision_data `557C/5588`; room arrays set to 0xFF:
  move_coll_to_prev `550F/5513/551A`; prev arrays: move_coll_to_prev `54F8/5504`.
- Readers of the flag/room arrays: check_collisions (`5442..5475`), is_obstacle_at_col `570D` (room), check_chomped_kid
  `5B29/5B30`, check_gate_push `5C2E/5C34`. The below/above arrays are only reached through move_coll_to_prev
  (their addresses appear only as immediates at `540A..5432` and `54AF..54E6`).

---

## 2. set_char_collision `0000:745F` and its inputs

### 2.1 load_frame_to_obj `0000:C110`
```
call 7243 (resets drawing clip vars [42E6] [4306] [4300]=320 [40C2]=192)
load_frame()                                   ; 6E96: rec = frame[Char.frame]
obj_direction = Char.direction                 ; C128
obj_id        = rec.image                      ; C12E  (0-based KID.DAT image, resource 401 + id)
obj_chtab     = 2 + (rec.sword >> 6)           ; C131
obj_x = 2 * char_dx_forward(s8(rec.dx)) - 116  ; C143..C152, 16-bit, char_dx_forward = (dir<0 ? -d : d) + Char.x (u8, no wrap)
obj_y = u8(rec.dy + Char.y)                    ; C155..C15C
if s8(rec.flags XOR obj_direction) >= 0: obj_x += 1   ; C15F..C168 (bit 7 of flags = parity, inverted when facing left)
```

### 2.2 set_char_collision `0000:745F`
```
img   = chtab_ptrs[obj_chtab]->images[obj_id]   ; 7466..7488: si = [4376 + 2*chtab], far ptr at [si + 4*id + 6]
w     = img.word1 (width, pixels)               ; 748B
half  = (w + 1) / 2                             ; 748F..7495  signed trunc; w > 0 so = floor
char_height = img.word0 (height)                ; 7498
char_x_left = obj_x / 2 + 58                    ; 749E..74A9  signed trunc toward 0 (obj_x = -3 -> -1)
if s8(Char.direction) >= 0:                     ; 74AC  facing right (dir 0)
    char_x_left -= half
char_x_left_coll  = char_x_left                 ; 74BD
char_x_right      = char_x_left + half          ; 74C0..74C4
char_x_right_coll = char_x_right                ; 74C7
char_top_y = obj_y - char_height + 1 ; if >= 192 (signed) then 0      ; 74CA..74DC
char_top_row = y_to_row_mod4(char_top_y)                             ; 74E7
char_bottom_row = y_to_row_mod4(obj_y); if -1 then 3                 ; 74F4..74FD
char_col_left  = max(div_mod(char_x_left), 0)    (no -7!)            ; 7503..751B
char_col_right = min(div_mod(char_x_right), 9)                       ; 751E..7538
if rec.flags & 0x20:                            ; 753B  thin
    char_x_left_coll  += 4
    char_x_right_coll -= 4
```
- The width is the image width **in pixels**, compared against x in logic units (an original quirk, keep it).
- Facing left: box = [obj_x/2+58, +half]; facing right: [obj_x/2+58-half, obj_x/2+58]. Only the `_coll` copies are thinned.
- rec is the record loaded by the last load_frame (in the tick: the frame after play_seq).
- `y_to_row_mod4(y)` `716F` = `((y + 60) idiv 63) idiv-remainder 4 - 1`, both signed (remainder takes the dividend's sign).
- Frame without image (image byte 255): there is **no test**; images[255] is read past the pointer table. See section 8.
- `div_mod(x)` `712C` is a raw table lookup `col = s8(ds:22BA[x])`, `tile_mod = ds:23BA[x]` with x used as a signed
  16-bit index; `col_m7(x)` `711A` = div_mod(x - 7). For 0 <= index <= 255 this equals `floor((index - 58) / 14)` and
  `(index - 58) mod 14`. Outside that range it reads neighbouring static data: index -16..-1 gives
  `66,0,-127,0,-64,0,-8,-1,55,0,118,0,-75,0,-12,0` (so `col_m7(6) = 0`, not -5), index 256.. gives the mod table
  `12,13,0,1,2,...`. A bit-exact port must embed `ds:22AA..24B9` (or at least reproduce these values).

### 2.3 Worked numbers (kid images from KID.DAT resource 401+id, (height, width))
- Frame 15 (image 14, 41x12, dx 0, flags 0x43), x = 146, facing right: obj_x = 2*146-116+1 = 177, half 6,
  left = 88+58-6 = 140, right = 146 (not thin). Facing left: flags^0xFF has bit 7 set -> obj_x = 176, left = 146, right = 152.
- Frame 5 (image 4, 39x26, flags 0xE6 = thin + parity), x = 94, right: obj_x = 72, half 13, left 81, right 94, coll 85..90.

---

## 3. check_collisions `0000:53AC`, move_coll_to_prev `5487`, get_row_collision_data `552D`

### 3.1 check_collisions
```
bump_col_right_of_wall = bump_col_left_of_wall = -1         ; 53B3..53B8 (always, also for action 7)
if Char.action == 7: return                                  ; 53BB  -> arrays, [4302] untouched this tick
coll_row = Char.curr_row                                     ; 53C5
move_coll_to_prev()                                          ; 53CC
prev_coll_row = coll_row                                     ; 53CF
c = col_m7(char_x_right_coll) + 2                            ; 53D5..53DF (16-bit)
right_checked_col = (c > 11) ? 11 : u8(c)                    ; 53E0..53F4
left_checked_col  = u8(col_m7(char_x_left_coll) - 1)         ; 53F7..5402
get_row_collision_data(curr_row,     curr_room_arr 43B2, curr_flags 59CE)   ; 5405..5413
get_row_collision_data(curr_row + 1, below_room 3D46,    below_flags 4C8A)  ; 5416..5425
get_row_collision_data(curr_row - 1, above_room 4312,    above_flags 4E28)  ; 5428..5437
for col = 9 downto 0:                                        ; 543A..5480
    if s8(curr_room_arr[col]) < 0: continue                  ; 5442
    if prev_coll_room[col] != curr_room_arr[col]: continue   ; 544B..5453 (byte compare)
    if (prev_flags[col] & 0x0F) == 0 and (curr_flags[col] & 0x0F) != 0: bump_col_left_of_wall  = col
    if (prev_flags[col] & 0xF0) == 0 and (curr_flags[col] & 0xF0) != 0: bump_col_right_of_wall = col
```
Because the loop runs 9 -> 0 and overwrites, the result is the **lowest** index with a transition. Columns are compared
**index by index with their own room id**; there is no comparison of the whole record's room.

### 3.2 move_coll_to_prev `5487`
```
r = s8(coll_row); p = s8(prev_coll_row)
if r == p or r + 3 == p or r - 3 == p:  src = curr arrays  (43B2 / 59CE)     ; 548F..54B4
elif r + 1 == p or r - 2 == p:          src = above arrays (4312 / 4E28)     ; 54BB..54DA  (kid moved up a row)
else:                                   src = below arrays (3D46 / 4C8A)     ; 54E1 (moved down, or anything else)
for i in 0..9:
    prev_coll_room[i]  = src_room[i]; prev_coll_flags[i] = src_flags[i]
    below_room[i] = above_room[i] = curr_room_arr[i] = 0xFF                   ; flags arrays NOT cleared
```
The ±3 / -2 / +2 cases are the vertical room changes (rows renumbered by goto_other_room or by do_fall's row++ to 3).

### 3.3 get_row_collision_data(row, room_arr, flags_arr) `552D` (`ret far 6`)
```
room = s8(Char.room)                                  ; 5534
col  = s8(left_checked_col)                           ; 553B
coll_tile_left_x = ds:24BF[col] (UNSIGNED byte) + 7   ; 5541..554A  x_bump[col+5]+7: col 0..9 -> 65+14col; col -1 -> 51; col 10 -> 205
while col <= s8(right_checked_col):                   ; 5592..5599
    lw = left_wall_x(room, col, row)                  ; 559B..55A8
    rw = right_wall_x(room, col, row)                 ; 55AB..55B8  (the second get_tile sets tile_col / curr_room)
    f  = (lw < char_x_right_coll ? 0x0F : 0)          ; 55BB..5558 signed 16-bit
       | (rw > char_x_left_coll  ? 0xF0 : 0)          ; 555B..556F signed 16-bit
    flags_arr[tile_col] = f                           ; 5572..557C  index = NORMALISED column
    room_arr[tile_col]  = u8(curr_room)               ; 557E..5588  room actually read (0 = outside the level)
    coll_tile_left_x += 14; col++                     ; 558A..558F
```
- `left_wall_x(room,col,row)` `55CE`: `wt = wall_type(get_tile(room,col,row))`; `wt ? s8(ds:24CE[wt]) + coll_tile_left_x : 0xFF`.
- `right_wall_x(room,col,row)` `5607`: `wt ? coll_tile_left_x - s8(ds:24D4[wt]) + 13 : 0`.
- wall_type `7D0B`: 4 gate, 7 doortop-with-floor, 12 doortop -> 1; 13 mirror -> 2; 18 chomper -> 3; 20 wall -> 4; else 0.
  `ds:24CE` wall_dist_from_left = {0,10,0,-1,0,0}, `ds:24D4` wall_dist_from_right = {0,0,10,13,0,0}.
  So per wall_type: wall [tl, tl+13], gate/doortop [tl+10, tl+13], mirror [tl, tl+3], chomper [tl-1, tl] (tl = tile left x).
- coll_tile_left_x is **not** adjusted for side rooms here: it is derived from the raw column, which is already the right
  coordinate in the kid's room (col -1 -> 51, col 10 -> 205).
- Meaning of the bits: 0x0F = "the box's right edge is past the obstacle's left face", 0xF0 = "the box's left edge is
  before the obstacle's right face". 0xFF = overlap. A non-wall tile gives lw 0xFF / rw 0, so it can still set 0x0F when
  char_x_right_coll > 255 or 0xF0 when char_x_left_coll < 0 (edge positions).
- Loop bounds: raw columns `col_m7(left_coll)-1 .. min(col_m7(right_coll)+2, 11)`, i.e. up to -2..11 at the room edges.

### 3.4 Consequences (important for the port)
1. A bump fires only on the tick on which the bit **turns on** for an index whose **room id is the same as last tick**.
   A column that enters the checked range already overlapping does not bump (its previous room byte is 0xFF).
2. Data survives room changes. After a horizontal room change the shared columns have the same (index, room) pairs, so the
   comparison continues seamlessly (test vectors J, J2: the bump happens on the first tick after the room change).
3. During action 7 (turn) nothing is recomputed or copied: next tick compares against the last non-turn tick, and during
   the turn check_gate_push / check_chomped_kid see the old arrays.

---

## 4. check_bumped and the bump handlers

### 4.1 check_bumped `0000:5645` (called every tick from 0F3E)
```
if action == 2 or action == 6: return                         ; 5645..5651
if 135 <= frame < 149: return                                 ; 5653..565F (unsigned byte compares)
if s8(bump_col_left_of_wall) >= 0:  check_bumped_look_right() ; 5661 -> 56B2   (bump_col_right is then ignored)
elif s8(bump_col_right_of_wall) >= 0: check_bumped_look_left(); 566E -> 567A
```
check_bumped_look_right `56B2` (kid left of the obstacle, moving right):
```
if Char.sword != 2 and Char.direction != 0: return            ; 56B2..56BE (direction must be exactly 0 = right)
if !is_obstacle_at_col(bump_col_left_of_wall): return         ; 56C0..56CB
delta = left_wall_x(curr_room, tile_col, tile_row) - char_x_right_coll   ; 56CD..56DD  (uses the globals just set)
bumped(delta, -1)                                             ; 56E1..56E7
```
check_bumped_look_left `567A` (kid right of the obstacle, moving left):
```
if Char.sword != 2 and s8(Char.direction) >= 0: return       ; 567A..5686
if !is_obstacle_at_col(bump_col_right_of_wall): return
delta = right_wall_x(curr_room, tile_col, tile_row) - char_x_left_coll   ; 5695..56A5
bumped(delta, 0)
```

### 4.2 is_obstacle_at_col(col) `56EB` and is_obstacle `5726`
```
row = s8(Char.curr_row); if row < 0: row += 3; if row >= 3: row -= 3     ; 56F1..5706
get_tile(s8(curr_row_coll_room[col]), col, row)                          ; 570A..5719  room = the room STORED for that index
return is_obstacle()
```
```
is_obstacle():                                                ; 5726
  t = curr_tile2
  if t == 10 (potion): return 0
  if t == 4 (gate):    if !can_bump_into_gate(): return 0
  elif t == 18 (chomper): if curr_room_modif[curr_tilepos] != 2: return 0          ; exact 2
  elif t == 13 (mirror) and charid == 0 and 39 <= frame < 44 and s8(direction) < 0:
       curr_room_modif[curr_tilepos] = 0x56; [42E0] jumped_through_mirror = -1; return 0   ; 577E..578B
  coll_tile_left_x = xpos_in_drawn_room(ds:24BF[tile_col] unsigned) + 7            ; 5793..57A5
  return 1                       ; any other tile, wall-like or not, is an obstacle
```
xpos_in_drawn_room(x) `57AD`: if curr_room != drawn_room `[40DE]`: curr_room == room_L `[4DFE]` or room_BL `[4440]`
-> x - 140; curr_room == room_R `[4E0C]` or room_BR `[4658]` -> x + 140. (Set by `9CFD` when drawn_room changes:
room_L/R = links of drawn_room, room_BL = link_left(room_B) or, if no room below, link_down(room_L); room_BR likewise;
room_AL `[441C]` / room_AR `[4652]` exist but are **not** used here, so an obstacle in an above-left/right room gets no
±140 -- original quirk.) For the kid, drawn_room == Char.room during the tick (check_the_end `0FE8` follows exit_room).

can_bump_into_gate `59FF`: `(curr_room_modif[curr_tilepos] >> 2) + 6 < char_height` (unsigned). char_height = this
tick's set_char_collision value.

### 4.3 bumped(delta, push_dir) `0000:57F0` (`ret far 4`; delta at [bp+8], push_dir at [bp+6])
```
if s8(alive) >= 0: return                    ; 57F3  (dead)
if frame == 177: return                      ; 57FD
Char.x = u8(Char.x + u8(delta))              ; 5807
if push_dir < 0:                             ; pushed back to the LEFT (kid was left of the obstacle)
    if curr_tile2 == 20:                     ; 5814  only a full wall
        get_tile(curr_room, tile_col - 1, tile_row)          ; 581B..5867 (may step into the left room)
else:                                        ; pushed back to the RIGHT
    if curr_tile2 in (12, 7, 20):            ; 5825..5838 doortop, doortop-with-floor, wall
        tile_col += 1                        ; 583A
        if curr_room == 0 and tile_col == 10: curr_room = Char.room; tile_col = 0   ; 583E..5853
        get_tile(curr_room, tile_col, tile_row)              ; 5859..5867
if tile_is_floor(curr_tile2): bumped_floor(push_dir)   ; 586A..587F
else: bumped_fall()                                    ; 5885
```
"Which tile is looked at": curr_room / tile_col / tile_row are the obstacle's normalised position (set by the get_tile in
right_wall_x/left_wall_x of check_bumped, i.e. the room stored in curr_row_coll_room for that index, the index, and the
wrapped curr_row). For a wall the tile on the kid's side of it (one column toward the kid) is tested; for every other
obstacle (gate, chomper, mirror, doortop when pushed left, ...) **the obstacle tile itself** is tested (gate 4, chomper 18,
mirror 13, 7 are floor -> bumped_floor; doortop 12 is not floor -> bumped_fall).

### 4.4 bumped_fall `588C`
```
a = Char.action
Char.x = u8(char_dx_forward(-4))             ; 5899..58A2
if a == 4: Char.fall_x = 0                   ; 58A5..58AB
else: seq 45 (5E72); play_seq                ; 58B2..58BD   first frame of seq 45 shown this tick
bumped_sound()                               ; 58C0 -> 5999: [43AE] = 1 (guard notice), play_sound(8). In BOTH branches.
```
### 4.5 bumped_floor(push_dir) `58C8`
```
if Char.sword != 2 and u16(y_land[row+1] - Char.y) >= 15: bumped_fall(); return     ; 58CE..58F2 (16-bit unsigned)
Char.y = u8(y_land[row+1])                                                            ; 58F5..5901
if s8(fall_y) >= 22: Char.x = u8(char_dx_forward(-5)); return        ; 5904..5917  no seq, no sound
fall_y = 0                                                                            ; 5919
if alive == 0: return                                                ; 591E (never true here: bumped returned for dead)
if Char.sword == 2:
    if u8(push_dir) == Char.direction:  seq 65; play_seq; Char.x = u8(char_dx_forward(1)); return   ; no sound
    s = 64
else:
    s = 46 if frame in {24, 25} or 40 <= frame < 43 or 102 <= frame < 107 else 47    ; 594E..597D
seq s; play_seq; bumped_sound()                                                       ; 5982..5990
```
y_land = `ds:22B0` {-8, 55, 118, 181, 244}; `[bx+22B2]` with bx = 2*row = y_land[row+1].
Sequences: 45 = `act5 dx+1 dy+3 (feather jump) f102 dx+2 dy+6 f103 dx-1 dy+9 f104 dy+12 f105 dx-2 fall(0,15) -> seq 12`;
46 = `act5 dx-1 dy-4 f102 dx-1 dy+3 dx-3 dy+1 knockdown dx+1 snd f107 dx+2 f108 snd f109 -> seq 49`;
47 = `act5 dx-4 f50 f51 f52 -> seq 2`; 64 = `act5 dx-8 -> ds:19E6`; 65 = `act5 f160 f157 -> ds:19E6`.

### 4.6 What is NOT refreshed after a bump (VERIFIED by the absence of calls in 57F0..5999)
bumped changes Char.x and (via play_seq) Char.frame / action / y, but **does not** reload rec, re-run determine_col or
set_char_collision. For the rest of the tick: `rec`/`[66CE]` flags, Char.curr_col, char_height and the box are those of
the pre-bump frame and x. Same after check_gate_push's ±5 (no det_col). After a bump the action is 5, so check_action
only calls check_on_floor on frame 109.

---

## 5. Other wall code

### 5.1 get_edge_distance `5A24` (used by forward_pressed, safe_step, jump_up, grab_up...)
```
determine_col(); load_frame_to_obj(); set_char_collision()      ; 5A2C..5A36 box of the CURRENT frame/x (before play_seq)
t = get_tile_at_char()
if wall_type(t):
    tile_col = s8(Char.curr_col)                                 ; 5A4D (raw column, overrides the normalised one)
    d = dist_from_wall_forward(t)
    if d >= 0: goto have_wall
t = tile_front()                                                 ; 5A72 (col + dir_front, raw col kept in [60C0])
if t == 12 and s8(dir) >= 0:            edge_type = 0; d = distance_to_edge_weight
elif wall_type(t):
    tile_col = s8([60C0]); d = dist_from_wall_forward(t)
    if d >= 0: goto have_wall
    else fall through to the tests below with t
if ... (after the wall test fails or t not wall-like):
    t == 11:                 edge_type = 0; d = distance_to_edge_weight
    t in (6, 22, 10):        d = distance_to_edge_weight; d != 0 ? edge_type 0 : (edge_type 2, d = 11)
    !tile_is_floor(t):       edge_type = 0; d = distance_to_edge_weight
    else:                    edge_type = 2; d = 11
have_wall: d < 14 ? edge_type = 1 : (edge_type = 2, d = 11)
curr_tile2 = t (the last tile examined); return d
```
### 5.2 dist_from_wall_forward(t) `5DB8`
```
if t == 4 and !can_bump_into_gate(): return -1
coll_tile_left_x = ds:24BF[tile_col] (unsigned) + 7              ; no side-room adjustment
wt = wall_type(t); if wt == 0: return -1
facing left : return s8(wall_dist_from_right[wt]) + char_x_left_coll - coll_tile_left_x - 13
facing right: return s8(wall_dist_from_left[wt]) + coll_tile_left_x - char_x_right_coll
```
distance_to_edge(x) `78D0`: col_m7(x) sets tile_mod; `dir == 0 ? 13 - mod : mod`. distance_to_edge_weight `78C6` =
distance_to_edge(dx_weight()). dx_weight `6F31` = char_dx_forward(s8(u8(rec.dx - (rec.flags & 0x1F)))).

### 5.3 in_wall `7809`
```
d = distance_to_edge_weight()
if d >= 8 or tile_front() == 20: d = 6 - d       ; tile_front only evaluated when d < 8
else: d += 4
Char.x = u8(char_dx_forward(d)); load_fram_det_col(); get_tile_at_char()
```
### 5.4 check_on_floor `7551` (from check_action for actions 0, 1, 7 and >= 8; action 5 only on frame 109)
```
if !(rec.flags & 0x40): return          ; rec = the record last loaded (stale after a bump, 4.6)
if get_tile_at_char() == 20: in_wall()  ; uses Char.curr_col as stored (stale after bump / gate push)
if tile_is_floor(curr_tile2): return    ; curr_tile2 = tile at char after in_wall (may still be 20 -> falls!)
if level 12 and [4FCE] < 0 and curr_row == 0 and (room == 2 or (room == 13 and tile_col >= 6)):
    make the two tiles floor (1) and redraw            ; 7576..75ED
else start_fall()
```
### 5.5 start_fall `75F5`, do_fall `5E98`, land `5EF8` (wall parts)
start_fall:
```
f0 = frame; Char.sword = 0; curr_row++ (85EF); start_chompers (950B); [40D6] = f0
seq = {9:7, 13:19, 26:18, 44:21}[f0], 81 <= f0 < 86: seq 19 after x = forward(5) + load_fram_det_col,
      150 <= f0 < 180: charid 2 -> guard cases (8388 / seq 82 / 83); otherwise [3D74] = 1, facing left and
      distance_to_edge_weight <= 7 -> x = forward(-5); seq 81.  Any other frame: seq 7
seq; play_seq; load_fram_det_col
if get_tile_at_char() == 20: in_wall(); return                 ; 762F..763E -- skips the front test
if tile_front() == 20:
    if [40D6] == 44 and distance_to_edge_weight() < 6: seq 104; play_seq     ; the frame BEFORE falling
    else: x = forward(-1)
    load_fram_det_col
```
do_fall:
```
if [4334] == 0 and fall_y >= 31: play_sound(1); [4334] = 1
if u16(y_land[row+1]) > Char.y: check_grab(); return          ; 5EB5..5ECA UNSIGNED: row -1 (y_land -8 = 0xFFF8) never lands
if get_tile_at_char() == 20: in_wall()
if tile_is_floor(curr_tile2): land()                            ; tested on the tile after in_wall
else: curr_row++ (85EF, no start_chompers)
```
land: `[4334] = 0; y = y_land[row+1]`; tile at char == 2 -> spike test; **tile in front not floor (a wall counts) and
distance_to_edge_weight < 3 -> x = forward(-3)** (`5F1C..5F40`); start_chompers; then spikes / seq 17 (fall_y < 22,
sound 17, guard notice) / seq 20 (< 33, take_hp(1), sound 16) / seq 22 (take_hp(100)); sword/charid special cases
(`5F85..5F9F`: guards and a drawn sword land in seq 63); play_seq; fall_y = 0.

### 5.6 check_gate_push `5BDB`
```
if !(action == 7 or frame == 15 or 108 <= frame < 111): return
get_tile_at_char(); orig = tile_col                              ; NORMALISED column of the kid's tile (Char.curr_col as stored)
if curr_tile2 != 4:
    if get_tile(curr_room, tile_col - 1, tile_row) != 4: return  ; may move into the left room (tile_col 9)
c = tile_col
if (curr_row_coll_flags[c] & prev_coll_flags[c]) == 0xFF and can_bump_into_gate():
    bumped_sound()
    Char.x = u8(Char.x + (orig <= c ? -5 : +5))                  ; 5C4D..5C63
```
No room-id check on the arrays. Quirk: kid in column 0 with the gate in the left room's column 9: orig = 0 <= c = 9 ->
pushed **left** (into the gate).

### 5.7 check_chomped_kid `5B13`
```
row = s8(curr_row)
for col in 0..9:                                                 ; no early exit
    if curr_row_coll_flags[col] != 0xFF: continue                ; room array NOT checked (stale flags possible)
    if get_tile(s8(curr_row_coll_room[col]), col, row) != 18: continue      ; row not wrapped (get_tile wraps it)
    if (curr_room_modif[curr_tilepos] & 0x7F) != 2: continue
    chomped():                                                   ; 5B67
        modif |= 0x80
        if frame == 178: continue loop
        if Char.room != curr_room: continue loop                 ; a chomper in another room only gets the 0x80 bit
        x = u8(ds:24BF[tile_col] + 7); x = u8(char_dx_forward(dir == 0 ? 6 : 7))
        y = y_land[row+1]; take_hp(100); play_sound(46); seq 54; play_seq
```
### 5.8 exit_room `3ABB` / leave_room `3BF8` / goto_other_room `3B7A`
exit_room: if `[5CE8]` exit_room_timer != 0: decrement, return. Else loadkid, load_frame_to_obj, set_char_collision
(box of the final state of the tick), leave_room; if it returns >= 0: savekid, next_room `[4E06]` = Char.room, guard
handling. leave_room (y = Char.y as u8, action, frame):
```
if action not in (5,4,3) and -16 <= s8(y) <= 9: dir = 2 (up)
elif y >= 211: dir = 3 (down)
elif 135<=frame<150 or 110<=frame<120 or 150<=frame<163 or 166<=frame<169 or action == 7: return -1
elif direction != 0 (left): char_x_left <= 54 -> 0 (left); char_x_left >= 198 -> 1 (right); else -1
else (right): t = get_tile(Char.room, 9, curr_row)
     if t not in (7, 12) and char_x_right >= 201: 1 (right)
     elif char_x_right <= 57: 0 (left); else -1
side effects (0: 3DC7, 3D59, 3D37; 1: 3D74, 3DA2; 3: level 6 room 1 returns -2), then goto_other_room(dir)
```
Uses char_x_left / char_x_right (not the thinned copies). goto_other_room: `room = links[room][dir]` (also 0);
left: x += 140, right: x -= 140 (byte), up: y += 189 and row = y_to_row_mod4(y), down: y -= 189 and row recomputed.
**The flag arrays are not touched.**

### 5.9 Other readers of char_x_left / char_x_right / the flag arrays
- check_spike_below `7EE1`: columns col_m7(char_x_left) .. col_m7(char_x_right) (returns if the right column < 0); per
  column, rows from curr_row down while the tile is not floor **and get_tile stayed in Char.room** (room 0 also stops).
- Guard-only helpers that read the box (not used for the kid): `5C6B` (guard bumped while running with sword), `5D31/5D58`
  (guard and chomper), `5E23` (backward wall distance).
- Drawing: `4DEA`, `4EB9`, `7F79`, `813B` (from draw_kid/draw_guard).

---

## 6. Differences to our port (`crates/pop-core/src/kid.rs`)

Each item: what differs -> exact fix. Ordered by impact.

**D1. Arrays reset on room change** (`check_room_change`, `self.prev_coll = None`). The original never resets them
(5.8, 1.3). Effect: a wall just across the border is walked through (vector J: original bumps on the first tick in the new
room). Fix: delete `self.prev_coll = None;`.

**D2. No side-room columns** (`check_collisions`: `for col in lo.max(0)..=hi.min(9)`, `wall_xs` reads `self.tile(env,
col, row)` with `X0 + 14*col + 7`). Original loops raw columns `lo..=hi` (can be -2..11), reads tiles through get_tile
normalisation and stores at the normalised index with the room actually read. Fix: replace `RowFlags`/`prev_coll` by
the original layout: `flags: [[u8;10];3]`, `rooms: [[i8;10];3]` (current, below, above), `prev_flags: [u8;10]`,
`prev_rooms: [i8;10]`, `coll_row: i8`, `prev_coll_row: i8` (init -1, rooms init -1). In the loop, for raw col in
`lo..=hi`: `tile_x = X_BUMP_U8[col+5] + 7` (byte read unsigned; equals 65+14col for -4..=14), normalise
`(room', col', row') = env.level.wrap(...)` (room 0 = wall, keep room 0 as a valid id), store at `col'`.

**D3. Per-record room/row matching instead of move_coll_to_prev** (`check_collisions`: `idx = self.row - prow + 1`,
`*proom == self.room`). Fix: implement 3.2 literally (same row or ±3 -> current, p == r+1 or r-2 -> above, else below),
copy flags and rooms, set the three room arrays to -1 (leave flags), then compare **per index**: skip if
`rooms_cur[i] < 0 || prev_rooms[i] != rooms_cur[i]`.

**D4. Columns entering the range** fire a bump in the port (`p[col]` is 0 for columns not computed last tick). Fixed by D3
(room -1 in the previous data blocks the comparison). Also prevents a spurious bump_left that shadows a real
bump_right in the same tick (check_bumped tests bump_left first).

**D5. Action 7 clears the previous flags** (`check_collisions`: `self.coll_prev = [0;10]` before the action-7 return).
Original resets only the two bump columns and returns, leaving all arrays and prev_coll_row. Fix: move the reset of
`coll_prev` away; on action 7 only set `bump_left = bump_right = -1` and return. (Restores gate pushes during a turn.)

**D6. is_obstacle / wall_xs in check_bumped use the kid room and raw index** (`check_bumped`, `is_obstacle(env, col,
...)`, `wall_xs(env, col, self.row)`). Fix: `is_obstacle_at_col(i)`: row = curr_row wrapped into 0..2, tile from
`(rooms_cur[i], i, row)`; on success `coll_tile_left_x = xpos_in_drawn_room(X_BUMP_U8[i+5]) + 7` where the ±140 applies
when that room is the left / below-left (-140) or right / below-right (+140) neighbour of `self.room` (computed as `9CFD`
does; no adjustment for above-left/right); then `delta = left_wall_x - right_coll` or `right_wall_x - left_coll` with the
wall type of that same tile. Without this, vector J2 (wall in the left room's column 9) never bumps.

**D7. is_obstacle for other tiles**: port `_ => wall_type(t) != 0`; original returns 1 for every tile that is not potion /
non-bumpable gate / chomper != 2 / jumped-through mirror. Fix: `_ => true`. Also add the mirror side effect
(modifier = 0x56, jumped_through_mirror) when that state exists, and `sword == 2` as an alternative to the facing test in
both check_bumped branches once swords exist.

**D8. bumped looks at the wrong tile for side rooms** (`bumped`: `self.tile(env, col, self.row)` with the raw index).
Fix: use the obstacle's (room, index, wrapped row) from D6; pushed left & wall -> tile at (room, i-1, row) through wrap;
pushed right & tile in {12, 7, 20} -> i+1, with the room-0 rule `room == 0 && i+1 == 10 -> (self.room, 0)`; else the
obstacle tile itself.

**D9. bumped_fall does not play the sequence** (`bumped_fall`: `start_seq(45)` without `play_seq`). Fix: add
`self.play_seq(env)` after `start_seq(env, 45)`. Also push `Event::Sound(8)` in both branches (5999), same at the end
of bumped_floor's seq 46/47/64 path (not in the fall_y >= 22 path and not in the seq 65 path). (Vector F5.)

**D10. bumped_floor height test is 8-bit** (`(land - self.y as i32) as u8 >= 15`). Fix: `((land - y) as u16) >= 15`.
Add the sword paths (skip the height test, seq 64 / seq 65 + forward(1)) with combat.

**D11. Stale state after a bump / gate push is not modelled.** The port recomputes `col`, the frame record and the box
from the current state everywhere. Original (4.6): after bumped / check_gate_push, Char.curr_col, rec (flags 0x40 test),
char_height and the box keep the values from `0F2A/0F2F`. Fix: store `curr_col: i8` in `Kid` and update it only where
the original calls determine_col (6E80/6E89: tick start, after play_seq + fall_speed, in get_edge_distance, in_wall,
start_fall, fall_speed for action 4, running/standing helpers that call 6E80); keep the per-tick `CollBox` and
`rec` in `Kid` (computed once after fall_speed) and use them in check_bumped, check_gate_push (height), check_on_floor
(flags), check_press, check_spike_below.

**D12. check_gate_push** (`check_gate_push`): uses `self.col(env)` (fresh), raw `tc - 1`, `clamp(0, 9)` index,
`coll_prev` that is zero when rooms differ. Fix: orig = normalised column of the tile at (self.room, stored curr_col,
row); second candidate through wrap of (that room, orig-1, row); index the arrays with the normalised column; no room
check; direction `orig <= c ? -5 : +5` on normalised columns (keeps the column-0 quirk).

**D13. check_on_floor** (`!is_floor(here) && here != Wall`): a tile that is still a wall after in_wall must start a
fall. Fix: `if !is_floor(self.here(env)) { start_fall }`. Level-12 special floor (5.4) is missing.

**D14. start_fall** (a) compares `self.frame == 44` after play_seq; original uses the frame before the fall (`[40D6]`).
Fix: `let f0 = self.frame;` at entry, test `f0 == 44`. (b) After in_wall the original returns; fix: `return` after
`self.in_wall(env)`. (c) seq 104 needs `play_seq`. (d) `sword = 0` and the frames 150..179 branch (seq 81, forward(-5)
when facing left and distance <= 7) are missing.

**D15. do_fall** (a) `(self.y as i32) < land` must be `(land as u16) > self.y as u16` (row -1 never lands). (b) After
in_wall, re-read the tile and fall through to the floor test (land if floor, else row += 1). (c) land: add "tile in front
not floor and distance_to_edge_weight < 3 -> forward(-3)" before the landing sequence.

**D16. check_chomped_kid** (port): uses `coll_now` (room ignored, OK) but tiles of the kid room and returns after the
first match. Fix: tile from `(rooms_cur[i], i, curr_row)`; continue the loop; set modifier |= 0x80 for every match; kill
only if `frame != 178` and the chomper's room == self.room.

**D17. running_jump (not collision, but it decides where wall-hitting jumps start)**: port loop `for i in 1..=2` with
`adj = dist + 14*i - 14` is 14 too large, and treats adj 0/1 as valid. Original `6963`: `for i in 0..2 { col += dir;
if spike or !floor { adj = dist_to_edge(x+fwd4) + 14*i - 14; if !(-8..=-1).contains(&adj) { if adj < 128 { return }
adj = -3 } x = forward(adj+4); break } }` (the unsigned compare at `69DB` makes 0 and 1 return).

**D18. Column lookup outside the table** (`floor_div` in `col`, `distance_to_edge`, `m7`): original reads raw table
bytes (2.2). Only matters for x < 7 or x > 262; embed the table if exactness at the room edges is wanted.

**D19. Missing sounds / guard notice**: `bumped_sound` (sound 8, `[43AE]=1`) after bumped_fall and bumped_floor's
sequence path; gate push already pushes `Sound(8)`.

**D20. Ordering details**: bump_into_opponent (5122) between set_char_collision and check_collisions is missing (combat);
when added it must not recompute the box. exit_room_timer (`[5CE8]`) gating of the room change is missing.

Correct as is (checked): `coll_box` arithmetic (obj_x, parity, half, truncating `/2`, thin ±4, side by direction);
flag formula and bit meaning; bump loop order (lowest index wins); check_bumped skip conditions (action 2/6, frames
135..148) and "bump_left first"; push deltas; can_bump_into_gate; chomper `== 2`; bumped's look-one-column rule for the
kid's own room; bumped_floor's hard-bump frame set and fall_y >= 22 branch; dist_from_wall / edge_distance (box based,
raw columns, tile x = 65 + 14col); in_wall; leave_room box tests.

---

## 7. Test vectors

Computed by tracing sections 1-5 instruction by instruction (scratch model of the original, not of our port).
Common setup unless stated: level with all tiles = 1 (floor), modifiers 0, room 1, links all 0 (outside = room 0 =
wall), the tiles listed below changed. Kid: row 1, y = 118, alive, sword 0, fall 0, charid 0, frame 15 with the sequence
pointer inside seq 2 (stand), arrays in the level-init state (rooms -1, flags 0, prev_coll_row -1). `t0` is the first
tick. "fwd" = the arrow in the facing direction, pressed (fresh) at t0 and held; "up" fresh at the tick given.
Columns in flag lists are array indices; `--` = room -1. Image sizes (h x w) from KID.DAT resource 401+image:

| frame | img | h x w | dx | flags | | frame | img | h x w | dx | flags |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 0 | 42x12 | 1 | C4 | | 15 | 14 | 41x12 | 0 | 43 |
| 2 | 1 | 42x14 | 1 | 44 | | 24 | 23 | 35x53 | 0 | 07 |
| 3 | 2 | 41x19 | 3 | 47 | | 25 | 24 | 28x56 | 0 | 05 |
| 4 | 3 | 40x19 | 4 | 48 | | 43 | 42 | 35x40 | 0 | 01 |
| 5 | 4 | 39x26 | 0 | E6 | | 44 | 43 | 33x31 | 0 | C4 |
| 6 | 5 | 39x26 | 0 | 49 | | 50 | 49 | 37x19 | 4 | 67 |
| 7 | 6 | 40x27 | 0 | 4A | | 102 | 112 | 35x28 | 0 | C6 |
| 8 | 7 | 40x33 | 0 | C5 | | 106 | 116 | 36x18 | 0 | C2 |
| 9 | 8 | 40x33 | 0 | 44 | | 107..112 | 117..122 | 29x18, 23x19, 19x20, 20x21, 22x21, 24x24 | 0 | C4,C5,46,47,47,49 |
| 10 | 9 | 39x25 | 0 | 47 | | 121 | 133 | 40x13 | 0 | 43 |
| 11..14 | 10..13 | 38x26, 40x34, 40x30, 39x21 | 0 | 4B, 43, C3, 47 | | 129..132 | 141..144 | 38x24, 39x19, 40x14, 41x13 | 0 | 43,43,44,44 |

Box notation `c[l,r]` = (char_x_left_coll, char_x_right_coll).

**A. Run right into a wall.** Wall (20) at row 1 col 6 (left face 149). Kid x = 86 facing right, keys fwd.
t0: forward_pressed -> edge_type 2 -> seq 1, frame 1. Run frames: t4 f5 x=94 c[85,90]; t13 f14 x=122 c[111,122]
(index 6 enters the range: lo 2, hi 6, flags 6:F0, no bump: previous room -1). t21 f14 x=148 c[137,148], flags 6:F0.
**t22**: f7 x=152 c[138,152], lo 4 hi 8, flags `4:00 5:00 6:FF 7:00 8:00`, prev 6:F0 -> bump_col_left = 6 ->
is_obstacle (wall) -> delta = 149 - 152 = -3 -> x = 149 -> pushed left, wall -> tile (1, col 5, row 1) = floor ->
bumped_floor: y diff 0 -> frame 7 -> **seq 47**, play_seq: act 5, dx-4 -> **x = 145, frame 50**; sound 8.
t23 f51 x=145, t24 f52, t25 f15 x=145 (standing, box c[139,145]).

**B. Mirror: run left into a wall.** Wall at row 1 col 1 (right face 79+13 = 92). Kid x = 184 facing left, fwd.
t0 seq 1 (index 0 holds room 0 = outside, flags F0). t26 f11 x=106 c[106,119]: index 1 enters (0F). t30 f7 x=92
c[92,106] flags 1:0F. **t31** f8 x=87 c[87,104], flags `0:00 1:FF 2:00 3:00 4:00` -> bump_col_right = 1 -> delta =
92 - 87 = +5 -> x = 92 -> pushed right, wall -> tile col 2 = floor -> seq 47 (dx-4 facing left = +4) -> **x = 96,
frame 50**, sound 8.

**C. Other run phase.** As A with x = 90: t20 f13 x=149 c[134,149] 6:F0; **t21** f14 x=152 c[141,152] 6:FF ->
delta -3 -> x 149 -> seq 47 -> x = 145, frame 50.

**D. Start running right next to the wall.** As A with x = 139: t0 edge distance = 149 - 140 = 9 (>= 8) -> seq 1.
t4 f5 x=147 c[138,143] (thin) 6:F0. **t5** f6 x=150 c[137,150] 6:FF -> delta -1 -> x 149 -> seq 47 -> x = 145.

**E. Safe step to the wall, no bump.** As A with x = 143: t0 get_edge_distance: front tile is the wall, d = 149 - 143 = 6
< 8, fwd fresh -> safe_step -> **seq 34** (28 + 6). Frames 121..132, x: 143,144,145,147,149,149,149,149, max
right_coll 149 (149 < 149 is false, flags stay 6:F0). No bump; ends in seq 2 at x = 149, box c[143,149].

**F. Standing jump into the wall.** (a) x = 110 facing right, keys fwd+up at t0 -> seq 3; lands at x = 148, box never
passes 149: **no bump**. (b) x = 120: t8 f24 x=146 c[119,146] 6:F0; **t9** f25 x=151 y=112 c[123,151] 6:FF -> delta -2
-> x = 149; tile col 5 floor; y diff 118-112 = 6 < 15 -> y = 118, fall_y = 0; frame 25 -> **seq 46** (hard bump),
play_seq: dx-1 dy-4 -> x = 148, y = 114, frame 102; sound 8. t10 f107 x=145 y=118, t11 f108 x=147, t12 f109,
t13 f110 x=148 (seq 49), t14 f111, **t15** f112 x=150 c[138,150] 6:FF (prev F0) -> second bump: delta -1 -> x 149 ->
seq 47 -> x = 145, frame 50.

**G. Running jump into a wall over floor.** Wall at col 7 (face 163). Kid x = 86 right, fwd held, up fresh at t8.
t8: frame 8, running_jump: x+4 = 109 -> col 3; cols 4, 5 floor -> no adjustment -> seq 4, frame 34, x 105.
t14 f40 x=143 y=115 (index 7 enters, F0); t17 f43 x=163 y=115 c[143,163] 7:F0; **t18** f44 x=167 y=118 c[151,167]
7:FF -> delta -4 -> x = 163 -> tile col 6 floor -> frame 44 is not a hard frame -> **seq 47** -> x = 159, frame 50.

**H. Running jump into a wall over a pit (bump-fall).** As G, plus col 6 = 0 (empty). Same until t18: delta -4 -> x = 163
-> tile col 6 empty -> **bumped_fall**: x = forward(-4) = 159, action 1 -> **seq 45**, play_seq: act 5, dx+1 dy+3 ->
**x = 160, y = 121, frame 102**; sound 8. t19 f103 x=162 y=127 ... t22 f106 action 4 (falls into the pit).

**I. Falling past a wall (action 4).** Wall col 6, floor col 5. Kid x = 144 right, frame 106, seq 12 loop, action 4,
y = 60, fall_x = 2, fall_y = 6. t0 fall_y 9 y 69 x 146 c[137,146]; t1 fall_y 12 y 81 x 148; **t2** fall_y 15 y 96 x 150
c[141,150] 6:FF -> delta -1 -> x 149 -> tile col 5 floor -> bumped_floor: u16(118 - 96) = 22 >= 15 -> bumped_fall:
**x = 145, fall_x = 0** (no sequence), sound 8. t3 y 114 x 145; t4 y 135 >= 118 -> land on col 5, fall_y 21 -> seq 17.

**J. Wall in the right neighbour room (bump after the room change).** Room 1 links {L 0, R 2}, room 2 {L 1, R 0};
room 2 row 1 col 0 = wall. Kid in room 1, x = 150 right, fwd. t11 f12 x=181: raw col 10 is checked, stored at
**index 0 with room 2**, flags 0:F0. t17 f10 x=198 c[185,198]; t18 f11 x=202 c[189,202]: left face of raw col 10 is
205, 205 < 202 false -> no bump; leave_room: char_x_right 202 >= 201, tile (9,1) floor -> **room 2, x = 62**.
**t19** (room 2): f12 x=67 c[50,67], lo -3 hi 2 (raw -3..-1 stored at indices 7..9 with room 1), flags `0:FF`
(room 2), prev 0:F0 (room 2) -> bump_col_left = 0 -> tile (room 2, 0, 1) wall, coll_tile_left_x = 58+7 = 65 ->
delta = 65 - 67 = -2 -> x = 65 -> pushed left, wall -> get_tile(2, -1, 1) = room 1 col 9 = floor -> seq 47 ->
**x = 61, frame 50**, sound 8. (Our port resets the arrays on the room change and would not bump here.)

**K. Wall in the left neighbour room (index 9).** Same links; room 1 row 1 col 9 = wall. Kid in room 2, x = 120, facing
left, fwd. t15 f8 x=75 c[75,92]: raw col -1 -> **index 9, room 1**, flags 9:0F. t18 f11 x=68 c[68,81] 9:0F.
**t19** f12 x=63 c[63,80], lo -2, flags 9:FF -> bump_col_right = 9 -> is_obstacle_at_col(9): get_tile(1, 9, 1) = wall,
room 1 = room_L of drawn room 2 -> coll_tile_left_x = (184 - 140) + 7 = 51 -> right_wall_x = 64 -> delta = 64 - 63 = +1
-> x = 64 -> pushed right, wall -> tile_col 10 -> get_tile(1, 10, 1) = room 2 col 0 floor -> seq 47 -> **x = 68**.

**L. Closed gate.** Gate (4) at col 6, modifier 0 (left face 149+10 = 159). Kid x = 86 right, fwd. t24 f9 x=158
c[141,158] 6:F0; **t25** f10 x=160 c[147,160] 6:FF -> can_bump_into_gate: (0>>2)+6 = 6 < 39 -> obstacle ->
delta = 159 - 160 = -1 -> x 159 -> pushed left, not a wall -> gate tile itself, floor -> seq 47 -> **x = 155**.
With modifier 188 (open): same flags (6:FF at t25, bump_col_left = 6) but (188>>2)+6 = 53 >= 39 -> no bump; the kid
runs through (flags stay FF, no new transition).

**M. Gate pushing the standing kid.** Gate at col 5, modifier 100 ((100>>2)+6 = 31 < 41). No keys.
(a) x = 146 facing right (frame 15 box [140,146], foot 143 -> col 5 = the gate): t0 flags 5:FF but prev flags are the
init zeros -> no push. **t1**: cur 5:FF & prev 5:FF, orig 5 <= 5 -> **x = 141**, sound 8. t2: box [135,141], flags 5:F0
-> no more pushes. (b) x = 152 (foot 149 -> col 6, gate in col 5): t1 orig 6 > 5 -> **x = 157**; t2 flags 5:0F.

**N. Running jump timing (D17).** Running right, frame >= 7, pit in the next column: adj = distance_to_edge(x+4) - 14;
the jump starts only for adj in -8..-1 (distance 6..13) with x += adj + 4; pit two columns ahead -> adj 0..13 -> no
jump this tick (returns, the kid keeps running).

---

## 8. Unresolved / not checked

1. **Frames without an image** (kid frames 0, 66, 100, 101, 120, 184, 189, 190, 206, 211..216): set_char_collision
   reads `images[255]`, 1020 bytes past the start of the chtab pointer list, i.e. whatever heap data follows the chtab.
   Tried: the code has no guard (`7466..749B`); the chtab structure is allocated at runtime, so the value cannot be read
   from the EXE. Recommendation: treat as width 0 / height 0 (as SDLPoP does) and avoid relying on it.
2. check_chomped_kid on an index whose flags are a stale 0xFF while its room byte is -1: `get_tile(-1, ...)` calls
   get_room_address(-1) and reads outside the level buffer. Only possible if the overlap column falls out of the checked
   range in one tick; not observed in the vectors.
3. The level-12 floor trick in check_on_floor, the sword (charid/sword == 2) branches of bumped_floor / start_fall /
   land, bump_into_opponent and the mirror jump are documented from the code above but not traced further (combat pass).
4. The test vectors were traced with a scratch model written from this document, not by running the DOS game. The model
   follows every instruction listed here; the remaining risk is in parts not listed (input edge flags of control()).
