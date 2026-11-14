# Roadmap

Goal: a modern 2.5D Windows x64 port of Prince of Persia (DOS) with original-faithful mechanics.
Stack: Rust workspace. `pop-core` (headless: data formats + deterministic 12 Hz simulation), `pop-app` (Bevy: rendering, audio, input).
Original assets are read at runtime from `Original_data/` (not committed; override with `POP_DATA`).

## Status

- [x] Workspace, `.DAT` archive reader, level parser (tiles + room links), Bevy window drawing room 5 of level 1 as placeholder blocks.
- [x] Test: level 1 loads, room links match the original data.
- [x] M1.1-1.3 done: level start (room/tile/dir), guards (room/tile/dir), full tile-type enum, typed button -> gate/exit door links. Verified by tests against all shipped levels (every button resolves to a gate or exit door).
- [x] M1.4 done: image decoder (`pop-core::image`: raw / RLE / LZG, left-right and up-down, 1 and 4 bpp) and VGA palettes. All 1300+ image resources in the shipped DATs decode; title frame, kid sprite and the intro text were checked visually. `cargo run -p pop-core --example dump_image` writes a PPM for inspection.
- [x] M1.5 (data part) done: `pop-core::exe` reads PRINCE.EXE v1.1: three frame tables (kid 241 frames, guard-type 41 frames, cutscene 86 frames; image, sword, dx, dy, flags) and all 114 animation sequences as typed ops. Verified: every sequence decodes, frame numbers are valid, kid and guard image numbers all exist in KID.DAT / GUARD.DAT. Guesses: opcode names for knock/end-level/get-item/die, meaning of the flag bits, and whether `flags` belongs to its own record or the previous one (chosen so that standing frames are "thin", running frames are not).
- Character inventory (from the shipped files): kid (KID.DAT, 219 images), shadow (SHADOW.DAT, 32), guard (GUARD.DAT, 34; GUARD1/2.DAT are palette-only colour variants), fat guard (FAT.DAT, 34), skeleton (SKEL.DAT, 28), vizier (VIZIER.DAT, 34), princess/mouse/vizier cutscenes (PV.DAT, ~102 images), title/intro (TITLE.DAT). Guard, fat guard and vizier share the guard frame table (all guard images exist in GUARD.DAT); skeleton has fewer images and the shadow reuses kid frames - both mappings still to be confirmed. The cutscene table's second byte (64/128) seems to select an image set; its DAT mapping is unresolved.
- Not yet extracted: sword position table (just after the cutscene table, ~51 entries), and the *control logic* (which sequence an input/state starts, collision rules) - that is code, not data, and has to be re-derived for M3 by disassembling PRINCE.EXE.
- Known gaps: guard skill/colour and the other tail bytes (2115-2118, 2167+, room coordinates at 2048) are unparsed; resource 2015 has a deviating layout (room links padded with 255); tile flag bits (high 3 bits of fg) have no meaning assigned yet; start tile semantics (which tile the kid stands on) are unchecked visually.

## Milestones

Each milestone ends in something runnable and, where possible, a headless test in `pop-core`.

### M1 - Data completeness (`pop-core`)
1. Decode the level tail: start room/tile/direction, guards (tile, direction, skill, colour), shadow/special events. Verify against known level 1 facts.
2. Define the full tile-type table (floor, spikes, pillar, gate, plate, loose floor, exit, wall, torch, ...) and tile modifiers; add `Tile` enum + unit tests.
3. Door link tables (gates/plates/ shaking floors) parsed into typed links.
4. Decode image resources (`KID.DAT`, `GUARD.DAT`, `VIZIER.DAT`, tile sets `*PALACE.DAT`/`*DUNGEON.DAT`): palette + compressed bitmap formats. Used as reference/layout data for the modern assets, not as final art.
5. Extract the animation/frame tables and sequence scripts (in `PRINCE.EXE`), the basis for M3.

### M2 - Room rendering (`pop-app`) - partly done: M2.1 (placeholder geometry per tile type, torch lights), M2.4 (debug text), room/level navigation; open: smooth scroll with neighbour rooms (`room::spawn_room` takes an origin for this), live tile state from the sim, real art
1. Real tile geometry per tile type (3D models, simple at first), palace and dungeon themes.
2. Room-to-room camera: screen-flip like the original, optionally smooth scroll with neighbour rooms visible.
3. Lighting: torches, ambient, shadows; modern look decided here (art direction doc).
4. Debug overlay: room/tile coordinates, tick counter.

