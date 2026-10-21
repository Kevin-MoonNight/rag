# Experiment 99: PP-OCRv6 on Windows (native exe, onnxruntime.dll loaded at run time), 36 scanned pages.
# Measures seconds per page (times.csv written by the tool), peak working set and peak private bytes of the process.
# Usage: powershell -File experiments\win_99_ppocr.ps1   (run with the Docker containers paused so the CPU is idle)
$root = "C:\Users\k0937\Desktop\rag"
$env:RAG_ROOT = $root
$exe = "$root\bin\ppocr-win\ppocr99.exe"
$m = "$root\models\ocr\ppocr"
foreach ($v in @("small", "medium")) {
    $out = "$root\results\99\win-ppocrv6-$v"
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    $a = @("$m\pp-ocrv6_${v}_det.onnx", "$m\pp-ocrv6_${v}_rec.onnx", "$m\ppocrv6_dict.txt", $out, "6", "99", "vertical")
    $t = Get-Date
    $p = Start-Process -FilePath $exe -ArgumentList $a -NoNewWindow -PassThru -RedirectStandardOutput "$root\results\99\win-$v.log"
    $peakPrivate = 0; $peakWs = 0
    while (-not $p.HasExited) {
        try { $p.Refresh(); if ($p.PrivateMemorySize64 -gt $peakPrivate) { $peakPrivate = $p.PrivateMemorySize64 }; if ($p.PeakWorkingSet64 -gt $peakWs) { $peakWs = $p.PeakWorkingSet64 } } catch {}
        Start-Sleep -Milliseconds 100
    }
    $p.WaitForExit()
    $total = ((Get-Date) - $t).TotalSeconds
    "{0}: exit {1}, total {2:N1} s, peak working set {3:N0} MB, peak private {4:N0} MB" -f $v, $p.ExitCode, $total, ($peakWs / 1MB), ($peakPrivate / 1MB)
}
