# 實驗 73：記憶體不夠、索引檔被擠出快取時，搜尋會慢多少。
# 用 Docker 的記憶體上限（頁面快取也算在上限裡）模擬記憶體不足的電腦。索引放在 Docker 的 volume（rag-lowmem），
# 不放在 Windows 的資料夾，免得讀檔被 Windows 的快取接住。每個上限開一個新容器，先清掉虛擬機的快取。
# 可以中途停掉：建資料和每個上限做完會留下 results\win\73_<步驟>.done，重跑時跳過。-Fresh 從頭來。
# 用法（在 rag 目錄下）：powershell -File experiments\win_73_lowmem.ps1 [-Points 100000] [-Fresh]
param([int]$Points = 100000, [switch]$Fresh)
$root = Split-Path -Parent $PSScriptRoot
$win = Join-Path $root 'results\win'
$out = Join-Path $root "results\73_lowmem_$Points.txt"
if ($Fresh) { Remove-Item (Join-Path $win '73_*.done') -ErrorAction SilentlyContinue; "" | Set-Content -Encoding utf8 $out }
function Log($s) { $s | Add-Content -Encoding utf8 $out }
function Done($name) { Test-Path (Join-Path $win "73_$name.done") }
function Mark($name) { "" | Set-Content (Join-Path $win "73_$name.done") }
docker volume create rag-lowmem | Out-Null
$data = "/data/b$Points"
if (-not (Done "build$Points")) {
    Log "===== build $Points points ($(Get-Date -Format 'MM-dd HH:mm'))"
    docker run --rm -v rag-lowmem:/data -v "${root}:/rag" rag-rust sh -c "rm -rf $data; /rag/bin/app-linux/rag bench-store --data $data --points $Points --queries 20; du -sm $data $data/*" 2>&1 | ForEach-Object { Log $_ }
    if ($LASTEXITCODE -eq 0) { Mark "build$Points" } else { Log "build failed"; exit 1 }
}
foreach ($mem in '4g', '1g', '600m', '400m', '300m', '200m', '150m') {
    if (Done "$Points-$mem") { continue }
    docker run --rm --privileged rag-rust sh -c "sync; echo 3 > /proc/sys/vm/drop_caches" 2>&1 | Out-Null
    Log "===== memory limit $mem ($(Get-Date -Format 'MM-dd HH:mm'))"
    docker run --rm --memory $mem --memory-swap $mem -v rag-lowmem:/data -v "${root}:/rag" rag-rust sh -c "/rag/bin/app-linux/rag bench-store --data $data --points 0 --queries 300; grep -E '^(max|oom) ' /sys/fs/cgroup/memory.events" 2>&1 | ForEach-Object { Log $_ }
    Log "exit=$LASTEXITCODE"
    Mark "$Points-$mem"
}
Log "===== done"
