# 在 Windows 上直接跑 llama-bench.exe，比較不同版本的 E2B 速度。
# 用法（在 rag 目錄下）：powershell -File experiments\win_bench.ps1 -Bins "win-b11567,win-p2-noomp" -Threads "4,6" -Out results\win\01_e2b_bench.txt
param(
    [Parameter(Mandatory = $true)][string]$Bins,
    [string]$Threads = '4,6',
    [string]$Model = 'models\gemma-4-e2b\gemma-4-E2B_q4_0-it.gguf',
    [string]$Out = '',
    [string]$BenchArgs = '-p 512 -n 64 -r 3'
)
$root = Split-Path -Parent $PSScriptRoot
$tmpDir = Join-Path $root 'results\win'
New-Item -ItemType Directory -Force $tmpDir | Out-Null
$lines = @()
foreach ($b in $Bins.Split(',')) {
    foreach ($t in $Threads.Split(',')) {
        $exe = Join-Path $root "bin\$b\llama-bench.exe"
        $cmd = "-m `"$(Join-Path $root $Model)`" -t $t -ngl 0 -lzm on $BenchArgs"
        $outFile = Join-Path $tmpDir "bench-tmp-$PID.txt"
        Start-Process -FilePath $exe -ArgumentList $cmd -NoNewWindow -Wait -RedirectStandardOutput $outFile -RedirectStandardError "$outFile.err"
        foreach ($l in (Get-Content $outFile | Where-Object { $_ -match '^\|\s*gemma' })) {
            $cols = $l.Split('|') | ForEach-Object { $_.Trim() }
            $line = '{0,-18} threads={1}  {2,-8} {3}' -f $b, $t, $cols[7], $cols[8]
            $lines += $line
            $line
        }
        Remove-Item $outFile, "$outFile.err" -ErrorAction SilentlyContinue
    }
}
if ($Out -ne '') { $lines | Set-Content -Encoding utf8 (Join-Path $root $Out) }
