# Experiment 84: 1-bit oversampling sweet spot - vector query speed at os 5, 8, 12 under memory limits (same method as exp 82).
# Resumable: each step leaves resultswin84_<step>.done.
# Usage (from the rag directory): powershell -File experimentswin_84_lowmem_os.ps1
# 可以中途停掉：每一步做完會留下 results\win\82_<步驟>.done，重跑時跳過。
# 用法（在 rag 目錄下）：powershell -File experiments\win_82_lowmem_real.ps1
$root = Split-Path -Parent $PSScriptRoot
$win = Join-Path $root 'results\win'
$out = Join-Path $win '84_lowmem_os.txt'
function Log($s) { $s | Add-Content -Encoding utf8 $out }
function Done($name) { Test-Path (Join-Path $win "84_$name.done") }
function Mark($name) { "" | Set-Content (Join-Path $win "84_$name.done") }
docker volume create rag-lowmem | Out-Null
if (-not (Done 'copy')) {
    Log "===== copy x20 indexes into the volume ($(Get-Date -Format 'MM-dd HH:mm'))"
    docker run --rm -v rag-lowmem:/data -v "${root}:/rag" rag-rust sh -c "rm -rf /data/x20-1bit; cp -r /rag/work/scale-x20-1bitq8 /data/x20-1bit; du -sm /data/x20-1bit" 2>&1 | ForEach-Object { Log $_ }
    if ($LASTEXITCODE -eq 0) { Mark 'copy' } else { Log 'copy failed'; exit 1 }
}
foreach ($case in @(@('1bit-os5', 'x20-1bit', '5'), @('1bit-os8', 'x20-1bit', '8'), @('1bit-os12', 'x20-1bit', '12'))) {
    foreach ($mem in 'none', '300m', '200m', '150m') {
        $step = "$($case[0])-$mem"
        if (Done $step) { continue }
        docker run --rm --privileged rag-rust sh -c "sync; echo 3 > /proc/sys/vm/drop_caches" 2>&1 | Out-Null
        Log "===== $step ($(Get-Date -Format 'MM-dd HH:mm'))"
        $limit = @()
        if ($mem -ne 'none') { $limit = @('--memory', $mem, '--memory-swap', $mem) }
        docker run --rm @limit -v rag-lowmem:/data -v "${root}:/rag" rag-rust sh -c "/rag/bin/app-linux/rag bench-store --data /data/$($case[1]) --points 0 --queries 300 --oversampling $($case[2]); grep -E '^(max|oom) ' /sys/fs/cgroup/memory.events" 2>&1 | ForEach-Object { Log $_ }
        Log "exit=$LASTEXITCODE"
        Mark $step
    }
}
Log "===== done"
