# Which images the original draws for each tile type (pixel evidence)

Method (tools/room_shots): 282 screenshots of the original in DOSBox, one per non-empty room of levels 1..14
(`all_rooms.py`), and the images of VDUNGEON.DAT / VPALACE.DAT (`tile_export` example). `learn.py` takes the cells of one tile
type (30 per set, levels 1,2,3,7,8,9,12,13 = dungeon, 4,5,6,10,11,14 = palace), searches for every image of the type's record
near the cell and counts where it sits. Positions are the bottom-left corner relative to the cell's left edge and its floor line
(`65 + 63 * row`); `dx` +32 = in the next column. A hit needs 98 % of the image's opaque pixels equal to the screenshot (colours within
one 6-bit step: the game's palette differs a little from resource 200). Hit rates are about 50 % at best, because other pieces cover
cells (kid, guards, neighbours); a low rate therefore does not prove an image is not drawn. The screenshots wait out the cutscenes
before levels 2, 4, 6, 8, 9 and 12 (they show the game's health triangles). This is evidence for the ids and
offsets, not a full picture comparison: no 2D reference renderer exists yet.

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

## Differences to the port (`tiles.rs`, `room.rs`)

* **Pillar, gate, doortop:** the original draws its own floor piece (292, 246, 285 with 249) and, for the pillar, the right piece 294.
  The port puts the plain floor (241/242/243) under the column 295 / post 249. Same look at a glance; not the same images.
* **Torch stub 346:** found one column to the right of the torch cell (+32). Check where the port places it (docs said x-28 relative
  to a different origin).
* **Balcony panels** (palace): also in the next column.
* **Wall 368:** not confirmed (see above); 370 and 364 are.
* The port's choice for loose floors, debris, spikes, buttons, big pillars, chompers, exit doors, lattices agrees with the pixels.

## Not covered

* Palace walls (kind 20) and loose floors (kind 11) in either set at the 98 % threshold (walls are drawn through the 1-bit strip
  path with palette 360 in the palace; the loose floor is the plain floor under wobble).
* Potions, swords, flames, the kid and guards (PRINCE.DAT, KID.DAT, GUARD*.DAT images), the gate bar frames, wall marks beyond
  371/372, tile types 5, 7, 13, 22, 25 (too few or no cells in these levels).
* Which pixels of a room no known image explains (missing pieces): needs a composite of the expected pieces per room.
