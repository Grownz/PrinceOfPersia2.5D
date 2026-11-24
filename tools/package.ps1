# Builds the release exe and packs dist/PrinceOfPersia-x64/ (exe, README.txt) plus dist/PrinceOfPersia-x64.zip.
# Never copies Original_data or any other game file: players supply their own copy.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# the tests as CI runs them (no game data) in parallel with the release build; both must succeed
$tests = Start-Job { & $using:PSScriptRoot\test_without_data.ps1 *>&1 }

cargo build --release -p pop-app
$built = $LASTEXITCODE
Receive-Job $tests -Wait -ErrorVariable testErr | Select-Object -Last 8
if ($tests.State -ne 'Completed' -or $testErr) { throw "tests failed: $testErr" }
if ($built -ne 0) { throw "cargo build failed" }

$out = Join-Path $root 'dist\PrinceOfPersia-x64'
$zip = Join-Path $root 'dist\PrinceOfPersia-x64.zip'
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
if (Test-Path $zip) { Remove-Item -Force $zip }
New-Item -ItemType Directory -Force $out | Out-Null

Copy-Item (Join-Path $root 'target\release\pop-app.exe') (Join-Path $out 'PrinceOfPersia.exe')

@'
Prince of Persia x64 - a Rust remake
====================================

GAME DATA (required, not included)
This package contains only the remake's own program. The original game data is
copyrighted and is NOT included. You need your own copy of the DOS game
Prince of Persia (1990). Copy its files (LEVELS.DAT, PRINCE.EXE, KID.DAT,
GUARD*.DAT, PRINCE.DAT, TITLE.DAT, the sound .DAT files, ...) into a folder
named Original_data next to PrinceOfPersia.exe.

The game looks for the data in this order:
  1. the folder in the environment variable POP_DATA
  2. Original_data next to PrinceOfPersia.exe
  3. Original_data in the current folder
  4. "data_path" in settings.json
If nothing is found, the window explains what is missing.

CONTROLS
  Arrow keys      move (up = jump, down = crouch)
  Shift           action: grab, careful step, drink, strike; hold after death to restart
  Esc / P         pause            Ctrl+A   restart level
  Ctrl+G          save (levels 3-13)   Ctrl+L   load (title screen)
  M               mute             - / =    volume
  F11             fullscreen       F3       debug text
Controller (Controls > Controller picks the layout; default Modern): stick / d-pad =
left, right, jump; A = jump, B = crouch, X = action / strike, Y = parry, RB / RT = hold to
grab, Start = pause, Back = restart level, any button starts the game.

SETTINGS
settings.json (created next to the exe on the first run): window mode and size,
volume, mute, sound ("synth" or "original"), key bindings for the kid's keys,
data path. Errors are written to pop.log next to the exe.

LEGAL
Prince of Persia is a trademark of its owners; the original game, its graphics,
sounds and level data are copyrighted and are not distributed with this
remake. This program reads your own copy at run time. The remake's code,
synthesized sounds and procedural geometry are its own.
'@ | Set-Content -Encoding ascii (Join-Path $out 'README.txt')

# guard: only our own files may be packed
$unexpected = Get-ChildItem $out -Recurse -File | Where-Object { $_.Name -notin @('PrinceOfPersia.exe', 'README.txt') }
if ($unexpected) { throw "unexpected files in package: $($unexpected.Name -join ', ')" }

Compress-Archive -Path $out -DestinationPath $zip
"{0}: {1:N1} MB" -f $zip, ((Get-Item $zip).Length / 1MB)
