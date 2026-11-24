# Intro, attract loop and cutscenes (PRINCE.EXE v1.1)

Analysis for the remake's intro and cutscenes. Addresses are `SEG:OFF` in the load image as in `tools/disasm/README.md`
(game code is segment `0000`, runtime and graphics are `0C83`, data is `ds:`). Names follow SDLPoP where the code matches.
Status tags: **VERIFIED** = read in the disassembly; **INFERRED** = follows from the code but depends on a routine that was not
traced to the end; **UNVERIFIED** = depends on hardware, sound card or something not checked.

Time units: the game timer `[336C + 2*n]` counts down in **1/60 s units** (the in-game frame is 5 units, edge_cases.md 5.2).
"Frame" below means one cutscene frame (one `proc_cutscene_frame` step), not a game tick.

This replaces the "not analysed further" note in edge_cases.md 4.1, with two corrections to that section:
- launcher `322D(which_imgs, func, free_sounds)`: play_level `4919..4933` pushes `level > 2` **first**, so the first argument is
  `which_imgs = (level > 2)` and the last one is `free_sounds = 1` (edge_cases.md had them swapped);
- `2724` starts a sequence for the **princess** (shad/Guard slot), `273D` for the **kid slot** (vizier, mouse or kid).

---

## 1. Routines (VERIFIED)

| addr | name | what it does |
|---|---|---|
| `181D` | show_title | title, story page 1, intro scene, story page 2, credits, hall of fame, then the demo (section 2) |
| `0358` | start_game | `[42F8]` == 0 (no game started yet): show_title; else starts the game |
| `04E2` | process_key | while `[42F8]` == 0: any key or Shift sets `[42F8]` = 1, clears the screen and calls start_game (Ctrl+L `0x0C`: load the saved game) |
| `14C0` | idle / key poll | reads the keyboard through process_key; returns non-zero if a key or Shift was pressed |
| `322D` | load_intro(which_imgs, func, free_sounds) | builds the princess room and runs `func` (1.1) |
| `2E22` | reset_cutscene | clears the actors and state (1.2) |
| `2540` | proc_cutscene_frame(n) | plays n frames; returns 0 when done, 1 = key, 2 = fade finished (1.3) |
| `2621` | play_both_seq | kid slot: loadkid `718B`, play_seq `6F90` if Char.frame != 0, savekid `71A0`; then the same for the princess: loadshad `7211`, play_seq, saveshad `71CA` |
| `262A` | draw_proom_drects | draw_princess_room_bg `269A`, copy the dirty rectangles to the screen (`15A0`), then a star twinkle if n is odd |
| `269A` | draw_princess_room_bg | actors, pillar piece, torches, hourglass (1.4) |
| `2724` | seqtbl_offset_shad_char(seq) | loadshad `71B5`, set the sequence `5E72`, saveshad (no play_seq: the new sequence shows from the next frame) |
| `273D` | seqtbl_offset_kid_char(seq) | loadkid, `5E72`, savekid |
| `2D1F` | set_hourglass_state(s) | `[43A8]` hourglass_sandflow = 0, `[5AA2]` hourglass_state = s |
| `2D32` | hourglass_frame() | `6 - count(t in ds:0E00 {6, 17, 33, 65} with t <= rem_min [50BA])` |
| `2D60` | princess_room_torch | two flames per frame (1.4) |
| `2DBB` | draw_hourglass | sand stream and hourglass (1.4) |
| `3380` | draw_star(i, redraw) | one 1-pixel star (1.4) |
| `358D` | fade_in_1 | palette fade-in while the scene animates (1.5) |
| `3691` | fade_out_1 | palette fade-out while the scene animates (1.5) |
| `2E60` / `2ED9` | do_flash(colour) / remove_flash | VGA: background colour = colour / 0 (`0C83:198F`) |
| `12F0` | play_sound(id) | **queues** the sound in `[40A2]` (if its priority `ds:2528[id]` is not lower than the queued one) |
| `132F` | play_next_sound | starts the queued sound; proc_cutscene_frame calls it only when no sound is playing |
| `0C83:720D` | check_sound_playing | |
| `0C83:0412` | prandom(max) | shared RNG, `(seed >> 16) % (max + 1)` (combat.md 3.4) |
| `8CC5` | get_torch_frame(prev) | `r = prandom(255)`; `r < 9 && r != prev` ? r : `(prev + 1) % 9` |
| `1C6D` | draw_full_image(id, chtab, ?, x, y, blit) | blits `chtab.images[id]` at (x, y) to the current target |
| `1BFE` | transition_ltr | copies offscreen to screen in 2-px columns left to right, 160 steps, `pop_wait(timer 1, 0)` per step |
| `0C83:19FB` / `0C83:1CD5` | fade_in_2(surface, rows) / fade_out_2(rows) | blocking palette fades, 64 steps, 2 units per step (1.5) |
| `0C83:0EB5` / `0C83:0EEB` | do_wait(timer) / pop_wait(timer, n) | wait until the timer is 0 (keys go through the hook `[297A]`) |
| `2314` | load_title_images(bgcolor) | TITLE.DAT 40 (chtab `[4DF8]`, palette row 11) and 50 (`[4E0E]`, row 12) |
| `2F2F` | end_sequence | ending cutscene, "hail" page, hall of fame, back to the title (section 4.8) |
| `31F4` | expired | time out: load_intro(1, `2A1B`, 1), then start_game (title) |

Data (VERIFIED, `ds:` offsets):

| ds | content |
|---|---|
| `0FB4` | tbl_cutscenes: far pointer per level 0..15; 2 `2B04`, 4 `2AD0`, 6 `2B04`, 8 `2891`, 9 `28FB`, 12 `2A71`, others 0 |
| `0DD6` | 6 stars {x, y, colour index} (words): (20,97,0) (16,104,1) (23,110,2) (17,116,3) (24,120,4) (18,128,0) |
| `0DFA` | star colours: 8, 7, 15, 15, 7 |
| `0E00` | hourglass thresholds: 6, 17, 33, 65 |
| `0E08` / `0E0C` | torch xh {11, 26} / xl {5, 3} -> x = 8*xh + xl = **93, 211**, ybottom 116 |
| `0E10` | torch frames, initial {1, 6} |
| `034A` | rect_titles {top 106, left 24, bottom 195, right 296} |
| `3D54` | cutscene_frame_time (6 after reset; pv_scene sets 8, then 7) |
| `431C` / `4396` / `3D62` | Guard (princess) / Kid (kid slot) / Char records (mechanics.md) |
| `5AA2`, `43A8`, `4E02`, `438E`, `4ED2`, `59CA` | hourglass_state, hourglass_sandflow, which_torch, flash_time, flash_colour, disable_keys |

