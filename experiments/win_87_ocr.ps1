# ASCII-only (PowerShell 5.1 reads scripts in the ANSI codepage).
# Experiment 87: OCR the real scanned pages (work\87_scan_pages.json) with PaddleOCR-VL 1.6 on Windows.
# Resumable: pages whose output file exists are skipped. Output: results\win\87_ocr\NNN.txt and 87_ocr_times.csv.
# Usage (from the rag directory): powershell -File experiments\win_87_ocr.ps1 [-Threads 6] [-Port 8187]
# -Scale 0.75: shrink the page image before sending (fewer image tokens). -Limit N: only the first N pages. -Suffix: output name suffix.
param([int]$Threads = 6, [int]$Port = 8187, [double]$Scale = 1.0, [int]$Limit = 0, [string]$Suffix = '')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root ('results\win\87_ocr' + $Suffix)
New-Item -ItemType Directory -Force $out | Out-Null
$times = Join-Path $root ('results\win\87_ocr' + $Suffix + '_times.csv')
$pages = Get-Content (Join-Path $root 'work\87_scan_pages.json') -Encoding UTF8 -Raw | ConvertFrom-Json
$count = $pages.Count
if ($Limit -gt 0) { $count = [math]::Min($Limit, $count) }
$todo = @()
for ($n = 0; $n -lt $count; $n++) {
    if (-not (Test-Path (Join-Path $out ('{0:D3}.txt' -f $n)))) { $todo += $n }
}
if ($todo.Count -eq 0) { Write-Output 'all pages done'; exit 0 }

$m = Join-Path $root 'models\ocr\paddleocr-vl-1.6'
$exe = Join-Path $root 'bin\win-p4-noomp\llama-server.exe'
$sargs = @('-m', "$m\PaddleOCR-VL-1.6-GGUF.gguf", '--mmproj', "$m\PaddleOCR-VL-1.6-GGUF-mmproj.gguf",
          '-t', "$Threads", '-c', '8192', '-ngl', '0', '--no-webui', '--parallel', '1', '--cache-ram', '0',
          '--host', '127.0.0.1', '--port', "$Port")
$log = Join-Path $root 'results\win\87_ocr_server.log'
$t0 = Get-Date
$proc = Start-Process -FilePath $exe -ArgumentList $sargs -PassThru -WindowStyle Hidden -RedirectStandardError $log
try {
    $ready = $false
    for ($i = 0; $i -lt 240; $i++) {
        Start-Sleep -Milliseconds 500
        try { $r = Invoke-WebRequest -UseBasicParsing "http://127.0.0.1:$Port/health" -TimeoutSec 2; if ($r.StatusCode -eq 200) { $ready = $true; break } } catch {}
    }
    if (-not $ready) { throw 'server did not start' }
    $load = ((Get-Date) - $t0).TotalSeconds
    Write-Output ("server ready in {0:N1} s" -f $load)
    if (-not (Test-Path $times)) { 'n,seconds,prompt_tokens,prompt_ms,out_tokens,gen_ms,peak_private_mb' | Set-Content $times -Encoding ASCII }
    foreach ($n in $todo) {
        $p = $pages[$n]
        $img = $p.image -replace '^/rag/', ($root + '\') -replace '/', '\'
        $bytes = [IO.File]::ReadAllBytes($img)
        if ($Scale -ne 1.0) {
            $src = [Drawing.Image]::FromFile($img)
            $w = [int]($src.Width * $Scale); $h = [int]($src.Height * $Scale)
            $dst = New-Object Drawing.Bitmap $w, $h
            $g = [Drawing.Graphics]::FromImage($dst)
            $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $g.DrawImage($src, 0, 0, $w, $h)
            $ms0 = New-Object IO.MemoryStream
            $dst.Save($ms0, [Drawing.Imaging.ImageFormat]::Png)
            $bytes = $ms0.ToArray()
            $g.Dispose(); $dst.Dispose(); $src.Dispose()
        }
        $b64 = [Convert]::ToBase64String($bytes)
        $body = '{"messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,' + $b64 +
                '"}},{"type":"text","text":"OCR:"}]}],"temperature":0,"max_tokens":4000,"cache_prompt":false}'
        $s = Get-Date
        $resp = Invoke-WebRequest -UseBasicParsing -Method Post -Uri "http://127.0.0.1:$Port/v1/chat/completions" `
            -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 1800
        $sec = ((Get-Date) - $s).TotalSeconds
        $ms = New-Object IO.MemoryStream
        $resp.RawContentStream.CopyTo($ms)
        $json = [Text.Encoding]::UTF8.GetString($ms.ToArray()) | ConvertFrom-Json
        $text = $json.choices[0].message.content
        [IO.File]::WriteAllText((Join-Path $out ('{0:D3}.txt' -f $n)), $text, (New-Object Text.UTF8Encoding $false))
        $proc.Refresh()
        $peak = [math]::Round($proc.PeakPagedMemorySize64 / 1MB)
        $tm = $json.timings
        ('{0},{1:F1},{2},{3:F0},{4},{5:F0},{6}' -f $n, $sec, $tm.prompt_n, $tm.prompt_ms, $tm.predicted_n, $tm.predicted_ms, $peak) | Add-Content $times -Encoding ASCII
        Write-Output ("page {0}: {1:N1} s, out {2} tokens, peak private {3} MB" -f $n, $sec, $tm.predicted_n, $peak)
    }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
Write-Output 'done'
