# ASCII-only (PowerShell 5.1 reads scripts in the ANSI codepage).
# Experiment 88: MTP draft together with the early output crop (patch 5, bin\win-p5-noomp).
# Same 40 questions as experiment 74. Resumable: each config leaves results\win\88_<tag>.done.
# Usage (from the rag directory): powershell -File experiments\win_88_mtp_crop.ps1 [-Limit 40] [-Fresh]
param([int]$Limit = 40, [switch]$Fresh)
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'bin\app-win\rag.exe'
$log = Join-Path $root 'results\win\88_mtp_crop.txt'
if ($Fresh) { Remove-Item (Join-Path $root 'results\win\88_*.done') -ErrorAction SilentlyContinue; '' | Set-Content -Encoding utf8 $log }
Set-Location $root
$configs = @(
    @{ tag = 'p4_none'; bin = 'win-p4-noomp'; draft = '' },
    @{ tag = 'p5_none'; bin = 'win-p5-noomp'; draft = '' },
    @{ tag = 'p5_Q4_0'; bin = 'win-p5-noomp'; draft = 'Q4_0' }
)
foreach ($c in $configs) {
    $done = Join-Path $root ("results\win\88_" + $c.tag + ".done")
    if (Test-Path $done) { continue }
    $extra = @('--server', ("bin\" + $c.bin + "\llama-server.exe"))
    if ($c.draft -ne '') { $extra += @('--gen-draft', ("models\gemma-4-e2b\mtp-gemma-4-E2B-it-" + $c.draft + ".gguf")) }
    ("===== " + $c.tag + " (" + (Get-Date -Format 'MM-dd HH:mm') + ")") | Add-Content -Encoding utf8 $log
    & $exe ask-eval testdata\realdocs\queries.jsonl --root $root --data work\index-d256 --limit $Limit `
        --out ("results\win\88_" + $c.tag + ".jsonl") @extra 2>&1 | Select-Object -Last 8 | Add-Content -Encoding utf8 $log
    ("exit=" + $LASTEXITCODE) | Add-Content -Encoding utf8 $log
    if ($LASTEXITCODE -eq 0) { '' | Set-Content $done }
}
