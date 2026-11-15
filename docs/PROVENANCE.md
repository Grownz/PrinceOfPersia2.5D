# Provenance

## What is original (and never redistributed)

The DOS game Prince of Persia (1990): `PRINCE.EXE`, `LEVELS.DAT`, `KID.DAT`, the guard, title, palace/dungeon and sound
`.DAT` files and everything inside them (level layouts, sprites, palettes, animation and frame tables, digitized sounds,
PC-speaker tunes). These are copyrighted by their owners. None of them is committed to this repository
(`Original_data/` is in `.gitignore`), none is embedded in the binary, and the packaging script (`tools/package.ps1`)
packs only the exe and a README and refuses any other file.

At run time the app reads the user's own copy (see "Supplying the data" below). Tables the engine needs from
`PRINCE.EXE` (frame tables, sequences, sword table) are read from that copy when the game starts, not copied into the
source code. Where the source quotes addresses or constants (`0000:4DA2`, `ds:2BBA`, ...), they are references for
verification; some small tables (for example the 16 EGA colours in `main.rs`) are stated in code with a test that checks
them against the user's exe when it is present.

## What is ours

- All Rust code in `crates/` (the headless simulation `pop-core`, the Bevy app `pop-app`) and the tools in `tools/`.
- The synthesized sound effects (`crates/pop-app/src/synth.rs`): generated procedurally at start-up, no samples.
  The game's own sounds are used only when the user selects `"sound": "original"` / `POP_SOUND=original`.
- The procedural 3D room geometry (`crates/pop-app/src/room.rs`): boxes and colours, no textures from the game.
- The documentation in `docs/`.

The character sprites and the title picture shown in the app are decoded at run time from the user's own data files.

## Sources consulted

- Disassembly of the user's own copy of `PRINCE.EXE` (behaviour, timings, tables); findings are recorded in
  `docs/mechanics.md`, `docs/edge_cases.md` and the other notes in `docs/`, marked VERIFIED or UNVERIFIED.
- Third-party open-source reimplementations (for example SDLPoP) were consulted for documentation of the file formats
  only, as the project rules in `ROADMAP.md` allow; the behaviour reference is the exe itself. Places where a detail follows such a source
  without being checked against the exe are marked UNVERIFIED in the code.

## Supplying the data

Copy the files of your own DOS installation into a folder named `Original_data` next to the exe (or in the current
folder), or point `POP_DATA` or `"data_path"` in `settings.json` at the folder. Required: `LEVELS.DAT`, `PRINCE.EXE`,
`KID.DAT`; the guard, title and sound files are needed for the full game. Without the data the app shows a message
listing the folders it searched instead of starting.
