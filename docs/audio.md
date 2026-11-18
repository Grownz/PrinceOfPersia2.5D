# Audio (M7)

How the original's sounds are identified, stored and timed, and how the remake plays them.

## 1. Where sounds come from in the simulation

`pop-core` never plays audio. Every `play_sound(id)` of the original becomes a push onto `World::sounds: Vec<u8>` during the
logic tick that would play it:

- `World::sound` / `sound_if_visible` (tiles, potions, gates, doors, loose floors, spikes, chompers) - `sim.rs`;
- `Event::Sound(id)` from the characters (seq op `F2`, bumps, hurt, draw sword, chomped, spiked) - `kid.rs`, `kid/combat.rs`,
  forwarded by `World::apply_event`;
- level events and combat (`sim.rs`, `sim/combat.rs`): swords clash, death music, mirror/shadow, Jaffar, skeleton, guard killed.

Seq op `F2 n` (`0000:7049`): 0 = guard notice only, 1 = footstep 23, 2 = bump 8, 3 = drink 18, **4 = level music**: shadow music
32 on level 4, nothing on levels 13 and 15, else end-level music 41 (the kid emits 41, `World::apply_event` picks the cue; tested
in `world_tests::level_exits_4_1`).

The app drains the log in the fixed step right after `World::step`, so sounds start in the 60 Hz step of the logic tick that
caused them (frame length 5 or 6 included, edge_cases.md 5.2).

## 2. Sound ids

Meaning and origin per id ("where" = PRINCE.EXE offset in segment 0000, from tiles.md 9, combat.md 10, edge_cases.md, mechanics.md).
Columns digi / speaker give the original resource and its length (computed from the data, section 3). Ids 50..56 are the
story/title music; their names follow SDLPoP's `sound_ids` (UNVERIFIED against PRINCE.EXE), the simulation does not emit them.

| id | meaning | where | emitted | digi | speaker |
|---|---|---|---|---|---|
| 0 | fell to death (fatal landing) | land, edge_cases 3.1 | yes | 0.15 s | 0.07 s |
| 1 | falling scream (fall_y >= 31) | do_fall `5E98` | yes | 1.09 s | - |
| 2 | loose floor crashes | `97AD` | yes | 0.56 s | 0.30 s |
| 3 | button pressed | `92B8` | yes | 0.11 s | 0.04 s |
| 4 | gate closing (slow, every 4th unit, visible) | `8B94` | yes | 0.10 s | 0.01 s |
| 5 | gate opening (every 8 units) | `8BAC` | yes | 0.12 s | 0.01 s |
| 6 | gate fast close finished (slam) | `8B57` | yes | 0.50 s | 0.13 s |
| 7 | gate stops (open / closed / permanent) | `8BC1`, `8BDC` | yes | 0.07 s | 0.01 s |
| 8 | bumped (wall, gate push) | `599F` | yes | 0.15 s | 0.04 s |
| 9 | grab a ledge | mechanics 451 | yes | 0.25 s | 0.01 s |
| 10 | swords clash (frame 167) | `1394` | yes | 0.32 s | 0.04 s |
| 11 | sword swish, a miss | `4506` | yes | 0.11 s | - |
| 12 | guard hurt | `43B6` | yes | 0.12 s | 0.06 s |
| 13 | kid hurt (sword, hurt potion) | `43B1`, `8297` | yes | 0.34 s | 0.05 s |
| 14 | exit door closed | `8C1D` | yes | 0.69 s | 0.18 s |
| 15 | exit door sliding (each tick) | `8C93` | yes | 1.61 s (2750 Hz) | 0.25 s |
| 16 | hard landing (take_hp 1) | land `5EF8` | yes | 0.43 s | 0.06 s |
| 17 | soft landing | land `5EF8` | yes | 0.38 s | 0.04 s |
| 18 | drink (seq op F2 3) | seq | yes | 0.63 s | 1.46 s |
| 19 | draw the sword | `6AA6` | yes | 0.25 s | - |
| 20, 21, 22 | loose floor shaking (random, never twice in a row) | `9467` | yes | 0.12 / 0.10 / 0.11 s | - |
| 23 | footstep (seq op F2 1) | seq | yes | 0.12 s (8200 Hz) | 0.01 s |
| 24 | death music | `832B` | yes | - | 2.97 s |
| 25 | shadow appears / mirror music (levels 4, 6) | `37BF`, `3DEA` | yes | - | 1.82 s |
| 26 | embrace (ending cutscene `2960`) | edge_cases 4.1 | scene | - | 11.69 s |
| 27 | princess waiting (cutscene before 2, 4, 6) | `2B04`, `2AD0` | scene | - | 10.73 s |
| 28 | death music, holding the sword | `8324` | yes | - | 6.45 s |
| 29 | meet Jaffar | `3DB7` | yes | - | 3.94 s |
| 30 | life potion | `8249` | yes | - | 2.33 s |
| 32 | shadow music (death by the shadow; level-4 stairs) | `8316`, F2 4 | yes | - | 4.92 s |
| 33 | heal potion | `8223` | yes | - | 2.48 s |
| 35 | princess and the mouse (cutscene before 8, 9) | `2891`, `28FB` | scene | - | 9.75 s |
| 36 | time is up (cutscene `2A1B`) | edge_cases 3.5 | scene | - | 12.72 s |
| 37 | guard killed / sword picked up | `837E`, `81CC` | yes | - | 2.54 s |
| 38 | blink beep ("Press Button" blink, UNVERIFIED) | `0C83:7198`? | no | - | 0.06 s |
| 39 | feather potion | `162B` | yes | - | 18.61 s |
| 40 | cutscene before 12 | `2A71` | scene | - | 5.18 s |
| 41 | end of level (seq op F2 4) | seq 70 | yes | - | 11.31 s |
| 43 | Jaffar killed | `8372` | yes | - | 3.38 s |
| 44 | skeleton wakes | `45F6` | yes | 0.24 s | 0.38 s |
| 45 | jump through the mirror | `509B` | yes | 0.39 s | 0.17 s |
| 46 | chomped | `5BC2` | yes | 0.42 s | 0.07 s |
| 47 | chomper closes | `8A79` | yes | 0.47 s | 0.11 s |
| 48 | spiked | `605A` | yes | 0.16 s | 0.15 s |
| 49 | spikes come out | `8F02` | yes | 0.40 s | 0.06 s |
| 50 | story 2: princess | pv_scene `2B38` | scene | - | 9.50 s |
| 51 | princess door opening | pv_scene | scene | 0.44 s (14000 Hz) | 0.54 s |
| 52 | story 4: Jaffar leaves | pv_scene | scene | - | 27.98 s |
| 53 | story 3: Jaffar comes | pv_scene | scene | - | 14.85 s |
| 54 | intro music | show_title `181D` | title | - | 24.74 s |
| 55 | story 1: absence | show_title | title | - | 14.86 s |
| 56 | ending music | end_sequence `2F2F` | title | - | 120.04 s |

31, 34 and 42 are empty everywhere (1-byte resources). "scene" = started by a cutscene script (`pop_core::cutscene`, queued
until no sound plays, docs/cutscenes.md 1.3); "title" = the attract loop / ending pages (flow.rs). Not modelled: the original's "stop sounds" before the potion sounds and at Ctrl+A/level exits is kept only
for world changes (all sounds stop when a new `World` starts).

## 3. Resource files (VERIFIED by decoding)

All are standard DAT archives (dat.rs); resource id = **10000 + sound id**.

| file | ids | format |
|---|---|---|
| DIGISND1.DAT | 0, 2..13, 16, 17, 19..23 | digitized |
| DIGISND2.DAT | 44..49, 51 | digitized |
| DIGISND3.DAT | 1, 14, 15, 18 | digitized |
| IBM_SND1.DAT | 0..43 | PC speaker |
| IBM_SND2.DAT | 44..56 | PC speaker |
| MIDISND1.DAT | 24..30, 32, 33, 35..37, 39..41, 43 | MIDI |
| MIDISND2.DAT | 50, 52..56 | MIDI |

First byte = type; bits 0..1 pick the player in play_sound_from_buffer `0C83:7198` (0 speaker `7744`, 1 digitized `7583`,
2 MIDI `76D8`), bit 7 = **loop**: the digi end-of-buffer handler (`0C83:7381`) and the MIDI completion (`0C83:72AF`) restart the
same buffer when it is set (VERIFIED for digi and MIDI; speaker path not checked). Only id 15 (exit door sliding) has it:

- **1 digitized**: u16 sample rate (11000 Hz for most; 8200 for 23, 2750 for 15, 14000 for 51), u16 sample count (= payload
  size - 8), u16 unknown (0), u8 bits per sample (8), then unsigned 8-bit mono samples.
- **0 PC speaker**: u16 tempo, then notes {u16 frequency in Hz (0 or 1 = rest), u8 length in 1/tempo seconds}, ended by a
  frequency of 0x12. Feather 39 at tempo 80 sums to 18.61 s, close to SDLPoP's 18.75 s feather length, which supports the reading.
- **2 MIDI**: a standard MIDI file (`MThd ...`, format 1, 480 ticks per quarter) after the type byte; not played by the remake
  yet. Program numbers are **0..17, not General MIDI**: they index the game's own AdLib instrument bank (section 6.4). The driver
  also knows an older `MSeq` format (`ds:31E0`); no resource uses it.

The music cues exist only as speaker and MIDI; which one the original plays depends on the sound card (section 6.3). The
speaker lengths above are the ones used where timing matters; the MIDI lengths are in 3.1.

### 3.1 Inventory (generated)

