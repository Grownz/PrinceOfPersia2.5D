# Texture inventory: tile images and the tile-type mapping (PRINCE.EXE v1.1)

Sources: `Original_data/PRINCE.EXE`, `VDUNGEON.DAT`, `VPALACE.DAT`, palettes in both files. Tools: `tools/disasm` (pop-disasm), and `crates/pop-core/examples/dat_inventory.rs` (copied into the main checkout, not committed).

Confidence marks: **[V]** verified from the code or the data bytes. **[I]** inferred from consistent evidence (image sizes, id presence in the DAT, call structure) but not fully traced. **[G]** guess from image size or name only.

## 0. Summary

- Each tile set is one DAT per palace flag: `VDUNGEON.DAT` (dungeon levels) or `VPALACE.DAT` (palace levels). The VGA file name is built as `"V" + "DUNGEON"|"PALACE" + ".DAT"` **[V]**.
- Both sets have 16-colour palette **200** (loaded into chtab 6) and palette **360** (chtab 7). **[V]**
- Tile images are loaded as chtab 6 (base 200: image `k` is resource `200 + k`; the loader loop is **[V]**, the id-1 convention of add_backtable is from docs/cutscenes.md **[I]**). Chtab 7 (base 360) is loaded too, but its use is not traced. A third base, **1200**, is used for big pillars, spikes, skeleton and some palace pieces **[I]**.
- The per-tile-type draw data is a 12-byte record per tile type at `ds:25B0 + 12*type` in PRINCE.EXE **[V for layout and byte 3; I for the other slots]**. Byte +3 of the record is the image id that the draw code passes to add_backtable **[V]**.
- Image counts: VDUNGEON 128 images, VPALACE 145 images. All of them decode with `pop_core::image::decode` **[V]**. Palette ids 200 and 360 are not images.
- Wall (type 20) is drawn by routine `0000:A9DD` (path `AB3A..AB4C`), not by A332 (A332 returns early for type 20). Dungeon wall = chtab 7 (palette 360), resource `360 + T27BA[mod & 7Fh]`, which gives 368 (modifier 0) or 370 (modifier 1). Real levels only contain modifiers 0 and 1 for walls **[V]**. Section 3a has the details.
- Loader `0C83:12FB` (not `0000:12FB`) loads base-1200 slots; `0000:142F` calls it for 8 slots. All 8 per-level flags are 1 in the EXE, so every level loads all slots **[V]**. Ranges are in section 3b.
- Images were exported to PNG with palette 200 (index 0 transparent) and checked by eye: contact sheets in `scratchpad\png\VDUNGEON_sheet.png` and `VPALACE_sheet.png` (index: `*_sheet_index.txt`). Visual checks are marked in section 3.

## 1. Where the tables are

File offset of data-segment offset `d` in PRINCE.EXE = `112832 + d` (the same rule as `tools/disasm`; DS = 1B03). All segment:offset addresses are load-relative, as in `tools/disasm/README.md`.

| what | address | evidence |
|---|---|---|
| DAT name strings: `"%s%s.DAT"`, `"DUNGEON"`, `"PALACE"`, `"V"` etc. | ds `0x2A1..0x2B6` (`ds:2A7` = DUNGEON, `ds:2AF` = PALACE, `ds:2B6` = format) | string dump at file 113440..113999 **[V]** |
| name pointer tables | `ds:2E0` (adapter prefix, index = `ds:3035`), `ds:2EC` (set, index = palace flag `[0FB2]+1CE`) | the code at 0000:0D70..0D8C reads them **[V]** |
| tile-set name build | `0000:0D8C` (`mov ax,2B6h` = format string), a string call `0C83:B0E6` builds the name (arguments not decoded) **[I]** | pop-disasm `xref 0x2B6` |
| chtab 6 load (tile set, palette 200, flag 0x20) | `0000:0D9C..0DB1`: `push 6; push 0C8h (200); push name; push 20h; push -1; call 139D` **[V]** | |
| chtab 7 load (same DAT, palette 360, flag 0x40) | `0000:0E23..0E38`: `push 7; push 168h (360); push name; push 40h; push -1; call 139D` **[V]** | |
| chtab loader | `0000:139D` calls `0C83:105A`, which loops over the images and loads resource `base + i + 1` for image `i` (`0C83:1135..1195`, the `[bp+0Ch]+[bp-0Ch]+1` push at `0C83:115B..1161`) **[V]** | |
| extra chtab set, base 1200 | `0000:142F..14BD`: loop over 8 slots (`[bx+si+10Eh]` per level), `push 4B0h` (1200) and call `0000:12FB` **[I]** (12FB not decoded) | |
| tile-type records | `ds:25B0 + 12*t` for t = 0..30 (`ds:2724` is data, not a type record) **[V layout; I meaning]** | 12 bytes per type; t = the Tile enum of `crates/pop-core/src/level.rs` |
| room draw loop | `0000:9E40..9E8E` (column loop, `[3510]` = 0..9); row draw calls at `0000:9E8F..9EB9` **[V]** | |
| piece draw | `0000:A332`: uses `mul byte [66D0]` (12 * type), `cmp [4E0B],14h` (Wall returns early), and emits `call 0ACDF` (add_backtable) with chtab 6 and the record byte as id **[V]** | |
| helper draw for type 0/9/12 near a gate | `0000:A299`: `[5A8E]=4` (cell is a gate) **[V]** | |
| add_backtable / add_midtable | `0000:ACDF` (stores id - 1, per docs/cutscenes.md) and `0000:AE02` **[V]** | |
| tile lookup | `0000:9FC6` writes the type (`& 1Fh`) and the modifier of a neighbour cell **[V]** | |

Slot semantics inside a record (verified in the A332 code): byte +3 = image id (passed as the id to ACDF, `A373..A4FD`) **[V]**; byte +4 = presence flag (`cmp byte [bx+25B4h],0` at `A1D6`) **[V]**; byte +5 and +6 are y and blit values **[V]**. Bytes +0, +7, +8, +9 are used as image ids in the table below, but the code that draws them was not traced **[I]**.

## 2. Full inventory (both sets)

Columns: id = DAT resource id (images are resource ids; `image k` of chtab 6 is resource 200 + k). WxH in pixels. bpp = bits per pixel (1 or 4). comp = compression (0 raw, 1 RLE left-right, 2 RLE up-down, 3 LZG left-right, 4 LZG up-down, as in `crates/pop-core/src/image.rs`). payload = bytes of the resource payload (after the 1-byte checksum). Role = the best determination. "unreferenced" means no byte of the 31 type records (section 3) points at this id; the id may still be used by code that was not traced. Empty resources (2 zero bytes) are not listed.

Palettes: **200** (16 colours) and **360** (16 colours) are in both files. Palette 200 goes with chtab 6 and palette 360 with chtab 7 **[V]**.

### 2.1 VDUNGEON.DAT (128 images)

| id | WxH | bpp | comp | payload | role |
|---|---|---|---|---|---|
| 232 | 18x8 | 4 | 4 | 39 | unreferenced by the record table |
| 233 | 42x6 | 4 | 1 | 88 | unreferenced by the record table |
| 234 | 42x6 | 4 | 1 | 82 | unreferenced by the record table |
| 235 | 26x16 | 4 | 1 | 82 | StuckButton +3 (up/down button face) |
| 236 | 32x3 | 4 | 4 | 15 | StuckButton +8 (floor top 32x3) |
| 237 | 32x60 | 4 | 3 | 86 | ExitLeft +3 (exit door block 32x60) |
| 238 | 32x16 | 4 | 4 | 65 | ExitLeft +7 (exit lintel) |
| 239 | 26x62 | 4 | 4 | 102 | ExitRight +3 (exit door 26x62) |
| 240 | 26x16 | 4 | 4 | 59 | ExitRight +7 (exit lintel right) |
| 241 | 32x13 | 4 | 1 | 101 | Floor +0 (face 32x13); also Potion, Torch, Sword, Balcony, DropButton, StuckButton +0 |
| 242 | 26x15 | 4 | 1 | 112 | Floor +3 (26x15); also Potion, Mirror, Chomper, Torch, Sword, DropButton +3, RaiseButton +3 |
| 243 | 32x3 | 4 | 1 | 18 | floor top 32x3 (Floor +8, Gate +8, Pillar +8, Spikes +8, Debris +8, Exit +8 and most other types +8) |
| 244 | 28x27 | 4 | 2 | 129 | unreferenced by the record table |
| 245 | 27x22 | 4 | 2 | 98 | unreferenced by the record table |
| 246 | 32x60 | 4 | 2 | 100 | Gate +0 (32x60 back) |
| 247 | 26x62 | 4 | 2 | 125 | Gate +3 (26x62) |
| 248 | 25x13 | 4 | 2 | 22 | Gate +7 (25x13) |
| 249 | 8x60 | 4 | 2 | 18 | Gate +9 (gate bars 8x60); DoortopWithFloor +9 |
| 250 | 24x12 | 4 | 2 | 51 | unreferenced by the record table |
| 251 | 16x10 | 4 | 2 | 41 | unreferenced by the record table |
| 252 | 24x8 | 4 | 2 | 73 | unreferenced by the record table |
| 253 | 24x7 | 4 | 2 | 66 | unreferenced by the record table |
| 254 | 24x6 | 4 | 2 | 60 | unreferenced by the record table |
| 255 | 24x5 | 4 | 2 | 51 | unreferenced by the record table |
| 256 | 24x4 | 4 | 2 | 47 | unreferenced by the record table |
| 257 | 24x3 | 4 | 2 | 35 | unreferenced by the record table |
| 258 | 24x2 | 4 | 0 | 30 | unreferenced by the record table |
| 259 | 24x1 | 4 | 0 | 18 | unreferenced by the record table |
| 260 | 20x9 | 4 | 2 | 61 | unreferenced by the record table |
| 261 | 20x9 | 4 | 2 | 56 | unreferenced by the record table |
| 262 | 20x9 | 4 | 2 | 37 | unreferenced by the record table |
| 263 | 20x9 | 4 | 2 | 43 | unreferenced by the record table |
| 264 | 20x9 | 4 | 2 | 49 | unreferenced by the record table |
| 265 | 20x9 | 4 | 2 | 47 | unreferenced by the record table |
| 266 | 20x9 | 4 | 2 | 54 | unreferenced by the record table |
| 267 | 20x9 | 4 | 2 | 60 | unreferenced by the record table |
| 268 | 24x9 | 1 | 2 | 24 | unreferenced by the record table |
| 269 | 32x14 | 4 | 1 | 107 | unreferenced by the record table |
| 270 | 32x13 | 4 | 1 | 108 | unreferenced by the record table |
| 271 | 25x16 | 4 | 2 | 116 | unreferenced by the record table |
| 272 | 26x16 | 4 | 1 | 116 | unreferenced by the record table |
| 273 | 32x3 | 4 | 1 | 26 | unreferenced by the record table |
| 274 | 32x3 | 4 | 1 | 24 | unreferenced by the record table |
| 285 | 32x4 | 4 | 2 | 16 | Doortop +8 (32x4 lintel) |
| 292 | 32x60 | 4 | 4 | 239 | Pillar +0 (32x60 column); LatticePillar +0 |
| 293 | 26x62 | 4 | 2 | 386 | Pillar +3 (26x62) |
| 294 | 13x7 | 4 | 4 | 30 | Pillar +7 (13x7 capital) |
| 295 | 19x60 | 4 | 3 | 180 | Pillar +9 (19x60); LatticePillar +9 |
| 296 | 32x3 | 4 | 1 | 16 | DropButton +8 (floor top 32x3, pressed state) |
| 297 | 32x13 | 4 | 1 | 169 | Debris +0 (32x13); TorchDebris +0 |
| 298 | 26x16 | 4 | 1 | 152 | Debris +3 (26x16) |
| 299 | 41x4 | 4 | 2 | 20 | unreferenced by the record table |
| 300 | 32x9 | 4 | 1 | 123 | Debris +9 (32x9) |
| 324 | 27x32 | 4 | 2 | 104 | unreferenced by the record table |
| 325 | 28x33 | 4 | 2 | 116 | unreferenced by the record table |
| 326 | 29x52 | 4 | 2 | 516 | unreferenced by the record table |
| 344 | 41x45 | 4 | 4 | 85 | unreferenced by the record table |
| 346 | 32x15 | 4 | 2 | 38 | unreferenced by the record table |
| 347 | 32x14 | 4 | 1 | 105 | RaiseButton +0 (32x14) |
| 348 | 32x14 | 4 | 1 | 105 | unreferenced by the record table |
| 349 | 32x3 | 4 | 1 | 16 | RaiseButton +8 (floor top 32x3) |
| 350 | 22x10 | 4 | 3 | 48 | unreferenced by the record table |
| 351 | 21x10 | 4 | 4 | 46 | unreferenced by the record table |
| 361 | 25x62 | 4 | 4 | 204 | unreferenced by the record table |
| 362 | 25x12 | 4 | 4 | 66 | unreferenced by the record table |
| 363 | 32x3 | 4 | 1 | 10 | unreferenced by the record table |
| 364 | 32x60 | 4 | 1 | 251 | unreferenced by the record table |
| 365 | 32x3 | 4 | 1 | 16 | unreferenced by the record table |
| 366 | 32x60 | 4 | 2 | 348 | unreferenced by the record table |
| 367 | 32x3 | 4 | 1 | 22 | unreferenced by the record table |
| 368 | 32x60 | 4 | 2 | 344 | unreferenced by the record table |
| 369 | 32x3 | 4 | 1 | 26 | unreferenced by the record table |
| 370 | 32x60 | 4 | 1 | 328 | unreferenced by the record table |
| 371 | 9x21 | 4 | 2 | 47 | unreferenced by the record table |
| 372 | 8x21 | 4 | 2 | 40 | unreferenced by the record table |
| 373 | 32x21 | 4 | 2 | 108 | unreferenced by the record table |
| 374 | 14x5 | 4 | 1 | 40 | unreferenced by the record table |
| 375 | 14x5 | 4 | 1 | 40 | unreferenced by the record table |
| 376 | 7x10 | 4 | 2 | 41 | unreferenced by the record table |
| 377 | 7x10 | 4 | 2 | 41 | unreferenced by the record table |
| 1230 | 32x19 | 4 | 4 | 154 | Skeleton +0 (1200 base, dungeon only) [I] |
| 1231 | 26x21 | 4 | 4 | 132 | Skeleton +3 (1200 base, dungeon only) [I] |
| 1286 | 32x60 | 4 | 2 | 217 | BigPillarBottom +0 (1200 base) [I] |
| 1287 | 26x62 | 4 | 2 | 262 | BigPillarBottom +3 (1200 base) [I] |
| 1288 | 23x60 | 4 | 2 | 183 | BigPillarBottom +9 (1200 base) [I] |
| 1289 | 15x63 | 4 | 2 | 113 | BigPillarTop +3 (1200 base) [I] |
| 1290 | 17x6 | 4 | 2 | 39 | BigPillarTop +7 (1200 base) [I] |
| 1291 | 21x63 | 4 | 2 | 147 | BigPillarTop +9 (1200 base) [I] |
| 1301 | 32x60 | 4 | 2 | 436 | unreferenced by the record table |
| 1302 | 32x60 | 4 | 2 | 421 | unreferenced by the record table |
| 1303 | 32x30 | 4 | 2 | 202 | unreferenced by the record table |
| 1304 | 32x21 | 4 | 2 | 154 | unreferenced by the record table |
| 1305 | 32x17 | 4 | 2 | 130 | unreferenced by the record table |
| 1306 | 14x60 | 4 | 4 | 131 | unreferenced by the record table |
| 1307 | 14x60 | 4 | 4 | 134 | unreferenced by the record table |
| 1308 | 14x60 | 4 | 4 | 129 | unreferenced by the record table |
| 1309 | 14x60 | 4 | 4 | 91 | unreferenced by the record table |
| 1310 | 14x60 | 4 | 4 | 71 | unreferenced by the record table |
| 1311 | 27x23 | 4 | 2 | 163 | unreferenced by the record table |
| 1312 | 24x13 | 4 | 2 | 91 | unreferenced by the record table |
| 1313 | 23x10 | 4 | 2 | 68 | unreferenced by the record table |
| 1314 | 6x29 | 1 | 1 | 34 | unreferenced by the record table |
| 1315 | 6x25 | 1 | 0 | 31 | unreferenced by the record table |
| 1316 | 6x18 | 1 | 0 | 24 | unreferenced by the record table |
| 1317 | 6x9 | 1 | 0 | 15 | unreferenced by the record table |
| 1318 | 4x5 | 1 | 0 | 11 | unreferenced by the record table |
| 1319 | 2x29 | 1 | 1 | 18 | unreferenced by the record table |
| 1320 | 2x25 | 1 | 1 | 22 | unreferenced by the record table |
| 1321 | 2x18 | 1 | 1 | 22 | unreferenced by the record table |
| 1322 | 2x9 | 1 | 1 | 13 | unreferenced by the record table |
| 1323 | 2x5 | 1 | 1 | 10 | unreferenced by the record table |
| 1327 | 32x13 | 4 | 2 | 107 | Spikes +0 (1200 base) [I] |
| 1328 | 24x10 | 4 | 4 | 19 | unreferenced by the record table |
| 1329 | 28x20 | 4 | 4 | 54 | unreferenced by the record table |
| 1330 | 31x27 | 4 | 4 | 81 | unreferenced by the record table |
| 1331 | 31x26 | 4 | 4 | 79 | unreferenced by the record table |
| 1332 | 30x26 | 4 | 4 | 68 | unreferenced by the record table |
| 1333 | 26x15 | 4 | 1 | 120 | Spikes +3 (1200 base) [I] |
| 1334 | 5x3 | 4 | 2 | 14 | unreferenced by the record table |
| 1335 | 13x15 | 4 | 2 | 38 | unreferenced by the record table |
| 1336 | 15x15 | 4 | 2 | 47 | unreferenced by the record table |
| 1337 | 15x20 | 4 | 2 | 47 | unreferenced by the record table |
| 1338 | 15x19 | 4 | 2 | 47 | unreferenced by the record table |
| 1339 | 12x5 | 4 | 2 | 16 | unreferenced by the record table |
| 1340 | 28x13 | 4 | 4 | 46 | unreferenced by the record table |
| 1341 | 31x24 | 4 | 4 | 73 | unreferenced by the record table |
| 1342 | 31x23 | 4 | 4 | 70 | unreferenced by the record table |
| 1343 | 30x21 | 4 | 4 | 57 | unreferenced by the record table |

