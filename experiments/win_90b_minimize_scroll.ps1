# ASCII-only. Experiment 90b: follow-up of 90.
#  1. Memory as Task Manager shows it (private working set) next to private bytes, for the three
#     minimize behaviours of the app (RAG_WV_MINIMIZE default / none / low) with --disable-gpu.
#  2. Scrolling cost with and without the GPU process: after a search, the results page is scrolled
#     for 180 animation frames; records frame intervals and the CPU time used by the WebView2 processes.
# Resumable: results\win\90b\<label>.done.  Usage: powershell -File experiments\win_90b_minimize_scroll.ps1 [-Runs 2]
param([int]$Runs = 2, [string]$Only = '', [int]$Port = 9232)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win\90b'
New-Item -ItemType Directory -Force $outDir | Out-Null
$query = ([IO.File]::ReadAllLines((Join-Path $root 'work\win-args\desktop-query.txt'), [Text.Encoding]::UTF8))[0]
# CalculateNativeWinOcclusion is off only for the measurement: the app window may be covered by other windows,
# and an occluded WebView2 stops drawing (no animation frames). The window is never activated.
$tauriDefault = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion'
$configs = @(
    @{ label = 'gpu_full';   args = $tauriDefault },
    @{ label = 'nogpu_full'; args = "$tauriDefault --disable-gpu" },
    @{ label = 'nogpu_none'; args = "$tauriDefault --disable-gpu"; min = 'none' },
    @{ label = 'nogpu_low';  args = "$tauriDefault --disable-gpu"; min = 'low' },
    # after the change: the app only sets the LOW memory target when minimized (RAG_WV_MINIMIZE is gone)
    @{ label = 'final';      args = $tauriDefault }
)
Add-Type -Namespace W -Name U -MemberDefinition '[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);'

function Tree([int]$rootId) {
    $all = Get-CimInstance Win32_Process
    $found = @(); $queue = @($rootId)
    while ($queue.Count -gt 0) {
        $next = @()
        foreach ($p in $all) { if ($queue -contains $p.ParentProcessId) { $found += $p; $next += $p.ProcessId } }
        $queue = $next
    }
    $found
}
# WebView2 processes only: private bytes, private working set (Task Manager "Memory"), CPU seconds
function WvSnap([int]$appId) {
    $ids = @(Tree $appId | Where-Object { $_.Name -eq 'msedgewebview2.exe' } | ForEach-Object { [int]$_.ProcessId })
    $perf = Get-CimInstance Win32_PerfRawData_PerfProc_Process | Where-Object { $ids -contains [int]$_.IDProcess }
    $priv = 0; $pws = 0; $cpu = 0.0
    foreach ($p in $perf) { $priv += [int64]$p.PrivateBytes; $pws += [int64]$p.WorkingSetPrivate }
    foreach ($id in $ids) { try { $cpu += (Get-Process -Id $id -ErrorAction Stop).TotalProcessorTime.TotalSeconds } catch { } }
    @{ priv = $priv / 1MB; pws = $pws / 1MB; cpu = $cpu; count = $ids.Count }
}

