# Runs the whole test suite the way GitHub CI does: without Original_data (tests that need it must skip, not fail).
# The folder is renamed for the run and always put back. Own target dir (target\test), so it can run next to a release build.
# A passed run is remembered for the exact file contents (tracked + untracked, not ignored); an unchanged tree is not tested again.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$stamp = Join-Path $root 'target\tests_ok.txt'
$state = (git ls-files -co --exclude-standard | git hash-object --stdin-paths | Out-String).Trim()
$state = [BitConverter]::ToString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($state)))
if ((Test-Path $stamp) -and (Get-Content $stamp -Raw).Trim() -eq $state) { 'tests already passed for this exact tree, skipped'; return }
$data = Join-Path $root 'Original_data'; $off = Join-Path $root 'Original_data_off'
if (Test-Path $off) { throw "$off exists: a previous run was interrupted, rename it back to Original_data first" }
$moved = Test-Path $data
if ($moved) { Rename-Item $data 'Original_data_off' }
$env:CARGO_TARGET_DIR = Join-Path $root 'target\test'
try { cargo test --workspace -q; $code = $LASTEXITCODE }
finally { if ($moved) { Rename-Item $off 'Original_data' } }
if ($code -ne 0) { throw "tests fail without game data" }
Set-Content $stamp $state
