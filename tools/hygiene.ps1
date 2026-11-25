# Housekeeping after a build (tools/package.ps1 calls it at the end): removes what is no longer used and only costs disk.
#  1. git worktrees under .claude/worktrees that are clean, not locked, fully merged into HEAD and untouched for -MinAgeHours
#     (agent worktrees pile up with a full target/ of several GB each), then their merged branch
#  2. the incremental caches target/*/incremental (tens of GB; a debug build recreates them, a release build has none by default)
# Never touches uncommitted or unmerged work, the main checkout, locked worktrees or target/release/deps.
# -DryRun: only say what would go. -KeepCaches: leave target/*/incremental. Not run on CI (nothing piles up there).
param([int]$MinAgeHours = 24, [switch]$DryRun, [switch]$KeepCaches)
if ($env:CI) { return }
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Remove-Tree($path) {
    if ($DryRun) { "would remove $path"; return }
    Remove-Item -Recurse -Force $path -ErrorAction Stop
    "removed $path"
}

git worktree prune
# `git worktree list --porcelain`: blocks of worktree / HEAD / branch / locked lines, separated by an empty line
$blocks = (git worktree list --porcelain) -join "`n" -split "`n`n"
foreach ($b in $blocks) {
    $path = ([regex]::Match($b, '(?m)^worktree (.+)$').Groups[1].Value) -replace '/', '\'
    if (-not $path -or ((Resolve-Path $path).Path -eq $root)) { continue }
    $branch = [regex]::Match($b, '(?m)^branch refs/heads/(.+)$').Groups[1].Value
    $name = Split-Path -Leaf $path
    if ($b -match '(?m)^locked') { "keep ${name}: locked"; continue }
    if (git -C $path status --porcelain) { "keep ${name}: uncommitted changes"; continue }
    git merge-base --is-ancestor "$(git -C $path rev-parse HEAD)" HEAD 2>$null
    if ($LASTEXITCODE -ne 0) { "keep ${name}: has commits that are not in HEAD"; continue }
    # shortcut: the age is the newer of the folder and its index; an agent that only edits files deep inside for hours is not seen
    $gitdir = (git -C $path rev-parse --absolute-git-dir)
    $index = Join-Path $gitdir 'index'
    $last = (Get-Item $path).LastWriteTime
    if (Test-Path $index) { $last = [datetime][Math]::Max($last.Ticks, (Get-Item $index).LastWriteTime.Ticks) }
    if (((Get-Date) - $last).TotalHours -lt $MinAgeHours) { "keep ${name}: used in the last $MinAgeHours h"; continue }
    if ($DryRun) { "would remove worktree $name" } else {
        git worktree remove --force $path   # clean and merged (checked above); --force only for the ignored files (target/)
        if ($branch) { git branch -d $branch 2>$null | Out-Null }
        if (Test-Path $path) { Remove-Item -Recurse -Force $path }
        "removed worktree $name"
    }
}

if (-not $KeepCaches) {
    foreach ($inc in Get-ChildItem (Join-Path $root 'target') -Directory -ErrorAction SilentlyContinue | ForEach-Object { Join-Path $_.FullName 'incremental' } | Where-Object { Test-Path $_ }) {
        Remove-Tree $inc
    }
}
