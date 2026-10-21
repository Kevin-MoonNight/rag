# 實驗 74：MTP 草稿模型（推測解碼）在 CPU 上能讓產生回答快多少，回答是否完全相同。Windows 原生。
# 同樣 40 題，分別不用草稿、用 Q4_0 和 Q8_0 的草稿模型（ggml-org/gemma-4-E2B-it-GGUF 的 mtp-*.gguf）。
# 可以中途停掉：每一種草稿跑完會留下 results\win\74_mtp_<草稿>.done，重跑時跳過已完成的；-Fresh 從頭來。
# 用法（在 rag 目錄下）：powershell -File experiments\win_74_mtp.ps1 [-Limit 40] [-Fresh]
param([int]$Limit = 40, [switch]$Fresh)
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'bin\app-win\rag.exe'
$log = Join-Path $root 'results\win\74_mtp.txt'
if ($Fresh) { Remove-Item (Join-Path $root 'results\win\74_mtp_*.done') -ErrorAction SilentlyContinue; "" | Set-Content -Encoding utf8 $log }
Set-Location $root
foreach ($d in 'none', 'Q4_0', 'Q8_0') {
    $done = Join-Path $root "results\win\74_mtp_$d.done"
    if (Test-Path $done) { continue }
    $extra = @()
    if ($d -ne 'none') { $extra = @('--gen-draft', "models\gemma-4-e2b\mtp-gemma-4-E2B-it-$d.gguf") }
    "===== draft $d ($(Get-Date -Format 'MM-dd HH:mm'))" | Add-Content -Encoding utf8 $log
    & $exe ask-eval testdata\realdocs\queries.jsonl --root $root --data work\index-d256 --limit $Limit `
        --out "results\win\74_mtp_$d.jsonl" @extra 2>&1 | Select-Object -Last 8 | Add-Content -Encoding utf8 $log
    "exit=$LASTEXITCODE" | Add-Content -Encoding utf8 $log
    if ($LASTEXITCODE -eq 0) { "" | Set-Content $done }
}
