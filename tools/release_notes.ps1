# Writes the CHANGELOG.md section of one release to a file (release notes contain only that release's changes).
# Usage: pwsh tools/release_notes.ps1 v0.4.1 notes.md
param([Parameter(Mandatory)][string]$Tag, [Parameter(Mandatory)][string]$OutFile)
$v = $Tag.TrimStart('v')
$c = Get-Content (Join-Path $PSScriptRoot '..\CHANGELOG.md') -Raw
$m = [regex]::Match($c, "(?ms)^## \[$([regex]::Escape($v))\][^\r\n]*\r?\n(.*?)(?=^## \[|\z)")
if (-not $m.Success) { Write-Error "no CHANGELOG.md section for $v"; exit 1 }
Set-Content -Path $OutFile -Value $m.Groups[1].Value.Trim() -Encoding utf8
