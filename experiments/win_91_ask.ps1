# ASCII-only. Experiment 91: answer quality on the medical (MOHW gazette) corpus.
# Resumable: leaves results\win\91_ask.done when finished. Usage: powershell -File experiments\win_91_ask.ps1
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$done = Join-Path $root 'results\win\91_ask.done'
if (Test-Path $done) { exit 0 }
$log = Join-Path $root 'results\win\91_ask.txt'
("===== " + (Get-Date -Format 'MM-dd HH:mm')) | Add-Content -Encoding utf8 $log
& (Join-Path $root 'bin\app-win\rag.exe') ask-eval testdata\meddocs\queries.jsonl --root $root --data work\index-med `
    --out results\win\91_ask.jsonl --server bin\win-p4-noomp\llama-server.exe 2>&1 | Select-Object -Last 8 | Add-Content -Encoding utf8 $log
("exit=" + $LASTEXITCODE) | Add-Content -Encoding utf8 $log
if ($LASTEXITCODE -eq 0) { '' | Set-Content $done }
