# 電腦空下來時依序跑的 Windows 量測（實驗 74、78、79、1 位元的 10 萬筆比較、73）。每一項的結果在 results\win\ 或 results\。
# 可以隨時停掉（experiments\win_stop_suite.ps1，或直接關機）再重跑：做完的步驟會留下 results\win\*.done，重跑時跳過；
# 做到一半的那一步從頭重做。進度記在 results\win\80_suite.log（只會往後加）。
# 用法（在 rag 目錄下）：powershell -File experiments\win_80_idle_suite.ps1
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$win = Join-Path $root 'results\win'
$log = Join-Path $win '80_suite.log'
New-Item -ItemType Directory -Force $win | Out-Null
function Step($s) { "$(Get-Date -Format 'MM-dd HH:mm:ss') $s" | Add-Content -Encoding utf8 $log }
function Done($name) { Test-Path (Join-Path $win "$name.done") }
function Mark($name) { "" | Set-Content (Join-Path $win "$name.done") }
$exe = Join-Path $root 'bin\app-win\rag.exe'
Step 'suite (re)started'

# 這兩個腳本自己記錄每一小步，已完成的會跳過
Step 'exp 74: MTP draft model'
powershell -NoProfile -File experiments\win_74_mtp.ps1 -Limit 40
Step 'exp 78: scale x2, x20, x60, x100'
powershell -NoProfile -File experiments\win_78_scale.ps1

foreach ($case in @(@('x2', 'work\scale-x2'), @('x20', 'work\scale-x20'), @('mini', 'work\index-win-mini'))) {
    $name = "79_desktop_$($case[0])"
    if ((Done $name) -or -not (Test-Path $case[1])) { continue }
    Step "exp 79: desktop scenarios on $($case[0])"
    powershell -NoProfile -File experiments\win_79_desktop_scenarios.ps1 -Data $case[1] -Name $case[0]
    if ((Get-Content -Encoding utf8 (Join-Path $win "$name.txt") -Raw) -notmatch 'ERROR') { Mark $name }
}

if (-not (Done '80_1bit_x20') -and (Test-Path work\scale-x20)) {
    Step '1-bit at 100k: requantize x20 and compare store memory and speed'
    $q = 'work\scale-x20-1bitq8'
    if (Test-Path $q) { Remove-Item -Recurse -Force $q }
    $out = Join-Path $win '80_1bit_x20.txt'
    "" | Set-Content -Encoding utf8 $out
    & $exe requantize --data work\scale-x20 --out $q --quant 1bit-q8 2>&1 | Add-Content -Encoding utf8 $out
    foreach ($d in 'work\scale-x20', $q) {
        "== $d store only" | Add-Content -Encoding utf8 $out
        & $exe bench-store --data $d --points 0 --queries 300 2>&1 | Add-Content -Encoding utf8 $out
        "== $d 128 questions" | Add-Content -Encoding utf8 $out
        & $exe eval testdata\realdocs\queries.jsonl --root $root --data $d 2>&1 | Select-Object -Last 3 | Add-Content -Encoding utf8 $out
    }
    Mark '80_1bit_x20'
}

if (-not (Done '73_lowmem')) {
    Step 'exp 73: low memory (Docker memory limits), idle CPU'
    powershell -NoProfile -File experiments\win_73_lowmem.ps1 -Points 100000
    Mark '73_lowmem'
}

Step 'done'
