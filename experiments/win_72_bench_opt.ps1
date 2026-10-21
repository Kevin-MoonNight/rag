# 實驗 72：建索引途中定期整理，能不能壓低 10 萬筆寫入時的記憶體高峰。Windows 原生。
# 用法（在 rag 目錄下）：powershell -File experiments\win_72_bench_opt.ps1
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'bin\app-win\rag.exe'
$out = Join-Path $root 'results\win\72_bench_opt.txt'
"" | Set-Content -Encoding utf8 $out
foreach ($every in 0, 5000, 20000) {
    $data = Join-Path $root "work\bench-opt$every"
    if (Test-Path $data) { Remove-Item -Recurse -Force $data }
    "===== optimize-every $every" | Add-Content -Encoding utf8 $out
    & $exe bench-store --data $data --points 100000 --queries 200 --optimize-every $every 2>&1 | Add-Content -Encoding utf8 $out
    "exit=$LASTEXITCODE" | Add-Content -Encoding utf8 $out
    $mb = [math]::Round(((Get-ChildItem -Recurse $data | Measure-Object Length -Sum).Sum) / 1MB)
    "disk $mb MB" | Add-Content -Encoding utf8 $out
    Remove-Item -Recurse -Force $data
}
"===== done" | Add-Content -Encoding utf8 $out
