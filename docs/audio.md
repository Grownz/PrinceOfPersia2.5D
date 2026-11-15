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
| 26 | embrace (ending cutscene `2960`) | edge_cases 4.1 | card | - | 11.69 s |
| 27 | princess waiting (cutscene before 2, 4, 6) | `2B04`, `2AD0` | card | - | 10.73 s |
| 28 | death music, holding the sword | `8324` | yes | - | 6.45 s |
| 29 | meet Jaffar | `3DB7` | yes | - | 3.94 s |
| 30 | life potion | `8249` | yes | - | 2.33 s |
| 32 | shadow music (death by the shadow; level-4 stairs) | `8316`, F2 4 | yes | - | 4.92 s |
| 33 | heal potion | `8223` | yes | - | 2.48 s |
| 35 | princess and the mouse (cutscene before 8, 9) | `2891`, `28FB` | card | - | 9.75 s |
| 36 | time is up (cutscene `2A1B`) | edge_cases 3.5 | card | - | 12.72 s |
| 37 | guard killed / sword picked up | `837E`, `81CC` | yes | - | 2.54 s |
| 38 | blink beep ("Press Button" blink, UNVERIFIED) | `0C83:7198`? | no | - | 0.06 s |
| 39 | feather potion | `162B` | yes | - | 18.61 s |
| 40 | cutscene before 12 | `2A71` | card | - | 5.18 s |
| 41 | end of level (seq op F2 4) | seq 70 | yes | - | 11.31 s |
| 43 | Jaffar killed | `8372` | yes | - | 3.38 s |
| 44 | skeleton wakes | `45F6` | yes | 0.24 s | 0.38 s |
| 45 | jump through the mirror | `509B` | yes | 0.39 s | 0.17 s |
| 46 | chomped | `5BC2` | yes | 0.42 s | 0.07 s |
| 47 | chomper closes | `8A79` | yes | 0.47 s | 0.11 s |
| 48 | spiked | `605A` | yes | 0.16 s | 0.15 s |
| 49 | spikes come out | `8F02` | yes | 0.40 s | 0.06 s |
| 50 | story 2: princess | - | no | - | 9.50 s |
| 51 | princess door opening | - | no | 0.44 s (14000 Hz) | 0.54 s |
| 52 | story 4: Jaffar leaves | - | no | - | 27.98 s |
| 53 | story 3: Jaffar comes | - | no | - | 14.85 s |
| 54 | intro music | - | no | - | 24.74 s |
| 55 | story 1: absence | - | no | - | 14.86 s |
| 56 | ending music | - | no | - | 120.04 s |

31, 34 and 42 are empty everywhere (1-byte resources). "card" = played by the app on the text card that stands in for the
cutscene (flow.rs). Not modelled: the original's "stop sounds" before the potion sounds and at Ctrl+A/level exits is kept only
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

First byte = type (bit 7 is a flag, seen on id 15 in both digi and speaker form; meaning UNVERIFIED):

- **1 digitized**: u16 sample rate (11000 Hz for most; 8200 for 23, 2750 for 15, 14000 for 51), u16 sample count (= payload
  size - 8), u16 unknown (0), u8 bits per sample (8), then unsigned 8-bit mono samples.
- **0 PC speaker**: u16 tempo, then notes {u16 frequency in Hz (0 or 1 = rest), u8 length in 1/tempo seconds}, ended by a
  frequency of 0x12. Feather 39 at tempo 80 sums to 18.61 s, close to SDLPoP's 18.75 s feather length, which supports the reading.
- **2 MIDI**: a standard MIDI file (`MThd ...`) after the type byte; not decoded by the remake.

The music cues exist only as speaker and MIDI; which one the original plays depends on the sound card, so the real music lengths
on an AdLib/MIDI setup are UNVERIFIED. The speaker lengths above are the ones used where timing matters.

## 4. Playback in the remake (pop-app)

- `synth.rs`: every emitted id and the card cues are synthesized at startup (oscillators, filtered noise, envelopes; music cues
  are short original jingles, not the game's melodies), mono 22050 Hz, peak 0.5. No audio files in the repository. Unit test:
  every clip is non-silent, finite, within its length range, not clipped.
- `POP_SOUND=original`: digitized clip where the game has one, else the PC-speaker tune rendered as a square wave, else the
  synthesized clip. Read from the user's own game data at startup (nothing copied into the repository).
- `audio.rs`: a custom `Decodable` PCM asset (no decoder feature needed: bevy's default `audio` feature already brings
  `bevy_audio`). Polyphonic; the same id again restarts that sound (approximates the single channel for repeated ticks).
- Headroom instead of a limiter (bevy_audio has no hook on the mixed output): synthesized clips peak at 0.5, digitized ones at
  0.5, speaker tunes at 0.25, and the same id restarts instead of stacking, so typical overlaps stay below 1.0 at full volume.
- Master volume (`-` / `=`, 10 % steps, default 70 %) and mute (`M`), applied to every playing or just-spawned sound (effects and cues); Paused pauses the playing sounds; Title and cards stop
  them; a card plays its cue (27 / 35 / 40 cutscenes, 36 time out, 26 the end).

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

## 6. Open / UNVERIFIED

- The original's sound priority / interruption rules (one channel; which sounds cut which) are not traced.
- Bit 7 of the type byte; the 2750 Hz rate of digi 15.
- The MIDI/AdLib lengths of the music cues; title/story music (50..56) is not played.
- Audio output itself was not listened to during development (verified by unit tests and debug logging: `RUST_LOG=pop_app=debug`
  prints every played id).
