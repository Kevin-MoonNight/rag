# 實驗 78：大規模索引（1 萬到 50 萬個 chunk）會不會壞掉、記憶體和查詢速度。Windows 原生。
# 用 rag replicate 把真實的 d256 索引（4,966 個文字 chunk）放大成 N 倍：內容是真實文件，向量加了一點擾動。
# 每個規模量：放大（等於寫入）的時間和記憶體高峰、磁碟大小、只開儲存層的查詢速度和記憶體（bench-store），
# 以及用 128 題真實問題搜尋的速度（rag eval，含查詢模型）。
# 可以中途停掉：每個規模做完會留下 results\win\78_x<N>.done，重跑時跳過；做到一半的規模會刪掉重做。-Fresh 從頭來。
# 用法（在 rag 目錄下）：powershell -File experiments\win_78_scale.ps1 [-Times 2,20,60,100] [-Keep 2,20] [-Fresh]
param([int[]]$Times = @(2, 20, 60, 100), [int[]]$Keep = @(2, 20), [switch]$Fresh)
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'bin\app-win\rag.exe'
$log = Join-Path $root 'results\win\78_scale.txt'
New-Item -ItemType Directory -Force (Join-Path $root 'results\win') | Out-Null
if ($Fresh) { Remove-Item (Join-Path $root 'results\win\78_x*.done') -ErrorAction SilentlyContinue; "" | Set-Content -Encoding utf8 $log }
function Log($s) { $s | Add-Content -Encoding utf8 $log }
function Run([string[]]$argv) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $exe @argv 2>&1 | ForEach-Object { Log "  $_" }
    Log ("  exit={0} seconds={1:N1}" -f $LASTEXITCODE, $sw.Elapsed.TotalSeconds)
    $LASTEXITCODE
}
Set-Location $root
foreach ($t in $Times) {
    $done = Join-Path $root "results\win\78_x$t.done"
    if (Test-Path $done) { continue }
    $data = "work\scale-x$t"
    Log "===== x$t ($(Get-Date -Format 'MM-dd HH:mm:ss'))"
    if (Test-Path $data) { Remove-Item -Recurse -Force $data }
    Log "-- replicate"
    $code = Run @('replicate', '--data', 'work\index-d256', '--out', $data, '--times', "$t")
    if ($code -ne 0) { Log "-- replicate failed, skipping x$t"; continue }
    $mb = (Get-ChildItem -Recurse $data | Measure-Object Length -Sum).Sum / 1MB
    Log ("-- disk {0:N0} MB" -f $mb)
    foreach ($d in Get-ChildItem $data -Directory) {
        Log ("   {0} {1:N0} MB" -f $d.Name, ((Get-ChildItem -Recurse $d.FullName | Measure-Object Length -Sum).Sum / 1MB))
    }
    Log "-- store only: open and 300 random queries"
    [void](Run @('bench-store', '--data', $data, '--points', '0', '--queries', '300'))
    Log "-- 128 real questions (search incl. query model)"
    [void](Run @('eval', 'testdata\realdocs\queries.jsonl', '--root', $root, '--data', $data, '--out', "results\win\78_eval_x$t.jsonl"))
    if ($Keep -notcontains $t) {
        Remove-Item -Recurse -Force $data
        Log "-- removed $data"
    }
    "" | Set-Content $done
}
Log "===== done ($(Get-Date -Format 'MM-dd HH:mm:ss'))"
