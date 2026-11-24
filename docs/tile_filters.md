# Tile scaling filters: test results

What was tested: the tile images of the original (VDUNGEON / VPALACE, 14 tiles: wall faces, speckled stone, floor surfaces, torch
wall, palace lattice and wall pieces) with nearest, EPX (Scale2x), MMPX and xBRZ, and in the game with `POP_UPSCALE`
(`nearest`, `epx`, `mmpx`, `sharp`). Tools: `tools/tile_filters/compare.py` (contact sheets and metrics, needs `tile_export`
output; the xBRZ column needs a scratch build around the GPL-3.0 crate `xbrz-rs`, which is not part of this repository) and
`crates/pop-app/src/relief.rs` (`mmpx`, `Upscale`). The contact sheets are made from the user's game data and are not committed.

## Metrics (14 tiles, 2x)

"kept" = share of isolated source pixels (all four neighbours differ) that survive, "changed" = share of output pixels that differ from a
plain 2x copy, "new colours" = colours the source did not have, per tile.

| filter | isolated pixels kept | pixels changed | new colours | tile edge: output px that differ from the same tile inside a tiling (outer 2 px) |
|---|---|---|---|---|
| EPX / Scale2x | 76 % | 2 % | 0 | 0-3 % |
| **MMPX** | **91 %** | 2 % | 0 | 0-6 % |
| xBRZ | 5 % | 6 % | 33.5 | 1-16 % |

Timing (separate scratch tool, release build): xBRZ 2x + 4x 0.48 ms per tile, the `mmpx` crate 0.54 ms per tile. In the game (dev build,
all tiles of both sets, 6.3 MB RGBA at 1x): nearest 12 ms; EPX 4x 246 ms, MMPX 4x 436 ms, sharp 224 ms, each 100.9 MB RGBA; relief
(normal maps) alone 389 ms and 31.5 MB, MMPX 4x with relief 802 ms and 126 MB.

## What the pictures show

- **xBRZ** removes the speckle of the stone and the stars of the palace sky almost completely and adds colours: not faithful, and GPL-3.0
  (the repository is MIT). Dropped.
- **EPX** keeps the palette but loses a quarter of the single pixels (the top course 373 loses all of them), and thins the small marks
  to line figures.
- **MMPX** keeps the palette and nearly all single pixels, rounds the brick corners and the lattice diagonals; the small marks of the
  stone become slightly bolder blobs. With relief (normal maps) the rounded bricks get lit bevels: the best-looking combination here.
- **sharp bilinear** (nearest 4x, linear sampling) is the original's look exactly, only the pixel edges are softer at fractional scales.
  Cheapest, no filter.
- **Neural upscalers** were not run (a model download of tens of MB and a runtime would be needed; their licences often restrict the
  output).

## Result

`tile_relief` (Settings > Graphics) now uses MMPX 4x with normal maps. `POP_UPSCALE` stays as a test hook for the other filters.

## Mip maps (CPU, `POP_MIPS`)

`relief::mip_chain` builds the levels at load time (colour: 2 x 2 box filter in linear light, weighted by alpha; normal maps:
averaged and renormalised); `POP_MIPS=<levels>` sets how many (default 1 = none). Measured in the dev build (frame time with
`POP_PERF=1`; the 16 ms of the baseline is the 60 Hz limit, so the true baseline is lower), 4x colour images, Classic camera, level 1 room 5:

| levels (colour only, `POP_UPSCALE=sharp`) | 1 | 2 | 3 | 4 | 6 | 8 (full) |
|---|---|---|---|---|---|---|
| ms per frame | 16.2 | 21.8 | 27.4 | 39.4 | 56.2 | 59-80 |

The cost grows with every level, also when the sampler is clamped to level 0 and with the images kept only in the render world, so it is
not the sampling and not a per-frame copy. Relief alone (`tile_relief`, no mips) already measures 25.5 ms against 16.1 ms; with 2 levels
37.5 ms. At 480 x 300 the screenshots with 1 and with 2 levels look the same.

### Cause: the number of loaded textures

