# 實驗 81：10 萬個 chunk（x20）時 1 位元量化的完整準確度。80_1bit_x20.txt 只留了最後一組的數字，這裡留完整的「全部 128 題」和頁的指標。
# 比較 int8（原本）、1bit-q8 粗篩 2 倍（預設）、1bit-q8 粗篩 4 倍。只看準確度，搜尋時間僅供參考。
# 可以中途停掉：每一組做完會留下 results\win\81_<組>.done，重跑時跳過。
# 用法（在 rag 目錄下）：powershell -File experiments\win_81_1bit_eval.ps1
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$exe = Join-Path $root 'bin\app-win\rag.exe'
$out = Join-Path $root 'results\win\81_1bit_eval.txt'
foreach ($case in @(@('int8', 'work\scale-x20', '2'), @('1bitq8-os2', 'work\scale-x20-1bitq8', '2'), @('1bitq8-os4', 'work\scale-x20-1bitq8', '4'))) {
    $done = Join-Path $root "results\win\81_$($case[0]).done"
    if (Test-Path $done) { continue }
    "===== $($case[0]) ($(Get-Date -Format 'MM-dd HH:mm:ss'))" | Add-Content -Encoding utf8 $out
    & $exe eval testdata\realdocs\queries.jsonl --root $root --data $case[1] --oversampling $case[2] `
        --pages testdata\realdocs\page_labels.jsonl --out "results\win\81_$($case[0]).jsonl" 2>&1 |
        Add-Content -Encoding utf8 $out
    "exit=$LASTEXITCODE" | Add-Content -Encoding utf8 $out
    if ($LASTEXITCODE -eq 0) { "" | Set-Content $done }
}
