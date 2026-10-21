# Experiment 99: the desktop app opens an index built WITHOUT OCR (work/ocr-win-test/index-plain copied to index-desktop);
# the startup update should redo only files with scanned pages, now with OCR, then unload the OCR model.
# Samples the private bytes of rag-desktop.exe every second for $seconds, then closes the window.
# Usage: powershell -File experiments\win_99_desktop_ocr.ps1 [seconds]
param([int]$seconds = 300)
$root = "C:\Users\k0937\Desktop\rag"
$env:RAG_ROOT = $root
$env:RAG_DATA = "$root\work\ocr-win-test\index-desktop"
$p = Start-Process -FilePath "$root\bin\app-win\rag-desktop.exe" -PassThru
$log = "$root\results\99\win-desktop-ocr-memory.csv"
"second,private_mb" | Out-File -Encoding ascii $log
for ($i = 1; $i -le $seconds; $i++) {
    Start-Sleep -Seconds 1
    try { $p.Refresh(); "{0},{1:N0}" -f $i, ($p.PrivateMemorySize64 / 1MB) | Out-File -Encoding ascii -Append $log } catch {}
}
$p.CloseMainWindow() | Out-Null
if (-not $p.WaitForExit(15000)) { Stop-Process -Id $p.Id }
Get-Content $log | Select-Object -Last 3
