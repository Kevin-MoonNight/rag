# Experiment 99: Windows rag.exe index of the 18 PDFs with scanned pages (work/ocr-win-test/pdf), CPU only, with and without --ocr.
# Reports wall time and the peak private bytes of rag.exe (OCR runs inside rag.exe; the embedding model runs in llama-server).
# Usage: powershell -File experiments\win_99_index_ocr.ps1   (pause the Docker containers first so the CPU is idle)
$root = "C:\Users\k0937\Desktop\rag"
$exe = "$root\bin\app-win\rag.exe"
foreach ($mode in @("plain", "ocr")) {
    $data = "$root\work\ocr-win-test\index-$mode"
    if (Test-Path $data) { Remove-Item -Recurse -Force $data }
    $a = @("index", "$root\work\ocr-win-test\pdf", "--root", $root, "--data", $data, "--server", "$root\bin\win-p5-noomp\llama-server.exe", "--quiet")
    if ($mode -eq "ocr") { $a += "--ocr" }
    $t = Get-Date
    $p = Start-Process -FilePath $exe -ArgumentList $a -NoNewWindow -PassThru -RedirectStandardOutput "$root\results\99\win-index-$mode.txt" -RedirectStandardError "$root\results\99\win-index-$mode.err"
    $peak = 0
    while (-not $p.HasExited) {
        try { $p.Refresh(); if ($p.PrivateMemorySize64 -gt $peak) { $peak = $p.PrivateMemorySize64 } } catch {}
        Start-Sleep -Milliseconds 100
    }
    $p.WaitForExit()
    "{0}: total {1:N1} s, rag.exe peak private {2:N0} MB" -f $mode, ((Get-Date) - $t).TotalSeconds, ($peak / 1MB)
}
