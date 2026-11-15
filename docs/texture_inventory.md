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
| Floor, Sword, Balcony, Potion, Torch, Chomper, Mirror, Lattice*, Doortop* floors | floor slab with a 32x16 texture: the unsheared top surface (241 row runs + 242 row runs without the end face) over the lip 243; same texture on top and sides | verified visually (both sets). Doortop (12) has no floor piece in its record (+8 = 285); its floor slab is kept and gets the plain floor: guess |
| Skeleton | 1230 / 1231 / 243 (dungeon); palace has no 1230 -> plain floor | composite built; not looked at in a screenshot |
| Debris, TorchDebris | 297 / 298 / 243 | verified visually (rubble on the slab); the procedural dark box is kept on top |
| Spikes | 1327 / 1333 / 243 (holes in the surface), animated metal spikes kept | verified visually |
| StuckButton / DropButton / RaiseButton | 241+235+236 / 241+242+296 / 347+242+349; the marker box is dropped | composites built; the difference to a plain floor is subtle (as in the original), not checked closely |
| LooseFloor | wood slab | KEEP (no record images; falling floors are mobs) |
| Pillar, LatticePillar | column of 295 (19 px wide dungeon, 16 px palace, cut-out), square, `TILE_H` tall | verified visually |
| BigPillarBottom / BigPillarTop | 1288 / 1291 columns; the top cell has no floor slab (its record has no +8) | verified visually (level 2 room 5, level 5 room 7) |
| LatticeSupport | stone column | KEEP (record values are chtab-7 guesses) |
| Gate, DoortopWithFloor, Doortop | post 249 (8 px) at the right edge of the cell, back half; gate bars stay procedural | post placement is a guess (the 2D post is a fore piece); looks plausible |
| ExitLeft / ExitRight | frame 237 / 239 cut to 47 rows over a dark doorway, lintel 238 / 240 above | verified visually (both sets); each frame is drawn in its own column (the original draws +3 pieces one column to the right): guess |
| Wall | dungeon 368 (mod 0) / 370 (mod 1) on the block, one row tall (`TILE_H - SLAB` top, no overlapping faces); palace: procedural | dungeon verified visually; palace KEEP (1-bit overlays need the unknown fill routine) |
| SmallLattice, LatticeLeft, LatticeRight, Unknown31 | procedural | KEEP |
| cutscene chamber (`spawn_chamber`) | procedural | KEEP (its own look) |

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