Bevy 0.20 makes `StandardMaterial` bindless: all textures of all materials go into binding arrays of one bind group (a slab of up to 2048
resources, `AUTO_BINDLESS_SLAB_RESOURCE_LIMIT`; the Bevy docs name wgpu performance problems with many bindless resources as the reason for the
limit), so every frame touches every loaded texture, level by level. We load 881 images and materials (dungeon set about 67, palace set 814, of
which 750 are the palace wall pieces, 25 rooms x 30 tiles). Measured with a level that uses one set only (dev build, `POP_PERF=1`, the
16.7 ms are the 60 Hz limit; the temporary switch to load one set only is removed again):

| loaded | 8 mip levels, colour only | relief (4x + normal maps) |
|---|---|---|
| both sets (881) | 75.8 ms | - |
| dungeon only (about 67) | 16.3 ms | 19-20 ms with 8 levels, 16.7 ms without mips |
| palace only (814) | 74.8 ms | - |

So with a few dozen textures the full chain and the normal maps are affordable; the palace wall pieces are what makes it slow.

### The atlas (built, `atlas.rs`, used by `TileSet::load`)

All images of a tile set are packed into a few pages; every material keeps its own `StandardMaterial` (uniform data only) with the page as
`base_color_texture` (and `normal_map_texture`) and a `uv_transform` that selects its rectangle. Identical texture views share one slot in
Bevy's binding arrays, so the 881 textures became 2 (plain look) to 22 (relief with 4 mip levels, pages and normal pages of both sets).

- **No bleeding:** every image is surrounded by a gutter of replicated edge pixels (`gutter_for`: 1 source pixel for no mip maps, 2 for 4
  levels with 4x colour, 4 with normal maps at 2x). Bilinear filtering, anisotropy and the first `ATLAS_MAX_MIPS` = 4 levels only read the
  image's own colours. Tested in `atlas.rs`: the pixels outside an image are its own edge, never its neighbour's; the rectangles
  with gutters never overlap; `uv_transform` maps 0..1 onto the content (colour and normal pages); the pages are cropped to the used extent.
- **Growing:** `Atlas::insert` takes any image at any time; without room a new page starts (shelf packing, best-fit shelf, heights rounded
  to 8). New images (more DAT ids found) only need a `texture(..)` call in `TileSet::load`; to make an image with another size or a
  different atlas, `Atlas::new` takes the page edge, the colour and normal pixel scales and the gutter.
- Mip levels above `ATLAS_MAX_MIPS` would need larger gutters (`POP_MIPS` is clamped to 4).

Measured (dev build, `POP_PERF=1`, 4 mip levels = `POP_MIPS=4`; the earlier tables ran into the 60 Hz limit at 16 ms, vsync is off here):

| case | before (separate textures) | atlas |
|---|---|---|
| plain look, palace level 4 | 16 ms (limit) | 12.6 ms |
| relief, palace level 4 | about 25 ms (level 1) | 13.5 ms |
| relief + 4 mip levels, palace level 4 | about 75 ms (8 levels, colour only) | 14.0 ms |
| relief + 4 mip levels, dungeon level 1 | - | 12.7 ms |

Load time and tile texture memory (both sets, the log line `tile textures: loaded`): plain 37 ms and 9.0 MB (12 ms and 6.3 MB before: the
gutters), relief 1.26 s and 156 MB, relief with 4 mip levels 2.2 s and 191 MB. The plain look is pixel-equivalent to the version before the
atlas (mean difference 0.9 of 765 per pixel in a torch room, from the animated flames and sparks).

Limits: `solari` (ray tracing, off by default) was not checked with `uv_transform`; the cut-out dark fringe with linear sampling is still
there; the atlas is built per tile set at load time, the palette is still rebuilt as a whole when `tile_textures`, columns, objects or
`tile_relief` change.

## Notes

- Port: `relief::mmpx` follows the algorithm of the MIT `mmpx` crate 0.2.0, which reads the neighbours P and S (two rows up / down) only
  at the start of each row; with that change the Python port equals the crate on every tile here (10 of 14 tiles differ otherwise, 0 to 28 of
  the output pixels), the Rust port equals the Python port on a test pattern (`relief.rs` tests).
- 4x colour images cost 16x the texture memory (100 MB here). Without mip maps, 1280 x 800 and 4K are fine; much smaller windows would
  alias. Cut-outs (alpha 0) get a dark fringe with linear sampling; not handled.
- Filtering is per tile without its neighbours, so tile edges are only approximately right (table above).
- Not tested: Classic mode, the other palace / dungeon levels, 4K screenshots, frame time in the release build.