### M3 - Player mechanics (core of fidelity)
Status: the flat-ground controller is in (`pop-core::kid`, rules from docs/mechanics.md): edge-triggered input flags, standing/start run/running/stop/turn/run turn, standing and running jump, crouch/stand up, safe steps, falling with gravity and soft/hard/fatal landing, room changes. Headless replay tests cover run/stop, turn, standing jump distance (39 units, matches the sequence data) and falling. In the app: arrows + Shift control the kid, Ctrl+arrows change room, `,` `.` browse sequences. Facing/drawing and level start follow the original (docs/mechanics.md sections 7, 8). Ported from sections 9-11: collision box, wall bumps (seq 45/46/47), in_wall, dist_from_wall_forward via the box, leave_room/goto_other_room (box-edge trigger, vertical moves, room 0), jump up, grab, hang, climb up, hang fall, climb down (one condition reconstructed). Tests: wall bump, next room, grab + climb. Still missing for M3: level doors, mirror/chomper/gate behaviour, hit-point display, sword, shadow/guards; the blitter y rule is unverified.

1. Simulation steps at 60 Hz; original logic runs every 5th step (12 ticks/s, `sim::World`), `World::alpha()` drives render interpolation. Done in the skeleton, the interpolation itself comes with the kid.
2. Input -> action mapping (keyboard + gamepad) matching original controls.
3. Kid state machine from the original sequence tables: stand, run, turn, jump (standing/running), climb, hang, drop, crouch, fall damage, death.
4. Collision and tile interaction: floors, walls, edge grabbing, falling between rooms.
5. Headless replay tests: recorded input sequences must give identical positions each run (determinism); compare against published reference behaviour (e.g. jump distances).

### M4 - Tile mechanics
Done: runtime tile state and the original tick machine in `pop-core::sim` (`World`: trobs, mobs, event queue from the kid; order mobs -> trobs -> kid). Gates (open/close/hold/push), buttons with shared link timers and chain triggers, loose floors (shake, fall in 10 ticks, mob physics, landing effects, hit on the kid), spikes (23-tick cycle, harm rules, impaling), chompers (staggered start, kill), exit door timing, knocks, sound ids logged in `World::sounds`. Tests: button -> gate, loose floor timing, spike cycle and kill, chomper stagger, all 15 levels run 400 ticks. App renders the runtime level (gates, spikes, falling floors move). Not simulated: torch/potion/sword animation (they also use the shared RNG in the original), guards/shadow, entering the level door, skeleton, mirror, potion effects, pressed-button and chomper drawing.
Loose floors, pressure plates + gates, spikes, chompers, exit doors, raise/drop buttons, mirror, skeletons, flame/torch events.

### M5 - Enemies and combat
Guard AI per skill level, sword fighting states (advance, retreat, block, strike), health and damage, shadow, skeleton, Vizier. Cutscenes/level events (level 3 mirror, level 12 etc.).

### M6 - Game flow
Title screen, 15 levels (+ final), 60-minute timer, health potions and max HP, level progression, save/restore state (original has level-password style progress), menus, pause.

### M7 - Audio
Replace PC-speaker/MIDI/digi sounds with modern effects and music; event hooks driven by simulation events (not by rendering) so timing stays faithful.

### M8 - Packaging and polish
Windows x64 release build, installer/zip, settings (resolution, controls, volume), controller support, performance pass, CI build, licence/asset-provenance notes.

## Cross-cutting

- Fidelity first: any mechanics change must be justified against the original; keep a `docs/mechanics.md` of verified behaviours.
- Reference for behaviour: the DOS binary (`PRINCE.EXE`) and its resource data. Third-party open-source reimplementations may be consulted for documentation of the formats.
- Determinism: simulation uses integers/fixed-point only, no wall-clock or RNG outside a seeded source.
- Legal: the original game data is copyrighted and must never be committed or redistributed; the app expects the user's own copy.
- Tests: `cargo test -p pop-core` must stay green; add a test with every new parser or state machine.
