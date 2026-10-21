# 實驗 82：記憶體不足時，真實內容的 10 萬個 chunk（實驗 78 的 x20）搜尋會慢多少；int8 和 1 位元量化（粗篩 2 倍、20 倍）比較。
# 實驗 73 用的是隨機向量；這裡用真實文件放大的索引。1 位元粗篩 20 倍時每次查詢要從磁碟讀的完整向量多約 10 倍，
# 檔案快取被擠掉時可能變慢，這是決定要不要改預設前要知道的。做法和實驗 73 相同（Docker 記憶體上限，索引放在 volume）。
# 可以中途停掉：每一步做完會留下 results\win\82_<步驟>.done，重跑時跳過。
# 用法（在 rag 目錄下）：powershell -File experiments\win_82_lowmem_real.ps1
$root = Split-Path -Parent $PSScriptRoot
$win = Join-Path $root 'results\win'
$out = Join-Path $win '82_lowmem_real.txt'
function Log($s) { $s | Add-Content -Encoding utf8 $out }
function Done($name) { Test-Path (Join-Path $win "82_$name.done") }
function Mark($name) { "" | Set-Content (Join-Path $win "82_$name.done") }
docker volume create rag-lowmem | Out-Null
if (-not (Done 'copy')) {
    Log "===== copy x20 indexes into the volume ($(Get-Date -Format 'MM-dd HH:mm'))"
    docker run --rm -v rag-lowmem:/data -v "${root}:/rag" rag-rust sh -c "rm -rf /data/x20 /data/x20-1bit; cp -r /rag/work/scale-x20 /data/x20; cp -r /rag/work/scale-x20-1bitq8 /data/x20-1bit; du -sm /data/x20 /data/x20-1bit" 2>&1 | ForEach-Object { Log $_ }
    if ($LASTEXITCODE -eq 0) { Mark 'copy' } else { Log 'copy failed'; exit 1 }
}
foreach ($case in @(@('int8', 'x20', '2'), @('1bit-os2', 'x20-1bit', '2'), @('1bit-os20', 'x20-1bit', '20'))) {
    foreach ($mem in 'none', '600m', '300m', '200m', '150m', '100m') {
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