### 1.1 load_intro `322D` (VERIFIED)

```
draw_rect(screen, 0)                                   ; black
if free_sounds: free optional sounds 44..56 (22B9)
free_all_chtabs_from(3)
load chtab 8 <- PV.DAT 950 (palette bit 13), chtab 9 <- PV.DAT 980 (bit 14)
target = offscreen
blit chtab8.images[0] (PV 951, 320x200) at (0,0)       ; the room
blit chtab9.images[0] (PV 981, 240x41) at (0,142), OR  ; the bed / carpet
free chtab 9 and the 951 image (memory)
load chtab 3 <- PV.DAT 800 (bit 9), chtab 4 <- PV.DAT 850 + 50*which_imgs (bit 10)
for i in 0..6: draw_star(i, 0)                         ; into the offscreen background
target = onscreen
while check_sound_playing(): idle()                    ; e.g. the level-end music must finish first
[4392] = 1; reset_cutscene(); [430A] is_cutscene = 1
func()
[430A] = 0; free_all_chtabs_from(3); draw_rect(screen, 0)
```
Callers: title `1A8A` (0, pv_scene `2B38`, 0); play_level `4933` (level > 2, tbl_cutscenes[level], 1); end_sequence `2F51`
(1, `2960`, 1); expired `321E` (1, `2A1B`, 1).

### 1.2 reset_cutscene `2E22` (VERIFIED)

Guard.frame = Kid.frame = 0 (both slots empty, nothing drawn), which_torch = 0, disable_keys = 0, hourglass_state = 0,
8 bytes at `ds:40CE` = 0 (meaning UNVERIFIED), hourglass_sandflow = -1, cutscene_frame_time = 6, `9102` (UNVERIFIED),
queued sound `[40A2]` = -1.

### 1.3 proc_cutscene_frame(n) `2540` (VERIFIED)

```
wait = n
loop:
  timer0 = cutscene_frame_time
  play_both_seq()                        ; kid slot first, then princess (only actors with frame != 0)
  if flash_time: do_flash(flash_colour)
  draw_proom_drects()                    ; 1.4; if wait is odd: draw_star(prandom(5), 1)
  if flash_time: flash_time -= 1; remove_flash()
  if !check_sound_playing(): play_next_sound()     ; a queued sound starts here
  do:
    if !disable_keys and idle() != 0:    ; key or Shift
       stop_sounds(); draw_rect(screen, 0); free the fader; return 1
    if fading and fader.step() finished: free the fader; return 2
  while timer0 != 0
  wait -= 1
  if wait != 0: goto loop
return 0
```
Every cutscene function returns as soon as a call returns non-zero, so **a key skips the rest of the scene with no fade-out**
(black screen, sounds stopped). The time-out and the ending set disable_keys = 1 and cannot be skipped. In the title
(`[42F8]` == 0) idle() -> process_key starts the game instead (INFERRED: start_game does not return to the scene).

The flash colour is set before drawing and removed after drawing in the same frame, so on VGA it is visible only while the frame
is being drawn (INFERRED: a short flicker per frame, 5 frames). SDLPoP reorders this so the flash is clearly visible.

Sounds: play_sound only queues. The queued sound starts in step "play_next_sound" of the next frame **in which no sound is
playing**, so a sound queued while another one plays waits for it (VERIFIED `132F`, priority table `ds:2528`, "interruptible"
table `ds:24EE`, not decoded further).

### 1.4 Drawing (VERIFIED)

Order in draw_princess_room_bg `269A`: clear the object tables; kid slot -> load_frame_to_obj, obj_tilepos = 30, add_objtable;
princess slot -> same; redraw the needed background (`B7CE`); foretable pillar piece; torches; hourglass; draw_tables `B8B3`.
Both actors have tilepos 30; the kid slot is added first (draw order between the two INFERRED: princess on top).

| element | image | position (screen px, 320x200) | layer |
|---|---|---|---|
| room | PV 951 (320x200) | (0, 0) | offscreen background |
| bed / carpet | PV 981 (240x41) | top-left (0, 142), OR blit | offscreen background |
| stars | 1x1 rectangles | `ds:0DD6`, colour `star_colours[(idx + 1) % 5]`, index advanced on every draw | background; twinkle drawn on screen |
| pillar piece | PV 952 (16x48), id 2 | x 240, ybottom 167 (y 120..167), blit 0x10 | foretable (in front of the actors) |
| torch flames | PRINCE.DAT 151 + frame (16x18), id frame + 1 of chtab 1 | x 93 and 211, ybottom 116 (y 99..116) | backtable |
| hourglass | PV 952 + state (19x28), id state + 2, state 1..7 | x 152, ybottom 168 (y 141..168), peel | midtable |
| sand stream | PV 960 + sandflow (2x8), id sandflow + 10 | x 160, ybottom 164 (y 157..164), blit 0x10 | foretable |
| actors | chtab 2 + (frame.sword >> 6), see 3 | 1.6 | objtable |

Table ids are 1-based (`add_backtable` `ACDF` stores id - 1), so id k is `images[k-1]` = resource base + k.

princess_room_torch `2D60`, twice per frame: `which_torch = !which_torch; torch_frame[which] = get_torch_frame(torch_frame[which])`,
draw flame `torch_frame + 1` at the torch `which`. So each frame draws both torches and makes 2 prandom calls (+1 for a star on
odd frames). The remake does not need to reproduce the RNG consumption (pop-core seeds each World itself, sim.rs `Rng`).

draw_hourglass `2DBB`:
```
if sandflow >= 0:
    sandflow = (sandflow + 1) % 3
    if hourglass_state >= 7: return          ; with sand flowing an empty glass is not drawn
    foretable(chtab 8, id sandflow + 10, x 160, ybottom 164)
if hourglass_state != 0:
    midtable(chtab 8, id hourglass_state + 2, x 152, ybottom 168)
```

