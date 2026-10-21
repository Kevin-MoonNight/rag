# ASCII-only. Experiment 95: the desktop app with MTP on by default (bin\win-p5-noomp + MTP Q4_0 draft).
# Starts bin\app-win\rag-desktop.exe on work\index-win-1bit, searches the query in work\win-args\desktop-query.txt,
# presses "generate answer" and waits for it, then records the timing line shown under the answer, the answer
# length, the command lines of the llama-server processes, and their private memory. Each run is a fresh start.
# Usage (from the rag directory): powershell -File experiments\win_95_mtp_desktop.ps1 [-Runs 3]
param([int]$Runs = 3, [int]$Port = 9234)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win\95'
New-Item -ItemType Directory -Force $outDir | Out-Null
$query = ([IO.File]::ReadAllLines((Join-Path $root 'work\win-args\desktop-query.txt'), [Text.Encoding]::UTF8))[0]
$log = Join-Path $outDir 'summary.txt'

for ($run = 1; $run -le $Runs; $run++) {
    $wvdir = Join-Path $root 'work\webview2-95'
    if (Test-Path $wvdir) { Remove-Item -Recurse -Force $wvdir }
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = Join-Path $root 'bin\app-win\rag-desktop.exe'
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.EnvironmentVariables['RAG_ROOT'] = $root
    $psi.EnvironmentVariables['RAG_DATA'] = Join-Path $root 'work\index-win-1bit'
    $psi.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = $wvdir
    # occlusion detection off only for the test: the window may be covered by other windows
    $psi.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$Port --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion"
    $app = [Diagnostics.Process]::Start($psi)
    $lines = @()
    try {
        $target = $null
        for ($i = 0; $i -lt 80 -and -not $target; $i++) {
            Start-Sleep -Milliseconds 500
            try { $target = (Invoke-RestMethod "http://127.0.0.1:$Port/json" -TimeoutSec 2) | Where-Object { $_.type -eq 'page' } | Select-Object -First 1 } catch { }
        }
        if (-not $target) { throw 'no debugging port' }
        $ws = New-Object Net.WebSockets.ClientWebSocket
        $none = [Threading.CancellationToken]::None
        $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $none).Wait()
        $script:id = 0
        function Cdp([string]$method, $params) {
            $script:id++
            $json = @{ id = $script:id; method = $method; params = $params } | ConvertTo-Json -Depth 6 -Compress
            $bytes = [Text.Encoding]::UTF8.GetBytes($json)
            $ws.SendAsync((New-Object ArraySegment[byte] (, $bytes)), [Net.WebSockets.WebSocketMessageType]::Text, $true, $none).Wait()
            while ($true) {
                $ms = New-Object IO.MemoryStream
                do {
                    $buf = New-Object byte[] 262144
                    $task = $ws.ReceiveAsync((New-Object ArraySegment[byte] (, $buf)), $none)
                    if (-not $task.Wait(180000)) { throw 'cdp timeout' }
                    $r = $task.Result
                    $ms.Write($buf, 0, $r.Count)
                } while (-not $r.EndOfMessage)
                $text = [Text.Encoding]::UTF8.GetString($ms.ToArray())
                if ($text -match ('^\{"id":' + $script:id + '[,}]')) { return $text }
            }
        }
        function Js([string]$expression) {
            $obj = (Cdp 'Runtime.evaluate' @{ expression = $expression; awaitPromise = $true; returnByValue = $true }) | ConvertFrom-Json
            if ($obj.result.exceptionDetails) { return "JS ERROR: $($obj.result.exceptionDetails.exception.description)" }
            $obj.result.result.value
        }
        Start-Sleep -Seconds 10
        $qj = $query | ConvertTo-Json
        $search = "new Promise(async (done) => { const input = document.querySelector('form.search input'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(input, $qj); input.dispatchEvent(new Event('input', { bubbles: true })); await new Promise(r => setTimeout(r, 200)); document.querySelector('form.search').requestSubmit(); for (let i = 0; i < 600; i++) { await new Promise(r => setTimeout(r, 100)); if (document.querySelector('ol.results')) break; } await new Promise(r => setTimeout(r, 3000)); done('results ' + document.querySelectorAll('ol.results > li').length); })"
        $res = Js $search
        # press the answer button and wait until the timing line appears (or 150 s)
        $answer = "new Promise(async (done) => { const panel = document.querySelector('section.answer'); const button = [...panel.querySelectorAll('button')].find(b => b.textContent.includes(String.fromCharCode(0x7522, 0x751f, 0x56de, 0x7b54))); if (!button) { done('no answer button'); return; } button.click(); const t0 = performance.now(); for (let i = 0; i < 1500; i++) { await new Promise(r => setTimeout(r, 100)); const m = [...panel.querySelectorAll('p.muted')].map(p => p.textContent).find(s => s.includes(String.fromCharCode(0x5168, 0x90e8))); if (m) { const a = panel.querySelector('.answer-text'); done(Math.round(performance.now() - t0) + ' ms | ' + m + ' | answer chars ' + (a ? a.textContent.length : 0) + ' | sources ' + panel.querySelectorAll('ol.sources > li').length); return; } } done('timeout'); })"
        $ans = Js $answer
        $images = Js "String([...document.querySelectorAll('img.page-image')].filter(i => i.naturalWidth > 0).length) + ' page images shown'"
        $lines += "run $run (" + (Get-Date -Format 'MM-dd HH:mm') + ") search: $res | answer: $ans | $images"
        foreach ($p in (Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $app.Id -and $_.Name -eq 'llama-server.exe' })) {
            $mem = (Get-Process -Id $p.ProcessId).PrivateMemorySize64 / 1MB
            $cmd = $p.CommandLine -replace '--api-key \S+', '--api-key ***'
            $lines += ('  llama-server private {0:F0} MB: {1}' -f $mem, $cmd)
        }
        if ($run -eq 1) {
            $shot = Cdp 'Page.captureScreenshot' @{ format = 'png' }
            if ($shot -match '"data":"([^"]+)"') { [IO.File]::WriteAllBytes((Join-Path $outDir 'answer.png'), [Convert]::FromBase64String($Matches[1])) }
        }
        $ws.Dispose()
    } catch {
        $lines += "run $run failed: $_"
    } finally {
        $kids = @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $app.Id -and $_.Name -eq 'llama-server.exe' })
        if (-not $app.HasExited) { $app.Kill() }
        Start-Sleep -Seconds 3
        foreach ($k in $kids) { Stop-Process -Id $k.ProcessId -Force -ErrorAction SilentlyContinue }
    }
    $lines | Add-Content -Encoding utf8 $log
    $lines
}
if (Test-Path (Join-Path $root 'work\webview2-95')) { Remove-Item -Recurse -Force (Join-Path $root 'work\webview2-95') }