### 2.2 VPALACE.DAT (145 images)

| id | WxH | bpp | comp | payload | role |
|---|---|---|---|---|---|
| 232 | 18x8 | 4 | 3 | 41 | unreferenced by the record table |
| 233 | 48x6 | 4 | 4 | 39 | unreferenced by the record table |
| 234 | 48x6 | 4 | 3 | 28 | unreferenced by the record table |
| 235 | 25x15 | 4 | 3 | 74 | StuckButton +3 (up/down button face) |
| 236 | 32x3 | 4 | 4 | 15 | StuckButton +8 (floor top 32x3) |
| 237 | 32x60 | 4 | 3 | 87 | ExitLeft +3 (exit door block 32x60) |
| 238 | 32x13 | 4 | 3 | 64 | ExitLeft +7 (exit lintel) |
| 239 | 31x62 | 4 | 4 | 126 | ExitRight +3 (exit door 26x62) |
| 240 | 31x13 | 4 | 3 | 67 | ExitRight +7 (exit lintel right) |
| 241 | 32x13 | 4 | 3 | 70 | Floor +0 (face 32x13); also Potion, Torch, Sword, Balcony, DropButton, StuckButton +0 |
| 242 | 25x15 | 4 | 3 | 74 | Floor +3 (26x15); also Potion, Mirror, Chomper, Torch, Sword, DropButton +3, RaiseButton +3 |
| 243 | 32x3 | 4 | 2 | 10 | floor top 32x3 (Floor +8, Gate +8, Pillar +8, Spikes +8, Debris +8, Exit +8 and most other types +8) |
| 244 | 32x14 | 4 | 4 | 43 | unreferenced by the record table |
| 245 | 32x17 | 4 | 4 | 88 | unreferenced by the record table |
| 246 | 32x60 | 4 | 4 | 92 | Gate +0 (32x60 back) |
| 247 | 32x62 | 4 | 4 | 128 | Gate +3 (26x62) |
| 248 | 25x13 | 4 | 2 | 22 | Gate +7 (25x13) |
| 249 | 8x60 | 4 | 2 | 18 | Gate +9 (gate bars 8x60); DoortopWithFloor +9 |
| 250 | 24x12 | 4 | 4 | 43 | unreferenced by the record table |
| 251 | 15x10 | 4 | 4 | 33 | unreferenced by the record table |
| 252 | 24x8 | 4 | 4 | 52 | unreferenced by the record table |
| 253 | 24x7 | 4 | 4 | 52 | unreferenced by the record table |
| 254 | 24x6 | 4 | 4 | 50 | unreferenced by the record table |
| 255 | 24x5 | 4 | 3 | 43 | unreferenced by the record table |
| 256 | 24x4 | 4 | 3 | 39 | unreferenced by the record table |
| 257 | 24x3 | 4 | 3 | 34 | unreferenced by the record table |
| 258 | 24x2 | 4 | 3 | 28 | unreferenced by the record table |
| 259 | 24x1 | 4 | 0 | 18 | unreferenced by the record table |
| 260 | 20x9 | 4 | 4 | 34 | unreferenced by the record table |
| 261 | 20x9 | 4 | 4 | 36 | unreferenced by the record table |
| 262 | 20x9 | 4 | 4 | 32 | unreferenced by the record table |
| 263 | 20x9 | 4 | 4 | 36 | unreferenced by the record table |
| 264 | 20x9 | 4 | 4 | 39 | unreferenced by the record table |
| 265 | 20x9 | 4 | 4 | 38 | unreferenced by the record table |
| 266 | 20x9 | 4 | 4 | 38 | unreferenced by the record table |
| 267 | 20x9 | 4 | 4 | 38 | unreferenced by the record table |
| 268 | 24x9 | 1 | 2 | 24 | unreferenced by the record table |
| 269 | 32x14 | 4 | 3 | 81 | unreferenced by the record table |
| 270 | 32x13 | 4 | 3 | 80 | unreferenced by the record table |
| 271 | 25x16 | 4 | 3 | 80 | unreferenced by the record table |
| 272 | 25x16 | 4 | 3 | 79 | unreferenced by the record table |
| 273 | 32x3 | 4 | 3 | 19 | unreferenced by the record table |
| 274 | 32x3 | 4 | 3 | 17 | unreferenced by the record table |
| 284 | 8x7 | 4 | 4 | 30 | unreferenced by the record table |
| 285 | 32x4 | 4 | 3 | 15 | Doortop +8 (32x4 lintel) |
| 292 | 32x60 | 4 | 3 | 191 | Pillar +0 (32x60 column); LatticePillar +0 |
| 293 | 32x62 | 4 | 4 | 197 | Pillar +3 (26x62) |
| 294 | 13x10 | 4 | 3 | 45 | Pillar +7 (13x7 capital) |
| 295 | 16x60 | 4 | 3 | 131 | Pillar +9 (19x60); LatticePillar +9 |
| 296 | 32x3 | 4 | 4 | 15 | DropButton +8 (floor top 32x3, pressed state) |
| 297 | 32x13 | 4 | 3 | 127 | Debris +0 (32x13); TorchDebris +0 |
| 298 | 25x16 | 4 | 3 | 123 | Debris +3 (26x16) |
| 299 | 48x4 | 4 | 3 | 31 | unreferenced by the record table |
| 300 | 32x9 | 4 | 3 | 90 | Debris +9 (32x9) |
| 324 | 32x14 | 4 | 4 | 43 | unreferenced by the record table |
| 325 | 32x17 | 4 | 4 | 88 | unreferenced by the record table |
| 326 | 31x50 | 4 | 4 | 271 | unreferenced by the record table |
| 344 | 48x45 | 4 | 4 | 166 | unreferenced by the record table |
| 345 | 32x7 | 4 | 4 | 37 | unreferenced by the record table |
| 346 | 32x15 | 4 | 4 | 74 | unreferenced by the record table |
| 347 | 32x14 | 4 | 3 | 74 | RaiseButton +0 (32x14) |
| 349 | 32x3 | 4 | 4 | 15 | RaiseButton +8 (floor top 32x3) |
| 350 | 22x10 | 4 | 3 | 52 | unreferenced by the record table |
| 351 | 21x10 | 4 | 3 | 52 | unreferenced by the record table |
| 361 | 25x62 | 4 | 4 | 145 | unreferenced by the record table |
| 362 | 25x13 | 4 | 4 | 61 | unreferenced by the record table |
| 363 | 8x7 | 4 | 2 | 14 | unreferenced by the record table |
| 364 | 8x7 | 4 | 2 | 15 | unreferenced by the record table |
| 365 | 8x7 | 4 | 2 | 15 | unreferenced by the record table |
| 366 | 32x12 | 1 | 3 | 18 | unreferenced by the record table |
| 367 | 32x12 | 1 | 3 | 18 | unreferenced by the record table |
| 368 | 32x12 | 1 | 3 | 23 | unreferenced by the record table |
| 369 | 32x12 | 1 | 3 | 22 | unreferenced by the record table |
| 370 | 32x11 | 1 | 3 | 21 | unreferenced by the record table |
| 371 | 28x11 | 1 | 3 | 18 | unreferenced by the record table |
| 372 | 30x5 | 1 | 3 | 14 | unreferenced by the record table |
| 373 | 30x5 | 1 | 3 | 14 | unreferenced by the record table |
| 374 | 30x4 | 1 | 3 | 14 | unreferenced by the record table |
| 375 | 32x3 | 4 | 1 | 28 | unreferenced by the record table |
| 376 | 32x3 | 4 | 1 | 32 | unreferenced by the record table |
| 377 | 32x3 | 4 | 1 | 29 | unreferenced by the record table |
| 1201 | 32x53 | 4 | 4 | 252 | unreferenced by the record table |
| 1202 | 32x3 | 4 | 1 | 34 | unreferenced by the record table |
| 1203 | 32x43 | 4 | 4 | 183 | unreferenced by the record table |
| 1204 | 32x43 | 4 | 4 | 182 | unreferenced by the record table |
| 1205 | 32x43 | 4 | 4 | 162 | unreferenced by the record table |
| 1206 | 32x63 | 4 | 4 | 217 | unreferenced by the record table |
| 1209 | 32x7 | 4 | 4 | 25 | unreferenced by the record table |
| 1210 | 32x60 | 4 | 3 | 245 | BalconyLeft +3 if the 1200 base applies (palace only) [I] |
| 1211 | 32x20 | 4 | 4 | 150 | BalconyLeft +7 if the 1200 base applies (palace only) [I] |
| 1212 | 32x62 | 4 | 4 | 280 | Potion +9 and BalconyRight +3 if the 1200 base applies (palace only) [I] |
| 1213 | 27x20 | 4 | 4 | 132 | BalconyRight +7 if the 1200 base applies (palace only) [I] |
| 1275 | 32x60 | 4 | 4 | 249 | Mirror +0 (1200 base, palace only) [I] |
| 1277 | 22x60 | 4 | 4 | 140 | Mirror +9 (1200 base, palace only) [I] |
| 1278 | 32x62 | 4 | 4 | 366 | unreferenced by the record table |
| 1280 | 32x62 | 4 | 4 | 196 | unreferenced by the record table |
| 1281 | 25x12 | 4 | 4 | 56 | unreferenced by the record table |
| 1282 | 25x62 | 4 | 4 | 331 | unreferenced by the record table |
| 1283 | 25x12 | 4 | 4 | 58 | unreferenced by the record table |
| 1286 | 32x60 | 4 | 4 | 203 | BigPillarBottom +0 (1200 base) [I] |
| 1287 | 32x62 | 4 | 4 | 261 | BigPillarBottom +3 (1200 base) [I] |
| 1288 | 22x60 | 4 | 3 | 158 | BigPillarBottom +9 (1200 base) [I] |
| 1289 | 15x63 | 4 | 3 | 105 | BigPillarTop +3 (1200 base) [I] |
| 1290 | 17x9 | 4 | 2 | 10 | BigPillarTop +7 (1200 base) [I] |
| 1291 | 24x63 | 4 | 3 | 132 | BigPillarTop +9 (1200 base) [I] |
| 1301 | 32x60 | 4 | 4 | 255 | unreferenced by the record table |
| 1302 | 32x60 | 4 | 4 | 243 | unreferenced by the record table |
| 1303 | 32x60 | 4 | 4 | 224 | unreferenced by the record table |
| 1304 | 32x60 | 4 | 4 | 157 | unreferenced by the record table |
| 1305 | 32x60 | 4 | 4 | 134 | unreferenced by the record table |
| 1306 | 14x60 | 4 | 4 | 132 | unreferenced by the record table |
| 1307 | 14x60 | 4 | 4 | 135 | unreferenced by the record table |
| 1308 | 14x60 | 4 | 4 | 130 | unreferenced by the record table |
| 1309 | 14x60 | 4 | 4 | 94 | unreferenced by the record table |
| 1310 | 14x60 | 4 | 4 | 77 | unreferenced by the record table |
| 1311 | 27x23 | 4 | 4 | 114 | unreferenced by the record table |
| 1312 | 24x13 | 4 | 4 | 75 | unreferenced by the record table |
| 1313 | 23x10 | 4 | 4 | 59 | unreferenced by the record table |
| 1314 | 6x29 | 1 | 1 | 34 | unreferenced by the record table |
| 1315 | 6x25 | 1 | 0 | 31 | unreferenced by the record table |
| 1316 | 6x18 | 1 | 0 | 24 | unreferenced by the record table |
| 1317 | 6x9 | 1 | 0 | 15 | unreferenced by the record table |
| 1318 | 4x5 | 1 | 0 | 11 | unreferenced by the record table |
| 1319 | 2x29 | 1 | 1 | 18 | unreferenced by the record table |
| 1320 | 2x25 | 1 | 1 | 22 | unreferenced by the record table |
| 1321 | 2x18 | 1 | 1 | 22 | unreferenced by the record table |
| 1322 | 2x9 | 1 | 1 | 13 | unreferenced by the record table |
| 1323 | 2x5 | 1 | 1 | 10 | unreferenced by the record table |
| 1327 | 32x13 | 4 | 3 | 89 | Spikes +0 (1200 base) [I] |
| 1328 | 23x12 | 4 | 3 | 28 | unreferenced by the record table |
| 1329 | 28x19 | 4 | 4 | 56 | unreferenced by the record table |
| 1330 | 31x27 | 4 | 4 | 99 | unreferenced by the record table |
| 1331 | 31x26 | 4 | 3 | 105 | unreferenced by the record table |
| 1332 | 30x26 | 4 | 4 | 95 | unreferenced by the record table |
| 1333 | 25x15 | 4 | 3 | 84 | Spikes +3 (1200 base) [I] |
| 1334 | 5x7 | 4 | 2 | 14 | unreferenced by the record table |
| 1335 | 13x12 | 4 | 2 | 32 | unreferenced by the record table |
| 1336 | 15x15 | 4 | 3 | 40 | unreferenced by the record table |
| 1337 | 15x20 | 4 | 3 | 43 | unreferenced by the record table |
| 1338 | 15x19 | 4 | 3 | 41 | unreferenced by the record table |
| 1339 | 11x6 | 4 | 2 | 16 | unreferenced by the record table |
| 1340 | 28x13 | 4 | 4 | 40 | unreferenced by the record table |
| 1341 | 31x24 | 4 | 3 | 82 | unreferenced by the record table |
| 1342 | 31x23 | 4 | 3 | 84 | unreferenced by the record table |
| 1343 | 30x22 | 4 | 3 | 73 | unreferenced by the record table |
## 3. Tile type to image ids (the 12-byte records)