draw_star `3380`: colour 15 in CGA/Hercules modes; else `idx = (idx + 1) % 5`, colour = `ds:0DFA[idx]`; draws the 1x1 rectangle on
the current target; `redraw` adds it to the dirty rectangles.

### 1.5 Fades

- fade_in_1 / fade_out_1 (cutscenes, VGA): a palette fader over rows `0x6689` (rows 0, 3, 7, 9, 10, 13, 14) with wait 0, stepped
  inside proc_cutscene_frame's wait loop; they call `proc_cutscene_frame(1)` until it returns 2 (done) or 1 (key -> return 1).
  **The scene animates during the fade** (frames, torches, sounds). Each step moves every component one level (64 steps,
  `0C83:1B94`: counter 0x40) and writes 7 palette rows with a retrace wait before rows #0 and #4 (`0C83:9519` flag, `95A0`
  assumed to be a retrace wait). Estimate: 64 * 2 retraces / 70 Hz = **1.83 s, about 18-19 frames** at 6 units. INFERRED /
  machine dependent. Other modes: a screen wipe (`0C83:18C5`) plus one frame.
- fade_in_2 / fade_out_2 (title, story, end): blocking, 64 steps, timer 1 = 2 units per step -> **128 units = 2.13 s** (INFERRED
  from the step function; at least 2 units per step is VERIFIED).
- The ending cutscene has **no fade-in**: it copies the whole offscreen buffer to the screen (`15A0(screen)`).

### 1.6 Actors (VERIFIED)

Actors are ordinary character records driven by play_seq. Cutscene charids: 5 princess, 6 vizier (Jaffar), 24 mouse, 0 kid.
load_frame `6E96`: charid 5/6 -> cutscene frame table (`Exe::cutscene_frames`, file offset 118328, 86 frames); 0 and 24 -> kid
table. Image set: `chtab = 2 + (frame.sword >> 6)` (collisions.md 2.1), so in the cutscene table **sword 64 = chtab 3 = PV.DAT
800 set** and **sword 128 = chtab 4 = PV.DAT 850 (which_imgs 0) or 900 (which_imgs 1)**; image i = resource base + 1 + i.
This resolves the open point in ROADMAP M1.5. Frames 19..47 are meant for the 900 set, 48..85 for the 850 set (frames 78..85 do
not exist in the 900 set and are never used with it).

Screen position (same rule as in-game, `pop-app/src/kid.rs` `screen_x`): `px = (2*(x + dx_fwd(frame.dx)) - 116 + parity) * 320/280`,
i.e. about `(x - 58) * 16/7`; ybottom = `y + frame.dy`. Images face left as stored and are mirrored when facing right.

Setup routines (all set Char then call `seqtbl_offset_char` `5E72` + `play_seq` `6F90`, so the first frame is set at once):

| addr | name | charid | x | y | dir | seq | first frame |
|---|---|---|---|---|---|---|---|
| `2828` | init_princess | 5 | 120 | 166 | left | 94 | 11 |
| `27F2` | init_princess_right | init_princess, then dir = right | | | right | 94 | 11 |
| `27CB` | init_princess_x156 | init_princess, then x = 156 | 156 | 166 | left | 94 | 11 |
| `27AE` | princess_stand | init_princess_right, x = 144, y = 169 | 144 | 169 | right | 94 | 11 |
| `2791` | princess_crouching | init_princess, x = 131, y = 169 | 131 | 169 | left | 110 | 37 |
| `27D5` | princess_lying | init_princess, x = 92, y = 162 | 92 | 162 | left | 103 | 19 |
| `27FC` | init_ending_princess | init_princess, x = 136, y = 164 | 136 | 164 | left | 109 | 20 |
| `284B` | init_vizier | 6 | 198 | 166 | left | 95 | 54 |
| `286E` | init_ending_kid | 0 | 198 | 164 | left | 1 | 1 |
| `276E` | init_mouse_go | 24 | 199 | 167 | left | 105 | 186 |
| `2756` | init_mouse_cu8 | init_mouse_go, x = 144 | 144 | 167 | left | 106 | 186 |
| `2819` | init_mouse_ending | init_mouse_go, x -= 2, y = 164 | 197 | 164 | left | 105 | 186 |

The princess goes into the shad slot (saveshad), Jaffar, the mouse and the kid into the kid slot (savekid).

---

## 2. Title and attract loop `181D` (VERIFIED unless marked)

Resources: TITLE.DAT 50 set (palette 50, row 12): 51 title background 320x200, 52 "Broderbund Software presents" 128x47,
53 "a game by Jordan Mechner" 122x30, 54 "Prince of Persia" logo 272x65, 55 copyright line 224x11.
TITLE.DAT 40 set (palette 40, row 11): 41 story frame 320x200, 42 page "In the Sultan's absence..." 272x139 (1 bpp),
43 "Marry Jaffar... or die within the hour" 267x142, 44 "The tyrant Jaffar lies dead..." 264x134 (ending), 45 credits 256x140.
1-bpp pages are drawn with the colour blitter from `9EFC(0x800, 15, 15)` (colour 15 of row 11 = white).

`t` below = 1/60 s units from the start of music 54 (it starts right after the first fade-in).

