# ASCII-only strings (comments may be Chinese).
# 實驗 83：docling.rs（docling-rs.exe 2.24.0，Windows 原生）和 LiteParse（rag convert）比較：條文對照表的文字順序、時間、記憶體。
# 每個檔案、每種方式跑一次，輸出存在 results\win\83\<代號>_<方式>.txt，時間和記憶體高峰記在 results\win\83_docling.txt。
# 記憶體每 20 毫秒取樣一次（私有記憶體和工作集的最高值）。可以中途停掉：做完的組合會留下 .done，重跑時跳過。
# 用法（在 rag 目錄下）：powershell -File experiments\win_83_docling.ps1 [-Modes liteparse,textlayer,noocr] [-Codes FL006726,...]
param(
    [string[]]$Modes = @('liteparse', 'textlayer'),
    [string[]]$Codes = @('FL006726', 'FL006727', 'FL044349', 'FL006469')
)
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
# with -File, a comma-separated list arrives as one string
$Modes = $Modes -split ','
$Codes = $Codes -split ','
$outDir = Join-Path $root 'results\win\83'
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $root 'results\win\83_docling.txt'
$rag = Join-Path $root 'bin\app-win\rag.exe'
$docling = Join-Path $root 'work\docling\bin\docling-rs.exe'

function Run-Measured([string]$exe, [string]$argLine, [string]$outFile, [string]$workers = '') {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = $argLine
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [Text.Encoding]::UTF8
    $psi.CreateNoWindow = $true
    $psi.EnvironmentVariables['RAG_ROOT'] = $root
    if ($workers) { $psi.EnvironmentVariables['DOCLING_RS_PDF_WORKERS'] = $workers }
    $p = New-Object Diagnostics.Process
    $p.StartInfo = $psi
    $sw = [Diagnostics.Stopwatch]::StartNew()
    [void]$p.Start()
    $o = $p.StandardOutput.ReadToEndAsync()
    $e = $p.StandardError.ReadToEndAsync()
    $priv = 0; $ws = 0
    while (-not $p.HasExited) {
        try { $p.Refresh(); $priv = [Math]::Max($priv, $p.PrivateMemorySize64); $ws = [Math]::Max($ws, $p.WorkingSet64) } catch {}
        Start-Sleep -Milliseconds 20
    }
    $p.WaitForExit()
    $sec = $sw.Elapsed.TotalSeconds
    [IO.File]::WriteAllText($outFile, $o.Result + "`n" + $e.Result, (New-Object Text.UTF8Encoding $false))
    '{0,-28} exit={1} seconds={2:N2} peak private {3:N0} MB, peak working set {4:N0} MB' -f (Split-Path -Leaf $outFile), $p.ExitCode, $sec, ($priv / 1MB), ($ws / 1MB)
}

foreach ($code in $Codes) {
    $pdf = Get-ChildItem (Join-Path $root "testdata\realdocs\pdf\$code") -Filter '*.pdf' | Where-Object { $_.Name -notmatch ([char]0x7E3D + [char]0x8AAA + [char]0x660E) } | Select-Object -First 1
    foreach ($m in $Modes) {
        $done = Join-Path $outDir "${code}_$m.done"
        if (Test-Path $done) { continue }
        $out = Join-Path $outDir "${code}_$m.txt"
        $q = '"' + $pdf.FullName + '"'
        $line = $null
        switch ($m) {
            'liteparse' { $line = Run-Measured $rag "convert $q" $out }
            'textlayer' { $line = Run-Measured $docling "$q --text-layer-only" $out }
            'noocr' { $line = Run-Measured $docling "$q --no-ocr" $out }
            'noocr-notable' { $line = Run-Measured $docling "$q --no-ocr --no-table-former" $out }
            'noocr-w1' { $line = Run-Measured $docling "$q --no-ocr" $out '1' }
            'noocr-w2' { $line = Run-Measured $docling "$q --no-ocr" $out '2' }
        }
        if (-not $line) { "unknown mode $m"; continue }
        $line | Add-Content -Encoding utf8 $log
        $line
        "" | Set-Content $done
    }
}