Record for tile type `t` = `ds:25B0 + 12*t` (type numbers from `crates/pop-core/src/level.rs`, `Tile`). Byte +3 is the image that the draw code passes to add_backtable with chtab 6 **[V]**. The other slots (+0, +7, +8, +9) hold image ids as well; their layer and draw position were not traced **[I]**.

How a value `v` becomes a resource id (per set):
1. `200 + v` if that resource is an image in the set **[I, presence rule; base 200 is V for chtab 6]**.
2. Else `1200 + v` if that resource is an image in the set **[I; base 1200 from `0000:142F`]**.
3. Small values `v` in 1..13 (records of Wall, Balcony, Lattice) are candidates for chtab 7, resource `360 + v` **[G]**.

"-" = not an image in that set.

| type | slot | v | VDUNGEON | VPALACE | conf |
|---|---|---|---|---|---|
| 0 Empty | - | - | nothing | nothing | V (all zero) |
| 1 Floor | +0 / +3 / +8 | 41 / 42 / 43 | 241 / 242 / 243 | 241 / 242 / 243 | +3 V, others I |
| 2 Spikes | +0 / +3 / +8 | 127 / 133 / 43 | 1327 / 1333 / 243 | 1327 / 1333 / 243 | I (1200 base) |
| 3 Pillar | +0 / +3 / +7 / +8 / +9 | 92 / 93 / 94 / 43 / 95 | 292 / 293 / 294 / 243 / 295 | same | I |
| 4 Gate | +0 / +3 / +7 / +8 / +9 | 46 / 47 / 48 / 43 / 49 | 246 / 247 / 248 / 243 / 249 | same | I |
| 5 StuckButton | +0 / +3 / +8 | 41 / 35 / 36 | 241 / 235 / 236 | same | I |
| 6 DropButton | +0 / +3 / +8 | 41 / 42 / 96 | 241 / 242 / 296 | same | I |
| 7 DoortopWithFloor | +0 / +8 / +9 | 46 / 43 / 49 | 246 / 243 / 249 | same | I |
| 8 BigPillarBottom | +0 / +3 / +8 / +9 | 86 / 87 / 43 / 88 | 1286 / 1287 / 243 / 1288 | same | I (1200 base) |
| 9 BigPillarTop | +3 / +7 / +9 | 89 / 90 / 91 | 1289 / 1290 / 1291 | same | I (1200 base) |
| 10 Potion | +0 / +3 / +8 / +9 | 41 / 42 / 43 / 12 | 241 / 242 / 243 / - (1212 palace only) | 241 / 242 / 243 / 1212 | I; +9 G |
| 11 LooseFloor | none | - | no image bytes | no image bytes | unknown |
| 12 Doortop | +8 / +9 | 85 / 49 | 285 / 249 | 285 / 249 | I |
| 13 Mirror | +0 / +3 / +8 / +9 | 75 / 42 / 43 / 77 | - (1275 palace only) / 242 / 243 / - (1277 palace only) | 1275 / 242 / 243 / 1277 | I |
| 14 Debris | +0 / +3 / +8 / +9 | 97 / 98 / 43 / 100 | 297 / 298 / 243 / 300 | same | I |
| 15 RaiseButton | +0 / +3 / +8 | 147 / 42 / 149 | 347 / 242 / 349 | same | I |
| 16 ExitLeft | +0 / +3 / +7 / +8 | 41 / 37 / 38 / 43 | 241 / 237 / 238 / 243 | same | I |
| 17 ExitRight | +3 / +7 / +8 | 39 / 40 / 43 | 239 / 240 / 243 | same | I |
| 18 Chomper | +3 / +8 | 42 / 43 | 242 / 243 | same | I |
| 19 Torch | +0 / +3 / +8 | 41 / 42 / 43 | 241 / 242 / 243 | same | I |
| 20 Wall | +3 / +7 (not used) | - | wall = 368 (mod 0) or 370 (mod 1), chtab 7; see 3a | same ids, palace images are 1bpp (see 3a) | V for code path, I for images (visual: dungeon 368 = brick block) |
| 21 Skeleton | +0 / +3 / +8 | 30 / 31 / 43 | 1230 / 1231 / 243 | - / - / 243 (1230, 1231 dungeon only) | I |
| 22 Sword | +0 / +3 / +8 | 41 / 42 / 43 | 241 / 242 / 243 | same | I |
| 23 BalconyLeft | +0 / +3 / +7 / +8 | 41 / 10 / 11 / 43 | 241 / 370 or 1210 / 371 / 243 | same | G |
| 24 BalconyRight | +3 / +7 / +8 | 12 / 13 / 43 | 372 or 1212 / 373 or 1213 / 243 | same | G |
| 25 LatticePillar | +0 / +3 / +8 / +9 | 92 / 42 / 43 / 95 | 292 / 242 / 243 / 295 | same | I |
| 26 LatticeSupport | +0 / +8 / +9 | 1 / 2 / 9 | 361 / 362 / 369 (chtab 7 candidates) | same | G |
| 27 SmallLattice | +0 / +9 | 3 / 9 | 363 (chtab 7 candidate) | same | G |
| 28 LatticeLeft | +0 / +9 | 4 / 9 | 364 (chtab 7 candidate) | same | G |
| 29 LatticeRight | +0 / +9 | 5 / 9 | 365 (chtab 7 candidate) | same | G |
| 30 TorchDebris | +0 / +3 / +8 / +9 | 97 / 98 / 43 / 100 | as Debris (297 / 298 / 243 / 300) | same | I |
| 31 (no record) | - | - | bytes at ds:2724 are not a type record | same | unknown |

Notes:
- Value 43 (image 243, 32x3) is in the +8 slot of nearly every type. It is the most likely floor cap **[I]**.
- The `ds:2780` table (32 bytes, then `3C..43` = ids 260..267 and `34..3B` = ids 252..259) is an ordered list of image ids whose sizes fit a sequence (24xN, N = 8 down to 1; 20x9 x8). It is probably an animation or crumble sequence. Not traced **[G]**.

### 3a. Wall drawing (verified in code)

