# Which images the original draws for each tile type (pixel evidence)

Method (tools/room_shots): 282 screenshots of the original in DOSBox, one per non-empty room of levels 1..14
(`all_rooms.py`), and the images of VDUNGEON.DAT / VPALACE.DAT (`tile_export` example). `learn.py` takes the cells of one tile
type (30 per set, levels 1,2,3,7,8,9,12,13 = dungeon, 4,5,6,10,11,14 = palace), searches for every image of the type's record
near the cell and counts where it sits. Positions are the bottom-left corner relative to the cell's left edge and its floor line
(`65 + 63 * row`); `dx` +32 = in the next column. A hit needs 98 % of the image's opaque pixels equal to the screenshot (colours within
one 6-bit step: the game's palette differs a little from resource 200). Hit rates are about 50 % at best, because other pieces cover
cells (kid, guards, neighbours); a low rate therefore does not prove an image is not drawn. The screenshots wait out the cutscenes
before levels 2, 4, 6, 8, 9 and 12 (they show the game's health triangles). This is evidence for the ids and
offsets; the next section is the 2D model built on it.

## Confirmed by the original's pixels (offset, cells found / cells tried)

| tile type | images found | notes |
|---|---|---|
| 1 Floor | 241 (0,-2) 16/30, 243 (0,+1) 22/30 | 242 never found as a whole image (drawn partly under neighbours?) |
| 2 Spikes | 1327 (0,-2) 15/30, 243 | |
| 3 Pillar | 292 (0,-2) 20/30, 294 (+32,-16) 15/30, 295 (+8,-2) 20/30, 243 | 293 only 1/30 |
| 4 Gate | 246 (0,-2) 13/30, 249 (+24,-2) 12/30, 243 | frames 260..267 not found |
| 6 DropButton | 241 (0,-2) 7/15, 296 (0,+1) 7/15 | |
| 8 / 9 Big pillar | 1286 (0,-2), 1288 (+8,-2) / 1289 (+32,+1), 1291 (+8,+1) | |
| 11 LooseFloor | 241 (0,-2) 11/30, 243 (0,+1) 12/30 | drawn as a plain floor: as the port does |
| 12 Doortop | 285 (0,0), 249 (+24,0) | |
| 14 Debris | 297 and 300 (0,-2) 8/30, 243 | |
| 15 RaiseButton | 347 (0,-2) 13/30, 349 (0,+1) 16/30 | |
| 16 ExitLeft | 237 (+32,-2), 238 (+32,-62), 233 (+40,-15), 234 (+40,-66), 241, 243; palace 299 (-2,-3) | |
| 17 ExitRight | 240 (+32,-62), 243 | |
| 18 Chomper | 1304 / 1305 / 1309 (0,-2), 1312 (0,-49), 243 | |
| 19 Torch | 241 (0,-2) 21/30, 243 (0,+1) 23/30, stub 346 (+32,-27) 14/30 | the stub sits in the next column |
| 20 Wall | 370 and 364 (0,-2) 12/30, 363 / 365 / 367 (0,-62) as 32x3 top pieces, marks 371/372 (+4,+1) | wall faces by runtime modifier 1 and 3 as `WALL_FACE`; 368 (modifier 0) hidden by its marks at the 80 % threshold |
| 21 Skeleton | 1230 (0,-2) 3/5, 243 | dungeon only |
| 23 / 24 Balcony (palace) | 1210 (+32,-2), 1211 (+32,-62) / 1213 (+32,-62) | panels in the next column |
| 26..29 Lattice (palace) | 1201 (0,-2) + 1202 (0,+1) + 1209 (0,-55) / 1203, 1204, 1205 (0,-12) + 1209 (0,-55) | |

Palace levels give the same images at the same offsets, mostly with higher rates (palace floor 241 16/30 and 243 22/30, pillar
292 / 295 and 1201, gate 246 / 249 23/30, doortop 285 / 249 24/30, exit 233 / 234 / 237 / 238 and 299 (-2,-3) 11/13, chomper 1304 /
1309 / 1312, torch stub 346 (+32,-27), lattices 1201 / 1202 / 1203 / 1204 / 1205 with 1209 (0,-55) 20/20..30/30, balconies 1210 /
1211 / 1213 at +32). The palette on screen is VPALACE palette 200: an earlier "palace palette differs" finding was wrong, the
palace screenshots were cutscene frames.

## 2D reference model (`tools/room_shots/render2d.py`)

Pipeline: `all_rooms.py` shoots rooms of the original in DOSBox (the start room of a copy of `LEVELS.DAT` is set to the room; the kid
starts on a plain floor tile in the middle columns, never on a loose floor, which would fall, and never at the edge, where he leaves
the room; a second pass with `POP_ONLY` / `POP_OUT_DIR` uses other floor-like tiles, results in `out2/`). `which_room.py` finds which
room a shot really shows (a shot can show a neighbour) and `render2d.py` composes that room from the model; `compare2d.py` asks, for
every non-black pixel (HUD strip, the kid's and the guards' boxes left out), whether some piece of the model has exactly its colour.
It is a coverage model, not the original's draw order. The model has:

* the images of each tile type at the offsets above (`learned_*.json` adds pieces found by `learn_pieces.py`: the image and offset
  that explain most still-unexplained pixels over all cells of that type), the exit door's panel (233 stacked every 4 px, 13 slices),
  the background pieces from the left neighbour (`tiles.rs back_decor`: 324 / 325 / 326 windows and crosses, 244 / 245), the flame
  frames 151..159 at x + 40, bottom 42 px above the floor line, the potion bottles 162..165 at (+22, -5), the floor sword 160 at (0,-5);
* the port's own wall code ported to Python: dungeon wall faces with the seeded random marks (`tiles.rs dungeon_wall_marks`), palace
  walls from the room's brick colour table and the five speckle strips (`palace_pattern`, `palace_wall`);
* the screen palettes (`screen_pal.json`, `palette_learn.py`): the colours the game shows differ from the resources by a 6-bit step
  here and there, and wall pieces (images 361..377) use palette 360, whose index 10 is the dark top course of dungeon walls;
* learned fixed textures (`learn_textures.py`, `textures_*.npz`, not committed: they hold game pixels): bitmaps that are not one image
  of the DAT files (the palace wall's side face next to an empty cell, the level edge strip, ...), the colour that at least 85 % of at
  least 2 instances of a context (tile type, tile type to the left, row) share. These are measured, not traced; the table gives the
  share with and without them.

**Result** (238 of the 332 non-empty rooms have a usable picture; 6 pictures that are not rooms, the ending screens of level 14 and
one level 6 picture, are left out):

| level | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | all |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| explained % | 98.1 | 98.7 | 97.0 | 96.3 | 95.4 | 95.8 | 97.6 | 97.7 | 98.1 | 94.3 | 95.8 | 89.6 | 98.3 | 96.3 | **96.7** |
| rules only % | 97.6 | 98.3 | 96.5 | 93.4 | 87.3 | 88.8 | 97.1 | 96.6 | 97.6 | 87.7 | 89.3 | 89.5 | 98.2 | 92.0 | **94.1** |

193 of the 238 rooms are at or above 95 %; the 45 below (level, room, %): 1/15 91.8, 2/4 93.7, 3/21 84.6, 4/3 93.8, 4/9 92.7, 4/11 94.3,
4/15 93.2, 5/3 91.6, 5/4 94.8, 5/5 89.7, 5/6 91.7, 5/7 91.7, 5/8 90.2, 5/13 90.0, 5/23 94.3, 5/24 94.7, 6/5 91.3, 6/13 86.7, 6/22 94.3,
6/24 93.3, 7/6 81.6, 9/16 91.8, 9/17 89.9, 9/22 94.0, 10/1 92.3, 10/2 90.8, 10/8 89.5, 10/10 94.5, 10/12 92.1, 10/14 93.6, 10/18 86.9,
10/19 89.6, 11/1 94.2, 11/2 90.5, 11/6 89.8, 11/14 85.6, 11/19 93.8, 11/24 93.3, 12/11 54.3, 12/16 75.4, 12/18 59.0, 13/17 94.9, 14/4 94.1,
14/10 94.3, 14/16 92.7. What is left in the palace rooms: the gate lattices (stacked bars 252, 260..268), the dotted blue frieze band
across the back wall, thin door and torch pieces; in level 12 rooms 11 / 16 / 18 the picture is not explained by any room of the level
(the kid is shown in a room the search does not reach).

**What the comparison showed about the port:**

* The dungeon wall marks (random seed, joints, cracks) reproduce the original pixel for pixel; so do the palace brick colours
  (checked in about 85 rooms: every brick of the 100 % rooms matches the table).
* Palace wall speckle strips use the colour (192, 152, 80), not palette 200's colour 6: the port's `MONO` colour is fixed
  (`tiles.rs`, 194 / 154 / 81).
* The strip variants are drawn with `random(2)` (three variants per group), as in the port.
* The dungeon wall's dark top course is image 373 in palette 360 index 10 (88, 112, 128 on screen); the port uses palette 360 for
  chtab 7, which agrees.
* Torch holder and flame: one column to the right of the torch's tile; fixed in `room.rs` (`TORCH_DX`).

## Differences to the port (`tiles.rs`, `room.rs`) not changed

* **Pillar, gate, doortop:** the original draws its own floor piece (292, 246, 285 with 249) and, for the pillar, the right piece 294
  (293 on the next column, found by `learn_pieces.py`). The port puts the plain floor (241/242/243) under the column 295 / post 249.
* **Balcony panels** (palace): in the next column in the original; the port places them on the tile (shared code with the lattices).
* **Wall edges and side faces of the palace walls** are drawn by the original as fixed bitmaps; the port has none (its 3D walls show
  the side as geometry).
* **Gate lattices:** the original draws stacked bars; the port keeps its moving procedural bars.

## Palace tapestries (found by pixel search, 2026-11)

The wall hangings in the palace levels are VPALACE 1278 / 1280 / 1282 (32x62, 32x62, 25x62) and their small tops 1281 / 1283 (25x12).
The record table does not name them. Search (all palace screenshots, 24 sample pixels then a 97 % match): 36 cells, every one in the
cell to the right of a Doortop (12) or DoortopWithFloor (7) - across a room link in column 0 - bottom edge on the row's floor line, x at
the cell's left edge. Which image is chosen by the **left neighbour's modifier**: 0 = 1278, 1 = 1280, 2 = 1282 (36 of 36 cells; the
original's table `ds:27A2` = 4E 50 52 00 00 51 53). Palace only; nothing in front of a wall. The port draws them in `tiles::back_decor` (as a plate turned like the gates, in the y-z plane against the post, back wall to floor front since 0.6.12)
(cut at the floor surface like the exit frames); `render2d.decor` has the same rule. Effect on the 2D model (palace levels 4, 5, 6, 10, 11,
14): 95.8 % -> 96.0 % explained (rules only 90.0 % -> 90.2 %), rooms below 95 %: 34 -> 30.

Images of the DATs still without a confirmed use (the list the task asked to keep): palace 232, 253..259 (24 wide strips), 260..267 (gate
frames, need motion), 268 and 1314..1323 (1-bit), 269 / 270 / 271 / 272 / 273 / 274 / 345 / 350 / 351 / 362, 1202, 1206 (see below), 1281 /
1283 (only the modifiers 5 and 6, none in the levels' hits), 299 / 361. 1206 (32x63) was found at four Doortop cells in column 9 whose left
neighbour is a LatticeSupport (levels 4, 5, 6, 10) - rule not traced. 272 (25x16) was found at 14 places (floor edges, rows 1-2) - not
attributed. Further unreferenced dungeon images: see docs/texture_inventory.md.

## Loose floors (captured by `tools/room_shots/loose_floors.py`)

The script patches the start room of a level (a floor row 2, six loose floors in row 1, the kid starts on the one in column 4), runs the
original in DOSBox and saves every changed frame (`out_loose/`, not committed: game pixels). Dungeon (level 1) and palace (level 5) show
the same thing, with the images of both sets:

* At rest: 241 / 242 / 243 (the plain floor), as the port draws it.
* The other loose floors of the room shake for a moment when one is triggered, in two frames that replace the face, the right piece and
  the front lip: **A** = 269 (32x14) + 271 + 273, **B** = 270 + 272 + 274. Their bottoms sit on the same lines as 241 (126) and 243
  (129); the differences are the highlight row shifted left or right by a few pixels.
* The triggered floor itself: lip 273 under the kid, then B; it falls with the face 270 at y 126, 129, 135, 144, 156 (steps 3, 6, 9, 12 px:
  the fall speed), the lip 274 3 px below the face, the right piece 272 one column to the right; 274 alone at y 174 on the last frame.
* After landing: the debris 297 / 300 on the floor below (bottom 189), as the port draws it.

So 269..274 are the loose floor's shake and fall frames, not other tile pieces. The port animates the slab geometrically (roll and bounce
of `room.rs::wobble`) with the plain floor texture; the original swaps between two nearly identical images (a few shifted pixels).
Which tick shows A or B is not measured (frames are captured at about 60 ms, the logic ticks at 83 ms); a tick-exact capture would need
the counter in `tools/tick_trace` while grabbing. Using A / B as textures would change a handful of pixels on the lip and face.

## Not covered

* 94 of the 332 non-empty rooms have no usable picture (no floor-like tile to start on, or the picture is blank or not a room); they are
  not compared at all: level 1: 1, 5, 16, 17, 23; 2: 5, 9, 12, 16, 18, 24; 3: 2, 4, 9, 11, 14, 18, 20; 4: 13, 14, 16; 5: 2, 9, 15, 19, 22;
  6: 1, 3, 7, 8, 10, 19, 23; 7: 8, 10, 14, 15, 17, 18, 24; 8: 11, 17, 20, 21; 9: 8, 10, 18, 19; 10: 3, 6, 7, 9, 11, 13, 15, 17, 20, 22;
  11: 3, 7, 8, 9, 10, 11, 12, 17, 18; 12: 2, 5, 6, 9, 10, 12, 14, 20, 22, 23; 13: 2, 9, 10, 14, 19, 20, 22; 14: 3, 5, 8, 9, 11, 15, 18, 22,
  23, 24.
* The kid and the guards (boxes, not pixels), blood, sword sparks; potion bubbles; the gate bar frames; the HUD.
* The model's pieces and the learned textures are measured on the same pictures they are scored on; the rules-only column is the less
  circular number.
