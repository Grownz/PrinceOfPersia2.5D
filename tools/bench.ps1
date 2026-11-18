# Benchmark matrix for the renderer (docs/enhanced_perf.md). Runs the release exe once per configuration with
# POP_BENCH (vsync off, 500 warm-up frames), reads the BENCH line from pop.log and prints a table.
# Keep the window in the foreground and do not touch the mouse: Windows throttles background windows to 60 Hz
# (the BENCH line counts unfocused frames).
#   powershell -ExecutionPolicy Bypass -File tools/bench.ps1                 # default matrix
#   powershell -ExecutionPolicy Bypass -File tools/bench.ps1 -Quick          # 720p classic vs enhanced only
#   powershell -ExecutionPolicy Bypass -File tools/bench.ps1 -Configs @(@{POP_RENDER='enhanced'; POP_FX='no_ssao'})
param(
    [int]$Frames = 1000,
    [switch]$Quick,
    [switch]$NoBuild,
    [object[]]$Configs,
    [string]$Csv
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
if (-not $NoBuild) { cargo build --release -p pop-app; if ($LASTEXITCODE) { exit 1 } }
$exe = Join-Path $root 'target\release\pop-app.exe'
$log = Join-Path $root 'target\release\pop.log'

if (-not $Configs) {
    $scene = @{ POP_LEVEL = '1'; POP_ROOM = '13' }
    $Configs = @()
    $resList = if ($Quick) { @('1280x720') } else { @('1280x720', '1920x1080', '2560x1440', '3840x2160') }
    foreach ($res in $resList) {
        foreach ($mode in 'classic', 'enhanced') { $Configs += $scene + @{ POP_RENDER = $mode; POP_RES = $res } }
    }
    if (-not $Quick) {
        foreach ($fx in 'no_torch_shadows', 'no_sun_shadow', 'no_ssao', 'no_bloom', 'no_smaa', 'no_hdr', 'no_ambient', 'lights=2', 'point_map=512') {
            $Configs += $scene + @{ POP_RENDER = 'enhanced'; POP_RES = '1920x1080'; POP_FX = $fx }
        }
    }
}

$vars = 'POP_RENDER', 'POP_CHARS', 'POP_TILES', 'POP_COLUMNS', 'POP_OBJECTS', 'POP_LEVEL', 'POP_ROOM', 'POP_SCRIPT', 'POP_CUTSCENE', 'POP_RES', 'POP_FX', 'POP_GPU_TIMES', 'POP_BENCH_WARMUP'
$rows = foreach ($c in $Configs) {
    foreach ($v in $vars) { Remove-Item "env:$v" -ErrorAction SilentlyContinue }
    foreach ($k in $c.Keys) { Set-Item "env:$k" $c[$k] }
    $env:POP_BENCH = "$Frames"
    $p = Start-Process -FilePath $exe -PassThru
    if (-not $p.WaitForExit(180000)) { $p.Kill(); Write-Warning "timeout: $($c | ConvertTo-Json -Compress)"; continue }
    $line = Select-String -Path $log -Pattern 'BENCH ' | Select-Object -Last 1
    if (-not $line) { Write-Warning "no BENCH line: $($c | ConvertTo-Json -Compress)"; continue }
    $kv = @{}
    foreach ($m in [regex]::Matches($line.Line, '(\w+)=(\S*)')) { if (-not $kv.ContainsKey($m.Groups[1].Value)) { $kv[$m.Groups[1].Value] = $m.Groups[2].Value } }
    $gpu = if ($line.Line -match 'gpu=\[(.*)\]') { $Matches[1] } else { '' }
    $top = ($gpu -split ' ' | Select-Object -First 3) -join ' '
    [pscustomobject]@{
        scene = "L$($c.POP_LEVEL)R$($c.POP_ROOM)$(if ($c.POP_CUTSCENE) { " cut$($c.POP_CUTSCENE)" })$(if ($c.POP_SCRIPT) { ' script' })"
        render = $kv.render; chars = $kv.chars; res = $kv.res; fx = $kv.fx
        avg = [double]$kv.avg_ms; p95 = [double]$kv.p95; p99 = [double]$kv.p99; max = [double]$kv.max; fps = [double]$kv.fps
        gpu = [double]$kv.gpu_ms; unfocused = [int]$kv.unfocused_frames; top_gpu = $top
    }
}
foreach ($v in $vars + 'POP_BENCH') { Remove-Item "env:$v" -ErrorAction SilentlyContinue }
$rows | Format-Table -AutoSize | Out-String -Width 300
if ($Csv) { $rows | Export-Csv -NoTypeInformation $Csv }