- `0000:A222` (called from A1C2 when the current cell is type 0, 9, 12 or 26): if the cell kind at `5A8E[col]` is 20 (Wall) it draws chtab 7, id 2, i.e. resource **362** (`0000:A25E..A27E`) **[V code, I image: VDUNGEON 362 = 25x12, VPALACE 362 = 25x13]**. For kind 7 or 12 in the palace only, chtab 6 with an id from table `ds:27A6` (`A23D`).
- `0000:A9DD` path `AA25 -> AB3A`: current cell type 20 (Wall). If palace flag is 0 or adapter is not 5, draw chtab 7 with id `T27BA[mod & 7Fh]` at `[5AA4]` (y) and `[4364]` (x), via `0000:AD70`, then (adapter not 1 or 2) `0000:C357` with 1. **[V]**
- Table `ds:27BA` (16 bytes are used; index 0..15). Real levels contain only modifier 0 and 1 for walls (LEVELS.DAT, all 16 levels, counted by `dump_png`: dungeon 0:2287, 1:243; palace 0:1701, 1:90) **[V]**. So:
  - mod 0 -> `27BA[0] = 08h` -> resource **368** (dungeon 32x60, palace 32x12 1bpp)
  - mod 1 -> `27BA[1] = 0Ah` -> resource **370** (dungeon 32x60, palace 32x11 1bpp)
  - Index 2..15 is not used by level data. Values beyond index 15 are text, so the table is 16 entries long in practice.
- Visual: VDUNGEON 368 is a three-brick block (stone wall face), 370 is the same kind of block **[V by eye]**. VPALACE 368 is a 1-bit 32x12 strip: it does not look like a full wall, so the palace wall body is probably drawn with palette 360 as an overlay or in another routine. **Open.**
- Modifier 0 of walls comes from the raw `bg` byte; `load_alter_mod` does not change walls (only gates, potions, loose floors) **[V, level.rs]**.

### 3b. Base 1200 extra slots (`0C83:12FB`, loaded by `0000:142F`)

- `0000:142F` loops slot i = 0..7; if the flag `ds:10E + 16*i + level` is nonzero it calls `0C83:12FB` with base 1200, first = `ds:31C[i]`, last = `ds:324[i]` (inclusive). `12FB` calls `1226` per image and fills chtab 6 (flag 0x20) **[V]**.
- The flags `ds:10E..17E` are all 1 in the EXE and no code writes them in the xref sweep, so **every level loads all eight ranges** **[V static; I no runtime write]**.
- Ranges (resource = 1200 + v): slot 0: 1201..1209; slot 1: 1230..1231 (skeleton); slot 2: 1275..1277 (mirror); slot 3: 1278..1283; slot 4: 1286..1291 (big pillars); slot 5: 1301..1323; slot 6: 1327..1343 (spikes); slot 7: 1210..1213 (balcony, potion). The record values of these pieces fall in the same ranges (e.g. big pillar records use 86..91, spikes 127 and 133), which supports the presence rule in section 3 **[I]**.
- Palace levels (ds:1CE = 1 for levels 4, 5, 6, 10, 11, 14) match `kid.rs`. Only the ids that exist in the DAT are used. **[V]**

### 3c. Gate frames (260..267)

- `0000:A299` with current cell kind 4 (Gate) draws 268 and 264 (chtab 6) and then one of **260..267** chosen by `T2792[(min(gate_mod, 188) >> 2) & 7]`, where `ds:2792 = 3C..43` (**[V code]**). Gate open state is 0 (closed) or 188 (open) per `load_alter_mod` **[V]**. So 260..267 are the eight 20x9 frames of the gate opening. Visual check of 260..267 in the contact sheet: 20x9 bars **[I]**.

## 4. Palettes (which to use per set)

| id | colours | used by | in VDUNGEON | in VPALACE |
|---|---|---|---|---|
| 200 | 16 | chtab 6 (tile images), flag 0x20 | yes, starts `00 00 00 03 08 0f` | yes, starts `00 00 00 1e 16 0f` (different colours) |
| 360 | 16 | chtab 7, flag 0x40 | yes | yes |

Use the palette resource from the **same DAT** as the images. Both files have 200 and 360, and 200 differs between them. Palette layout: `res[3]` = colour count, then 6-bit VGA RGB triples from `res[4]` (`crates/pop-core/src/image.rs::palette`). The flag bits (0x20 / 0x40) select palette rows; their exact effect was not traced **[I]**.

Set choice: the palace flag at `[0FB2]+1CE` picks VPALACE, otherwise VDUNGEON **[V for the name build; I for which levels]**. `crates/pop-app/src/kid.rs` lists palace levels as 4, 5, 6, 10, 11, 14. That list was not re-checked here.

## 5. Mapping to `crates/pop-app/src/room.rs` tile() placeholders (implemented)

Implemented in `crates/pop-app/src/tiles.rs` (`tile_images` = the pure mapping, tested with the ids below) and `room.rs`; setting `tile_textures` (`original` | `procedural`, env `POP_TILES`). Images come from the set of the level (palace levels 4, 5, 6, 10, 11, 14 = VPALACE), palette 200 (360 for 361..377), nearest sampling, lit `StandardMaterial`. Resource id = `200 + v` if the set has a 4-bit image there, else `1200 + v`; anything missing falls back to the procedural material and is logged. Pixel scale: 32 px = 1 world unit across, 63 px = `TILE_H` up.

What the images are (checked by eye, sheets from `dump_png` at 6x): 241 / 242 are the floor **top surface** drawn sheared (241 on the tile, 242 its right piece in the next column, with a 5-px end face), 243 is the 3-px **front lip** (not the top). 292 / 293 are the pillar with the floor behind it; 295 is the bare column (the piece in front of the kid). 249 is a door post, not gate bars. 260..267 (20x9) are not gate bars either (they look like small rubble/crack pieces), so the gate keeps its moving procedural bars. 237 / 239 are the exit door posts with floor rows at the bottom, 238 / 240 the lintels. Dungeon 368 / 370 are full brick wall faces (32x60); palace 366..374 are 1-bit mortar overlays.

