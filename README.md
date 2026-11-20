# Prince of Persia x64

**The 1989 classic, rebuilt for modern PCs.** Run, jump, climb and fence through the sultan's dungeons and palace in the
original's exact rhythm, now in 3D lighting with real shadows, smooth motion and the music the way it was meant to sound.

The prince fights a guard in the enhanced render mode with voxel characters:

https://github.com/user-attachments/assets/498f5ae6-aa68-4cbb-ad87-2fd131e3076d

## Why play it

- **It plays like the original.** The game logic is a tick-exact port: every jump distance, every sword duel, every trap and
  all 14 levels behave the way you remember, including the hourglass, the shadow and the finale.
- **It looks like a new game.** Switch to the *enhanced* look and the dungeon gets torch-lit shadows, ambient occlusion and glow.
  The prince and the guards can become 3D figures built from the original pixels, and walls, pillars, spikes, gates and
  potions have real depth. Prefer the classic look? One key (F5) switches back.
- **Smooth at any frame rate.** Figures move at your screen's refresh rate instead of the 12 steps per second of 1989, and the
  camera glides from room to room.
- **The music is back.** Play the original MIDI scores through an emulated AdLib sound card, or through a General MIDI
  SoundFont you can download inside the game.
- **Made for today.** Windows 10/11, windowed or fullscreen, keyboard, mouse-driven menus and controller support, saving,
  loading and the hall of fame as in the original.

## Get started in three steps

1. **Download** the latest `PrinceOfPersia-x64.zip` from the [Releases page](../../releases/latest) and unzip it anywhere.
2. **Add your game files.** This is a remake of the engine, not a copy of the game: the original graphics, sounds and levels
   are copyrighted and are not included. Copy the files of your DOS copy of *Prince of Persia* (`LEVELS.DAT`, `PRINCE.EXE`,
   `KID.DAT`, `GUARD*.DAT`, `PRINCE.DAT`, `TITLE.DAT`, the sound `.DAT` files and the rest) into a folder named
   `Original_data` next to `PrinceOfPersia.exe`. The VGA version it is built for can be found at the Internet Archive:
   <https://archive.org/details/PERSIA_VGA> (check the rights situation for your country).
3. **Start `PrinceOfPersia.exe`** and press any key. If the files are missing, the window tells you where it looked.

## Controls

| | |
|---|---|
| Move | Arrow keys (up = jump, down = crouch) |
| Action | Shift: grab a ledge, careful step, drink, strike |
| Pause and menu | Esc or P |
| Save / load | Ctrl+G (levels 3 to 13) / Ctrl+L on the title screen |
| Restart level | Ctrl+A |
| Look | F5 classic / enhanced, F6 sprite / 3D figures, F11 fullscreen |
| Sound | M mute, - and = volume |

Controllers work too: d-pad or left stick, A as Shift, Start to pause. The full list and key rebinding are in
[docs/controls.md](docs/controls.md).

## Settings

Press Esc and open **Settings**: sliders, checkboxes and drop-down lists for video, audio and graphics, usable with mouse,
keyboard or controller. Changes to the look and the volume apply immediately. The **Downloads** tab fetches the optional
SoundFonts (MuseScore_General or FluidR3_GM, both MIT licensed) with a progress bar; you can cancel a download or delete a
font at any time. Please respect the licences of the sources you download from.

## Questions

**Do I need the original game?** Yes. Only the engine is rebuilt here; you supply the graphics, sounds and levels from your own
copy.

**Will it run on my PC?** The classic look runs on any graphics card with Vulkan or DirectX 12. The enhanced look needs a bit
more; the settings let you turn shadows and ambient occlusion down.

**Is it finished?** All 14 levels, the cutscenes, the hall of fame and saving work. See the [roadmap](ROADMAP.md) for what is
next and the [changelog](CHANGELOG.md) for what changed.

**Something looks wrong or you found a bug.** Please [open an issue](../../issues) and tell us the level and room.

## For developers

Rust and [Bevy](https://bevy.org): a headless simulation (`crates/pop-core`) kept separate from the app (`crates/pop-app`).
Building, debugging switches and every setting are in [docs/DEVELOPING.md](docs/DEVELOPING.md); how the original works is
written down in `docs/` (mechanics, tiles, audio, cutscenes). Where the code comes from and what it does not contain:
[docs/PROVENANCE.md](docs/PROVENANCE.md).

## Licence

The code and documentation are under the [MIT licence](LICENSE). *Prince of Persia* itself belongs to its owners and is not part
of this project.