$log = Join-Path $outDir 'summary.txt'
$trace = Join-Path $outDir 'trace.txt'
function Step([string]$s) { "$(Get-Date -Format HH:mm:ss) $s" | Add-Content $trace }
$trace = Join-Path $outDir "trace.txt"
function Step([string]$s) { "$(Get-Date -Format HH:mm:ss) $s" | Add-Content $trace }
foreach ($cfg in $configs) {
    if ($Only -ne '' -and $cfg.label -ne $Only) { continue }
    $done = Join-Path $outDir ($cfg.label + '.done')
    if (Test-Path $done) { continue }
    $lines = @("===== $($cfg.label) ($(Get-Date -Format 'MM-dd HH:mm')) args: $($cfg.args) min: $($cfg.min)")
    for ($run = 1; $run -le $Runs; $run++) {
        $wvdir = Join-Path $root ('work\webview2-90b-' + $cfg.label)
        if (Test-Path $wvdir) { Remove-Item -Recurse -Force $wvdir }
        $psi = New-Object Diagnostics.ProcessStartInfo
        $psi.FileName = Join-Path $root 'bin\app-win\rag-desktop.exe'
        $psi.WorkingDirectory = $root
        $psi.UseShellExecute = $false
        $psi.EnvironmentVariables['RAG_ROOT'] = $root
        $psi.EnvironmentVariables['RAG_DATA'] = Join-Path $root 'work\index-win-desktop2'
        $psi.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = $wvdir
        $psi.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = ("--remote-debugging-port=$Port " + $cfg.args).Trim()
        if ($cfg.min) { $psi.EnvironmentVariables['RAG_WV_MINIMIZE'] = $cfg.min }
        $app = [Diagnostics.Process]::Start($psi)
        try {
            $target = $null
            for ($i = 0; $i -lt 80 -and -not $target; $i++) {
                Start-Sleep -Milliseconds 500
                try { $target = (Invoke-RestMethod "http://127.0.0.1:$Port/json" -TimeoutSec 2) | Where-Object { $_.type -eq 'page' } | Select-Object -First 1 } catch { }
            }
            if (-not $target) { throw 'no debugging port' }
            function Connect { $script:ws = New-Object Net.WebSockets.ClientWebSocket; $script:ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, [Threading.CancellationToken]::None).Wait() }
            $none = [Threading.CancellationToken]::None
            $script:msgId = 0
            Connect
            function Cdp([string]$method, $params) {
                $script:msgId++
                $json = @{ id = $script:msgId; method = $method; params = $params } | ConvertTo-Json -Depth 6 -Compress
                $bytes = [Text.Encoding]::UTF8.GetBytes($json)
                $script:ws.SendAsync((New-Object ArraySegment[byte] (, $bytes)), [Net.WebSockets.WebSocketMessageType]::Text, $true, $none).Wait()
                while ($true) {
                    $ms = New-Object IO.MemoryStream
                    do {
                        $buf = New-Object byte[] 262144
                        $task = $script:ws.ReceiveAsync((New-Object ArraySegment[byte] (, $buf)), $none)
                        if (-not $task.Wait(30000)) { throw "cdp timeout" }
                        $r = $task.Result
                        $ms.Write($buf, 0, $r.Count)
                    } while (-not $r.EndOfMessage)
                    $text = [Text.Encoding]::UTF8.GetString($ms.ToArray())
                    if ($text -match ('^\{"id":' + $script:msgId + '[,}]')) { return $text }
                }
            }
            function Js([string]$expression) {
                $obj = (Cdp 'Runtime.evaluate' @{ expression = $expression; awaitPromise = $true; returnByValue = $true }) | ConvertFrom-Json
                if ($obj.result.exceptionDetails) { return "JS ERROR: $($obj.result.exceptionDetails.exception.description)" }
                $obj.result.result.value
            }
            Start-Sleep -Seconds 15
            $queryJson = $query | ConvertTo-Json
            $search = "new Promise(async (done) => { const input = document.querySelector('form.search input'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(input, $queryJson); input.dispatchEvent(new Event('input', { bubbles: true })); await new Promise(r => setTimeout(r, 200)); document.querySelector('form.search').requestSubmit(); for (let i = 0; i < 900; i++) { await new Promise(r => setTimeout(r, 100)); if (document.querySelector('ol.results') || document.querySelector('.error')) break; } await new Promise(r => setTimeout(r, 1500)); done('results ' + document.querySelectorAll('ol.results > li').length); })"
            Step "search"
            $res = Js $search
            Step "searched"
            Start-Sleep -Seconds 10
            $s0 = WvSnap $app.Id
            $scroll = "new Promise(done => { const el = document.scrollingElement; const max = el.scrollHeight - el.clientHeight; const t = []; let last = 0, y = 0; function step(now) { if (last) t.push(now - last); last = now; y += 60; if (y > max) y = 0; el.scrollTop = y; if (t.length < 180) requestAnimationFrame(step); else { t.sort((a, b) => a - b); const avg = t.reduce((a, b) => a + b, 0) / t.length; done('scroll ' + max + ' px; frame avg ' + avg.toFixed(1) + ' ms, p95 ' + t[Math.floor(t.length * 0.95)].toFixed(1) + ', max ' + t[t.length - 1].toFixed(1) + ', over 20 ms ' + t.filter(x => x > 20).length); } } requestAnimationFrame(step); setTimeout(() => done('no animation frames after 8 s (' + t.length + ')'), 8000); })"
            Step "scroll"
            $sc = Js $scroll
            Step "scrolled"
            $s1 = WvSnap $app.Id
            Start-Sleep -Seconds 10
            $s2 = WvSnap $app.Id
            $app.Refresh()
            [void][W.U]::ShowWindow($app.MainWindowHandle, 7)
            Start-Sleep -Seconds 30
            Step "minimized"
            $s3 = WvSnap $app.Id
            [void][W.U]::ShowWindow($app.MainWindowHandle, 4)
            Start-Sleep -Seconds 5
            Step "restored"
            $check = "String(document.querySelectorAll('ol.results > li').length) + ' results; ' + document.visibilityState"
            try { $after = Js $check } catch { Step "old connection: $_"; $script:ws.Dispose(); Connect; $after = 'old debugging connection stopped answering; new one: ' + (Js $check) }
            Step "checked"
            $again = Js $search
            Step "searched again"
            $s4 = WvSnap $app.Id
            $script:ws.Dispose()
            $lines += ('run {0} {1} | {2} | scroll cpu {3:F2} s | after restore: {4} | search again: {5}' -f $run, $res, $sc, ($s1.cpu - $s0.cpu), $after, $again)
            $lines += ('run {0} cpu while minimized 30 s: {1:F2} s' -f $run, ($s3.cpu - $s2.cpu))
            foreach ($pair in @(@('later', $s2), @('min', $s3), @('back', $s4))) {
                $lines += ('run {0} {1,-6} private {2,5:F0} MB  private working set {3,5:F0} MB  ({4} processes)' -f $run, $pair[0], $pair[1].priv, $pair[1].pws, $pair[1].count)
            }
        } finally {
            $children = @(Tree $app.Id | Where-Object { $_.Name -eq 'llama-server.exe' })
            if (-not $app.HasExited) { $app.Kill() }
            Start-Sleep -Seconds 3
            foreach ($c in $children) { if (Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue) { Stop-Process -Id $c.ProcessId -Force } }
        }
    }
    $lines | Add-Content -Encoding utf8 $log
    $lines
    '' | Set-Content $done
}