| t | action |
|---|---|
| start | load_opt_sounds(50, 55) (`0C89`); new offscreen; load_title_images(1); draw 51 offscreen; fade_in_2(row 12, ~128 units) |
| 0 | play sound **54** (intro music, speaker 24.74 s); timer 130; draw 52 at (96,106) offscreen |
| 130 | copy rect_titles to the screen: **"presents" visible**; timer 205; redraw 51 offscreen |
| 335 | copy: presents gone; timer 65; draw 51 + 53 at (96,122) offscreen |
| 400 | copy: **"a game by Jordan Mechner" visible**; timer 270; redraw 51 |
| 670 | copy: plain title; timer 235; draw 51 + 54 at (24,107) (transparent) + 55 at (48,184) offscreen |
| 905 | copy: **logo and copyright visible**; draw 41 + 42 at (24,25) offscreen; wait until sound 54 ends (~t 1484 with speaker music) |
| +0 | play sound **55** (story 1, 14.86 s); transition_ltr -> **page 42** |
| +600 | pop_wait(600) = 10 s, then fade_out_2(row 11, ~2.1 s); free the title images |
| | **load_intro(0, pv_scene, 0)**: black; waits until sound 55 has ended; intro scene (4.9) |
| | load_title_images(1); draw 41 + 43 at (24,25); fade_in_2(row 11) -> **page 43** |
| | draw 51 + 54 (24,107) + 55 (48,184) offscreen; **wait until no sound** (sound 52 from the intro scene, 27.98 s speaker) |
| | transition_ltr -> title with logo; pop_wait(120) |
| | draw 41 + 45 at (24,26); transition_ltr -> **credits**; pop_wait(360) |
| | if hof_count `[409C]` != 0: draw 41 + 54 at (24,24) + show_hof `33F8`; transition_ltr; pop_wait(240) |
| | wait until no sound; fade_out_2(rows 11+12); free; **init_game(0)** `4864` -> the demo (level 0) |

The demo ends with start_game (edge_cases.md 4.4), which shows the title again because `[42F8]` is still 0: the attract loop is
title -> story -> intro scene -> story -> credits -> (hall of fame) -> demo -> title. There is no idle timer (the remake's
former 15 s idle -> demo timer is gone: flow.rs `ATTRACT` plays these show_title phases and has no idle timer). Any key or Shift during the whole loop starts the game at level 1 (process_key `04E2`;
how the key reaches it during do_wait goes through the hook `[297A]`, INFERRED). Ctrl+L loads the saved game.

Page timing depends on the sound card: the waits "until sound 54 ends" and "until no sound" use the music length
(speaker lengths from audio.md; MIDI/AdLib lengths UNVERIFIED). transition_ltr uses `pop_wait(timer 1, 0)`, so its speed is the
machine's blit speed (UNVERIFIED; SDLPoP adds a delay). Suggested remake value: 160 steps over 0.5 s.

---

## 3. Images (VERIFIED: all decode with `pop_core::image::decode`)

Checked with a throwaway tool (scratchpad, not in the repo): every resource of PV.DAT and TITLE.DAT decodes, the 7 palettes parse,
and every image referenced by a cutscene frame exists in its set (except frames 78..85 in the 900 set, see 1.6). **No failures.**

| DAT ids | palette (row) | content | count, size |
|---|---|---|---|
| PV 800 / 801..817 | 800 (9) | princess in the story and frames 1..18 (standing, turning, stepping back) | 17, 18..28 x 42..43 |
| PV 850 / 851..888 | 850 (10) | Jaffar (story only, which_imgs 0) | 38, 13..35 x 47..60 |
| PV 900 / 901..930 | 900 (10, same colours as 800) | princess in-game cutscenes: 901 lying 49x25, 902 empty 1x1, 903..910 ending pose and hug start, 911..916 hug with the kid (48x40, 15-16 colours), 917..930 crouching / stroking the mouse | 30 |
| PV 950 / 951 | 950 (13) | princess room 320x200 | |
| PV 952 | 950 | pillar piece 16x48 (drawn in front) | |
| PV 953..959 | 950 | hourglass states 1..7, 19x28 (959 = empty) | 7 |
| PV 960..962 | 950 | sand stream 2x8 | 3 |
| PV 980 / 981 | 980 (14) | bed with cushions on a carpet, 240x41 | |
| PRINCE.DAT 151..159 | 150 | torch flames 16x18 (chtab 1, resource 150 INFERRED from the image sizes) | 9 |
| KID.DAT 531..533 | 400 | mouse (kid frames 186..188 -> images 130..132), 17x4, 19x4, 12x10 | 3 |
| KID.DAT 401..409, 465, 466 | 400 | kid in the ending (frames 1..9, 53, 54) | |
| TITLE.DAT 40-45, 50-55 | 40 / 50 | section 2 | |

Note: the room picture (951) shows a Moorish arcade: lattice screens above five arches, columns at about px 47, 111, 205, 248,
270, an arched door in the left wall (px 11..33), a dark blue night sky on the left with the stars, two torch holders under the
flames, and a black floor area where the bed is blitted.

VERIFIED (pixel dump of PV 951 / 952 / 981): the thick columns are two identical **front columns**, a 10-px shaft (palette
950 colours 1 2 3 4 5 6 7 7 6 4, ring bands y 121..128, capital y 107..116) running down to the screen bottom (y 199) at
**x 64..73** and **x 245..254**; the thin arcade columns behind them are 4-6 px wide. The bed 981 is fully transparent at
exactly x 64..73 (and 233..239, its right edge), and since it is OR-blitted over 951 the left front column shows through:
in the original that column stands in front of the bed and carpet (no gap is visible). PV 952 (16x48, opaque x 5..14) at
x 240 is the right front column, redrawn in front of the actors. The remake builds both front columns from 952's shaft
(section 6.2; docs/texture_inventory.md 17).

---

## 4. The scenes (VERIFIED scripts; frame numbers after the fade)

Notation: `P(n)` = proc_cutscene_frame(n); `SEQ_P(s)` = 2724 (princess), `SEQ_K(s)` = 273D (kid slot); `SND(id)` = play_sound;
`WAIT_SND` = `do P(1) while check_sound_playing()`. Every `P` returns early on a key (1.3). `F_in` / `F_out` = fade frames (1.5).
Frame times are 6 units unless stated. Positions are Char.x / Char.y; frames are cutscene-table frames for charid 5/6 and kid-table
frames for 0/24.

### 4.1 Before levels 2 and 6: `2B04` cutscene_2_6

```
SND(27); set_hourglass_state(hourglass_frame()); init_princess_right(); saveshad()
fade_in_1; P(26); fade_out_1
```
Princess x 120, y 166, facing right, frame 11 (PV 801) for the whole scene. Hourglass state from rem_min, sand flowing.
which_imgs = 0 on level 2 (850 set loaded but unused), 1 on level 6. Length: F_in + 26 + F_out frames (26 frames = 2.6 s).

### 4.2 Before level 4: `2AD0` cutscene_4

Same as 4.1 with `princess_lying()`: x 92, y 162, facing left, seq 103 = frame 19 (PV 901, lying on the bed), 26 frames.