`tools/sound_inventory` (standalone crate, not in the workspace; reads the user's Original_data or `POP_DATA`, read-only):
`cargo run --release -- --write` regenerates the block below, `cargo test --release` checks the SMF parser and that every
resource of the user's archives is recognised (skips without data). MIDI seconds = end of the longest track through the tempo
map (trailing rests included; whether the driver reports "playing" until End of Track is UNVERIFIED). "prog" = channel:program,
first program change per channel. The sysex in most files (`00 00 34 01 00 0C`, once `...18`) has no standard manufacturer id;
its meaning for the driver is UNVERIFIED.

<!-- inventory:begin (tools/sound_inventory --write) -->

| file | bytes | resources | kinds | ids |
|---|---|---|---|---|
| DIGISND1.DAT | 48545 | 20 | digitized | 10000..10023 |
| DIGISND2.DAT | 29143 | 7 | digitized | 10044..10051 |
| DIGISND3.DAT | 31000 | 4 | digitized | 10001..10018 |
| IBM_SND1.DAT | 3684 | 44 | speaker, empty | 10000..10043 |
| IBM_SND2.DAT | 3784 | 13 | speaker | 10044..10056 |
| MIDISND1.DAT | 9368 | 16 | MIDI | 10024..10043 |
| MIDISND2.DAT | 18408 | 6 | MIDI | 10050..10056 |

| file | id | sound | bytes | kind | seconds | details |
|---|---|---|---|---|---|---|
| DIGISND1 | 10000 | 0 | 1663 | digitized | 0.15 | 11000 Hz, 1655 samples, 8 bit |
| DIGISND1 | 10002 | 2 | 6208 | digitized | 0.56 | 11000 Hz, 6200 samples, 8 bit |
| DIGISND1 | 10003 | 3 | 1176 | digitized | 0.11 | 11000 Hz, 1168 samples, 8 bit |
| DIGISND1 | 10004 | 4 | 1076 | digitized | 0.10 | 11000 Hz, 1068 samples, 8 bit |
| DIGISND1 | 10005 | 5 | 1360 | digitized | 0.12 | 11000 Hz, 1352 samples, 8 bit |
| DIGISND1 | 10006 | 6 | 5540 | digitized | 0.50 | 11000 Hz, 5532 samples, 8 bit |
| DIGISND1 | 10007 | 7 | 830 | digitized | 0.07 | 11000 Hz, 822 samples, 8 bit |
| DIGISND1 | 10008 | 8 | 1626 | digitized | 0.15 | 11000 Hz, 1618 samples, 8 bit |
| DIGISND1 | 10009 | 9 | 2752 | digitized | 0.25 | 11000 Hz, 2744 samples, 8 bit |
| DIGISND1 | 10010 | 10 | 3512 | digitized | 0.32 | 11000 Hz, 3504 samples, 8 bit |
| DIGISND1 | 10011 | 11 | 1180 | digitized | 0.11 | 11000 Hz, 1172 samples, 8 bit |
| DIGISND1 | 10012 | 12 | 1284 | digitized | 0.12 | 11000 Hz, 1276 samples, 8 bit |
| DIGISND1 | 10013 | 13 | 3798 | digitized | 0.34 | 11000 Hz, 3790 samples, 8 bit |
| DIGISND1 | 10016 | 16 | 4744 | digitized | 0.43 | 11000 Hz, 4736 samples, 8 bit |
| DIGISND1 | 10017 | 17 | 4224 | digitized | 0.38 | 11000 Hz, 4216 samples, 8 bit |
| DIGISND1 | 10019 | 19 | 2802 | digitized | 0.25 | 11000 Hz, 2794 samples, 8 bit |
| DIGISND1 | 10020 | 20 | 1348 | digitized | 0.12 | 11000 Hz, 1340 samples, 8 bit |
| DIGISND1 | 10021 | 21 | 1054 | digitized | 0.10 | 11000 Hz, 1046 samples, 8 bit |
| DIGISND1 | 10022 | 22 | 1176 | digitized | 0.11 | 11000 Hz, 1168 samples, 8 bit |
| DIGISND1 | 10023 | 23 | 1004 | digitized | 0.12 | 8200 Hz, 996 samples, 8 bit |
| DIGISND2 | 10044 | 44 | 2677 | digitized | 0.24 | 11000 Hz, 2669 samples, 8 bit |
| DIGISND2 | 10045 | 45 | 4243 | digitized | 0.39 | 11000 Hz, 4235 samples, 8 bit |
| DIGISND2 | 10046 | 46 | 4616 | digitized | 0.42 | 11000 Hz, 4608 samples, 8 bit |
| DIGISND2 | 10047 | 47 | 5128 | digitized | 0.47 | 11000 Hz, 5120 samples, 8 bit |
| DIGISND2 | 10048 | 48 | 1808 | digitized | 0.16 | 11000 Hz, 1800 samples, 8 bit |
| DIGISND2 | 10049 | 49 | 4404 | digitized | 0.40 | 11000 Hz, 4396 samples, 8 bit |
| DIGISND2 | 10051 | 51 | 6196 | digitized | 0.44 | 14000 Hz, 6188 samples, 8 bit |
| DIGISND3 | 10001 | 1 | 12030 | digitized | 1.09 | 11000 Hz, 12022 samples, 8 bit |
| DIGISND3 | 10014 | 14 | 7592 | digitized | 0.69 | 11000 Hz, 7584 samples, 8 bit |
| DIGISND3 | 10015 | 15 | 4444 | digitized | 1.61 | 2750 Hz, 4436 samples, 8 bit, bit 7 (loop) |
| DIGISND3 | 10018 | 18 | 6890 | digitized | 0.63 | 11000 Hz, 6882 samples, 8 bit |
| IBM_SND1 | 10000 | 0 | 92 | speaker | 0.07 | tempo 400, 29 notes |
| IBM_SND1 | 10001 | 1 | 1 | empty | - | type 0 |
| IBM_SND1 | 10002 | 2 | 527 | speaker | 0.30 | tempo 576, 174 notes |
| IBM_SND1 | 10003 | 3 | 77 | speaker | 0.04 | tempo 576, 24 notes |
| IBM_SND1 | 10004 | 4 | 8 | speaker | 0.01 | tempo 72, 1 notes |
| IBM_SND1 | 10005 | 5 | 8 | speaker | 0.01 | tempo 144, 1 notes |
| IBM_SND1 | 10006 | 6 | 227 | speaker | 0.13 | tempo 576, 74 notes |
| IBM_SND1 | 10007 | 7 | 8 | speaker | 0.01 | tempo 72, 1 notes |
| IBM_SND1 | 10008 | 8 | 8 | speaker | 0.04 | tempo 72, 1 notes |
| IBM_SND1 | 10009 | 9 | 8 | speaker | 0.01 | tempo 72, 1 notes |
| IBM_SND1 | 10010 | 10 | 8 | speaker | 0.04 | tempo 72, 1 notes |
| IBM_SND1 | 10011 | 11 | 1 | empty | - | type 0 |
| IBM_SND1 | 10012 | 12 | 59 | speaker | 0.06 | tempo 288, 18 notes |
| IBM_SND1 | 10013 | 13 | 59 | speaker | 0.05 | tempo 350, 18 notes |
| IBM_SND1 | 10014 | 14 | 158 | speaker | 0.18 | tempo 576, 51 notes |
| IBM_SND1 | 10015 | 15 | 59 | speaker | 0.25 | tempo 72, 18 notes, bit 7 (loop) |
| IBM_SND1 | 10016 | 16 | 86 | speaker | 0.06 | tempo 450, 27 notes |
| IBM_SND1 | 10017 | 17 | 8 | speaker | 0.04 | tempo 72, 1 notes |
| IBM_SND1 | 10018 | 18 | 203 | speaker | 1.46 | tempo 144, 66 notes |
| IBM_SND1 | 10019 | 19 | 1 | empty | - | type 0 |
| IBM_SND1 | 10020 | 20 | 1 | empty | - | type 0 |
| IBM_SND1 | 10021 | 21 | 1 | empty | - | type 0 |
| IBM_SND1 | 10022 | 22 | 1 | empty | - | type 0 |
| IBM_SND1 | 10023 | 23 | 8 | speaker | 0.01 | tempo 72, 1 notes |
| IBM_SND1 | 10024 | 24 | 38 | speaker | 2.97 | tempo 60, 11 notes |
| IBM_SND1 | 10025 | 25 | 20 | speaker | 1.82 | tempo 34, 5 notes |
| IBM_SND1 | 10026 | 26 | 62 | speaker | 11.69 | tempo 48, 19 notes |
| IBM_SND1 | 10027 | 27 | 104 | speaker | 10.73 | tempo 30, 33 notes |
| IBM_SND1 | 10028 | 28 | 29 | speaker | 6.45 | tempo 60, 8 notes |
| IBM_SND1 | 10029 | 29 | 32 | speaker | 3.94 | tempo 34, 9 notes |
| IBM_SND1 | 10030 | 30 | 44 | speaker | 2.33 | tempo 72, 13 notes |
| IBM_SND1 | 10031 | 31 | 1 | empty | - | type 0 |
| IBM_SND1 | 10032 | 32 | 65 | speaker | 4.92 | tempo 64, 20 notes |
| IBM_SND1 | 10033 | 33 | 26 | speaker | 2.48 | tempo 64, 7 notes |
| IBM_SND1 | 10034 | 34 | 1 | empty | - | type 0 |
| IBM_SND1 | 10035 | 35 | 146 | speaker | 9.75 | tempo 28, 47 notes |
| IBM_SND1 | 10036 | 36 | 101 | speaker | 12.72 | tempo 36, 32 notes |
| IBM_SND1 | 10037 | 37 | 32 | speaker | 2.54 | tempo 96, 9 notes |
| IBM_SND1 | 10038 | 38 | 17 | speaker | 0.06 | tempo 72, 4 notes |
| IBM_SND1 | 10039 | 39 | 752 | speaker | 18.61 | tempo 80, 249 notes |
| IBM_SND1 | 10040 | 40 | 56 | speaker | 5.18 | tempo 28, 17 notes |
| IBM_SND1 | 10041 | 41 | 104 | speaker | 11.31 | tempo 32, 33 notes |
| IBM_SND1 | 10042 | 42 | 1 | empty | - | type 0 |
| IBM_SND1 | 10043 | 43 | 32 | speaker | 3.38 | tempo 72, 9 notes |
| IBM_SND2 | 10044 | 44 | 62 | speaker | 0.38 | tempo 50, 19 notes |
| IBM_SND2 | 10045 | 45 | 149 | speaker | 0.17 | tempo 288, 48 notes |
| IBM_SND2 | 10046 | 46 | 86 | speaker | 0.07 | tempo 400, 27 notes |
| IBM_SND2 | 10047 | 47 | 149 | speaker | 0.11 | tempo 450, 48 notes |
| IBM_SND2 | 10048 | 48 | 266 | speaker | 0.15 | tempo 576, 87 notes |
| IBM_SND2 | 10049 | 49 | 53 | speaker | 0.06 | tempo 288, 16 notes |
| IBM_SND2 | 10050 | 50 | 50 | speaker | 9.50 | tempo 240, 15 notes |
| IBM_SND2 | 10051 | 51 | 32 | speaker | 0.54 | tempo 240, 9 notes |
| IBM_SND2 | 10052 | 52 | 389 | speaker | 27.98 | tempo 240, 128 notes |
| IBM_SND2 | 10053 | 53 | 170 | speaker | 14.85 | tempo 260, 55 notes |
| IBM_SND2 | 10054 | 54 | 275 | speaker | 24.74 | tempo 240, 90 notes |
| IBM_SND2 | 10055 | 55 | 131 | speaker | 14.86 | tempo 64, 42 notes |
| IBM_SND2 | 10056 | 56 | 1847 | speaker | 120.04 | tempo 96, 614 notes |
| MIDISND1 | 10024 | 24 | 448 | MIDI | 3.60 | SMF 1 x8 trk, 480 tpq, 1 tempo, 27 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:1 3:2 4:2 5:6 6:17 7:17, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10025 | 25 | 366 | MIDI | 2.47 | SMF 1 x8 trk, 480 tpq, 1 tempo, 12 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:4 3:2 4:2 5:1 6:11 7:7, sysex 0, meta7F 0, tail 0 B |
| MIDISND1 | 10026 | 26 | 1035 | MIDI | 14.00 | SMF 1 x9 trk, 480 tpq, 7 tempo, 95 notes, ch 1,2,3,4,5,6,7,8, prog 1:14 2:14 3:2 4:6 5:17 6:6 7:6 8:12, sysex 1 (00 00 34 01 00 18), meta7F 0, tail 0 B |
| MIDISND1 | 10027 | 27 | 659 | MIDI | 12.00 | SMF 1 x8 trk, 480 tpq, 1 tempo, 59 notes, ch 1,2,3,4,5,6,7, prog 1:12 2:15 3:6 4:8 5:6 6:6 7:6, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10028 | 28 | 548 | MIDI | 7.20 | SMF 1 x9 trk, 480 tpq, 1 tempo, 34 notes, ch 1,2,3,4,5,6,7,8, prog 1:17 2:17 3:2 4:2 5:17 6:17 7:17 8:17, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10029 | 29 | 499 | MIDI | 5.29 | SMF 1 x8 trk, 480 tpq, 1 tempo, 29 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:4 3:2 4:2 5:1 6:11 7:7, sysex 0, meta7F 0, tail 0 B |
| MIDISND1 | 10030 | 30 | 582 | MIDI | 3.54 | SMF 1 x10 trk, 480 tpq, 3 tempo, 36 notes, ch 1,2,3,4,5,6,7,8,9, prog 1:14 2:14 3:14 4:6 5:6 6:6 7:13 8:13 9:13, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10032 | 32 | 574 | MIDI | 7.25 | SMF 1 x8 trk, 480 tpq, 3 tempo, 38 notes, ch 1,2,3,4,5,6,7, prog 1:5 2:4 3:2 4:2 5:6 6:2 7:17, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10033 | 33 | 446 | MIDI | 2.34 | SMF 1 x10 trk, 480 tpq, 1 tempo, 18 notes, ch 1,2,3,4,5,6,7,8,9, prog 1:14 2:14 3:14 4:6 5:6 6:6 7:13 8:13 9:13, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10035 | 35 | 422 | MIDI | 13.64 | SMF 1 x3 trk, 480 tpq, 1 tempo, 48 notes, ch 2,3, prog 2:15 3:2, sysex 0, meta7F 0, tail 0 B |
| MIDISND1 | 10036 | 36 | 940 | MIDI | 15.73 | SMF 1 x9 trk, 480 tpq, 9 tempo, 85 notes, ch 1,2,3,4,5,6,7,8, prog 1:15 2:15 3:2 4:2 5:17 6:16 7:16 8:11, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10037 | 37 | 524 | MIDI | 4.12 | SMF 1 x10 trk, 480 tpq, 1 tempo, 36 notes, ch 1,2,3,4,5,6,7,8,9, prog -, sysex 0, meta7F 0, tail 0 B |
| MIDISND1 | 10039 | 39 | 327 | MIDI | 18.67 | SMF 1 x7 trk, 480 tpq, 1 tempo, 15 notes, ch 1,2,3,5,6,7, prog 1:6 2:6 3:6 5:6 6:6 7:6, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10040 | 40 | 303 | MIDI | 6.86 | SMF 1 x5 trk, 480 tpq, 1 tempo, 20 notes, ch 1,2,3,4, prog 1:1 2:15 3:2 4:8, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10041 | 41 | 1013 | MIDI | 12.75 | SMF 1 x8 trk, 480 tpq, 1 tempo, 104 notes, ch 1,2,3,4,5,6,7, prog 1:5 2:4 3:2 4:2 5:6 6:2 7:9, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND1 | 10043 | 43 | 530 | MIDI | 3.83 | SMF 1 x10 trk, 480 tpq, 1 tempo, 36 notes, ch 1,2,3,4,5,6,7,8,9, prog -, sysex 0, meta7F 0, tail 0 B |
| MIDISND2 | 10050 | 50 | 494 | MIDI | 14.50 | SMF 1 x7 trk, 480 tpq, 4 tempo, 31 notes, ch 1,2,3,4,5,6, prog 1:12 2:6 3:6 4:14 5:6 6:6, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND2 | 10052 | 52 | 1783 | MIDI | 28.89 | SMF 1 x8 trk, 480 tpq, 8 tempo, 203 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:15 3:2 4:8 5:6 6:6 7:6, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND2 | 10053 | 53 | 816 | MIDI | 17.26 | SMF 1 x8 trk, 480 tpq, 2 tempo, 70 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:4 3:2 4:2 5:1 6:11 7:7, sysex 0, meta7F 0, tail 0 B |
| MIDISND2 | 10054 | 54 | 1738 | MIDI | 24.29 | SMF 1 x8 trk, 480 tpq, 6 tempo, 200 notes, ch 1,2,3,4,5,6,7, prog 1:0 2:0 3:2 4:2 5:6 6:2 7:3, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND2 | 10055 | 55 | 742 | MIDI | 15.75 | SMF 1 x8 trk, 480 tpq, 1 tempo, 59 notes, ch 1,2,3,4,5,6,7, prog 1:1 2:4 3:2 4:2 5:1 6:11 7:7, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |
| MIDISND2 | 10056 | 56 | 12773 | MIDI | 124.70 | SMF 1 x9 trk, 480 tpq, 3 tempo, 1800 notes, ch 1,2,3,4,5,6,7,8, prog 1:0 2:0 3:2 4:2 5:3 6:1 7:13 8:2, sysex 1 (00 00 34 01 00 0C), meta7F 0, tail 0 B |

<!-- inventory:end -->

Other sound data: **PRINCE.DAT resource 1** (289 bytes = 1 count byte 0x12 + 18 x 16-byte instruments) is the AdLib/OPL
instrument bank (6.4); PRINCE.DAT resource 2 (91 bytes) is not loaded by the device setup (meaning UNVERIFIED).

## 4. Playback in the remake (pop-app)

- `synth.rs`: every emitted id and the cutscene/title cues are synthesized at startup (oscillators, filtered noise, envelopes; music cues
  are short original jingles, not the game's melodies), mono 22050 Hz, peak 0.5. No audio files in the repository. Unit test:
  every clip is non-silent, finite, within its length range, not clipped.
- `POP_SOUND=original`: digitized clip where the game has one, else the PC-speaker tune rendered as a square wave, else the
  synthesized clip. Read from the user's own game data at startup (nothing copied into the repository).
- `audio.rs`: a custom `Decodable` PCM asset (no decoder feature needed: bevy's default `audio` feature already brings
  `bevy_audio`). Polyphonic; the same id again restarts that sound (approximates the single channel for repeated ticks).
- Headroom instead of a limiter (bevy_audio has no hook on the mixed output): synthesized clips peak at 0.5, digitized ones at
  0.5, speaker tunes at 0.25, and the same id restarts instead of stacking, so typical overlaps stay below 1.0 at full volume.
- Master volume (`-` / `=`, 10 % steps, default 70 %) and mute (`M`), applied to every playing or just-spawned sound (effects and cues); Paused pauses the playing sounds; a fresh title and every
  cutscene start silent; cutscenes and the title play their cues through the same path. Title music 50..56: own phrases cut or
  padded to the speaker lengths, because the attract loop and the intro scene wait for them (synth test checks the lengths).
- Outside Playing (title, cutscenes) `audio::drain` counts `busy` down per fixed step, so "until no sound plays" works there.

## 5. "A sound is playing": death counter and level end

The original waits while **any** sound plays (play_kid `0000:792A`, edge_cases.md 3.2) before each death-counter step, and after
the stairs until no sound plays (play_level_2 `4DA2`). The remake models this deterministically: `AudioState::busy` = fixed steps
(60 Hz) until the longest started clip ends, counted by the fixed step itself, not by the audio device (so mute, volume, pause
and device latency never change the game). Before each step `World::hold_death_counter = busy > 0`; `flow::playing` keeps a
`NextLevel` (not seamless, not the level-6 fall, which stops the sounds first) pending while `busy > 0`.

Chosen lengths (synthesized mode):

| cue | length | source |
|---|---|---|
| 24 death | 2.97 s | PC-speaker length (UNVERIFIED for MIDI/AdLib) |
| 28 death holding the sword | 6.45 s | PC-speaker length (UNVERIFIED for MIDI/AdLib) |
| 32 death by the shadow | 4.92 s | PC-speaker length (UNVERIFIED for MIDI/AdLib) |
| 41 end of level | ~3.5 s | own jingle (the speaker tune is 11.31 s; UNVERIFIED which the original waits for) |
| effects (hurt 13 etc.) | 0.05..0.9 s | own clips (digi lengths in section 2) |

So a sword death runs: f185, alive 1..6 (each as soon as the hurt sound has ended), death music, then "Press Button to
Continue" once the music has ended (about 3 s, or 6.45 s with the sword drawn). In original mode the lengths are those of the
decoded clips. `pop-core` alone still treats sounds as finished at once (deterministic tests).

Feather fall: the original ends it on the first frame with no sound playing (edge_cases.md 5.1). The remake keeps the fixed
`sim::FEATHER_TICKS` (224 ticks: the first 5/60 s frame after the 18.61 s speaker tune, tested against IBM_SND1 in pop-app) in the
core: with polyphony the "no sound playing" moment would depend on unrelated sounds, and the synthesized feather tune is 18.6 s,
matching the speaker tune, so effect and music end together.

## 6. The original sound engine (PRINCE.EXE)

Addresses: `0000:` game segment, `0C83:` runtime/driver segment (tools/disasm). VERIFIED = read in the disassembly.

### 6.1 Tables (VERIFIED, `ds:` offsets, one byte per sound id 0..57)

| ds | name | content |
|---|---|---|
| `2528` | priority | **lower = more important**. 0: 39 feather, 51 door, 57; 1: every music cue 24..28, 30..38, 40..43, 50, 52..56; then 10 clash, 11 swish 18, 12 guard hurt 12, 13 kid hurt 11, 46 chomped 15, 47 16, 29 meet Jaffar 19, 0 fall death 20, 49 22, 48 25, 1 scream 30, 2 35, 6/7 48, 4 50, 5 55, 8 75, 9 80, 3 102, 14 105, 15 110, 16 115, 17 120, 18 125, 19 130, 44 135, 45 140, 20 145, 21 150, 22 155, 23 footstep 160 |
| `24EE` | interruptible | 1 for 1..5, 7..13, 16, 17, 19..23, 29, 47; **0** for 0, 6, 14, 15, 18, 24..28, 30..46, 48..57 |
| `2562` | pcspeaker_exists | 0 for 1, 11, 19..22, 57 (their IBM_SND resources are 1-byte empties), else 1 |
| `4FD2` | sound_pointers | far pointer per id to the loaded resource (type byte first) |
| `40A2` | queued sound | -1 = none (reset by reset_cutscene `2E59`, play_level `4964`, play_next_sound) |
| `4360` | current sound | id of the last sound started by play_next_sound (only writer `1365`) |

(cutscene.rs already reads the priority table through `Exe::sound_priorities`, 57 bytes.)

### 6.2 Priority and interruption (VERIFIED)

```
play_sound(id)  0000:12F0                       ; only queues
  if queued >= 0 and prio[id] > prio[queued]: return      ; a less important sound never replaces the queued one (ties: the newer wins)
  if !pcspeaker_exists[id] and type(sound_pointers[id]) == 0: return   ; no speaker version: dropped
  queued = id

play_next_sound()  0000:132F                    ; once per game frame (main loop 0000:0AAB), and in cutscenes (2584) only
  if queued >= 0:                               ;   when no sound is playing
    if check_sound_playing(any):                ; 0C83:720D with a null pointer = digi | MIDI | speaker busy
      if !interruptible[current]: goto drop
      if prio[queued] > prio[current]: goto drop
    current = queued
    play_sound_from_buffer(sound_pointers[current])   ; 0C83:7198
  drop: queued = -1                             ; a sound that cannot start is lost, not delayed
```

Consequences: per frame at most **one** new sound starts (the most important of that frame's play_sound calls, the last of
equal ones). Music (priority 1, not interruptible) cuts any interruptible effect, and nothing cuts a non-interruptible sound,
not even the feather tune 39 (priority 0: it only wins within a frame and over interruptible sounds). While music, a
non-interruptible effect (0, 6, 14, 15, 18, 44..46, 48, 49, 51) or the feather plays, **every new sound is dropped**. In cutscenes the queue survives until silence (play_next_sound is not called while a sound plays), so
the ending's footsteps queued during 26 never play.

The player only stops the **same channel**: the digi starter calls `7480` (stop digi), the speaker starter `7548` (stop speaker),
the MIDI starter `8638` (stop MIDI). A digitized effect that is interrupted by music keeps sounding on its channel to its end
(VERIFIED in the code, not heard). check_sound_playing(ptr) answers per buffer: digi `[3216]`, MIDI `[327A]`, speaker `[3218]`.

Death counter (play_kid `0000:7980`): it waits while any sound plays **unless the current sound is 5** (gate opening; corrects
edge_cases.md 3.2, which read `[4360]` as a sound mode). Loose-floor shaking (`0000:947C`): with a digi device the sample rate of
the chosen 20/21/22 is set to 10500 + prandom(1000) Hz before play_sound (written into the resource header) - random pitch.

### 6.3 Devices, files and which version of a sound plays (VERIFIED unless marked)

Command-line words (`ds:0391..03B7`, parsed at `0000:1FD4..206A`): `stdsnd`, `adlib` 1, `gblast` 2, `sblast` 3, `covox` 4, `ibmg` 5,
`tandy` 7; else auto-detection `0C83:6E00` (AdLib timer test at `0C83:6F40`). set_sound_device `0C83:6F71`: `[31C0]` = device,
`[31C1]` bit 0 = digitized output available, bit 1 = MIDI driver installed. MIDI drivers (far entry `[3288]`): `690A` AdLib/OPL2
(devices 1 and 3, OPL writes at `0C83:6DD6`, port 388h), `7918` (2, Game Blaster, INFERRED), `7E94` (4), `821A` (5, 3 voices),
`87F6` (6, MPU-401 at 330h/331h; no command-line word found, INFERRED auto-detected), `894A` (7, Tandy BIOS int 1Ah).
For devices 1..3 the setup (`0000:2110`) loads **resource 1** (only PRINCE.DAT has an id 1: the instrument bank) and passes it to the driver
(`al = 5`, count byte, then 16-byte instruments; program change `0C83:6AEF` ignores programs >= the count).

load_sounds(first, last) `0000:0B9D` opens IBM_SND1, then DIGISND1 (+ DIGISND3 unless `[00B8]`, a low-memory flag, INFERRED)
when bit 0, then MIDISND1 when bit 1, and loads 10000 + id for each id; load_opt_sounds `0C89` does the same with the *2 files
(level sounds 44..49 by level, `0E46..0E8E`; title 50..55 at `182C`; ending 56 at `2F60`). The last opened archive is searched
first (VERIFIED by elimination: otherwise IBM_SND1, which holds 0..43, would always win), so per id: **MIDI > digitized >
speaker**. Typical setups:

| setup | effects | music cues (24..43, 50, 52..56) |
|---|---|---|
| PC speaker (`stdsnd`) | speaker (empty ones dropped by 2562) | speaker |
| AdLib | speaker | MIDI via OPL2 + PRINCE.DAT bank |
| Sound Blaster | digitized | MIDI via OPL2 + bank |
| MPU-401 (Roland) | speaker | MIDI to the external synth (programs 0..17 are not GM: sounds wrong on a GM module, UNVERIFIED which MT-32 patches) |

Music and effects are not different classes in the code: a "music cue" is simply an id whose best resource is MIDI or a long
speaker tune with priority 1 and interruptible 0.

### 6.4 The AdLib driver (`0C83:690A`, data segment `1AA7`) - traced

VERIFIED = read directly from code or data bytes (`tools/disasm`, PRINCE.EXE v1.1, PRINCE.DAT); INFERRED = a conclusion drawn from
that. The driver is only the voice layer; SMF parsing and the clock are in the sequencer (6.4.8). Its register write is `0C83:6DD6`
(`ah` = register, `al` = value): it keeps a shadow `ds1AA7:[48h + reg]` and writes ports 388h/389h (the settle delays are calibrated
loops, irrelevant for an emulator).

**6.4.1 Entry and dispatch (VERIFIED).** Far entry `690A`; `al <= 5` is a command, otherwise `al >> 4` is the MIDI status nibble;
jump through the word table at `1AA7:0004`. `ah` = channel, `dl` = data 1, `dh` = data 2 (C0/D0 have no data 2):

| index | handler | meaning |
|---|---|---|
| 0 | `692C` | init: calibrates the delay loops (runs the PIT at 5042 = 236.6 Hz for a moment, `6946`, then restores the game's PIT value), writes reg 01h = 0, BDh = 0, 01h = 20h (waveform select on), installs a dummy bank (count 1), reset |
| 1, 3 | `6A28` | reset / stop: A0h = 0 and B0h = 0 on all 9 voices (key off), BDh = 20h only if `[18C]` is not 9; `[148]` = FFFFh (all channels enabled), transpose `[191]` = 0, program of channel c = c if c < count else 0, voice tables cleared, `[18C]` = 9 |
| 4 | `6A46` | master volume 0..15 (`ah`), see 6.4.5 |
| 5 | `6ADF` | set bank: `[177]` = instrument count (`ah`), `[178]` = far pointer to the first instrument (`es:bx`), then reset of the channel table |
| 8 | `6BBE` | note off |
| 9 | `6B4D` | note on (velocity 0 = note off) |
| C | `6AEF` | program change: `[14A + ch] = dl`, only if `dl < [177]` (a program >= count is ignored) |
| F | `69D2` | sysex, see 6.4.6 |
| 2, 6, 7, A, B, D, E | `69D1` (`ret`) | **ignored: all controllers (7 volume, 10 pan, 64 sustain...), poly/channel pressure, pitch bend** |

All 16 channels are melodic (no drum channel 10, channel 9 is not special).

**6.4.2 Instrument layout, 16 bytes (VERIFIED: code `6C1C`/`6C4B`, data checked on the real bank).** PRINCE.DAT resource 1 = 289
bytes: u8 count (0x12 = 18; the DAT checksum byte in front is not counted) + 18 x 16. Register address = base + operator offset
(operator offsets by voice below).

| byte | register (melodic voice) | contents in the shipped bank |
|---|---|---|
| 0 | A0h, rhythm instruments only (fnum low) | 26h in all 18 |
| 1 | B0h, rhythm only (key-on bit stripped) | 0Ah in all 18 (fnum 226h, block 2) |
| 2 | C0h feedback/connection (bit 0 = 1: additive) | 04, 02, 0A, 08, 0E, 00 ... |
| 3 | modulator 20h (AM/VIB/EGT/KSR/MULT) | |
| 4 | modulator 40h (KSL bits 7..6, TL bits 5..0) | |
| 5 | modulator 60h (AR/DR) | |
| 6 | modulator 80h (SL/RR) | |
| 7 | modulator E0h (waveform) | |
| 8..12 | carrier 20h, 40h, 60h, 80h, E0h | same order |
| 13 | rhythm type: 0 melodic, 1 bass drum, 2 snare, 3 tom, 4 cymbal, 5 hi-hat | **0 in all 18** |
| 14, 15 | never read | 0 |

Write order for a melodic note: C0h, the 5 modulator registers, the 5 carrier registers (TL scaled, 6.4.5), then A0h, then B0h with
key-on. Example, instrument 0: `26 0A 04 | 52 88 41 21 00 | 73 86 71 14 00 | 00 00 00 00`.
Operator offsets (`ds:192`, standard OPL2), voice 0..8 = (modulator, carrier): (00,03) (01,04) (02,05) (08,0B) (09,0C) (0A,0D) (10,13) (11,14) (12,15).

**6.4.3 Note to pitch (VERIFIED, `6D56`).** `n = key - 31 + transpose` (`transpose` = `[191]`, signed 7 bit, default 0, set by the sysex
in 6.4.6; while `n < 0` add 12); `octave = n / 12`, `idx = n % 12`; `A0h = FNUM[idx] & 0xFF`;
`B0h = 0x20 | (octave << 2) | (FNUM[idx] >> 8)` (an octave >= 8 only spills into the key-on bit, so the block is `octave & 7`; keys that
high do not occur). B0h is remembered in `[16E + voice]`. `FNUM` (`ds:0030`, 12 words):
`0200 021E 023F 0261 0285 02AB 02D4 0300 032E 035E 0390 03C7` (idx 0 is key 31 + 12k; each step x1.0595, and 03C7 x1.0595 = 0400 = 0200 one block up).
With the OPL2 clock of 49716 Hz, key 60 at transpose 0 gives 129.5 Hz: **one octave below General MIDI numbering**. Every shipped file
contains the sysex "transpose +12" (one MIDISND1 file +24, 6.4.6), which gives 259 Hz (INFERRED to be the intent). The table is about 16 cents flat
against equal temperament (0200 at block 1 = 48.55 Hz vs G1 49.00 Hz, arithmetic only). **Pitch bend is ignored.**

**6.4.4 Voices, allocation and note off (VERIFIED, `6DA0`, `6BBE`, `6CC9`).** 9 voices (`[18C]` = 9; nothing ever writes another value). Per voice:
`[15C+v]` = key (0 = free), `[165+v]` = channel. Allocation is round robin: the search starts at the pointer `[2E]` and takes the first voice with
`[15C+v] == 0`; afterwards `[2E]` is advanced by exactly 1 (mod 9) from where it started, whichever voice was chosen. **There is no stealing**: with all 9
voices busy the note on is dropped. Note off (or note on with velocity 0) takes the first voice, in index order, whose key and channel both match, clears
`[15C+v]` and writes B0h = `[16E+v] & ~0x20` (key off); release velocity is ignored; only one voice is released per note off; no match = nothing happens.
The same key twice uses two voices. A note on is also dropped if master volume is 0 (`[190] == 3Fh`), if the channel is disabled in `[148]`, or if the
channel's program is >= the bank count.

**6.4.5 Volume (VERIFIED, `6C4B`, `6A46`).** Controller 7 and pressure do nothing. There are two inputs only.
- Velocity (data 2, 1..127), applied at note on to the carrier TL, and to the modulator TL only if the instrument is additive (`C0h & 1`):
  `tl = byte & 0x3F; v = ((tl + 64) * 225) / (vel + 161)` (integer division); `v = clamp(v, 64, 127) - 64`; `v = max(v, MASTER_FLOOR)`;
  `reg40 = (byte & 0xC0) | v` (KSL kept). A non-additive modulator TL is written unchanged. At vel 127 the factor is 0.78 (louder than the stored TL), at vel 1 it is 1.39.
- Master volume `[31C2]` (0..15), command 4: `MASTER_FLOOR = (vol >= 15) ? 0 : 63 - 4*vol` (`[190]`), a minimum attenuation. A change also rewrites reg 40h of every
  carrier (and additive modulator) whose current value is below the floor; raising the volume later does not restore sounding notes. `vol = 0`: floor 63 and every new
  note is dropped. The game only sets 0 or 15 (sound off/on: `0000:065D`, `0000:2966`; 15 at device setup `7090`), so in practice the floor is 0 and only the
  velocity formula matters.

**6.4.6 Sysex and other messages (VERIFIED).** Status F0 (`es:bx` = data after the length byte): accepted only if the payload starts `00 00 34 <00|01>`; then a
sub-command and an argument: `00 n` sets the transpose `[191]` to signed 7-bit n; `01 c` disables channel c (`[148]` bit cleared); `02 c` enables it. All shipped
files contain exactly `F0 07 00 00 34 01 00 0C F7` (transpose +12); one MIDISND1 file (offset 960) has `18` (+24). Everything else is ignored. The reset on every
stop/start sets the transpose back to 0 (a loop restart goes through the reset and then reads the sysex again).

**6.4.7 Rhythm mode (VERIFIED dormant, code `6CE3`).** Selected only by instrument byte 13 != 0 (0 in the whole bank) and it needs `[18C] != 9` (never set), so it
never runs. For reference: type 1 programs voice 6 like a melodic note, then A6h = byte 0, B6h = byte 1 & DFh, BDh |= 10h; types 2..5 write only the carrier part
(bytes 8..12, TL unscaled) to operator 14h/12h/15h/11h and A0h..C0h of voice 7/8/8/7 from bytes 0..2, then BDh |= 08h/04h/02h/01h; note off clears that BDh bit.
Tables: `ds:1BA` = 00 00 14 12 15 11, `ds:1C0` = 00 00 07 08 08 07, `ds:1C6` = 00 10 08 04 02 01. Not needed for a faithful remake.

**6.4.8 Tempo clock (VERIFIED; sequencer `0C83:8548` start, `86F8` rate, `8669` tick, `873E` events).** The PIT write at `6946` is only the driver's delay calibration.
The music is ticked by the game's software timer manager (`8D9B`) at **240 Hz** (`ds:3278` = 00F0, never written; PIT divisor 1193181/240 = 4971, so really 240.02 Hz).
Each timer tick adds a 16.16 increment `INC = floor(D2 * 65536 / D1)` to a phase accumulator, with `D1 = floor(256 * T / division)`, `D2 = floor(256000000 / 240)`
= 1066666, `T` = microseconds per quarter note, `division` = ticks per quarter from the header (480; an SMPTE division is rejected). So MIDI ticks per timer tick =
`division * 1e6 / (T * 240)`, 4.0 for 480 tpq at 500000. The integer part (with carry) is subtracted from the pending delta of every track and every due event runs in the
same interrupt; overshoot carries into the next delta (no drift). Tempo is 500000 until a tempo event, **only track 0 tempo events count**, and the default is re-applied at every start.
Meta events: 51 tempo (track 0), 07 cue point (first text byte stored in `[31C3]`), 2F end of track, all others skipped. Header `MThd` with format <= 1, `MTrk` chunks, running status
supported, F0/F7 go to the driver, C0/D0 take one data byte.
**Quirk (VERIFIED in code, audible effect INFERRED, worth checking against a DOSBox recording):** the three tempo bytes `t0 t1 t2` are fed to the rate routine as `t0, t2, t1`
(`87CF`/`87D3` into `86F8`, which expects the layout of its little-endian default), so the effective tempo is `T = t0*65536 + t2*256 + t1`: 07A120 becomes 0720A1 (467105 us, about 7 %
faster than the file), 061A80 becomes 06801A (about 6 % faster), 0B71B0 becomes 0BB071 (about 2 % slower), 0F4240 and 051615 hardly change. The MIDI lengths in 3.1 and 6.5 use the nominal tempo.
End of track 0 stops the sequence (reset above, all voices silent) and runs the completion callback `72AB`: with bit 7 (loop) of the resource type byte it restarts the same buffer at once
(`76D8`), otherwise it runs the game's callback.

**6.4.9 Data summary for the remake (Rust sketch, VERIFIED formulas).**

```rust
pub const FNUM: [u16; 12] = [0x200,0x21E,0x23F,0x261,0x285,0x2AB,0x2D4,0x300,0x32E,0x35E,0x390,0x3C7];
pub const OPS: [(u8, u8); 9] = [(0,3),(1,4),(2,5),(8,0xB),(9,0xC),(0xA,0xD),(0x10,0x13),(0x11,0x14),(0x12,0x15)]; // (mod, car)
// bank = PRINCE.DAT resource 1 minus the checksum byte: [count, 16 * count bytes]; inst b[]: 2=C0, 3..8=mod 20,40,60,80,E0, 8..13=car, 13=rhythm (0 in all)
fn tl_scaled(byte: u8, vel: u8, floor: u8) -> u8 {
    let v = (((byte as u32 & 0x3F) + 64) * 225 / (vel as u32 + 161)).clamp(64, 127) - 64;
    (byte & 0xC0) | (v as u8).max(floor)
}
fn key_regs(key: u8, transpose: i8) -> (u8, u8) { // (A0, B0 with key on)
    let mut n = key as i32 - 31 + transpose as i32;
    while n < 0 { n += 12 }
    let (oct, idx) = (n / 12, (n % 12) as usize);
    ((FNUM[idx] & 0xFF) as u8, 0x20 | ((oct << 2) as u8) | (FNUM[idx] >> 8) as u8)
}
// note on : voice = first free from `next` (round robin, free = key==0), else drop; next = (next+1)%9; prog = chan_prog[ch], drop if >= count;
//   C0=b[2]; mod: 20=b[3], 40= if b[2]&1 {tl_scaled(b[4])} else {b[4]}, 60=b[5], 80=b[6], E0=b[7];
//   car: 20=b[8], 40=tl_scaled(b[9]), 60=b[10], 80=b[11], E0=b[12]; then A0, B0.  floor = 0 for sound on.
// note off: first voice with same key+channel -> B0 &= !0x20, key=0.   Ignore CC, pitch bend, pressure; sysex 00 00 34 0x 00 n = transpose.
// clock : 240 Hz; inc16_16 = ((256_000_000/240) as u64 * 65536 / (256 * T / division) as u64), T = t0<<16 | t2<<8 | t1 (quirk) or nominal.
// reset : init regs 01=0, BD=0, 01=0x20; programs[c]=c (<count else 0); transpose 0; all channels enabled.
```

### 6.5 Title and story music 50..56 (VERIFIED, docs/cutscenes.md 2, 4.7, 4.8)

The cue ids are the resource ids minus 10000; no remapping. 54 intro music (show_title `181D`, after the first fade-in; the
logo page waits for its end), 55 story page 1 ("In the Sultan's absence"; load_intro waits for its end before the intro
scene), 50 princess theme, 51 door (digi only, else speaker), 53 Jaffar comes, 52 Jaffar leaves (all four in pv_scene `2B38`;
the title waits for 52 before the credits), 56 ending music (end_sequence `2F2F`, after cutscene `2960`). Lengths by setup:

| cue | speaker | MIDI (SMF) | page wait that depends on it |
|---|---|---|---|
| 54 | 24.74 s | 24.29 s | logo page -> story 1 (~t 1484 vs ~t 1457) |
| 55 | 14.86 s | 15.75 s | story 1 -> intro scene |
| 50 | 9.50 s | 14.50 s | intro scene: Jaffar waits (~95 vs ~145 frames) |
| 53 | 14.85 s | 17.26 s | intro scene: hourglass -> Jaffar leaves |
| 52 | 27.98 s | 28.89 s | page 43 -> title + credits |
| 56 | 120.04 s | 124.70 s | ending pages -> title (or a key) |

Other cues (speaker / MIDI): 24 2.97 / 3.60, 28 6.45 / 7.20, 32 4.92 / 7.25 (death-counter waits), 41 11.31 / 12.75 (stairs
exit wait), 39 18.61 / 18.67 (feather), 26 11.69 / 14.00, 27 10.73 / 12.00, 35 9.75 / 13.64, 36 12.72 / 15.73, 40 5.18 / 6.86.

### 6.6 MIDI channels and instruments (INFERRED where marked)

**Correction (melody check, 2026-11-18):** the key columns below are file key + transpose, but the driver sounds file key + transpose - 12 in GM numbering (6.4.3: +12 = GM pitch, so a +24 file sounds one octave up), i.e. 12 lower than listed when the transpose is +12 (cue 24 ch 1 sounds G#2-B2, not G#3-B3).

Source: `crates/pop-core/examples/midi_channels.rs` (throwaway analysis, `cargo run -p pop-core --example midi_channels --release`; reads `Original_data`, prints text only) over the 22 music resources of MIDISND1/2.DAT and PRINCE.DAT resource 1. Keys are **sounding GM-numbered keys = file key + the file's transpose** (6.4.3, C4 = 60). "prog" is the channel's starting program (a channel without a program change plays the instrument with its own channel number, 6.4.1 reset) followed by each program change as `>n@time`; the first change is at 0.0 s in every file that has any. Role is a heuristic (INFERRED) from register, polyphony and pitch variety (`rhythm` = at most 2 distinct pitches in 6+ notes, `chords` = 3+ simultaneous notes or over 35 % overlap, else `bass` below mean key 55, else `melody`); the track name in the file is the better hint.

**6.6.1 File layout and text events (VERIFIED).** Every file is SMF format 1, 480 ticks per quarter. Track 0 is an unnamed conductor track (tempo 51, SMPTE offset 54, time signature 58, End of Track 2F) with no notes. Every other track holds exactly **one channel** and exactly one text event: a **track name (meta 03)**. No other text/meta string exists (types 01, 02, 04..07 and 7F do not occur: no instrument names, lyrics, copyright or cue points). Channels are 0-based and contiguous from 0 (exceptions: cue 35 uses only ch 1 and 2; cue 39 skips ch 3). All channels are melodic (the driver has no drum channel).

The names do **not** name instruments; they name the musical part: `melody`, `harmony`, `mid`, `bass`, `lo bass`, `hi / mid / lo voice` (three-part writing, cues 30, 33, 37, 43), `heart`, `heart/tymp`, `tympani`, `tymp/gliss` (rhythmic hits), `comp`, `comp1/2` (accompaniment), `Bass chord`, `Bass build`. Suffix **`dbl`** = the same line doubled on a second channel with another instrument (the original layers two OPL voices per line, the only way to get a richer timbre from a chip with no controllers); **`-v`** / **`^`** = doubled lower / higher (measured: `-v` = -12, `^` = +12 or +24 against the parent line; the digits in `^8`, `^15` UNVERIFIED), `/gliss` = glissando figure, `+harm` / `/harm` = with a harmony note. Because the doublings use different programs, the 7-8 channels of a file typically carry only 3 to 4 distinct lines.

| cue | file | transpose | tracks | seconds | max simultaneous notes (all channels) |
|---|---|---|---|---|---|
| 24 | MIDISND1 | +12 | 8 | 3.60 | 7 |
| 25 | MIDISND1 | +0 | 8 | 2.47 | 6 |
| 26 | MIDISND1 | +24 | 9 | 14.00 | 9 |
| 27 | MIDISND1 | +12 | 8 | 12.00 | 7 |
| 28 | MIDISND1 | +12 | 9 | 7.20 | 8 |
| 29 | MIDISND1 | +0 | 8 | 5.29 | 8 |
| 30 | MIDISND1 | +12 | 10 | 3.54 | 9 |
| 32 | MIDISND1 | +12 | 8 | 7.25 | 7 |
| 33 | MIDISND1 | +12 | 10 | 2.34 | 9 |
| 35 | MIDISND1 | +0 | 3 | 13.64 | 2 |
| 36 | MIDISND1 | +12 | 9 | 15.73 | 9 |
| 37 | MIDISND1 | +0 | 10 | 4.12 | 9 |
| 39 | MIDISND1 | +12 | 7 | 18.67 | 6 |
| 40 | MIDISND1 | +12 | 5 | 6.86 | 4 |
| 41 | MIDISND1 | +12 | 8 | 12.75 | 8 |
| 43 | MIDISND1 | +0 | 10 | 3.83 | 9 |
| 50 | MIDISND2 | +12 | 7 | 14.50 | 6 |
| 52 | MIDISND2 | +12 | 8 | 28.89 | 8 |
| 53 | MIDISND2 | +0 | 8 | 17.26 | 8 |
| 54 | MIDISND2 | +12 | 8 | 24.29 | 8 |
| 55 | MIDISND2 | +12 | 8 | 15.75 | 8 |
| 56 | MIDISND2 | +12 | 9 | 124.70 | 8 |

The maximum is 9, reached in 26, 30, 33, 36, 37 and 43, exactly the OPL2 voice count, so the no-stealing allocation (6.4.4) never has to drop a note on count alone. Cues 37 and 43 have no program change and use the default channel-n-plays-instrument-n mapping; cue 35 plays channel 2 with instrument 2 and changes (to the same 2) only at 13.6 s.

**6.6.2 The 18 bank instruments decoded (VERIFIED decode, timbre guess INFERRED).** Cells: `xMULT TLtl AR/DR/SL/RR` then flags AM (tremolo), VIB (vibrato), EGT (sustaining: the envelope holds at SL until key off; absent = it decays on through RR, a plucked/struck sound), KSR (key-scale rate), KSL (level scaling 1..3). `fb` = modulator feedback 0..7. All 36 operators use **waveform 0 (sine)**, and all 18 use **FM connection (none additive)**, so velocity (6.4.5) only ever scales the carrier. Instruments 0..17 are the GM-independent bank; 10 is never used.

| # | fb | modulator | carrier | timbre guess (INFERRED) | where used |
|---|---|---|---|---|---|
| 0 | 2 | x2 TL8 4/1/2/1 VIB KSR KSL2 | x3 TL6 7/1/1/4 VIB EGT KSR KSL2 | soft sustained lead, reed/organ-like (carrier x3, attack 7; the modulator does not sustain, so the tone mellows) | melody (main theme 54, 56; 37, 43 ch 0) |
| 1 | 2 | x3 TL4 14/2/3/3 VIB EGT KSR KSL2 | x4 TL4 14/2/1/4 VIB KSR KSL2 | bright bell/glockenspiel-like (x3/x4, fast attack, carrier decays slowly) | melody/harmony, short notes |
| 2 | 5 | x2 TL14 9/7/2/4 KSL1 | x2 TL10 15/8/0/4 EGT KSR | bass: instant attack, sustained, feedback 5, modulator decays so the tone mellows; brass-like when played high | "bass" tracks in every cue |
| 3 | 2 | x2 TL13 2/2/2/2 VIB EGT KSR KSL2 | x0 TL11 3/1/4/4 VIB EGT KSR KSL2 | slow-swell pad: carrier x0 (sub-octave), attack 3, vibrato; string/choir-like | melody doubling in 56, 54, 37, 43 |
| 4 | 2 | x2 TL8 8/6/9/1 KSR KSL2 | x1 TL0 15/3/4/5 KSR | percussive thump: carrier x1, TL 0, EGT off, decay 3; timpani-like | "tymp"/"tympani" tracks |
| 5 | 2 | x3 TL4 14/2/3/1 VIB EGT KSR KSL2 | x4 TL3 14/2/1/1 VIB KSR KSL2 | bell/chime like 1 but with a long release (RR1) | melody, short notes |
| 6 | 1 | x4 TL19 1/1/1/1 VIB KSR KSL1 | x1 TL8 5/1/1/3 VIB EGT KSR KSL2 | mellow sustained mid voice: modulator x4 with very slow attack, carrier x1; flute/string-like | "mid", "mel dbl", harmony |
| 7 | 5 | x2 TL18 4/1/2/1 VIB KSR KSL2 | x6 TL8 7/1/1/4 AM VIB EGT KSR KSL2 | shimmering sustained organ-like: carrier x6 with tremolo+vibrato, feedback 5, slow attack | "Bass chord" (4-note chords) |
| 8 | 4 | x3 TL5 3/3/2/1 VIB EGT KSR KSL2 | x4 TL11 1/1/4/2 VIB EGT KSR KSL2 | bright sustained brass/string-like (strong modulator TL5, x3/x4, slow attack, long release) | melody doubling |
| 9 | 2 | x0 TL22 3/1/3/1 VIB KSR KSL2 | x3 TL8 6/2/1/4 VIB EGT KSR KSL2 | soft vibrato lead: modulator x0 and weak (TL22), carrier x3; flute/reed-like | melody (41, 52, 54, 55, 56) |
| 10 | 5 | x2 TL11 3/3/2/1 VIB EGT KSR KSL2 | x3 TL11 3/1/4/2 VIB EGT KSR KSL2 | similar to 6/8 (x2/x3, feedback 5, slow attack); **never used by any cue** | - |
| 11 | 2 | x0 TL6 6/1/14/1 KSL2 | x3 TL8 12/1/10/6 EGT KSR KSL2 | bass pad: fast attack then ducks to a plateau (SL10), modulator x0; plucked/string-bass-like | "Bass build dbl", "bass-v" |
| 12 | 0 | x3 TL15 7/3/2/1 VIB KSR KSL1 | x5 TL8 11/8/1/5 VIB EGT KSR KSL1 | clarinet/reed-like: no feedback, carrier x5 decaying (DR8) to a sustain | melody (26, 27, 50, 52) |
| 13 | 1 | x4 TL20 2/1/5/2 AM VIB EGT KSL2 | x6 TL4 5/2/5/8 AM VIB EGT KSL2 | glassy sustained: AM+VIB on both operators, x4/x6, slow attack; choir/ethereal | "hi voice" (30, 33), melody in 56 |
| 14 | 4 | x7 TL15 15/2/6/1 KSL1 | x2 TL23 15/2/7/1 KSR | bright pluck, harp- or marimba-like: modulator x7, fast attack, both operators decay (EGT off) | "hi voice" (26, 30, 33, 50) |
| 15 | 0 | x0 TL13 15/8/14/11 | x0 TL0 13/5/3/13 | pure percussion: both x0, EGT off, fast decay (DR8/DR5), RR11/13; heartbeat/tom thud | "heart" tracks (27, 35, 36, 40, 52) |
| 16 | 7 | x1 TL22 7/1/10/14 EGT | x1 TL0 8/1/9/14 EGT | harsh buzzy sustained: feedback 7, x1/x1, RR14 (cuts off at once); saw-like brass/string-section | "melody-^", "comp-^" (36 only) |
| 17 | 5 | x1 TL22 6/1/10/6 EGT | x1 TL9 11/1/9/5 AM VIB EGT | reedy/organ-like sustained: feedback 5, x1/x1, carrier AM+VIB, instant attack, ducks to SL9; accordion/brass-like | high "-^" doublings (24, 28; 26, 32, 36) |

The guesses follow the usual OPL patch reading (multiple 0/1 carrier with TL 0 and non-sustaining envelope = drum thud; modulator at low TL with multiples 3..7 = bright, bell- or pluck-like; sustained carrier with slow attack = pad/string; feedback 5..7 = buzzy) and are consistent with the track names that use them: 15 and 4 are the `heart`/`tymp` instruments, 2 is `bass`, 7 is `Bass chord`.

**6.6.3 Instrument by role and cue.** Notes counted per program in force at note on; roles are the heuristic of 6.6.4; "names" are the three most frequent track names (with notes).

| inst | notes | chans | roles (notes) | main track names (notes) | cues |
|---|---|---|---|---|---|
| 0 | 330 | 6 | melody 330 | melody 266, mid 50, hi 8 | 37, 43, 54, 56 |
| 1 | 303 | 15 | bass 3, chords 4, melody 296 | melody^ dbl 250, melody 16, melody dbl 16 | 24, 25, 29, 37, 40, 43, 52, 53, 55, 56 |
| 2 | 1345 | 39 | bass 612, melody 709, rhythm 24 | bass 554, bass dbl-v 473, bass^ dbl 168 | 24, 25, 26, 28, 29, 32, 35, 36, 37, 40, 41, 43, 52, 53, 54, 55, 56 |
| 3 | 273 | 4 | melody 273 | mel dbl 251, melody dbl 14, hi 8 | 37, 43, 54, 56 |
| 4 | 99 | 10 | bass 34, melody 42, rhythm 23 | tympani 40, comp/tympani 17, heart/tymp 17 | 25, 29, 32, 37, 41, 43, 52, 53, 54, 55 |
| 5 | 88 | 12 | bass 15, melody 73 | melody 56, melody/gliss 13, tymp/gliss 10 | 25, 29, 32, 37, 41, 43, 52, 53, 54, 55 |
| 6 | 137 | 34 | bass 16, chords 2, melody 119 | harmony 17, bass 15, mel dbl/harm 14 | 24, 26, 27, 30, 32, 33, 37, 39, 41, 43, 50, 52, 54 |
| 7 | 30 | 7 | bass 2, chords 18, melody 10 | Bass chord 18, mid 4, mid-^ 4 | 29, 37, 43, 53, 55 |
| 8 | 34 | 5 | bass 4, melody 30 | melody dbl 26, lo 4, lo-v 4 | 27, 37, 40, 43, 52 |
| 9 | 75 | 8 | chords 4, melody 71 | melody dbl 50, melody 15, mid 10 | 41, 52, 54, 55, 56 |
| 11 | 18 | 5 | bass 18 | Bass build dbl 10, bass-v 8 | 25, 29, 36, 53, 55 |
| 12 | 32 | 4 | melody 32 | melody 21, bass 11 | 26, 27, 50, 52 |
| 13 | 37 | 7 | melody 37 | mid^ dbl 19, hi voice 11, mid voice 4 | 30, 33, 56 |
| 14 | 49 | 9 | melody 49 | melody 12, mid 12, hi voice 11 | 26, 30, 33, 50 |
| 15 | 118 | 6 | melody 58, rhythm 60 | heart/tymp 66, heart 36, heart/comp1 8 | 27, 35, 36, 40, 52 |
| 16 | 18 | 2 | chords 9, melody 9 | comp-^ 9, melody-^ 9 | 36 |
| 17 | 69 | 13 | chords 17, melody 52 | melody 13, mid dbl+harm 13, melody-^ 9 | 24, 26, 28, 32, 36 |

**Findings.** (1) **Bass = instrument 2** in every file ("bass" and its `dbl-v` octave double, 612 bass-register notes in 17 cues; in 56 and 54 the same instrument also plays inner parts); **11** is the alternate soft bass pad (25, 29, 36, 53, 55) and 6 the "lo bass"/low voice. (2) **Melody**: 0 (main theme, 54/56), 1 and 5 (bell-like, the short-note melodies of 24, 25, 29, 53, 55), 9 (soft lead, 41/52/54/55/56), doubled by 3, 8, 12, 14, 13, 17, 16 (high-register `^` copies). (3) **Chords**: the only genuine chord channels (4 simultaneous notes) are the "Bass chord" tracks, instrument 7, in 29, 53, 55; every other channel is monophonic (a few overlap to 2, e.g. 26 ch 4, 32 ch 6, 36 ch 6, 41/54 ch 5). (4) **Rhythm/percussion-like**: 15 (heart: repeated D4/G2/G4/A2, cues 27, 35, 36, 40, 52) and 4 (tympani, one or two pitches, 17 hits in 52/54), plus cue 35 whose two channels both repeat G2 (the heartbeat). (5) The `hi/mid/lo voice` cues 30 and 33 and the default-program cues 37 and 43 are three-voice chorale blocks, each copied three times at different octaves with programs 14/6/13 (30, 33) or 0..8 (37, 43). (6) Surprise: a file has up to 8 notated lines but only 3-4 musical ones; names are the only text and they describe parts, not instruments. Instrument 10 and every non-sine waveform are unused; there is no volume or pitch-bend controller anywhere (consistent with the driver ignoring them).

**6.6.4 Channel tables, every music cue.** Columns: channel, file track name, program history, note count, sounding key range, mean velocity, max simultaneous notes on the channel, total sounding seconds (union of notes), median note length in seconds, role guess. The notes of the channels of one cue are the cross-tab of 6.6.3 per cue.

| cue | ch | track | prog | notes | keys | vel | poly | snd s | med s | role |
|---|---|---|---|---|---|---|---|---|---|---|
| 24 | 0 | harmony | 0>1@0.0s | 4 | 61-64 (C#4-E4) | 36 | 1 | 1.3 | 0.05 | melody |
| 24 | 1 | melody | 1>1@0.0s | 5 | 56-59 (G#3-B3) | 38 | 1 | 3.0 | 0.05 | melody |
| 24 | 2 | bass | 2>2@0.0s | 3 | 52-55 (E3-G3) | 64 | 1 | 2.8 | 1.20 | bass |
| 24 | 3 | bass-v | 3>2@0.0s | 3 | 40-43 (E2-G2) | 64 | 1 | 2.8 | 1.20 | bass |
| 24 | 4 | bass-v | 4>6@0.0s | 3 | 40-43 (E2-G2) | 64 | 1 | 2.8 | 1.20 | bass |
| 24 | 5 | harmony-^ | 5>17@0.0s | 4 | 85-88 (C#6-E6) | 64 | 1 | 1.3 | 0.05 | melody |
| 24 | 6 | melody-^ | 6>17@0.0s | 5 | 80-83 (G#5-B5) | 64 | 1 | 2.9 | 0.05 | melody |
| 25 | 0 | melody/gliss | 0>1@0.0s>5@0.3s | 4 | 54-56 (F#3-G#3) | 64 | 1 | 1.6 | 0.09 | melody |
| 25 | 1 | tymp/gliss | 1>4@0.0s | 1 | 38-38 (D2-D2) | 127 | 1 | 1.4 | 1.41 | bass |
| 25 | 2 | Bass | 2>2@0.0s | 1 | 43-43 (G2-G2) | 94 | 1 | 1.4 | 1.41 | bass |
| 25 | 3 | Bass dbl-v | 3>2@0.0s | 1 | 31-31 (G1-G1) | 94 | 1 | 1.4 | 1.41 | bass |
| 25 | 4 | melody dbl | 4>1@0.0s | 4 | 66-68 (F#4-G#4) | 64 | 1 | 1.7 | 0.09 | melody |
| 25 | 5 | Bass build dbl | 5>11@0.0s | 1 | 43-43 (G2-G2) | 64 | 1 | 1.4 | 1.41 | bass |
| 26 | 0 | melody | 0>14@0.0s | 12 | 85-102 (C#6-F#7) | 87 | 1 | 12.2 | 0.50 | melody |
| 26 | 1 | mid | 1>14@0.0s | 12 | 77-97 (F5-C#7) | 71 | 1 | 12.7 | 0.55 | melody |
| 26 | 2 | bass | 2>2@0.0s | 11 | 61-86 (C#4-D6) | 79 | 1 | 13.2 | 0.50 | melody |
| 26 | 3 | bass dbl-v | 3>6@0.0s>2@5.5s | 11 | 49-74 (C#3-D5) | 80 | 1 | 13.2 | 0.50 | melody |
| 26 | 4 | mid dbl+harm | 4>17@0.0s>6@9.5s | 15 | 49-97 (C#3-C#7) | 82 | 2 | 12.7 | 0.50 | chords |
| 26 | 5 | melody dbl | 5>6@0.0s | 12 | 85-102 (C#6-F#7) | 75 | 1 | 12.2 | 0.50 | melody |
| 26 | 6 | bass dbl | 6>6@0.0s>2@5.5s | 11 | 61-86 (C#4-D6) | 79 | 1 | 13.2 | 0.50 | melody |
| 26 | 7 | bass | 7>12@0.0s | 11 | 49-74 (C#3-D5) | 79 | 1 | 13.2 | 0.50 | melody |
| 27 | 0 | melody | 0>12@0.0s | 7 | 62-78 (D4-F#5) | 78 | 1 | 7.9 | 1.20 | melody |
| 27 | 1 | heart | 1>15@0.0s | 28 | 62-62 (D4-D4) | 64 | 1 | 8.4 | 0.40 | rhythm |
| 27 | 2 | bass | 2>6@0.0s | 4 | 62-72 (D4-C5) | 40 | 1 | 6.4 | 1.60 | melody |
| 27 | 3 | melody dbl | 3>8@0.0s | 7 | 74-90 (D5-F#6) | 62 | 1 | 5.9 | 1.20 | melody |
| 27 | 4 | bass dbl | 4>6@0.0s | 4 | 74-84 (D5-C6) | 28 | 1 | 5.6 | 1.20 | melody |
| 27 | 5 | mel dbl | 5>6@0.0s | 7 | 74-90 (D5-F#6) | 62 | 1 | 5.9 | 1.20 | melody |
| 27 | 6 | lo bass | 6>6@0.0s | 2 | 50-55 (D3-G3) | 40 | 1 | 4.8 | 3.20 | bass |
| 28 | 0 | harmony | 0>17@0.0s | 4 | 64-64 (E4-E4) | 64 | 1 | 4.7 | 1.57 | melody |
| 28 | 1 | melody | 1>17@0.0s | 4 | 59-60 (B3-C4) | 64 | 1 | 6.4 | 1.60 | melody |
| 28 | 2 | bass | 2>2@0.0s | 5 | 52-55 (E3-G3) | 64 | 1 | 6.4 | 1.60 | bass |
| 28 | 3 | bass dbl -v | 3>2@0.0s | 5 | 40-43 (E2-G2) | 74 | 1 | 6.4 | 1.60 | bass |
| 28 | 4 | harmony-^ | 4>17@0.0s | 4 | 76-76 (E5-E5) | 64 | 1 | 4.7 | 1.57 | melody |
| 28 | 5 | melody-^ | 5>17@0.0s | 4 | 71-72 (B4-C5) | 64 | 1 | 6.4 | 1.60 | melody |
| 28 | 6 | harmony -^15 | 6>17@0.0s | 4 | 88-88 (E6-E6) | 48 | 1 | 4.7 | 1.57 | melody |
| 28 | 7 | melody -^15 | 7>17@0.0s | 4 | 83-84 (B5-C6) | 48 | 1 | 6.4 | 1.60 | melody |
| 29 | 0 | melody/gliss | 0>1@0.0s>5@0.3s>2@2.4s | 6 | 42-56 (F#2-G#3) | 85 | 1 | 5.0 | 0.09 | bass |
| 29 | 1 | tymp/gliss | 1>4@0.0s | 2 | 38-38 (D2-D2) | 88 | 1 | 0.7 | 0.35 | bass |
| 29 | 2 | Bass | 2>2@0.0s | 5 | 38-43 (D2-G2) | 114 | 1 | 4.8 | 0.71 | bass |
| 29 | 3 | Bass dbl-v | 3>2@0.0s | 5 | 26-34 (D1-A#1) | 114 | 1 | 4.9 | 0.71 | bass |
| 29 | 4 | melody dbl | 4>1@0.0s | 4 | 66-68 (F#4-G#4) | 64 | 1 | 2.4 | 0.09 | melody |
| 29 | 5 | Bass build dbl | 5>11@0.0s | 1 | 43-43 (G2-G2) | 64 | 1 | 2.1 | 2.12 | bass |
| 29 | 6 | Bass chord | 6>7@0.0s | 6 | 26-54 (D1-F#3) | 56 | 4 | 2.5 | 2.47 | chords |
| 30 | 0 | hi voice | 0>14@0.0s | 7 | 78-84 (F#5-C6) | 80 | 1 | 2.6 | 0.12 | melody |
| 30 | 1 | mid voice | 1>14@0.0s | 3 | 74-78 (D5-F#5) | 80 | 1 | 2.9 | 1.17 | melody |
| 30 | 2 | lo voice | 2>14@0.0s | 2 | 67-71 (G4-B4) | 80 | 1 | 3.1 | 1.87 | melody |
| 30 | 3 | hi voice | 3>6@0.0s | 7 | 66-72 (F#4-C5) | 32 | 1 | 2.6 | 0.12 | melody |
| 30 | 4 | mid voice | 4>6@0.0s | 3 | 62-66 (D4-F#4) | 48 | 1 | 2.9 | 1.17 | melody |
| 30 | 5 | lo voice | 5>6@0.0s | 2 | 55-59 (G3-B3) | 64 | 1 | 3.1 | 1.87 | melody |
| 30 | 6 | hi voice | 6>13@0.0s | 7 | 90-96 (F#6-C7) | 32 | 1 | 2.6 | 0.12 | melody |
| 30 | 7 | mid voice | 7>13@0.0s | 3 | 86-90 (D6-F#6) | 32 | 1 | 2.9 | 1.17 | melody |
| 30 | 8 | lo voice | 8>13@0.0s | 2 | 79-83 (G5-B5) | 32 | 1 | 3.1 | 1.87 | melody |
| 32 | 0 | melody | 0>5@0.0s | 9 | 71-95 (B4-B6) | 78 | 1 | 6.5 | 0.08 | melody |
| 32 | 1 | tympani | 1>4@0.0s | 8 | 55-59 (G3-B3) | 114 | 1 | 1.9 | 0.19 | rhythm |
| 32 | 2 | bass | 2>2@0.0s>5@3.8s | 9 | 54-91 (F#3-G6) | 78 | 1 | 6.5 | 0.08 | melody |
| 32 | 3 | bass dbl-v | 3>2@0.0s | 3 | 42-43 (F#2-G2) | 80 | 1 | 3.9 | 0.88 | bass |
| 32 | 4 | harmony | 4>6@0.0s | 3 | 71-75 (B4-D#5) | 80 | 1 | 3.5 | 0.75 | melody |
| 32 | 5 | bass harmony | 5>2@0.0s | 2 | 62-63 (D4-D#4) | 72 | 1 | 3.9 | 2.25 | melody |
| 32 | 6 | melody dbl | 6>17@0.0s | 4 | 71-95 (B4-B6) | 64 | 2 | 6.0 | 3.00 | chords |
| 33 | 0 | hi voice | 0>14@0.0s | 4 | 78-83 (F#5-B5) | 80 | 1 | 1.9 | 0.09 | melody |
| 33 | 1 | mid voice | 1>14@0.0s | 1 | 74-74 (D5-D5) | 80 | 1 | 2.0 | 1.97 | melody |
| 33 | 2 | lo voice | 2>14@0.0s | 1 | 67-67 (G4-G4) | 80 | 1 | 2.1 | 2.06 | melody |
| 33 | 3 | hi voice | 3>6@0.0s | 4 | 66-71 (F#4-B4) | 32 | 1 | 1.8 | 0.09 | melody |
| 33 | 4 | mid voice | 4>6@0.0s | 1 | 62-62 (D4-D4) | 48 | 1 | 1.8 | 1.78 | melody |
| 33 | 5 | lo voice | 5>6@0.0s | 1 | 55-55 (G3-G3) | 64 | 1 | 1.9 | 1.88 | melody |
| 33 | 6 | hi voice | 6>13@0.0s | 4 | 90-95 (F#6-B6) | 32 | 1 | 1.7 | 0.09 | melody |
| 33 | 7 | mid voice | 7>13@0.0s | 1 | 86-86 (D6-D6) | 32 | 1 | 1.8 | 1.78 | melody |
| 33 | 8 | lo voice | 8>13@0.0s | 1 | 79-79 (G5-G5) | 32 | 1 | 1.9 | 1.88 | melody |
| 35 | 1 | heart/tymp | 1>15@0.0s | 24 | 43-43 (G2-G2) | 64 | 1 | 7.7 | 0.43 | rhythm |
| 35 | 2 | bass | 2>2@0.0s>2@13.6s | 24 | 43-43 (G2-G2) | 64 | 1 | 7.7 | 0.43 | rhythm |
| 36 | 0 | melody | 0>15@0.0s>17@4.2s | 17 | 53-73 (F3-C#5) | 63 | 1 | 13.4 | 0.46 | melody |
| 36 | 1 | heart/comp1 | 1>15@0.0s>17@4.0s | 14 | 53-68 (F3-G#4) | 66 | 1 | 13.4 | 0.67 | melody |
| 36 | 2 | bass | 2>2@0.0s | 16 | 48-63 (C3-D#4) | 61 | 1 | 13.4 | 0.67 | bass |
| 36 | 3 | bass-v | 3>2@4.3s | 8 | 36-51 (C2-D#3) | 64 | 1 | 10.2 | 1.00 | bass |
| 36 | 4 | comp2 | 4>17@4.3s | 4 | 56-65 (G#3-F4) | 64 | 1 | 8.6 | 2.00 | melody |
| 36 | 5 | melody-^ | 5>16@4.3s | 9 | 77-85 (F5-C#6) | 32 | 1 | 10.2 | 0.67 | melody |
| 36 | 6 | comp-^ | 6>16@4.3s | 9 | 68-80 (G#4-G#5) | 16 | 2 | 11.4 | 1.60 | chords |
| 36 | 7 | bass-v | 7>11@4.3s | 8 | 48-63 (C3-D#4) | 62 | 1 | 10.2 | 1.00 | bass |
| 37 | 0 | hi | 0 | 4 | 64-69 (E4-A4) | 48 | 1 | 1.9 | 0.38 | melody |
| 37 | 1 | mid | 1 | 4 | 57-62 (A3-D4) | 48 | 1 | 1.9 | 0.38 | melody |
| 37 | 2 | lo | 2 | 4 | 50-55 (D3-G3) | 48 | 1 | 1.9 | 0.38 | bass |
| 37 | 3 | hi | 3 | 4 | 76-81 (E5-A5) | 40 | 1 | 1.9 | 0.38 | melody |
| 37 | 4 | mid | 4 | 4 | 69-74 (A4-D5) | 40 | 1 | 1.9 | 0.38 | melody |
| 37 | 5 | lo | 5 | 4 | 62-67 (D4-G4) | 40 | 1 | 1.9 | 0.38 | melody |
| 37 | 6 | hi | 6 | 4 | 76-81 (E5-A5) | 64 | 1 | 1.9 | 0.38 | melody |
| 37 | 7 | mid | 7 | 4 | 69-74 (A4-D5) | 64 | 1 | 1.9 | 0.38 | melody |
| 37 | 8 | lo | 8 | 4 | 62-67 (D4-G4) | 64 | 1 | 1.9 | 0.38 | melody |
| 39 | 0 | hi 1 | 0>6@0.0s | 3 | 77-77 (F5-F5) | 48 | 1 | 13.3 | 5.33 | melody |
| 39 | 1 | hi 2 | 1>6@0.0s | 3 | 73-73 (C#5-C#5) | 48 | 1 | 16.0 | 5.33 | melody |
| 39 | 2 | bass | 2>6@0.0s | 3 | 45-45 (A2-A2) | 48 | 1 | 16.0 | 5.33 | bass |
| 39 | 4 | hi 1 | 4>6@0.0s | 1 | 77-77 (F5-F5) | 48 | 1 | 5.3 | 5.33 | melody |
| 39 | 5 | hi 2 | 5>6@0.0s | 3 | 73-73 (C#5-C#5) | 48 | 1 | 11.3 | 4.00 | melody |
| 39 | 6 | bass-^ | 6>6@0.0s | 2 | 57-57 (A3-A3) | 48 | 1 | 10.0 | 5.33 | melody |
| 40 | 0 | melody | 0>1@0.0s | 4 | 61-63 (C#4-D#4) | 94 | 1 | 3.7 | 0.11 | melody |
| 40 | 1 | heart | 1>15@0.0s | 8 | 67-67 (G4-G4) | 64 | 1 | 2.6 | 0.43 | rhythm |
| 40 | 2 | bass | 2>2@0.0s | 4 | 49-51 (C#3-D#3) | 94 | 1 | 3.6 | 0.11 | bass |
| 40 | 3 | melody dbl | 3>8@0.0s | 4 | 61-63 (C#4-D#4) | 94 | 1 | 1.2 | 0.11 | melody |
| 41 | 0 | melody | 0>5@0.0s>9@8.2s | 21 | 69-85 (A4-C#6) | 72 | 1 | 11.2 | 0.19 | melody |
| 41 | 1 | tympani | 1>4@0.0s | 17 | 45-59 (A2-B3) | 112 | 1 | 3.9 | 0.19 | bass |
| 41 | 2 | bass | 2>2@0.0s | 14 | 45-55 (A2-G3) | 70 | 1 | 10.8 | 0.69 | bass |
| 41 | 3 | bass dbl-v | 3>2@0.0s | 14 | 33-43 (A1-G2) | 57 | 1 | 10.8 | 0.69 | bass |
| 41 | 4 | harmony | 4>6@0.0s | 7 | 71-76 (B4-E5) | 89 | 1 | 8.2 | 0.75 | melody |
| 41 | 5 | bass harmony | 5>2@0.0s | 15 | 57-64 (A3-E4) | 69 | 2 | 10.8 | 0.69 | melody |
| 41 | 6 | melody dbl | 6>9@0.0s | 16 | 71-85 (B4-C#6) | 46 | 1 | 8.2 | 0.19 | melody |
| 43 | 0 | hi | 0 | 4 | 64-69 (E4-A4) | 80 | 1 | 2.5 | 0.50 | melody |
| 43 | 1 | mid | 1 | 4 | 57-62 (A3-D4) | 80 | 1 | 2.5 | 0.50 | melody |
| 43 | 2 | lo | 2 | 4 | 50-55 (D3-G3) | 80 | 1 | 2.5 | 0.50 | bass |
| 43 | 3 | hi | 3 | 4 | 64-69 (E4-A4) | 80 | 1 | 2.5 | 0.50 | melody |
| 43 | 4 | mid | 4 | 4 | 57-62 (A3-D4) | 80 | 1 | 2.5 | 0.50 | melody |
| 43 | 5 | lo | 5 | 4 | 50-55 (D3-G3) | 80 | 1 | 2.5 | 0.50 | bass |
| 43 | 6 | hi-^ | 6 | 4 | 76-81 (E5-A5) | 48 | 1 | 2.5 | 0.50 | melody |
| 43 | 7 | mid-^ | 7 | 4 | 69-74 (A4-D5) | 48 | 1 | 2.5 | 0.50 | melody |
| 43 | 8 | lo-v | 8 | 4 | 26-31 (D1-G1) | 96 | 1 | 2.5 | 0.50 | bass |
| 50 | 0 | melody | 0>12@0.0s | 7 | 62-90 (D4-F#6) | 41 | 1 | 13.0 | 1.25 | melody |
| 50 | 1 | lo bass at end | 1>6@0.0s | 2 | 50-55 (D3-G3) | 40 | 1 | 8.2 | 4.50 | bass |
| 50 | 2 | bass | 2>6@0.0s | 4 | 62-72 (D4-C5) | 40 | 1 | 13.0 | 4.50 | melody |
| 50 | 3 | melody dbl-^8 | 3>14@0.0s | 7 | 74-90 (D5-F#6) | 30 | 1 | 13.0 | 1.25 | melody |
| 50 | 4 | bass dbl-^8 | 4>6@0.0s | 4 | 74-84 (D5-C6) | 28 | 1 | 11.5 | 3.00 | melody |
| 50 | 5 | melody dbl-^8 | 5>6@0.0s | 7 | 74-90 (D5-F#6) | 30 | 1 | 13.0 | 1.25 | melody |
| 52 | 0 | melody | 0>1@0.0s>12@6.8s>5@16.1s>9@24.4s | 32 | 61-85 (C#4-C#6) | 76 | 1 | 23.4 | 0.19 | melody |
| 52 | 1 | heart/tymp | 1>15@0.0s>4@16.1s | 59 | 57-71 (A3-B4) | 78 | 1 | 16.3 | 0.21 | melody |
| 52 | 2 | bass | 2>2@0.0s>6@6.8s>2@16.1s | 26 | 45-72 (A2-C5) | 69 | 1 | 21.7 | 0.69 | bass |
| 52 | 3 | melody dbl | 3>8@0.0s>9@16.1s | 31 | 49-90 (C#3-F#6) | 59 | 1 | 17.2 | 0.19 | melody |
| 52 | 4 | bass dbl/harm | 4>6@6.8s>2@13.1s | 25 | 57-84 (A3-C6) | 61 | 2 | 17.7 | 0.69 | melody |
| 52 | 5 | mel dbl/harm | 5>6@6.8s | 14 | 71-90 (B4-F#6) | 76 | 1 | 14.0 | 1.14 | melody |
| 52 | 6 | lo bass | 6>6@6.8s>2@15.4s | 16 | 33-55 (A1-G3) | 55 | 1 | 15.4 | 0.69 | bass |
| 53 | 0 | melody/gliss | 0>1@0.0s>5@0.3s>2@2.4s>5@14.3s | 16 | 42-98 (F#2-D7) | 70 | 1 | 8.0 | 0.06 | melody |
| 53 | 1 | tymp/gliss | 1>4@0.0s>5@14.3s | 24 | 38-93 (D2-A6) | 56 | 1 | 5.8 | 0.09 | bass |
| 53 | 2 | Bass | 2>2@0.0s | 8 | 38-43 (D2-G2) | 98 | 1 | 9.1 | 1.41 | bass |
| 53 | 3 | Bass dbl-v | 3>2@0.0s | 8 | 26-34 (D1-A#1) | 94 | 1 | 9.2 | 1.41 | bass |
| 53 | 4 | melody dbl | 4>1@0.0s | 4 | 66-68 (F#4-G#4) | 64 | 1 | 2.4 | 0.09 | melody |
| 53 | 5 | Bass build dbl | 5>11@0.0s | 4 | 38-43 (D2-G2) | 40 | 1 | 6.4 | 1.41 | bass |
| 53 | 6 | Bass chord | 6>7@0.0s | 6 | 26-54 (D1-F#3) | 56 | 4 | 2.5 | 2.47 | chords |
| 54 | 0 | melody | 0>0@0.0s>5@14.5s>9@19.8s | 33 | 69-85 (A4-C#6) | 69 | 1 | 19.3 | 0.19 | melody |
| 54 | 1 | comp/tympani | 1>0@0.0s>4@11.1s | 23 | 45-75 (A2-D#5) | 97 | 1 | 11.0 | 0.19 | melody |
| 54 | 2 | bass | 2>2@0.0s | 47 | 45-67 (A2-G4) | 52 | 1 | 22.4 | 0.55 | melody |
| 54 | 3 | bass dbl-v | 3>2@0.0s | 47 | 33-55 (A1-G3) | 52 | 1 | 22.4 | 0.55 | bass |
| 54 | 4 | harmony | 4>6@0.0s | 7 | 71-76 (B4-E5) | 89 | 1 | 8.2 | 0.75 | melody |
| 54 | 5 | bass harmony | 5>2@0.0s | 15 | 57-64 (A3-E4) | 69 | 2 | 10.8 | 0.69 | melody |
| 54 | 6 | melody dbl | 6>3@0.0s>9@14.5s | 28 | 71-85 (B4-C#6) | 49 | 1 | 16.3 | 0.19 | melody |
| 55 | 0 | melody | 0>1@0.0s>5@6.7s>2@8.9s>7@11.9s | 8 | 49-65 (C#3-F4) | 76 | 1 | 8.4 | 0.09 | melody |
| 55 | 1 | tympani | 1>4@0.0s | 15 | 45-45 (A2-A2) | 113 | 1 | 3.2 | 0.19 | rhythm |
| 55 | 2 | Bass | 2>2@0.0s>7@11.9s | 10 | 45-69 (A2-A4) | 88 | 1 | 12.7 | 1.50 | bass |
| 55 | 3 | Bass dbl-v | 3>2@0.0s | 8 | 33-41 (A1-F2) | 94 | 1 | 9.8 | 1.50 | bass |
| 55 | 4 | melody dbl | 4>1@0.0s>9@11.9s | 8 | 69-81 (A4-A5) | 56 | 2 | 5.6 | 0.09 | chords |
| 55 | 5 | Bass build dbl | 5>11@0.0s | 4 | 45-50 (A2-D3) | 40 | 1 | 6.8 | 1.50 | bass |
| 55 | 6 | Bass chord | 6>7@0.0s | 6 | 33-61 (A1-C#4) | 56 | 4 | 2.7 | 2.62 | chords |
| 56 | 0 | melody | 0>0@0.0s | 252 | 57-87 (A3-D#6) | 75 | 1 | 112.0 | 0.17 | melody |
| 56 | 1 | mid | 1>0@0.0s>9@9.8s>0@20.0s>9@37.8s>0@48.0s | 60 | 64-79 (E4-G5) | 68 | 1 | 94.0 | 1.50 | melody |
| 56 | 2 | bass | 2>2@0.0s | 400 | 45-71 (A2-B4) | 64 | 1 | 121.7 | 0.17 | melody |
| 56 | 3 | bass dbl-v | 3>2@0.0s | 400 | 33-59 (A1-B3) | 64 | 1 | 121.7 | 0.17 | bass |
| 56 | 4 | mel dbl | 4>3@0.0s | 251 | 57-87 (A3-D#6) | 75 | 1 | 110.5 | 0.17 | melody |
| 56 | 5 | melody^ dbl | 5>1@0.0s | 250 | 69-99 (A4-D#7) | 53 | 1 | 107.5 | 0.17 | melody |
| 56 | 6 | mid^ dbl | 6>13@0.0s | 19 | 76-90 (E5-F#6) | 51 | 1 | 39.0 | 2.00 | melody |
| 56 | 7 | bass^ dbl | 7>2@0.0s | 168 | 57-73 (A3-C#5) | 74 | 1 | 52.5 | 0.17 | melody |

## 7. Playing the MIDI music: options

**What has to be built either way**: an SMF sequencer (tempo map, 16 channels, note on/off, program change, controllers 7/10/
64, pitch bend, the loop flag) feeding a synth that renders into the existing `Pcm`/`Decodable` path. The sequencer is small
(the inventory tool already walks the events); `midly` (Unlicense, 0.6 M downloads, stable since 2023) is the standard parser
if a crate is preferred.

| option | crates (checked on crates.io 2026-11) | licence | sound | data needed | notes |
|---|---|---|---|---|---|
| A. OPL2 emulation + original bank | `oplon` 1.0 (MIT, pure Rust, no_std, OPL2/OPL3, new: 2026-07, ~3 k downloads); `nuked-opl3` 0.1 (LGPL-2.1+, pure-Rust port of Nuked-OPL3, 2026-06); `opl3-rs` 0.2 (LGPL-2.1, bindings to the C Nuked-OPL3, needs a C compiler via `cc`); `opl-emu` (GPL-3.0); `opl-audio` 0.1 (MIT/Apache, OPL renderer with its own MIDI + AdLib-bank layer, new) | MIT or LGPL | **authentic** (AdLib / Sound Blaster) | PRINCE.DAT resource 1 (user's copy) | needs the driver behaviour of 6.4 traced; a few hundred lines; tiny binary cost |
| B. SoundFont | `rustysynth` 1.3 (MIT, pure Rust, plays SMF directly, 0.19 M downloads, active); `oxisynth` 0.1 (LGPL-2.1, FluidSynth port) | MIT | GM-style, not the original timbre | an .sf2 (not in the repo) + a hand-made map of programs 0..17 -> GM | GeneralUser GS (own permissive licence), FluidR3_GM / MuseScore_General (MIT) may be redistributed; TimGM6mb is GPL-2. Simplest is "user downloads a soundfont and sets its path"; bundling only with an MIT soundfont and its licence text |
| C. Keep synthesized / speaker | none | - | current own jingles / square-wave tunes | none | no MIDI at all; lengths stay the speaker ones |

Recommendation: **A with `oplon`** (MIT keeps every licence choice open; the project has no licence yet, and LGPL in a statically
linked Rust exe needs relinkable objects or source), `nuked-opl3` as the accuracy fallback if oplon misbehaves, `midly` or the
own walker for the SMF. Option B can be added later behind a setting; it does not need the driver tracing.

**How it plugs into audio.rs**: a new `sound` mode `adlib` (settings.json / `POP_SOUND`): digitized effects where they exist
(Sound Blaster setup), else speaker, music from MIDI. A `Midi` asset implementing `Decodable` whose decoder runs sequencer + OPL
lazily (streaming, 49716 Hz OPL rate resampled or rendered at 44100/22050) - no startup cost, nothing cached on disk. The clip
length (`steps`) comes from the SMF tempo map, not from rendering, so `busy`, the death counter, the stairs exit and the
cutscene/title waits stay deterministic and independent of the audio thread. The title/cutscene length tables (synth.rs
padding, cutscene.rs speaker lengths) become per-mode: speaker lengths for synthesized/original-speaker, SMF lengths for
adlib/soundfont.

**Limiter**: bevy_audio mixes each `AudioPlayer` itself and exposes no hook on the sum. Two ways:
1. *Minimal* (now): keep headroom (clips <= 0.5, speaker 0.25), give MIDI renders a peak of ~0.35 and, with the original rule
   (one new sound per frame, music not stacking), real overlaps are at most music + one tail.
2. *Own mixer* (if wanted): one endless `Decodable` "mixer" source played by a single `AudioPlayer`; `play`/`stop_all`/volume
   send commands over a channel; per sample: sum voices x gain -> master gain -> peak limiter (instant attack on |x| > 0.89
   (-1 dBFS), release ~80 ms, gain smoothing per sample) -> soft clip `x / (1 + |x|)`-style above the knee as a safety net.
   ~120 lines plus a test that feeds a full-scale sum and checks |out| <= 1 and transparency below the threshold.

## 8. M7 plan

Effort: XS < 0.5 d, S ~1 d, M 2-3 d, L ~1 week. All tests headless; data tests skip when Original_data is missing (as now).

| WP | what | effort | depends on | tests |
|---|---|---|---|---|
| 1 | `pop-core::sound`: original engine state machine (queued, current, play_sound / play_next_sound of 6.2, per-frame call), tables from `Exe` (priority 58 bytes, interruptible `24EE`, pcspeaker_exists `2562`), "playing" from caller-supplied lengths per channel | S | - | unit tests for each rule (drop on non-interruptible, ties, music over effect, feather over all, queue lost after one frame, cutscene queue survives); table test against PRINCE.EXE |
| 2 | Death counter exception (current sound 5 does not hold it) in `World::hold_death_counter` path; correct edge_cases.md 3.2 | XS | 1 | world test: gate opening during death does not delay the counter |
| 3 | pop-app: drive playback through WP1 (setting `sound_rules`: original / polyphonic for the synthesized set if preferred); `busy` = any channel busy; digi keeps playing when music starts | S | 1 | app unit tests on AudioState like the existing busy tests |
| 4 | SMF sequencer (own walker from tools/sound_inventory, or `midly`): event list with absolute 60 Hz-step times; length table per id | S | - | lengths match the inventory (54 = 24.29 s ...), synthetic SMF tests |
| 5 | Trace the AdLib driver (6.4): instrument layout, F-number table, voice allocation, volume/velocity, sysex, loop | M | - (disasm) | doc with VERIFIED labels; a register-write trace test: given an SMF, the emitted OPL register list for the first notes |
| 6 | OPL backend (`oplon`): driver re-implementation writing registers into the emulator; streaming `Decodable`; sound mode `adlib` | M | 4, 5 | render id 24 headless: non-silent, finite, length = SMF length, peak below the headroom; register trace from WP5 |
| 7 | Per-mode lengths for title/cutscene waits (synth.rs padding, cutscene.rs table, flow.rs) | S | 4 | cutscene timing tests per mode (intro scene frame counts with 9.50 vs 14.50 s for 50) |
| 8 | Limiter: minimal headroom for MIDI (XS) or own mixer + limiter (M) | XS / M | 6 | sum test of 7 |
| 9 | Optional: SoundFont backend (`rustysynth`, user-supplied .sf2 path in settings, program map 0..17 -> GM) | M | 4 | renders with a test soundfont if one is present, else skip |
| 10 | Docs, settings, README (where to get a soundfont, if WP9) | XS | all | - |

Critical path: 5 -> 6 (the driver tracing is the real work). 1-4 and 7 can start at once. Total for A: about 2 weeks; B adds ~3 d.

**Decisions (made by the user, 2026-11-18)**
1. A+B: OPL emulation with the original bank AND SoundFont, selectable at runtime in the settings menu.
2. Keep the MIT licence: MIT/Unlicense crates only (`oplon`, `midly`, `rustysynth`); no LGPL/GPL.
3. Stay polyphonic: the original single-sound rules (6.2) are NOT applied to playback (WP1/WP3 shrink to "playing"/length
   bookkeeping; WP2 stays a sim rule to re-check separately).
4. Waits follow the length of what is actually heard in each mode.
5. Own mixer with limiter (WP8 = M).
6. SoundFonts are NOT shipped: the user downloads them in Settings > SoundFonts after a licence notice (changed 2026-11-18).
   Sources (verified by HEAD / licence file, 2026-11): MuseScore_General.sf2 215 614 036 bytes from ftp.osuosl.org (MIT, notices must
   stay with the file; saved next to it); FluidR3_GM from Debian's `fluid-soundfont-gm_3.1-6_all.deb` 119 613 032 bytes (MIT, Frank
   Wen; unpacked with Windows tar.exe). `rustysynth` 1.3.8 rejects SF3 (Ogg samples, `soundfont_sampledata.rs`), so MuseScore's
   38 MB .sf3 is unusable; only SF2. Program map 0..17 -> GM (`music::GM_MAP`) is an INFERRED guess from the analysis in 6.6 (no source names the instruments; the MT-32 patch order does not fit), to be corrected by ear.
   The download and unpack path (`soundfont.rs`) is not yet tested against the real servers.
7. "Modern effects and music" replacement stays out of M7.

**Original option list (for reference)**
1. Music backend: A OPL emulation with the original bank (recommended), B SoundFont, C keep synthesized/speaker only.
2. Crate licence policy (the project has no licence yet): MIT/Unlicense only (`oplon`, `midly`, `rustysynth`) or LGPL acceptable
   (`nuked-opl3`, `opl3-rs`, `oxisynth`); GPL (`opl-emu`) excluded unless the project becomes GPL.
3. Interruption rules: faithful single-sound rules everywhere, only in the original/adlib modes, or keep polyphony for the
   synthesized set.
4. Which lengths drive the waits in each mode (proposal: the lengths of what is actually heard).
5. Limiter: headroom only, or the own mixer.
6. SoundFont (if B): user-downloaded only, or bundle an MIT soundfont (FluidR3_GM / MuseScore_General, 30-140 MB).
7. The roadmap's "modern effects and music" replacement stays out of M7 (it needs assets with a clear licence).

**Risks**: the AdLib driver details (WP5) take longer than estimated or have quirks (note stealing, volume curve) that make the
music sound "almost right"; `oplon` is new (fallback `nuked-opl3`, LGPL); faithful dropping rules may feel like missing sounds to
players used to polyphony; SMF length vs the driver's own end-of-music moment (waits off by up to ~1 s); streaming synthesis
on the audio thread must stay cheap (OPL2 at 49716 Hz is light, a SoundFont with long tails less so).

**What blocks verification**: no reference recordings of the original on AdLib / Sound Blaster / PC speaker (DOSBox captures
by the user would settle lengths, priorities as heard and timbre); nobody has listened to the remake's output; the MPU-401 path
and which MT-32 patches programs 0..17 mean; the sysex meaning; the speaker loop path (bit 7) and who stops the looping door
sound 15.

## 9. Open / UNVERIFIED

- The 2750 Hz rate of digi 15 (a looping sound: maybe intended, the loop hides its shortness).
- The AdLib driver details (6.4), the sysex, PRINCE.DAT resource 2.
- Whether "MIDI playing" ends at End of Track or at the last note (MIDI lengths in 3.1 include trailing rests).
- The remake still plays polyphonically and restarts the same id; the original rules of 6.2 are not implemented yet (M7 plan).
- Audio output itself was not listened to during development (verified by unit tests and debug logging: `RUST_LOG=pop_app=debug`
  prints every played id).

### 6.7 Melody check (2026-11-18)

Throwaway tool `crates/pop-core/examples/melody_check.rs`. (C) `adlib` / `soundfont`: every OPL key-on of cues 24, 26, 28, 32, 36, 39, 41, 52, 54, 56 matches an SMF note-on in pitch (file key + transpose - 12) and start time, no notes dropped or added (cue 56: 1800 of 1800), note-offs within 1 ms; the FNUM table is ~15 cents flat; the tempo-byte quirk (6.4.8) is not applied (+-7 % timing). (B) `original`: the speaker tunes decode correctly and are the MIDI melodies as one voice with the bass interleaved (cues 24, 25, 30, 35, 37, 40, 43 agree exactly with a MIDI line); the speaker versions of cues 26 and 56 are a whole tone below the MIDI ones (data difference, not verified by ear); cue 39's speaker version is an arpeggio, the MIDI holds long notes. (A) the old `synth` set used invented phrases (e.g. cue 41: C E G C B C D E instead of B B E F G# F E A A# C# A# A); since 0.4.1 every set except `original` plays the melodies from the MIDI files (OPL), `synth` keeps only its procedural effects.

## 10. Every play_sound call site of PRINCE.EXE v1.1 (audit 2026-11-18)

All 51 `call 0000:12F0` (tools/disasm `callers 0000:12F0`) with the id pushed (hex in the code, decimal here). CS offset = file offset minus 0x890. VERIFIED = read in the disassembly; INFERRED = condition or purpose guessed. "Remake" = where `pop-core` / the app emits it.
Ids 50..56 (and the title music) are queued by the cutscene scripts, `pop_core::cutscene` (docs/cutscenes.md). Ids 20..22 and the
death cue are variables (`94A7`: shake sound; `8335`: 24/28/32); `9CC4` is the shared "play if the room is visible" wrapper
(ids 4, 5, 7, 14, 47 at its callers).

| id | situation | condition | CS offset | remake |
|---|---|---|---|---|
| **25** | **level 1 start music** (need_level1_music `ds:4CE8`, set to 1 by play_level when level == 1, `48B2`) | kid control, frame 109 (crouch), level 1, state 1, no sound playing -> play, state 2; next such frame state 0. Skipped when sound is off (`ds:31C2`) | `6175` in `6153` (VERIFIED) | **was MISSING**; now `kid.rs control_crouched` + `Shared::level1_music` (test `level1_start_music`) |
| **41** | united with the level-12 shadow | draw of tile kind 0/1 while `ds:4FCE` == 2 (42 ticks after the union) | `BDFF` in `BD92` (VERIFIED code; INFERRED: it fires once, as a tile of the room is drawn) | **was MISSING**; now `sim.rs` timers (test `united_with_shadow_cue_41`) |
| 25 | level 6: shadow appears in room 1 | level 6, room `[40DE]` == 1, `[40DC]` != 0x4D, once | `37BF` (VERIFIED) | `sim/combat.rs:264` |
| 25 | level 4: mirror music | leaving room 11 to the left on row 0, door open != 0 / 0x4D | `3DEA` (VERIFIED) | `sim.rs:433` |
| 29 | meet Jaffar | level 13, door closed, room 3 | `3DB7` (VERIFIED) | `sim.rs:437` |
| 24/28/32 | death music | death counter step 6 (not levels 0, 15): shadow -> 32, sword drawn -> 28, else 24 | `832B`..`8335` (VERIFIED) | `sim.rs play_kid_counter` |
| 37, 43 | guard killed, Jaffar killed | `on_guard_killed` | `837E`, `8372` (VERIFIED) | `sim/combat.rs:498-500` |
| 37, 33, 30, 39, 13 | sword pick-up, heal, life, feather, hurt potion | proc_get_object | `81CC`, `8223`, `8249`, `162B`, `8297` | `sim.rs get_object` |
| 41 / 32 / none | level-end music | seq op F2 4 (and `ds:31C2` != 0): level 4 -> 32, 13 and 15 none, else 41 | `7097`, `70B0` (VERIFIED) | `sim.rs:472`; F2 1/2/3: 23, 8, 18 (`706E`, `707F`, `7084`) |
| 33, 30 | cheat keys heal / life | key handler | `088B`, `08A8` (INFERRED debug keys) | not modelled (docs/edge_cases.md) |
| 26, 27, 35, 36, 40, 50..53 and 4 | ending, princess, mouse, time up, intro scenes | cutscene scripts (`2897`..`2CE5`; id 4 = `2BA4`, the door in the intro scene) | `2897`..`2CE5` (VERIFIED) | `cutscene.rs` scripts |
| 0, 1, 16, 17 | land, scream, hard/soft landing | fall code | `6002`, `5EAC`, `5FE4`, `5FB1` | kid events (docs section 2) |
| 8, 9, 10, 11, 12, 13, 19, 44..49, 2..7, 14, 15, 3, 20..22 | bumps, grab, swords, hurt, draw, skeleton, mirror, chomper, spikes, loose floor, gates, exit door, button | see section 2 | see section 2 | yes |

Sounds 31, 34, 42, 38 and the story music 54..56 have no call through `12F0` (54..56 are started by the title / end code in
the runtime segment; 38 is not called from PRINCE.EXE game code). Table-driven (per level) ids do not exist: every site pushes an immediate or
one of the variables named above, so there is no "level start jingle" other than 25 on level 1. Levels 2..14 start silent in the
game (the cutscenes before 2, 4, 6, 8, 9, 12 play their own cues).

The level-1 cue relies on the app's `hold_death_counter` (= a sound plays, section 5) as the "sound playing" test (`0C83:720D`), so
the cue waits for the start sounds (door button) to end, as in the original.