| tile() arm | textured as | verdict |
|---|---|---|
| back wall, Empty | dark slab / nothing | KEEP |
| Floor, Sword, Balcony, Torch, Chomper, Mirror, Lattice*, Doortop* floors | floor slab with a 32x16 texture: the unsheared top surface (241 row runs + 242 row runs without the end face) over the lip 243; same texture on top and sides | verified visually (both sets). Doortop (12) has no floor piece in its record (+8 = 285); its floor slab is kept and gets the plain floor: guess |
| Skeleton | dungeon: plain floor slab plus a **voxel relief** of the bones (`items.rs`: 1230 / 1231 composed as drawn, base in the tile's column and the right piece in the next, bottom-aligned, minus the same composition of 241 / 242; cropped, centred on the tile, standing on the floor at z -0.1, behind the figures); palace has no 1230 -> plain floor + procedural box | verified visually (level 1 room 11, classic and enhanced); front projection = the bone pixels (test) |
| Debris, TorchDebris | 297 / 298 / 243 | verified visually (rubble on the slab); the procedural dark box is kept on top |
| Spikes | 1327 / 1333 / 243 (holes in the surface), animated metal spikes kept | verified visually |
| StuckButton / DropButton / RaiseButton | 241+235+236 / 241+242+296 / 347+242+349; the marker box is dropped. Plates that are up (link timer <= 1) stand **0.03 = 10% of the slab thickness** above the floor (top face raised, bottom kept); pressed (timer > 1) they are at floor level and use the pressed look of the original (`A03F`/`A061`: drop button as tile 5, raise button as floor); stuck buttons always down. Visual only | raised plate verified visually (level 2 room 13) |
| LooseFloor | the **normal floor** texture (Floor's 241/242/243; procedural: stone), its own entity: `room::animate` moves it every frame from the trob state (`wobble`: knock shake 0x80..0x83 decaying over 4 ticks, about-to-fall 1..10 growing; one bounce per tick, roll changes side every tick, interpolated with the tick alpha). Falling slabs (mobs) use the same texture, drawn one tick behind and interpolated (y - speed -> y) | fall verified visually (level 1 room 1, scripted run); wobble amplitude small (0.05 up, 0.08 rad) |
| Pillar, LatticePillar | column of 295 (19 px wide dungeon, 16 px palace, cut-out), square, `TILE_H` tall | verified visually |
| Potion | plain floor slab plus **voxel bottle** (PRINCE.DAT chtab 1, palette 150: 162 small, 163 big; looked at in dumped PNGs: 162/164 and 163/165 are the same bottles) at x + 0.25 (draw_xh + 3), z 0.4 (fore piece, in front of the figures) and the **bubble frames 166..172** (1-bit 5x9) on its neck, animated by the logic tick (frame = (tick + tile index) mod 7; the sim does not run potion trobs), in an emissive material of the potion colour. Kind -> look [I, SDLPoP from memory, not traced]: 0 empty (no bubbles), 1 heal / 2 life red, 3 feather / 4 invert green, 5 hurt / 6 open blue; big bottle for 2..4. Not in VDUNGEON/VPALACE (record +9 = 12 only resolves to the palace balcony piece 1212) | verified visually (red, blue, green bubbles; small and big; classic and enhanced) |
| BigPillarBottom / BigPillarTop | 1288 / 1291 columns; the top cell has no floor slab (its record has no +8) | verified visually (level 2 room 5, level 5 room 7) |
| LatticeSupport | stone column | KEEP (record values are chtab-7 guesses) |
| Gate, DoortopWithFloor, Doortop | post 249 (8 px) at the right edge of the cell, back half; gate: procedural metal grille **turned 90 degrees about the vertical axis**, standing across the corridor in the y-z plane at x = 11.5/14 of the tile (the gate's collision span tl+10..tl+13, collisions.md 3), from the post's front face (z -0.15) to the floor front (0.45): 4 vertical bars along z, a bottom and a middle rail; slides up with the modifier as before | verified visually (level 1 rooms 5 / 1, game and debug camera); edge-on from the game camera it reads as a narrow grille |
| ExitLeft / ExitRight | frame 237 / 239 cut to 47 rows over a dark doorway, lintel 238 / 240 above | verified visually (both sets); each frame is drawn in its own column (the original draws +3 pieces one column to the right): guess |
| Wall | dungeon 368 (mod 0) / 370 (mod 1) on the block, one row tall (`TILE_H - SLAB` top, no overlapping faces); palace: procedural | dungeon verified visually; palace KEEP (1-bit overlays need the unknown fill routine) |
| SmallLattice, LatticeLeft, LatticeRight, Unknown31 | procedural | KEEP |
| cutscene chamber (`spawn_chamber`) | procedural | superseded: palace mapping, section 17 |

## 6. Open questions and limits

1. Slots +0, +7, +8 and +9 of the records (back or fore layer, position, flip) are not traced. Only +3 (image) and +4 (present flag) are verified.
2. Base 1200: resolved in 3b (loader `0C83:12FB`, ranges, flags). Still open: the exact rule that turns record value v into resource 200+v or 1200+v (the `ACDF` lookup was not traced).
3. Chtab 7 (base 360) users found: Wall (A9DD/AB4C, ids 8 or 10), the wall face behind empty cells (A222, id 2 = 362), and two routines at `0000:ABDA` and `0000:AC56` (ids 14..17 = 374..377, called from `0C83`-side code at C817, C757, C7EE; the caller was not identified). Chtab 7 is also the "small lattice" set (363..365) only through the record values 3..5, not traced further.
4. Balcony and Potion: 1210..1213 (base 1200 slot 7, palace levels) are the only ids the balcony records use for the palace; 370..373 is not used by the balcony path found so far. Resolved in part.
5. Ids in the DAT with no record reference: 232..234, 244, 245, 250..274, 299, 324..326, 344, 346, 348, 350, 351, 361..377, and the other extras. Their use is unknown. 250..267 probably belong to the ds:2780 sequence (section 3).
6. Palace-only ids (1201..1213, 1275..1283) mean the same record can resolve differently in each set. Re-check when base 1200 is traced.
7. Prefix letter: the adapter byte `ds:3035` picks the prefix from the `ds:2E0` table. In 105A the value 5 is the VGA case, so the "V" file is assumed. The shipped static value is 0; the runtime value comes from the config (DESKTOP.CFG), not checked.
8. Visual check: contact sheets exist (`dump_png`); see section 5 for the in-game check. Checked by eye: 241/243 (floor top surface and front lip), 292/293/295 and BigPillar 1286..1291 (column shapes), 368/370 (wall brick blocks). Not yet checked well: 246..249 (gate, sizes only), 260..267 (gate frames), 362 (wall face), 1-bit palace images 366..377 (need palette 360). Visual check of roles in section 5 is only partial.
9. Not every type's piece draw was traced. A299 (types 0, 9, 12 with a gate neighbour) and the other A332 branches were read only partly.

## 7. Reproduce

    cargo run -q -p pop-core --example dat_inventory -- <Original_data dir> VDUNGEON.DAT 200 <existing outdir>
    cargo run -q -p pop-core --example dat_inventory -- <Original_data dir> VPALACE.DAT 200 <existing outdir>
    tools/disasm/target/release/pop-disasm xref 0x2B6
    tools/disasm/target/release/pop-disasm dis 0000:0D8C 40
    cargo run -q -p pop-core --example dump_png -- <Original_data dir> <outdir>   # PNGs, contact sheets, wall modifier histogram

The example writes PPM files into the output directory and panics if it does not exist.

## 8. Second pass (deep dive; supersedes sections 2/3/5 where they differ)

Method: three independent disassembly passes (record consumption, chtab 7 and wall, all other draw call sites). Reports are by subagents; statements marked [V] cite addresses in PRINCE.EXE, but the main session did not re-verify them. Where two passes disagree it is noted.

### 8.0 Corrections to earlier sections

- **Value to resource rule [V]:** resource = base + v (base 200 for chtab 6). Record value 41 is resource 241 (floor face 32x13). The floor top 242 (26x15) is a literal 42 at `0000:A3E4`. Slots of base 1200 use the same index space and overwrite the base-200 entry (range check from `ds:031C/0324`).
- **Slots [V by read address]:** +0 `A7BC` (kind-indexed back piece), +2 `A7DB` (x offset of +0), +3 `A379` (main back piece, 0 = skip), +4 `A1D6` (floor lip), +5 x offset (`A3FB/A48B/A4EC`; **not** y), +6 palace-only second piece at x-27 px (`A517`), +7 `A278` (wall face, kinds 7/12 palace), +8 `A68D` (fore piece via AD70), +9 `AA35` (extra piece from A9DD, 0 = skip), +10 dy, +11 dx (`AA6C/AA78`). +1 is never read.
- **Draw tables [V]:** back = `ACDF` (`467A`, stride 7, cap 200), fore = `AD70` (`6132`, stride 7), mid = `AE02` (`3D78`, stride 16, cap 50). Frame order (`B8C0..B8F8`): objects, back, mid, objects, fore. Per cell (`9EBA`): `A1C2, A299, A332, A639, A700, A9DD`.
- **Balcony [V negative]:** the guess 370..373 was wrong. Balcony pieces use chtab 6 ids 10..13 which resolve to **1210..1213**, palace only.
- **Wall on palace VGA [V]:** `A9DD` skips the `AB4C` draw when palace and adapter 5 (VGA), so 368/370 are not drawn there by `A9DD`. The palace wall goes through `C357` (`C3E5` list path with RAM tables 4E38..4E59, writers not traced).
- **Lattice [V negative]:** lattice types 26..29 draw only slot +9 = 9, which is **1209** (palace only). The +0 values 1/3/4/5 (361, 363..365) are read by no traced code.
- **Torch:** an extra stub **346** is drawn at x-28 (`A537..A560`, literal 0x92). One pass ties it to types 19 and 30, the other to type 30 only.

### 8.1 Roles of the previously unreferenced ids

Sets: D = VDUNGEON, P = VPALACE. ids may be ranges.

| ids | sets | role | evidence | conf |
|---|---|---|---|---|
| 232 | D+P | ramp wedge, no user found | none | G |
| 233 | D+P | fringe strip | routine `B9EF` loop at `BAA6`, literal id; trigger `[5902]!=[40DE]` | V id, G trigger |
| 234 | D+P | fringe strip (teal bar) | `B9EF` loop at `BACD`, literal id | V id, G trigger |
| 244-245 | D+P | wall crack silhouettes (types 0/5 side path) | table `ds:27B2` at `A444` | I |
| 250 | D+P | rubble / torch base (types 1,3,5,19 when `[66D0]==0`) | word `276C` at `BB2E` | I |
| 251 | D+P | ledge fragment | literal 0x33 at `B507`, type unknown | G |
| 252-259 | D+P | 24xN crumble or door slice sequence (N = 8..1) | table `ds:2799` at `B56B`, index 0..8; 252 also literal at `B54B` | I |
| 260-267 | D+P | gate opening frames, idx = (min(mod,188)>>2)&7 | table `ds:2792` at `A311/A32B` | V |
| 268 | D+P | dark wedge beside a gate (types 0/9/12 when cell is gate) | literal 0x44 at `A2C3`, call `A2DA` | V |
| 284 | P | unknown piece at x-27, y+3 | literal 0x54 at `A4AB` | V id, G type |
| 299 | D+P | fringe piece | `B9EF` at `BA33`, literal 0x63 | V id, G trigger |
| 324-326 | D+P | wall decor (326 = lattice arch window), types 0/5 side path | table `ds:27AA` at `A3C3` | I |
| 344 | D+P | staircase-like fringe piece | `B9EF` at `BA63`, literal 0x90 | V id, G trigger |
| 346 | D+P | torch stub | literal 0x92 at `A549`, call `A560` | V code, I visual |
| 348 | D+P | ramp wedge, no user found | none | G |
| 350-351 | D+P | ramp wedge, no user found | none | G |
| 362 | D+P | wall face drawn next to empty cells (chtab 7 id 2) | `A222`, `A25E..A27E` | V |
| 363-367 | D+P | chtab 7 table `ds:27B6` (367,369,365,363,368,370,366,364,360,376) read by the type 20 path, exact role open | `A673` | I |
| 368, 370 | D+P | wall body for modifier 0 / 1 via `T27BA`; dungeon = brick blocks, palace = 1-bit strips (VGA palace path via `C357`, body open) | `A9DD -> AB3A`, `ds:27BA` | V dungeon, open palace |
| 369 | D+P | same table `ds:27B6` as above | `A673` | I |
| 373 | D+P | chtab 7 id 13 via `C5BF` (dungeon/non-VGA wall edge) | `C5BF` | V |
| 374-375 | D+P | chtab 7 ids 14/15, wall edge pieces | `ABDA` (`C817`) | V |
| 376-377 | D+P | chtab 7 ids 16/17, wall edge pieces (376 also via `C5BF`) | `AC56` (`C757`, `C7EE`) | V |
| 1201-1208 | P | decor in base-1200 slot 0, no per-type user | loader only | G |
| 1209 | P | lattice support piece (slot +9 = 9) | records 26..29 | I |
| 1210-1213 | P | balcony left/right pieces | records 23/24 values 10..13 | I |
| 1278-1283 | P | palace caps (table `ds:27A2` branch `cmp [4E0B],5`); the chtab 7 pass found no user, passes disagree | `A475` | I |
| 1301-1305 | D+P | chomper jaw body, 5 frames, k = `ds:2754[min(mod&7F,6)]` | `A7F3 -> A945`, ACDF at `A97D` | V code, I visual |
| 1306-1310 | D+P | probable potion branch (14x60), role not visual | `AACB`, AD70 at `AAE5` | I code, G role |
| 1311-1313 | D+P | chomper lower teeth, x-37/-47/-50 | ACDF at `A9D6` | I |
| 1314-1318 | D+P | chomper dark strip (1-bit), when `[4366]` bit 0x80 | test `A980`, ACDF at `A9AA` | V code, G visual |
| 1319-1323 | D+P | no user found (same series as 1314..) | none | G |
| 1328-1332 | D+P | spike extend frames, table `ds:2774` = 80,81,82,83,84,83,81,80 | mov at `A835` | V code, I visual |
| 1334 | D+P | spike tip | `A5B9`, `A5D6` | I |
| 1335-1343 | D+P | thin stick sprites, table `ds:2780` read at `AB23`; unclear | `AB23` | G |

Not covered by any pass (still unknown): 269-274, 345, 371-372, 361 (record value 1 unread), 1202..1208 beyond the decor guess, palace 1209 beyond lattice slot use.

### 8.2 Corrected tile-type rows (replace the rows of section 3)

| type | slot | v | VDUNGEON | VPALACE | conf |
|---|---|---|---|---|---|
| 1 Floor | +0 / literal 42 / +8 | 41 / 42 / 43 | 241 / 242 / 243 | 241 / 242 / 243 | V (rule 200+v, literal at A3E4) |
| 2 Spikes | +0 / +3 / +8, frames | 127 / 133 / 43 | 1327 / 1333 / 243, frames 1328..1332, tip 1334 | same | V frames (A835), I rest |
| 4 Gate | +0 / +3 / +7 / +8 / +9, frames | 46 / 47 / 48 / 43 / 49 | 246 / 247 / 248 / 243 / 249, frames 260..267, wedge 268 | same | V frames and wedge |
| 10 Potion | +0 / +3 / +8 / +9 | 41 / 42 / 43 / 12 | 241 / 242 / 243 (1306..1310 probable) | 241 / 242 / 243 / 1212 | I |
| 18 Chomper | floor + frames | 42 / 43 | 242 / 243, jaw 1301..1305, teeth 1311..1313, dark strip 1314..1318 | same | V code, I visual |
| 19 Torch | +0 / +3 / +8 | 41 / 42 / 43 | 241 / 242 / 243, stub 346 | same | I (passes disagree on 19 vs 30 only) |
| 30 TorchDebris | +0 / +3 / +8 / +9 | 97 / 98 / 43 / 100 | 297 / 298 / 243 / 300, stub 346 | same | I |
| 20 Wall | chtab 7 | mod 0 / 1 | 368 / 370 (brick blocks), face 362 next to empty cells, edges 373..377 | 368/370 are 1-bit strips; VGA path via C357, body open | V dungeon, open palace |
| 23 BalconyLeft | +0 / +3 / +7 / +8 | 41 / 10 / 11 / 43 | 241 / - / - / 243 (no balcony images in the dungeon set) | 241 / 1210 / 1211 / 243 | I |
| 24 BalconyRight | +3 / +7 / +8 | 12 / 13 / 43 | - / - / 243 | 1212 / 1213 / 243 | I |
| 26 LatticeSupport | +9 | 9 | none (1209 is palace only) | 1209; 361, 362 not read by traced code | I |
| 27 SmallLattice | +9 | 9 | none | 1209; 363 not read by traced code | I |
| 28 LatticeLeft | +9 | 9 | none | 1209; 364 not read by traced code | I |
| 29 LatticeRight | +9 | 9 | none | 1209; 365 not read by traced code | I |

### 8.3 Still open after the second pass

- Flip/mode bits (`AE02 +80h` via `[259Ch]`), the globals stored in mid entries, and the tables `ds:27AA/27AE/27B2/27A2/2792/27BA` beyond what is listed above.
- Trigger of the fringe routine `B9EF` (no near callers found) and the caller of `C357` (`A6F7`).
- Palace wall body (RAM tables 4E38..4E59 on the `C3E5` path, writers not traced).
- Ids listed in 8.1 as unknown, and the visual verification with palette 360 (no palette-360 render was made).

## 9. Runtime probe (DOSBox 0.74-3, marker substitution)

Method: tools in `tools/rt_probe/` (`pdat.py` DAT writer, `showcase.py` level builder, `harness.py` DOSBox driver; run from a scratch dir holding a copy of `Original_data`). In a scratch copy of `VDUNGEON.DAT`/`VPALACE.DAT` a batch of up to 14 target ids is replaced by raw same-size images with one marker colour per id (1-bit images: all ones). The start room of each level in a scratch `LEVELS.DAT` is overwritten with a random tile/modifier showcase. `PRINCE.EXE megahit N` runs in DOSBox, Ctrl+F5 screenshots (640x400, 2x) are diffed against baseline shots and changed pixels are attributed by marker colour. Output: 210 runs, 16 batches; full table in the (scratch) `RESULTS.md`. Conf marks here: **R** = observed at runtime.
Reliability: **dungeon** rows with "high" are reliable. **Palace** results are noisy: DOSBox output was RGB565, so the near-identical tan colours of palette 360 collide; only rows rated high with exact sizes (1-bit pieces) are used below. Attribution by neighbouring cell offset (dx/dy) is statistical because the showcase is random. "Not seen" means not seen in these scenarios, not unused. Valid levels: dungeon 1, 2, 3, 7, 8, 9, 12, 13; palace 4, 6, 10, 11, 14 (palace L5 starts in an unmodified room).

### 9.1 Confirmed or corrected roles (supersede 8.1)

| ids | sets | role | evidence | conf |
|---|---|---|---|---|
| 233-234 | D | exit door body slices (stacked 42x6) at ExitLeft cells, **not** fringe strips | 233 in 16/27 clusters at ExitLeft (F 0.86), 234 at ExitLeft (F 0.67); seen in 8 levels | R high |
| 346 | D+P | torch stub at the Torch / TorchDebris cell x0 | seen at both cell types; palace TorchDebris F 0.89 | R high |
| 361 | D | wall body for modifier 2 | 14/31 clusters at Wall cells (F 0.61) | R high |
| 365-366 | D | wall body variants (modifier 1 / neighbour variants) | Wall cells, F 0.47 / 0.40 | R medium |
| 367-368 | D | wall bodies | 368: 34/46 clusters at Wall (F 1.11); 367 F 0.64 | R high |
| 366-370 | P | 1-bit wall pieces drawn at Wall cells (sizes match exactly; twin sizes resolved with binary-coded runs: 366, 367, 368, 369 in L5/L11/L14) | exact size match at Wall cells | R high |
| 372-374 | P | wall edge pieces at Wall cells | F 0.43 | R medium |
| 1303-1305, 1308-1309 | D | chomper pieces (jaw and teeth frames) at Chomper cells; 1308 fits Chomper modifier 1 exactly | 1308 F 0.78 (18/18 clusters), 1304 F 0.82, 1303/1309 F 0.53-0.56 | R high (1304, 1308), medium (others) |
| 1306-1310 | D+P | chomper pieces, **not** a potion branch (supersedes 8.1) | 1308 high at Chomper; 1306 only seen once beside DropButton | R medium |
| 1311-1312 | D | chomper lower teeth | Chomper cells, F 0.40 / 0.53 | R medium |
| 1312 | P | chomper lower teeth | 12/12 clusters at Chomper (F 0.8) | R high |
| 1329, 1331, 1333, 1335, 1340, 1342 | D | spike pieces: 1333 base (35/35 clusters, F 1.11); frame sets 1329, 1331, 1340, 1342 drawn one column left of the Spikes cell; 1335 spike piece | Spikes cells, F 0.40-0.53 for the frame sets | R high (1333), medium (others) |
| 1327, 1333 | P | spikes base pieces | 1327 F 0.74 (13/17), 1333 F 0.55 | R high / medium |
| 1209 | P | lattice support piece | LatticeSupport/SmallLattice cells | R medium |
| 1313 | P | drawn at BalconyLeft cells (23x10) | 3/7 clusters, F 0.67 | R high |
| 272 | P | seen at StuckButton cells (a button with a modifier is unexpected) | 16/16 clusters, F 1.68 | R high, role open |

### 9.2 Seen but role unresolved (hints only)

Attributed to neighbouring cells by offset, statistical in a random showcase: dungeon 244 (near BigPillarTop), 250 (near Pillar), 251 and 253 (near Gate), 252, 265, 266, 344 (near LatticeRight/LatticeSupport), 270 (Unknown31), 299, 326 (near Skeleton/Torch). Palace: 232, 233, 250, 251, 1204-1206 (near Potion), 253, 263, 273, 326, 1343 (Mirror), 258, 1281, 1282 (Doortop), 269, 271, 345, 1318 (RaiseButton). Treat these as unconfirmed.

### 9.3 Not seen in these scenarios

- **Dungeon:** 232, 245, 254-261, 263, 264, 267, 269, 271, 273, 274, 325, 348, 350, 351, 364, 373, 376, 377, and the 1-bit strips 1314-1316, 1319-1321. Gate frames 260-267 and wedge 268 were not seen because closed and open gates use 246-249; the frames probably appear only while a gate is moving, which needs a button press (not tried).
- **Palace:** 234, 244, 254, 256, 259, 261, 262, 265, 268, 284, 299, 361, 363-365, 375, 376, 1202, 1203, 1211, 1213, 1283, 1301, 1314-1316, 1319-1323, 1331, 1332.
- **Not in the DATs:** palace 348, 1207, 1208, 1279, 1324-1326; dungeon 284, 345, 1201-1213, 1278-1283, 1324-1326.

### 9.4 Follow-ups

1. Rerun the palace probe with a 32-bit DOSBox output mode (see caveat above).
2. Trigger a gate with a button press to see 260-268 and the loose-floor crumble frames (252-259).
3. Balcony 1210-1213 and lattice 1209 checks are only partial (1210 medium; 1211 and 1213 not seen).

## 10. Runtime probe, round 2 (clean palettes, isolated showcase)

Changes against section 9: (A) in the scratch DATs palettes 200 and 360 were replaced by 15 well-separated colours (baseline and marker runs use the same palette), so the RGB565 collisions are gone and **palace results are now clean**. (B) controlled showcase: 7 sheets x 10 variants = 62 tile/modifier variants, each at a known cell (cols 1,3,5,7,9 x rows 0,2) with plain Floor in every other cell; dungeon = level 3, palace = level 10; 140 runs; each hit is attributed to the nearest placed cell ("background" = hit on a plain Floor cell). Limit: the background is Floor only, so ids needing Empty/Wall neighbours (wall side-edges, ramps, fringes, 269-274) were not triggered. Results are in scratch `RESULTS2.md`; tools in `tools/rt_probe/` (`v2.py`, `run2.py`, `obs2.py`, `agg2.py`, `gen2.py`). Section 10 supersedes sections 8.1 and 9 where they differ.

### 10.1 Roles by runtime observation

| ids | sets | role | evidence | conf |
|---|---|---|---|---|
| 250, 267 | D | pieces of a statically open gate (Gate modifier 1); 267 is the last opening frame, the gate drawn open uses 250 + 267; closed gate uses 246, 247, 249 (248 never seen) | 2/2 clusters at the Gate cell | R high |
| 251-253 | D | left-edge pieces (x0 = 0) seen at Gate modifier 0 cells | single clusters | R medium |
| 251, 253, 256, 274 | P | pieces drawn on plain Floor cells (floor-side edge pieces, at the left room edge), seen on every Floor | 14/15 clusters (251, 253), 256 and 274 high | R high |
| 1280 | P | drawn on plain Floor cells | 14/14 clusters | R high |
| 1202 | P | drawn on a Floor cell | 1/1 cluster | R medium |
| 363, 367 | D | 32x3 floor strips drawn on plain Floor cells (**supersedes** 9.1 "367 = wall body") | 363: 36/49 clusters; 367: 7/7 clusters | R high (367), medium (363) |
| 324, 371, 372 | D | drawn on plain Floor cells | 324: 7/14, 371: 8/15, 372: 8/28 clusters | R medium (372 low) |
| 361 | D | drawn on plain Floor cells (**supersedes** 9.1 "wall body for modifier 2") | 14/27 clusters | R medium |
| 361 | P | wall body | Wall mods 0,2,3,4,5,7; 11/11 clusters | R high |
| 284 | P | wall piece (8x7), no longer unknown | Wall mods 0, 2, 4; 3/3 clusters | R high |
| 366-367, 369-372, 374 | P | 1-bit wall strips at Wall cells (exact sizes; twins separated by batch); 368 and 373 medium | Wall cells, mods 0,2,3,4,5,7 | R high |
| 368 | D | wall body | Wall mods 2-7 (6/7 clusters) | R high |
| 370 | D | wall body for modifier 0 | Wall mod 0 | R medium |
| 232, 234, 362, 375 | D | wall pieces seen at Wall modifier 3 (single hits) | 1 cluster each | R medium |
| 1301-1313 | D+P | **Chomper** (type 18): the pieces depend on the modifier: mod 1 and 0x81 draw 1303, 1308, 1311; mod 0, 5, 6 draw 1304, 1309, 1312; mod 2 draws 1301, 1307; mod 3 and 4 draw 1302 (palace mod 3), 1305, 1310, 1313. So 1306-1310 are chomper pieces in both sets (not potion) | all high or medium by cell, 1303/1304/1308/1311/1312 high | R high |
| 1327-1343 | D+P | **Spikes** (type 2): 1333 base at every modifier; mod 0 draws 1328, 1339; mod 1 and 6 draw 1329, 1340; mod 2 draws 1330, 1341; mod 2 and 7 draw 1335; mod 4 and 5 draw 1331, 1342; mod 4 also 1332, 1343, 1337; mod 5 draws 1338; mod 1 draws tip 1334; 1327 is a base piece seen at a Floor/Spikes cell | spikes cells | R high (1329, 1331, 1333, 1335, 1337, 1340, 1342), medium (rest) |
| 1201, 1203, 1204, 1205 | P | lattice: 1201 LatticePillar, 1203 LatticeSupport, 1204 SmallLattice, 1205 LatticeLeft (**1209 is only a low hit**, supersedes 8.2) | 1203 high, others medium | R high (1203), medium |
| 1209 | P | lattice, low | 1/4 clusters | R low |
| 1210 | P | BalconyLeft | 1 cluster | R medium |
| 1278 | P | at a DoortopWithFloor cell (single hit) | 1/1 cluster | R medium |
| 346 | D+P | torch stub at Torch cell (TorchDebris also) | confirmed again | R medium |
| 270 | D+P | at TorchDebris cell | 1/2 clusters | R medium |
| 233 | D+P | exit door slices at ExitLeft/ExitRight (mods 0, 12, 43) | confirmed | R high (palace), medium (dungeon) |

### 10.2 Corrections to earlier sections

- **Dungeon 361 and 367** are not wall bodies; they are drawn on plain Floor cells. Dungeon wall bodies are 368 (mods 2-7) and 370 (mod 0). Palace 361 is a wall body.
- **Palace lattice** is 1201 / 1203 / 1204 / 1205, not 1209 (section 8.2). Balcony 1210 holds.
- **Gate:** an open gate draws 250 + 267 (not only the 260-267 frames in motion). 248 was never seen.
- **Palace palette collisions** are resolved; the section 9 palace caveat no longer applies to round-2 rows.

### 10.3 Not seen in these scenarios (not "unused")

- Dungeon: 244, 245, 254-266, 268, 269, 271-274, 299, 325, 326, 344, 348, 350, 351, 364, 373, 374, 376, 377, 1314-1317, 1319-1322, 1336.
- Palace: 232, 245, 254, 255, 257-269, 271-273, 299, 325, 326, 344, 350, 351, 362-365, 375-377, 1206, 1211-1213, 1281-1283, 1314-1317, 1319-1322, 1336.

### 10.4 Moving states (not achieved)

Gate frames 260-267 in motion, the loose-floor crumble (252-259) and the exit-door opening animation were not observed: with the scripted keyboard (SendKeys RIGHT interleaved with Ctrl+F5) the kid only turned or stood still. Open question: use hardware scancodes (SendInput with KEYEVENTF_SCANCODE, extended flag for arrows) or time the screenshots separately. Also open: a wall-neighbour grid (Empty/Wall neighbours) to trigger side edges, ramps and fringes (244/245, 324-326, 299, 344, 348-351, 269-274). The kid walks left on his own after about 4.5 s in dungeon level 3.

## 11. Runtime probe, round 3 (neighbour sheets; input not solved)

Method: 12 dense random sheets per set (Empty / Wall mods 0-7 / Floor / Pillar / Gate / Doortop / Mirror / Torch / lattice neighbours), only still-unseen ids marked; each hit is reported with its tile type and left/right/up/down neighbours (not a full combinatorial grid). Hits shared by 3 or more different ids at one position were dropped (marker colours coincide with the kid's white sprite pixels). Tools: `inp.py`, `v3.py`, `obs3.py`, `gen3.py`, `c3.py` in `tools/rt_probe/`; results in scratch `RESULTS3.md`.

### 11.1 Roles (supersede 10.1 where they differ)

| ids | sets | role | evidence | conf |
|---|---|---|---|---|
| 373, 374, 376, 377 | D | wall edge pieces drawn at the left of a Wall cell (373: 7/7 clusters, 374: 8/14; 376 and 377 few clusters, 377 only at Wall mods 7 and 2) | Wall neighbour sheets | R high (373), medium (374), low (376, 377) |
| 364 | D | drawn at Wall cells with modifier 2 or 7 | Wall cells | R medium |
| 362 | D+P | wall face piece on Empty cells next to Empty or Wall (matches the static `A222` finding) | both sets | R high |
| 1210 | P | BalconyLeft | 23/24 clusters | R high |
| 1212 | P | BalconyRight | 6/7 clusters | R high |
| 1278 | P | at DoortopWithFloor cells | 4/6 clusters | R high |
| 1209 | P | next to LatticeLeft / SmallLattice | medium | R medium |
| 345 | P | generic floor-edge piece (30 clusters at a constant offset 0/+26 under many tile types) | 30 clusters | R medium |

Zufallsfunde: 260 and 1281 appear in the left 32 px (the left neighbour room's column); 244, 245 and 254 hit one Wall/4 cell with a LatticePillar on its left and a Sword on its right in a single dungeon sheet (low).

### 11.2 Moving states and input

Not achieved. Scancode input with the extended flag works only if the DOSBox window is re-focused before every key sequence and every Ctrl+F5; pure scancode events do not trigger the screenshot hotkey. The kid turns and runs, but the movement is erratic, so no clean 3-tile walk, Shift-run or jump was validated. Gate links (DropButton/RaiseButton/StuckButton to a gate, shots every 0.2 s) never changed the gate region, so either the kid never triggered the plate or the link table is wrong. Unobserved at runtime: gate frames 260-267 in motion and wedge 268, loose-floor crumble 252-259, exit-door opening, potion and sword pickups. **The gate frames 260-267 and wedge 268 are verified statically** (`A311/A32B`, `A2C3`, section 8.1); the crumble sequence 252-259 is only inferred from table `ds:2799`.

### 11.3 Still without a runtime role

Dungeon: 244, 255-266, 268, 269, 273, 299, 325, 326, 344, 350, 351, 1314-1317, 1319-1322. Palace: 232, 245, 254, 255, 257-259, 261, 263-266, 268, 269, 271-273, 299, 325, 326, 344, 351, 363-365, 375-377, 1206, 1211, 1213, 1282, 1283, 1314-1317, 1319-1321. All "not seen in these scenarios". Likely reasons: the gate and crumble frames need motion; the 1-bit chomper strips 1314-1323 may blend into the background colour; the rest need a trigger not covered yet (fringes, cracks, torch-head variants, ramp edges, specific palace wall modifier plus neighbour combinations). For several of them section 8.1 holds a static role with [V]/[I] marks (e.g. 260-268 gate, 299/344 `B9EF` fringe literals, 324-326 side-path table `ds:27AA`, 244/245 `ds:27B2`).

## 12. Static pass 3 (two disassembly passes; supersedes 8.1/11 where they differ)

Sources: two independent passes over PRINCE.EXE v1.1 (fringe/wall/palace tables, and crumble/strips/call-site sweep). Raw call-site tables are in the scratch folder `sweep\`. [V] = read in code bytes, [I] = inferred, [G] = guess. Runtime marks are from sections 9-11.

### 12.1 Roles

| ids | sets | role | evidence | conf |
|---|---|---|---|---|
| 233 | D+P | exit door body: stacked 42x6 slices drawn in a loop for **ExitLeft only** (type 16) | routine **`B9F1`** (the "B9EF" of 8.1 was mid-instruction), loop `BA8A..BAAF`, dispatched from `A608` in the type switch `A57D` | V code, R |
| 234 | D+P | exit door piece, one draw at x-64, row+1 | `BAB5` | V code, R |
| 299 | D+P | exit frame piece (ExitLeft), unconditional, x-13, row+1 | `BA33` | V |
| 344 | D+P | exit extra piece at x-17, row+1, drawn only if `[5902] != [40DE]` and column != 0; `[5902]` is written 2 at `4A29` | `BA36..BA45` | V code, G role |
| 244-245 | D+P | wall crack silhouettes (side path): id = 200 + `ds:27B2[c]` with c = column if <= 3, else 0 (c 0,1 -> 244; c 2,3 -> 245); dungeon skips c = 0, palace skips c = 1; x-20, class 0 | `A444` | V id, I type |
| 324-326 | D+P | side-path decor: 324 and 325 at x-20, 326 (lattice arch window) at x 0; id = 200 + `ds:27AA[si]`, si = 1..3 | `A3C3` | V data, I condition |
| 1278 | P | palace cap, column 0 | `A475`, `ds:27A2` | V |
| 1280 | P | palace cap, column 1 | `A475` | V |
| 1282 | P | palace cap, column 2 | `A475` | V |
| 1281 | P | palace cap, column 5 | `A475` | V |
| 1283 | P | palace cap, column 6 (columns 3-4 and 7-8 draw none; column 9 would use 1324, not in VPALACE) | `A475` | V |
| 284 | P | palace piece at x-27, row+3 (when column bit 80h is clear) | `A4AB` | V id, R |
| 251 | D+P | cap of the gate bar column (15x10, back at `B507`, fore at `B5AD`), at x = `[43AA]`-2 | shared column code `B4A0..B586`; `B4A0` is the type-4 (Gate) record path; runtime: 251 at Gate cells | I (runtime R medium) |
| 252-259 | D+P | **gate bar column slices**, not a loose-floor crumble: 252 = full 24x8 slice drawn every 8 px upward from `[43AA]`-12; 253..259 = partial 24xN slice (N = 7..1) in the 8-px band at the threshold `[4FCA]`, via `ds:2799` = 267, 259..252 | `B54B`, `B57F` | I (runtime R medium at Gate cells for 251-253) |
| 1334-1338 | D+P | spike extend pieces: `ds:2780` = 1335, 1336, 1337, 1338, 1337, 1335, 1334 (1334 = tip); reader not traced | table | I (runtime R) |
| 1339-1343 | D+P | thin stick cycle: `ds:2788` = 1339..1343 and back, index 0 none (8-frame extend/retract), read at `AB23` | table | I (runtime R) |
| 1314-1318 | D+P | chomper blood/drip strip, drawn when cell modifier bit 7 (`[4366]` & 80h) is set; the id source table is not traced (the literal `4Ch` at `A9AA` is 276, not an image) | `A980` | I |
| 1319-1323 | D+P | drip continuation (2xN), no static user | none | G |
| 363, 365, 367, 369 | D+P | wall **back** pieces (32x3): `ds:27B6[mod&7F]` = 367, 369, 365, 363, 368, 370, 366, 364 for modifiers 0..7 (static); the runtime saw 363, 367 on plain floor cells | `A639` | V code, **conflict with runtime** (see 12.2) |
| 368, 370, 366, 364 | D+P | wall **fore** pieces: `ds:27BA[mod]` = 368, 370, 366, 364, none, 376, 377 for modifiers 0..6 (palace VGA: back and fore are skipped) | `A9DD -> AB3A` | V code, R |
| 373 | D+P | wall edge via `C5BF` for modifier 3 | `C60D` | V code, R |
| 374-375 | D+P | wall edge pieces: id = 374 + (k&1), x = `[4390]` - `ds:2800[k]` (58, 41, 37, 20, 16, 52, 42, 31) | `ABDA` | V code |
| 376-377 | D+P | wall edge via `AC56` and fore pieces for modifiers 5, 6 | `AC56`, `ds:27BA` | I |

### 12.2 Conflicts and findings

- **Wall back/fore table vs runtime:** static says modifier 0 = back 367 + fore 368, modifier 1 = back 369 + fore 370. The runtime probe saw 368 at modifiers 2-7, 370 at modifier 0, and 367/363 on plain floor cells. Not resolved: the runtime attribution (nearest placed cell) may mix the neighbouring Floor with the wall.
- **Loose floor:** the type 11 record has no image bytes and no type-11 draw path was found; the original's loose-floor image sequence is **not established** (the port draws Floor plus its own wobble). The earlier crumble attribution of 252-259 is withdrawn: they belong to the gate bar column.
- **Palace wall (VGA):** the back and fore wall draws are skipped for palace + VGA; the wall body goes through `C357 -> C3E5` with RAM tables `4E38..4E59`; the writers of these tables are still not traced.
- **Value ambiguity:** small record values (6, 11, 12, 13) collide with chtab numbers in xref sweeps, so users of 1206, 1211, 1213 cannot be separated from them by xref alone (1212 is BalconyRight +3 / Potion +9 by record).

### 12.3 Ids with no user found (static or runtime)

232 (value 32 is the common flag literal), 269-274 (only unverified literal candidates: 45h at `44E3`, 46h at several `C4E5..C5B1` sites, 47h at `60A6`, 49h at `68A1`, 4Ah at `434A`), 345 (palace: runtime generic floor edge, R medium), 348 (chomper-side candidate at `A7AD`, G), 350 and 351 (compares only), 1206, 1211, 1213. Open next: the loose-floor draw (if any), the palace wall RAM table writers, the neighbour-pattern table in `C357/C5BF/C3E5`, the 6th ACDF argument (0x10 vs 0), and the id source of the chomper strips.

## 13. Implementation status in room.rs

(Numbered 13 because sections 11 and 12 already exist.) Pure mapping: `crates/pop-app/src/tiles.rs` (`tile_images` = floor + main piece, `pieces` = the pieces below, `palace_wall`), unit tests with the ids of this table; placement in `room.rs`. Resource rule: `1200 + v` if present, else `200 + v` (8.0). Missing images keep the procedural look per piece; `tile_textures=procedural` / `POP_TILES=procedural` is unchanged. Load: both sets in about 1.2 ms, 353 KB RGBA; release frame time 1.2-1.3 ms (POP_PERF, unchanged).

| tile | textured with | verdict |
|---|---|---|
| Wall, dungeon | face 368 (mod 0) / 370 (mod 1) on the one-row block, top strip 367 / 369 (`ds:27B6`, static V) as a 0.01 cap on the block top | looks right; the 367/369 role follows 12.1 (static) against the runtime "floor strip" reading (12.2); edge pieces 373..377 and the face 362 are left out (random-mark rule and 2D perspective pieces, the 3D block has its own sides) |
| Wall, palace | built faces: 4 courses of 15 rows in palette 360 colours 1..4 / 6..9 (random per brick, as SDLPoP's generate_wall_pattern, from memory), mortar and joints in 14, light edge 11, one 1-bit strip 366..371 per course as grain in 14; 8 variants, chosen per room and tile | reconstruction, convincing at game distance, not the exact original pattern (course heights and strip positions are guesses); 361, 372..374, 284 not used |
| Chomper | blade 1301 + k, front slice 1306 + k (in front of the figures), upper teeth 1311..1313 (k >= 2, bottom 24 / 34 / 37 px above the floor), k = ds:2754 [3,2,0,1,4,3,3][min(mod & 7F, 6)]; floor rows cut, the 47 px stretched to the slab above | looks right (closed, open, both sets); blood strips 1314..1323 left out (id source not traced) |
| Spikes | floor 1327 / 1333 / 243 as before; back 1328..1332, right piece 1334..1338 in the next column, front 1339..1343, frame = modifier (>= 0x80: 5), tables after ds:2774 | looks right in both sets; the right and front series order is inferred from the back table |
| Gate | unchanged: post 249 at the right edge, 3D grille across the corridor | 246 / 247 / 248, 250..259 (bar slices) and 260..268 (perspective frames, wedge) not used: the 3D grille replaces them |
| ExitLeft / ExitRight | frames and lintels as before; door panel = 234 at the bottom edge (modifier px above the floor) plus 233 slices up to the lintel, centred on the doorway between the 237 / 239 posts, ExitLeft only (sheet); stairs 344 behind the door when the room is not the level's start room (SDLPoP rule from memory) | looks right (closed, half open with stairs); 299 left out |
| Torch / TorchDebris | stub 346 on the back wall under the flame | dungeon right; palace 346 shows a blue lattice strip beside the holder: doubtful |
| BalconyLeft / Right (palace) | window 1210 / 1212 (floor rows cut) on the back wall, arch 1211 / 1213 above | looks right (level 6 room 18); 1313 at BalconyLeft left out (chomper id, role unclear) |
| LatticeSupport / SmallLattice / LatticeLeft (palace) | arches 1203 / 1204 / 1205 on the back wall, standing on the floor (1204 / 1205 are R medium, taken so the arcade is uniform) | plausible; LatticeRight (no image) keeps the procedural plate and stands out; LatticePillar keeps column 295 (1201 unused) |
| Floor extras | none added: 251 / 253 / 256 / 274 (gate column pieces per 12.1), 363 / 367 floor reading, 1202, 345, palace caps 1278..1283 (trigger not traced) | left out |
| LooseFloor | normal floor + wobble as before; 252..259 belong to the gate (12.1), so no crumble frames | unchanged |

## 14. Voxel columns

`column_style` = voxel (default, only with `tile_textures` = original; `flat` / `POP_COLUMNS=flat` keeps the image on a box). Code: `voxel.rs` (`VoxelModel::revolve`, `block`), `items.rs` (`column_sprite`, `column_model`, `COLUMN_IDS`), placement in `room.rs` (`voxel_column`). One mesh per (set, image), vertex colours from palette 200, the item material (lit and shadowed like the rooms), jitter 0.02.

| image | sets | model | placement |
|---|---|---|---|
| 295 pillar (Pillar, LatticePillar) | P | round: per row the opaque run through the axis (most common row centre) revolved; left / right half-widths from the image (ellipse when they differ), depth radius their mean; voxel colour = the pixel of its image column (orthographic cylinder: x = r sin a), so the front is the sprite and the back the same band, darkened by the mesh; 2 floor rows cut | image 8 px into the tile (295 lines up with the column in 292 there), base on the floor, top under the slab above |
| 295 pillar | D | square block: the image is a 13 px front face (colour 5) and a 6 px right side face (colour 3) of a stone pillar; a 13 x 13 px block, front = left part, right side = right part stretched over the depth | same |
| 1288 / 1291 big pillar bottom / top | D+P | round, 4 floor rows of 1288 cut (palace 1291: 3 stray px beside the capital dropped) | 8 px in; the bottom reaches the top of its tile, the top starts there (no floor in BigPillarTop): one continuous column, shafts line up (both at image x 6..16) |
| 249 door post (Gate, Doortop) | D+P | round (7 px gradient) | right edge of the cell, axis at z -0.275, the gate grille starts in front of it |

Column axis depth z = -0.2 (figures at 0.25, widest base radius 0.36 ends at 0.16). Side pieces 292 / 293 / 294 / 1286 / 1287 / 1289 / 1290 are the 2D perspective parts of the same columns and stay unused. The flat style drew 295 etc. on a square box whose side faces showed the image across the depth (the thin strips beside the columns); gone with voxel. Test `column_front_projection_equals_sprite`: front projection = the prepared image (block: the front face) for all 8 images. Build: 8 models, 23.8k faces, 4.7 ms at startup (release); frame time unchanged (POP_PERF level 5 classic 1.24-1.33 ms voxel vs 1.24-1.31 flat; enhanced room 13: 5.2-5.3 both). Rule for block vs round: an image whose top row reaches its left edge is a block (only the dungeon 295). Not verified: whether the palace 1291 capital (two flat bands) is a square abacus; it is revolved.

## 15. Voxel objects

`object_style` = voxel (default, only with `tile_textures` = original; `flat` / `POP_OBJECTS=flat` keeps the flat images and procedural boxes of section 13 for A/B; `procedural` tiles keep all old placeholders). Code: `voxel.rs` (`revolve_runs`, `extrude`, `plate`, `sheared`), `items.rs` (`load_objects`, `settle`, `spike_back_only`, `stack`, `debris_sprite`, `panel_slices`, `flame_frame`), placement in `room.rs` (`B::object`, `B::debris`, `exit_voxel`). One mesh per (set, image or composite), vertex colours from palette 200 (flames: PRINCE.DAT palette 150), built at startup; animated objects swap prebuilt meshes: spikes, chompers and the exit panel through the existing rebuild on `world.version` (the modifier changes), flames per frame in `room::animate` by logic tick. A piece whose images are missing keeps its section-13 look. Scale: 1 px = 1/32 unit across and in depth, TILE_H/63 up.

| object | ids | model | placement | verdict |
|---|---|---|---|---|
| Spikes | back 1328..1332 minus the spikes the fore frame repeats (bottom-aligned), right 1334..1338, fore 1339..1343; frame by modifier as section 13 | every 8-connected spike dropped to the bottom row and stood upright over its base, each row run revolved about its own centre (round rods, as deep as wide, pointed where the image is) | three rows on the floor (1 px sunk): fore z 0.4 (in front of the figures at 0.25), back-only z 0.05, right piece z -0.3 moved 6 px into the own tile (32 - 26 px floor shear) | reads as a bed of 3D spikes in both sets, kid stands / lies among them (voxel style; the sprite-style kid quad at z 0.6 is always in front, as before); floor 1327/1333 unchanged; the 2D lean is dropped |
| Chompers | blades 1301..1305 (13/15 floor rows cut), upper teeth 1311..1313, frame k = ds:2754 as section 13 | bevelled plate (rim 1 voxel, 3 inside), sheared: image column x stands round((31 - x) * 0.6) voxels forward | the 2D blade is a sheared view of a plate through the depth (the fore slice 1306..1310 = its left 14 px redrawn over the figures), so the plate stands diagonally: column 13.5 at z 0.25, left edge near the floor front, right part behind; stretched to the slab above as before, teeth lifted as before | closed, half and open frames checked in both sets, the kid is between the blade's front and back part; fore slices no longer used; blood strips still left out |
| Exit doors | frame 237 / 239 (47 rows) with lintel 238 / 240 on top, panel = n slices 233 on bottom piece 234 (n = `panel_slices(mod)`, 8 models), stairs 344 | frames: plate cap 3 (up to 7 voxels) + relief by luminance (bright ornament up to 2 voxels out); panel plate cap 1 + relief; stairs plate cap 2 | frame plane z -0.33 (front about -0.16), panel z -0.4 recessed between the posts and sliding up by the modifier (its top hides inside the lintel), stairs z -0.5 (not in the start room), dark doorway box -0.62..-0.58 | recessed slab look confirmed (closed, half open with stairs, palace); gates and doortops unchanged (the 3D grille and the voxel post 249 already were 3D; no voxel bars) |
| Torches | holder 346 (rod through the cup via `column_sprite`, palace lattice strip dropped), flames PRINCE.DAT 151..159 | holder revolved; flames `revolve_runs`, unlit material, vertex colours x1.5 linear (glow, bloom in enhanced) | holder axis z -0.77 (wall face -0.9), flame stem (image x 7.5) on the holder axis, 1 px into the cup; light unchanged; flame and holder cast no shadow | frame = (tick + per-tile phase) mod 9 (original: one step per tick from a random start, rare random jumps: shortcut); there was no per-torch light flicker to keep |
| Debris | 297 / 298 / 243 against 241 / 242 / 243 (both unsheared with `floor_pixels`), Debris and TorchDebris (also where a loose floor landed, the sim turns it into Debris) | the differing pixels seen from above, extruded upwards: layers min(edge distance - 1, 3) + 1 | laid on the plain floor: image rows over the tile depth (row 0 at the back), x over the tile width, 1/32 unit per layer | low rubble heaps in both sets; 300 (TorchDebris +9) not used; the skeleton keeps its relief from section 5 |

Tests: `object_front_projection_equals_sprite` (all object models of both sets plus the flames: front projection = the prepared image), `object_tables` (panel slices by modifier, flame frames), `spikes_settle_and_lose_the_fore_spikes`, `rods_plates_and_shear_keep_the_front`, `object_style_parse_env_and_fall_back`. Startup: 72 object models + 9 flame frames, about 16 ms (release), 111.6k faces in total (one room shows a few of them). Frame time (POP_PERF, level 1 room 13, release): classic 1.26-1.32 ms voxel vs 1.24-1.32 flat; enhanced 5.3-5.4 vs 5.7-5.8. Not verified: the impaled pose (mod 255) seen from the debug camera, chomper motion over consecutive ticks (static modifiers were used), exit opening in a live run (modifier set by hand).

## 16. Potions verified

Traced in PRINCE.EXE v1.1 (static analysis; no DOSBox run). Tile 10 modifier = kind << 5 | bubble frame (load_alter_mod `B6AB`).
Images are PRINCE.DAT chtab 1, resource = 150 + image number.

| kind (mod >> 5) | potion | bottle dungeon / palace | bubbles (EGA colour) |
|---|---|---|---|
| 0 | empty | 162 small / 164 | none |
| 1 | heal | 162 small / 164 | 12 light red |
| 2 | life | 163 big / 165 | 12 light red |
| 3 | feather | 163 big / 165 | 10 light green |
| 4 | invert | 163 big / 165 | 10 light green |
| 5 | hurt | 162 small / 164 | 9 light blue |
| 6 | open (level 15) | 162 small / 164 | 9 light blue |
| 7 | (never in the data) | 162 small / 164 | 12 light red |

- Bottle **[V]**: draw_tile_fore `A9DD`, potion part `AA45..AAAA`: image = tile_table fore id (ds:25B0 + 12 * 10 + 9 = **12**), **13** when
  mod & 0xE0 is in 0x40..0x9F (kinds 2..4); **+ 2** when tbl_level_type[current_level] (ds:01CE, levels 4, 5, 6, 10, 11, 14) is set
  (`AA8E`); add_foretable `AD70`(chtab 1, id, xh + fore_x **2**, xl **6**, y = draw_main_y + fore_y **-3**, blitter 0x10). Sizes 10x11 small,
  10x15 big; the palace bottles 164 / 165 have the same sizes but other pixel indices (other glass and liquid colours).
- Bubbles **[V]**: draw_tile_anim `A7F3`, potion part `A857..A8E5`: colour 12, small; kind 2: big; kinds 3, 4: colour 10, big; kinds
  5, 6: colour 9, small; kind 0: return (no bubbles). Mask image **23** (173, 5x9) with blitter 0x40 (mono, colour 0) via `ACDF`, then
  frame image ds:27BE[mod & 0x0F] (frames 1..7 -> images **16..22** = 166..172, 5x9) with blitter **0x40 + colour** via `AD70`, both at
  xh + **3**, xl **1**, y = draw_main_y - 14 - 4 * big. So the bubbles stand 3 px right of the bottle's left edge and their bottom row
  is right above the bottle's top row. The role of the mask (back table, clears the neck under the bubbles) is **[I]** (SDLPoP naming).
- Bubble timing **[V]** (tiles.md 8.2): start frame prandom(6) + 1 on room entry (`8E3F`), animate_potion `89DD` -> `8CA5`: frame + 1,
  8 -> 1, once per tick while the potion's room is drawn. The port: frame (tick + tile index) mod 7 (fixed phase instead of random).
- Colour RGB **[I]**: the mono blit colour is a raw palette index; palette 150 has grey / black / beige at 9 / 10 / 12, so the bubbles
  must use the base EGA entries (as the flash code does, edge_cases.md 3): 12 = (63,21,21), 10 = (21,63,21), 9 = (21,21,63).
- Pickup **[V]** (edge_cases.md 5.1, unchanged): kinds 1..6 = heal, life, feather, invert, hurt, open; the colour table above agrees
  (heal / life red, feather / invert green, hurt / open blue).
- Data: kinds in levels 1..14: 1, 2, 3, 4, 5 (level 2 room 13: hurt + heal; level 9 rooms 7 / 10: invert; level 7 room 1: feather).
  LEVELS.DAT 2014 stores pre-shifted bytes (32, 64, 160) in unused rooms; `<< 5` wraps them to 0 (empty), as in the original.

Port (items.rs `potion_look`, `bottle_image`, `POTION_X` = 22, `BUBBLE_DX` = 3): the kind -> size / colour mapping already matched.
Fixed: palace levels now use 164 / 165; the bottle stood 8 px into the tile (now 22), the bubbles were centred (2.5 px, now 3) and
overlapped the bottle top by one row (now on top). Tests `potion_kinds` (the table) and `level_potions_map_to_images` (every potion of
levels 1..14 maps to a bottle of the expected size). Screenshots (classic + enhanced): level 2 room 13 blue hurt next to red heal,
level 9 room 7 green invert (big), level 4 room 23 palace life bottle (165) with red bubbles. Not verified: on-screen RGB in DOSBox,
the open potion (level 15 is not reachable with POP_LEVEL).

## 17. Cutscene set (princess chamber)

All cutscenes (intro / attract, levels 2, 4, 6, 8, 9, 12, time out, ending) share one set built at `cutscene::ORIGIN`
(docs/cutscenes.md 6.2). It now uses the same mapping and builders as a **palace** level room (VPALACE, set index 1),
honouring `tile_textures`, `column_style` and `object_style`; lighting is the set's own torch lights in both render modes.
Code: `room.rs` `chamber_builder` (the room builder with the palace `TileSet` and voxel items, entities tagged `Chamber`),
`chamber_column`, `chamber_holder`, `chamber_flames`; placement in `cutscene.rs` `setup` / `render`. Composition, camera
and timing are unchanged; every added piece keeps the old box when its image is missing.

| element | was | now (original tiles) |
|---|---|---|
| side walls (row 1, cols 0 / 9) | stone boxes | built palace wall faces (section 13), variant per tile of room 1 |
| floors (row 2) | stone slabs | palace floor texture 241 / 242 / 243 |
| lattice row (row 0) | metal plates | arches 1203 / 1204 / 1205 on the back wall (LatticeRight has no image: procedural plate, as in the levels) |
| arcade columns (LatticePillar, cols 3 / 8) | stone boxes | column 295, voxel (round) or flat as `column_style` |
| column in the bed gap (px 68..77) | 10-px stone box z 0.25..0.5 | column 295 at px 72.5, axis z 0.25 (front behind the bed at 0.52) |
| front pillar piece (PV 952 at px 240, in front of the actors) | stone box z 0.8..1.2 | column 295 (16 px, as wide as PV 952) at px 248, axis z 1.0 |
| torch holders (px 101 / 219) | dark boxes | holder 346: voxel (object_style voxel, no shadow) or the flat image on the back wall, cup at the flame's bottom line 116 |
| torch flames | PRINCE.DAT 151..159 sprites | object_style voxel: the levels' voxel flame meshes, frame = the cutscene's own `torch[i]` (get_torch_frame), stem on the holder axis; flat: the sprites as before |

Not mapped (current look kept): the night-sky opening and its stars (an arched door in PV 951, no tile piece; a balcony
1210 / 1212 would need a layout change), the back wall slab, the hourglass, sand stream, bed (PV sprites), actors, fade and
flash. `tile_textures=procedural`: identical to before (pixel diff 0 for intro, 2, 8, end, timeout in classic and enhanced).
Known look: the voxel column in the bed gap is narrower than the 10-px gap, so the palace floor shows through it (the old box
filled it). Test: `cutscene::tests::chamber_pieces_use_the_palace_mapping`.