### 4.3 Before level 8: `2891` cutscene_8 (princess and the mouse)

```
SND(35); set_hourglass_state(hourglass_frame())
init_mouse_cu8(); savekid(); princess_crouching(); saveshad()
fade_in_1; P(20); SEQ_K(107); P(20); SEQ_P(111); P(20); Kid.frame = 0; fade_out_1
```
| frame | mouse (kid slot) | princess |
|---|---|---|
| 1..20 | x 144, y 167, left, frame 186 | x 131, y 169, left, frame 37 (crouching, PV 919) |
| 21..23 | seq 107: 186 (action 0) | 37 |
| 24..31 | 188 (sits up) | 37 |
| 32 | flip (faces right), x 152, 186 | 37 |
| 33..40 | runs right: 157, 160 (187), 164, 169, 172 (187), 176, 181, 184 (187) | 37 |
| 41..51 | keeps running right (186, 186, 187 cycle): x 188 ... 229 | seq 111 stands up: frames 37..47 (x 131) |
| 52..60 | x 232 ... 265 (off screen, px ~473) | frame 52: flip (faces right), x 143, frame 11; then 11 |
| after 60 | Kid.frame = 0 (mouse removed) | fade-out |

### 4.4 Before level 9: `28FB` cutscene_9

```
SND(35); set_hourglass_state(hourglass_frame()); princess_stand(); saveshad()
fade_in_1; init_mouse_go(); savekid(); P(5); SEQ_P(112); P(9); SEQ_K(114); P(58); fade_out_1
```
| frame | mouse | princess |
|---|---|---|
| (fade) | not present | x 144, y 169, right, frame 11 |
| 1..5 | enters from x 199 running left: 194, 191 (187), 187, 182, 179 (187) | 11 |
| 6..7 | 175, 170 | seq 112: 11, 11 |
| 8..14 | 167 (187), 163, 158, 155 (187), 151, 146, 143 (187) | frame 8: flip (faces left), x 131, frame 47; then 46..41 |
| 15..18 | seq 114: frame 188 at x 143 (stays to the end) | 40..37 |
| 19..66 | 188 | stroking the mouse: 36,36,36,35,35,35,34 x7,35,35 (twice), then 36,36,36,35,35,35,34 x9,35,35,35 (seq 112 data) |
| 67..72 | 188 | 36 (seq 112 ends in a loop at `ds:2250` that holds frame 36) |
Total 72 frames + fades.

### 4.5 Before level 12: `2A71` cutscene_12

```
s = hourglass_frame()
if s >= 6:                         ; rem_min < 6
    set_hourglass_state(s); init_princess_x156(); saveshad(); SND(40)
    fade_in_1; P(2); SEQ_P(98); P(24); fade_out_1
else:
    cutscene_2_6()                 ; sound 27, as 4.1
```
Short version: princess x 156, facing left, frame 11; frames 3..10 seq 98 turns (frames 2..9); frame 11 flip + dx 8: x 164,
facing right, frame 11 until the end (26 frames).

### 4.6 Time out: `2A1B` time_expired (from expired `31F4`)

```
disable_keys = 1; set_hourglass_state(7); sandflow = -1; SND(36)
fade_in_1; P(2); P(100); fade_out_1
while check_sound_playing(): idle()
```
No actors (both slots empty after reset_cutscene): the empty room with the **empty hourglass** (PV 959), no sand stream.
102 frames (10.2 s) + fades, then waits for sound 36 (12.72 s speaker), then start_game -> title. Not skippable.

### 4.7 Ending: `2960` end_sequence_anim (from end_sequence `2F2F`, level 14 room 5)

```
disable_keys = 1
if !sound_on [31C2]: turn sound on (0C83:71E1(15))    ; INFERRED meaning
copy offscreen to screen (no fade-in)
SND(26)
init_ending_princess(); saveshad(); init_ending_kid(); savekid()
P(8); SEQ_P(108); P(5); SEQ_K(13); P(2); Kid.frame = 0; P(39)
init_mouse_ending(); savekid(); P(9); SEQ_K(101); P(41)
fade_out_1
while check_sound_playing(): (busy wait, no idle)
```
| frame | kid slot | princess |
|---|---|---|
| 0 | kid x 198, y 164, left, frame 1 | x 136, y 164, left, frame 20 (PV 903) |
| 1..8 | runs in: frames 2..9, x 198, 198, 198, 190, 187, 184, 179, 178 | 20 |
| 9..13 | frames 10..14, x 176, 172, 167, 165, 162 | seq 108: 21..25 (x 136, 135, 135, 135, 134) |
| 14..15 | seq 13 (stop): frame 53 x 162, frame 54 x 160 | 26 (x 137), 27 (x 139) |
| 16..54 | Kid.frame = 0 (hidden: the hug images contain the kid) | 28..33 (x 143, 146, 148, 151, 152, 152), then 33 |
| 55..63 | mouse from x 197 running left: 192 ... 161 | 33 |
| 64.. | seq 101: 186, 186, then 188 | 33 |
104 frames (10.4 s) + fade-out, then the busy wait for sound 26 (11.69 s speaker). The kid's run sequence contains footstep ops
(F2 1 = sound 23); they are queued while 26 plays and are normally dropped (priority rules not traced, UNVERIFIED).

