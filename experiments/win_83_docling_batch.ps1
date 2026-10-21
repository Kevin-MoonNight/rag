# ASCII-only strings (comments may be Chinese).
# 實驗 83 之二：docling.rs 一次轉 400 份 PDF（1,766 頁）的總時間和記憶體高峰，和 LiteParse 比較。
# 用法（在 rag 目錄下）：powershell -File experiments\win_83_docling_batch.ps1 -Mode noocr|textlayer [-Workers 1]
param([string]$Mode = 'noocr', [string]$Workers = '1')
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$log = Join-Path $root 'results\win\83_docling.txt'
$out = Join-Path $root "work\docling\out-$Mode-w$Workers"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
$flag = '--no-ocr'
if ($Mode -eq 'textlayer') { $flag = '--text-layer-only' }
$psi = New-Object Diagnostics.ProcessStartInfo
$psi.FileName = Join-Path $root 'work\docling\bin\docling-rs.exe'
$psi.Arguments = '--input "' + (Join-Path $root 'testdata\realdocs\pdf') + '" --output "' + $out + '" ' + $flag + ' --page-break-placeholder "<!-- page break -->"'
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.CreateNoWindow = $true
$psi.EnvironmentVariables['DOCLING_RS_PDF_WORKERS'] = $Workers
$p = New-Object Diagnostics.Process
$p.StartInfo = $psi
$sw = [Diagnostics.Stopwatch]::StartNew()
[void]$p.Start()
$o = $p.StandardOutput.ReadToEndAsync()
$e = $p.StandardError.ReadToEndAsync()
$priv = 0; $ws = 0
while (-not $p.HasExited) {
    try { $p.Refresh(); $priv = [Math]::Max($priv, $p.PrivateMemorySize64); $ws = [Math]::Max($ws, $p.WorkingSet64) } catch {}
    Start-Sleep -Milliseconds 100
}
$p.WaitForExit()
$files = @(Get-ChildItem -Recurse $out -File).Count
$line = 'batch {0} workers={1}: exit={2} seconds={3:N1} outputs={4} peak private {5:N0} MB, peak working set {6:N0} MB' -f $Mode, $Workers, $p.ExitCode, $sw.Elapsed.TotalSeconds, $files, ($priv / 1MB), ($ws / 1MB)
$line | Add-Content -Encoding utf8 $log
$line
($e.Result -split "`n" | Select-Object -Last 15) -join "`n"