After the animation (end_sequence `2F2F`, matches SDLPoP): clear screen and sounds, load_opt_sounds(56, 56), play **56**
(ending music, 120 s speaker), draw 41 + **44** ("The tyrant Jaffar lies dead...") at (24,25), fade_in_2, pop_wait(900);
timer 240, draw 51, transition_ltr, wait; hall of fame (insert the time if it qualifies: rem_min/rem_tick vs `ds:59F1`, name
entry; section 4.10); wait until the music ends or a key; fade_out_2(row 12); `[42F8]` = 0; start_game -> title. (edge_cases.md 4.1 called
`0C89` "end screen"; it is load_opt_sounds, INFERRED from the title's `0C89(50, 55)`.)

### 4.8 Intro scene: `2B38` pv_scene (in the title, which_imgs = 0)

```
init_princess(); saveshad()                 ; x 120, y 166, left, frame 11
fade_in_1                                   ; Jaffar not yet present
init_vizier(); savekid()                    ; x 198 (right screen edge), left, frame 54
P(2)
SND(50); WAIT_SND                           ; "princess" music, 9.50 s speaker -> ~95 frames
frame_time = 8
P(5)
SND(4); WAIT_SND                            ; gate closing (door), ~1 frame
SND(51); P(3)                               ; princess door opening
SEQ_P(98); P(5)                             ; princess turns (to face right)
SEQ_K(96); P(6)                             ; Jaffar walks in
SND(53); SEQ_K(97); P(4); P(18)             ; "Jaffar comes"; he stops
SEQ_K(96); P(30)                            ; walks on
SEQ_K(97); P(35)                            ; stops
SEQ_K(102)                                  ; conjures
frame_time = 7
P(1)
SEQ_P(99); P(17)                            ; princess steps back
hourglass_state = 1; flash_time = 5; flash_colour = 15     ; the hourglass appears (full), white flash
WAIT_SND                                    ; until 53 ends (14.85 s speaker, ~10 frames left)
SEQ_K(100); sandflow = 0; P(6)              ; Jaffar turns to leave, the sand starts flowing
SND(52); P(24)                              ; "Jaffar leaves"
hourglass_state = 2; P(9)
SEQ_P(113); P(28)                           ; princess (frames 1, 18)
fade_out_1
```
Key positions (simulated play_seq, the sound waits taken from speaker lengths): Jaffar walks from x 198 to x 187 (first walk)
and to x 136 (second walk, net 10 units per 6 frames: dx 2, 6, 1, -1, 1, 1); the princess turns at x 120 and ends at x 128 facing
right, then steps back to x 111 facing left (seq 99: flip, dx 11, 1, 1, 3, 1); Jaffar's exit (seq 100) flips him around
(dx 16 + 3) and he walks right off screen from x ~157. Hourglass at px 152..170 stands between them.

### 4.10 Hall of fame (VERIFIED, read 2026-11-18)

The original DOS v1.1 has a hall of fame. Routines in segment 0000, data in ds:

| addr | what |
|---|---|
| `34D0` load_hof | called once at startup (`0000:033D`): `[409C]` hof_count = 0, open `PRINCE.HOF` (string ds:0E3A) read-only, read 2 bytes into `[409C]` and 0xAE = 174 bytes (6 x 29) into ds:59D8; no file = empty list |
| `347B` save_hof | open/create `PRINCE.HOF` (0x8101, mode 0x80), write the same 2 + 174 bytes; called only after a name is entered |
| `33F8` show_hof | for i < hof_count: sprintf ds:0E31 `"%d:%02d"` with rem_min - 1 and rem_tick / 12, draw the name left-aligned and the time right-aligned in rect ds:0DA6 + 8 i (rows top 84 + 14 i, bottom +12, x 72..248) |
| ds:59D8 + 29 i | entry i: name (25 bytes, NUL-terminated), word rem_min at +25 (ds:59F1), word rem_tick at +27 (ds:59F3) |
| `181D` (`1B7C`) | show_title: after the credits, if hof_count != 0: frame 41 + logo 54 at (24, 24), show_hof, transition_ltr, pop_wait(240) |
| `2F2F` (`3008..31C2`) | end_sequence after page 44 and the title: i = first slot whose (rem_min, rem_tick) is lower than the current [50BA] / [50C0] (an equal time stays above); i >= 6: no entry. Else shift the lower entries down (memcpy 29 bytes), empty name, the time, hof_count = min(count + 1, 6); fade out, draw frame 41 + logo, show_hof, fade in; name input `0C83:0544` into the slot with at most 0x18 = 24 characters (repeated until it returns > 0); save_hof; pop_wait(120); title picture, transition_ltr |

So the list ranks time remaining (not deaths; none are counted), holds 6 names, and a finish only enters it after the
ending. The remake does the same (pop-app `hof.rs`, `flow.rs`): `PRINCE.HOF` next to the exe, but as JSON
(`{"entries": [{"name", "rem_min", "rem_tick"}]}`), written atomically; a damaged file loads what it can (a non-JSON file
is kept as PRINCE.HOF.bad). The original's binary file is not read. Name entry: keyboard text (24 characters,
Backspace), Enter / Esc or a controller's A / Start confirms; an empty name becomes "PRINCE" (controller players). The
title menu also lists it (Hall of fame). Font and colours of the original text are not copied (the app's UI font).

### 4.9 Summary

| scene | which_imgs | sound | setup | frames after the fade | skippable |
|---|---|---|---|---|---|
| `2B04` before 2, 6 | 0 / 1 | 27 | princess right x120 | 26 | yes |
| `2AD0` before 4 | 1 | 27 | princess lying x92 | 26 | yes |
| `2891` before 8 | 1 | 35 | mouse x144, princess crouching x131 | 20 + 20 + 20 | yes |
| `28FB` before 9 | 1 | 35 | princess x144 right; mouse enters | 5 + 9 + 58 | yes |
| `2A71` before 12 | 1 | 40 / 27 | rem_min < 6: princess x156 turns; else 2B04 | 2 + 24 / 26 | yes |
| `2A1B` time out | 1 | 36 | empty room, empty hourglass | 2 + 100, then sound | no |
| `2960` ending | 1 | 26 | kid runs to the princess, hug, mouse | 8+5+2+39+9+41 = 104, no fade-in | no |
| `2B38` intro | 0 | 50, 4, 51, 53, 52 | princess, Jaffar, hourglass appears | ~300 (sound dependent) | key starts the game |

The scene's music keeps playing when load_intro returns (no stop); what the level start does with it is UNVERIFIED.

---

## 5. Test vectors

- hourglass_frame(rem_min): 60 -> 3, 33 -> 3, 32 -> 4, 17 -> 4, 16 -> 5, 6 -> 5, 5 -> 6, 1 -> 6. Image = PV 952 + state.
- cutscene_12 branch: rem_min 5 -> short version (sound 40), rem_min 6 -> cutscene_2_6 (sound 27).
- get_torch_frame(prev, r = prandom(255)): (1, 5) -> 5; (1, 1) -> 2; (8, 200) -> 0; (3, 9) -> 4; (8, 8) -> 0.
- draw_hourglass with sandflow 0 and state 3: sandflow becomes 1, draws PV 961 at (160, 157) and PV 955 at (152, 141).
- star colours after load_intro: stars 0..5 show 7, 15, 15, 7, 8, 7.
- Sequence positions: the tables in 4.3, 4.4, 4.5 (short) and 4.7 are exact play_seq results (frame ops, dx with the facing
  sign, flip, jmp) from `Exe::sequence`; use them as golden values for the pop-core script player.
- Screen x: princess x 120, frame 11 (dx 0, flags 0x80), facing left -> parity 0 -> px = (240 - 116) * 320/280 = 141.
- Durations at 60 Hz (without fades): 2/6, 4, 12-short 156 units; 8: 360; 9: 432; time out 612 (+ sound); ending 624 (+ sound).

---

## 6. Implementation design for the remake

### 6.1 pop-core: `cutscene` module (deterministic, headless)

Data-driven, close to the original so the tables above are the tests:
```rust
pub enum Slot { Kid, Princess }
pub enum Step {
    Spawn(Slot, Actor),        // Actor { charid, x, y, facing_left, seq }; plays the first frame at once (1.6)
    Seq(Slot, u8),             // 2724 / 273D: takes effect on the next frame
    Hide(Slot),                // Kid.frame = 0
    Frames(u16),               // P(n)
    WaitSound,                 // do P(1) while a sound plays
    Sound(u8), FrameTime(u8), Hourglass(u8), Sandflow(i8), Flash { frames: u8, colour: u8 },
    FadeIn, FadeOut, NoFadeShow, DisableKeys, WaitSoundEnd,
}
pub struct Script { pub which_imgs: u8, pub steps: Vec<Step> }   // one per scene, built from section 4
pub struct Cutscene { kid: Option<Actor>, princess: Option<Actor>, frame_time: u8, hourglass: u8, sandflow: i8,
                      torch: [u8; 2], which_torch: bool, flash: (u8, u8), sounds: Vec<u8>, pc: usize, ... }
impl Cutscene {
    pub fn new(script: Script, rem_min: u16) -> Self;
    /// one cutscene frame; `sound_busy` from the app (AudioState::busy), `key` = skip request
    pub fn frame(&mut self, key: bool, sound_busy: bool) -> Option<CutsceneEnd>;
    pub fn frame_units(&self) -> u8;   // 6/7/8 -> the app runs a frame every N 60 Hz steps
}
```
- Sequences come from `Exe::sequence` (no copied frame data); a small play_seq for the ops used (frame, dx, dy, flip, act, jmp,
  sound) is enough. Frame lookup: charid 5/6 cutscene table, 0/24 kid table; image set from `sword >> 6` and which_imgs.
- Fades as a fixed frame count (`FADE_FRAMES`, suggest 18, tunable; 1.5) during which the scene keeps animating.
- Sound waits: the core asks the caller (`sound_busy`), so headless tests pass a length table (audio.md speaker lengths, in
  units) and the app passes its deterministic `AudioState::busy`.
- Expose to the app: actors (charid, frame record, x, y, facing), hourglass state/sandflow, torch frames, flash, sounds queued.
- Tests: the vectors of section 5, every script runs to the end, key skip returns at once, time out / ending ignore keys.

The title/attract sequence is a separate, simpler state machine in pop-app (timers in units, images, transitions); only pv_scene
uses the cutscene player.

### 6.2 pop-app: renderer in the 2.5D style

- New `Screen::Cutscene` state replacing the cards in flow.rs for levels 2, 4, 6, 8, 9, 12, the time out and the end.
- Actors: the same textured quads as `kid.rs` (PV images cached like KID.DAT ones; same `screen_x` rule, mirrored when facing
  right). Vertical mapping: put the cutscene floor line **y 166 on the floor of row 2** (world y 0), i.e. use `y + 15` in the
  kid.rs formula `(181 - bottom)`. The painted room uses y for depth: treat y 167..169 (crouching princess, mouse, hourglass)
  as slightly in front (z + 0.1 per px) instead of lower, and y 162 (lying) as on top of the bed.
- The set ("princess chamber") built with `room.rs` primitives from a synthetic 10x3 tile layout, palace theme. Best match in the
  shipped levels: **level 14 rooms 2 and 3** (LatticeSupport / SmallLattice row above LatticePillar columns, torches), plus the
  balcony pair of **level 14 room 1** for the window. Proposed layout (row 0 top):
  - row 0: LatticeSupport, SmallLattice, LatticeSupport, LatticeLeft, LatticeRight, LatticeSupport, SmallLattice, LatticeSupport,
    SmallLattice, LatticeSupport (the lattice screens above the arches);
  - row 1: BalconyLeft, BalconyRight (night sky with the 6 stars as small emissive quads, colour cycle 8/7/15/15/7), then Empty
    (open arches) with LatticePillar at cols 3 and 6;
  - row 2: Floor everywhere, a Doortop/door at col 0 (the princess door, used by the "door opening" sound in the intro),
    LatticePillar at cols 1, 3, 6, 8;
  - torches: point lights + flame at screen px 101 and 219 (world x = px/32 - 0.5: 2.66 and 6.34), 50 px above the floor;
  - bed: low box (cushion colours from PV 981) in front, px 0..240 -> world x -0.5..7.0, top 4 px above the floor;
  - hourglass: a small model (two glass cones, sand level from state 1..7, a thin stream while sandflow >= 0) at px 152..170
    -> world x 4.25..4.8, standing on the floor in front of the actors' line;
  - pillar piece: a front column at px 240..255 -> world x 7.0..7.5, z in front of the actors (it occludes them, as the
    foretable does).
- **Depth (0.6.7)**: PV 951 is a perspective picture of a deep colonnade, and the set is built as that room, laid out from the picture
  itself. The camera is level and centred (`cutscene::CAM_*`: x 4.6, y 2.1, z 7.66, the distance at which the 3 x 63 picture lines of
  2 units fill the 45 degree view), so the plane z 0 (the actors' line 166) is the screen one to one. A thing standing on the floor on
  picture line L is at depth `floor_z(L)`: the thin arcade columns (base line 146) at z -3.3, the far floor edge (line 144) at -3.8, the
  front columns (the screen bottom, line 200) at +2.6, the rug of the bed (lines 142..183) from z -4.3 to +1.6. `far_at(depth, px,
  line)` puts a picture point on a plane that deep so it lands on its own screen position; things on it are `far_scale(depth)` times
  larger than a picture pixel. Built that way: the floor (palace floor slabs, level with the actors' feet, from z +3.6 back to the
  far wall and out to the sides), the bed's carpet as a mesh lying on it (one strip per picture line at its floor depth, so it looks
  as painted), the far wall (black below line 76, night sky above, the dotted band lines 119..131 on it), and **picture layers cut out
  of PV 951** as unlit cut-out sheets: the arcade (lattice ceiling, the big and the small arches, lines 0..89, on the thin columns'
  plane, with a mirrored copy on each side for wide views), the left wall with the arched door (lines 88..143, x 0..41, on the far
  wall; the stars are small quads in the door), and the two front piers with their capitals (x 56 and 241, lines 0..107) on the front
  columns' plane. The columns are cut out of PV 951 too (per row the run of palette colours 1..7 around the axis): the four thin ones
  (x 41..57, lines 90..146, axes 49 / 112 / 206 / 269) and the two front ones (x 57..81 / 238..262, lines 107..199, axes 69 / 250), voxel
  (revolved) or flat (a sheet with the image) like the level columns. Torch holders and flames are scaled to their plane. The set has no
  tiles of its own any more. Checked against the picture by a screenshot overlay at 16:10 (`POP_CUTSCENE=2`): positions within about 5 px
  of 320 at the far right (a 16:10 window shows 10.15 instead of 10 units across at z 0). Known: the flat torch holder image (346)
  carries a grey dotted background piece and is not scaled; the floor tiles are square (the original's joints run to a vanishing
  point); the actors stand at z 0 and 0.02. The bed picture PV 981 runs off the left edge of the picture; its carpet is completed procedurally
  (`cutscene::extend_carpet`, 56 px): the tan margin, which already slants down to the left at the picture's edge (2.5 px per row), goes on
  to the bottom-left corner, its outer rows are the right end's profile mirrored (tan, outline, blue and red border with dashes), the
  corner towards the bed is the border's red, the bottom border rows go on with their 7-px dash pattern and the bottom margin runs out.
- Implemented look: with `tile_textures` original the set uses the palace texture mapping of the levels (walls, floors,
  arches, columns 295 for the arcade, torch holders 346, voxel flames with `object_style` voxel); the two front columns
  (951 x 64..73, seen through the bed's gap, and x 245..254 = PV 952) use PV 952's 10-px shaft (voxel revolved or a flat
  textured box; procedural: 10-px stone boxes), the left one through the bed plane, both from the bed's bottom line up; the
  sky opening, bed, hourglass and actors keep their own look (docs/texture_inventory.md 17).
- Shadows (enhanced): the sprites are unlit (classic look), and an unlit material receives no shadows, so the bed carpet
  (PV 981) showed none of the actors'. In enhanced mode the carpet uses a lit, matte copy of its sprite material
  (`cutscene::lit_carpet`, roughness 0.9, reflectance 0.2; about 8 % darker than unlit where no shadow falls); classic keeps
  the unlit one. Only the sun (from the front, upper left) can shadow it: the torches stand behind the carpet plane (z -0.6
  vs 0.52) and light its back, so their shadows of the actors fall forward, off the set. The actors (z 0.6 / 0.62) stand
  just in front of the carpet, so the sun's shadow is a short offset to the lower right. Sprite actors cast through their
  alpha mask, voxel actors as meshes; the hourglass and sand stay unlit (they receive no shadow).
- Camera: the normal single-room gameplay framing, fixed (the original is a fixed full-screen view). Optional later: a slow
  dolly; keep it off by default for timing fidelity.
- Flash: full-screen white overlay for the 5 flash frames (the existing `World::flash` overlay code), not the original flicker.
- Fades: fade the whole render to black over FADE_FRAMES.
- Title: TITLE.DAT images as 2D overlays (they are 2D art: logo, pages); keep the original timers (section 2). Optionally
  replace the static title background 51 by the chamber set later.

### 6.3 Risks

1. Fade duration (1.5) and every "wait until the sound ends" depend on hardware and on speaker vs MIDI music; the remake must pick
   lengths (suggest the speaker lengths, already used for death music and feather).
2. The painted room's perspective does not map 1:1 to the tile grid: actor x is exact, but the depth cues (y 162..169) and
   the hourglass/pillar placement need a visual check against PV 951.
3. Draw order of the two actors (INFERRED princess on top) and the hug frames, which contain the kid (the kid is hidden).
4. Princess images face left like KID.DAT (assumed from the shared drawing code; check 911..916 and 917..930 visually).
5. The flash: the original's flicker vs a visible flash (cosmetic choice).
6. RNG: the original consumes prandom in cutscenes (torches, stars); pop-core does not carry the seed across levels, so nothing
   to match, but this is a known divergence.
7. Key skip: Shift counts as a key; in the title any key starts the game (from inside pv_scene too).
8. Music overlap: cutscene music (27 is 10.7 s) keeps playing after a 26-frame scene; what the level start does is UNVERIFIED.

### 6.4 Implementation order

Unambiguous now (exact data): items 1-3, 5 (except the timing constants), the scene scripts and positions.

1. pop-core `cutscene` module: scripts for the 8 scenes, actor setup table, play_seq subset over `Exe::sequence`, hourglass_frame,
   torch frames, golden tests from section 5. No app changes.
2. pop-app debug renderer: the actors and the original PV 951 picture as a flat backdrop (quick check of positions, facing,
   timing; F3 only).
3. The chamber set from tiles + hourglass, bed, front pillar, torches, stars; hook `Screen::Cutscene` into flow.rs for levels
   2, 4, 6, 8, 9, 12 (key skip) and the time out (no skip).
4. The ending: cutscene `2960` + page 44 + music 56; hall of fame done (4.10).
5. Title/attract state machine (section 2) with pv_scene, replacing the 15 s idle timer; story pages 42/43/45. (Done:
   flow.rs `ATTRACT`, no idle timer left.)
6. Tune FADE_FRAMES / transition speed by eye; decide MIDI vs speaker lengths for the waits.
