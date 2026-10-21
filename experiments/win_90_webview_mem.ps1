# ASCII-only. Experiment 90: memory of the WebView2 processes under the desktop app with different browser arguments.
#
# For each configuration the app (bin\app-win\rag-desktop.exe) is started with
# WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS set (no rebuild needed), then the private memory of every process
# below the app is recorded, grouped by WebView2 process type, at five points:
#   idle    20 s after the page is ready
#   search  after one search has put results on screen
#   later   20 s after that
#   min     20 s after minimizing the window (see RAG_WV_MINIMIZE in the app)
#   back    5 s after restoring it (also checks the results are still on the page)
# The search is driven through the remote-debugging port (same as win_desktop_test.ps1); it also checks that
# the results list rendered and saves a screenshot. Each configuration runs $Runs times.
# Resumable: a finished configuration leaves results\win\90\<label>.done.
# Usage (from the rag directory): powershell -File experiments\win_90_webview_mem.ps1 [-Runs 2] [-Only label]
param([int]$Runs = 2, [string]$Only = '', [int]$Port = 9231)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win\90'
New-Item -ItemType Directory -Force $outDir | Out-Null
$query = ([IO.File]::ReadAllLines((Join-Path $root 'work\win-args\desktop-query.txt'), [Text.Encoding]::UTF8))[0]

$tauriDefault = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection'
$leanFeatures = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,msEdgeTranslate,msEdgeShopping,AutofillServerCommunication,OptimizationHints,MediaRouter,SpareRendererForSitePerProcess,BackForwardCache,msEdgeSidebarV2'
$leanSwitches = '--disable-background-networking --disable-component-update --disable-extensions --disable-sync --disable-default-apps --no-first-run'
$configs = @(
    @{ label = 'a_none';      args = '' },
    @{ label = 'b_tauri';     args = $tauriDefault },
    @{ label = 'c_lean';      args = "$leanFeatures $leanSwitches" },
    @{ label = 'd_lean_r1';   args = "$leanFeatures $leanSwitches --renderer-process-limit=1" },
    @{ label = 'e_lean_nogpu'; args = "$leanFeatures $leanSwitches --disable-gpu" },
    @{ label = 'f_lean_lite'; args = "$leanFeatures $leanSwitches --js-flags=--lite-mode" },
    @{ label = 'g_all';       args = "$leanFeatures $leanSwitches --renderer-process-limit=1 --disable-gpu --js-flags=--lite-mode" },
    # what the app does when minimized (RAG_WV_MINIMIZE): nothing / only the LOW memory target / hide + LOW + suspend (default)
    @{ label = 'h_min_none';  args = $tauriDefault; min = 'none' },
    @{ label = 'i_min_low';   args = $tauriDefault; min = 'low' }
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
function Kind($p) {
    if ($p.Name -ne 'msedgewebview2.exe') { return $p.Name }
    if ($p.CommandLine -match '--type=([a-z-]+)') {
        $t = $Matches[1]
        if ($t -eq 'utility' -and $p.CommandLine -match '--utility-sub-type=([A-Za-z_.]+)') { return 'wv2 utility ' + ($Matches[1] -replace '\.mojom\..*', '') }
        return "wv2 $t"
    }
    'wv2 browser'
}
function MemSnap([int]$appId) {
    $groups = [ordered]@{}
    $groups['rag-desktop.exe'] = (Get-Process -Id $appId).PrivateMemorySize64
    $count = 0
    foreach ($c in (Tree $appId)) {
        try { $m = (Get-Process -Id $c.ProcessId -ErrorAction Stop).PrivateMemorySize64 } catch { continue }
        $k = Kind $c
        if ($k -like 'wv2*') { $count++ }
        if ($groups.Contains($k)) { $groups[$k] += $m } else { $groups[$k] = $m }
    }
    $wv = 0; foreach ($k in $groups.Keys) { if ($k -like 'wv2*') { $wv += $groups[$k] } }
    @{ groups = $groups; webview = $wv; count = $count }
}

$log = Join-Path $outDir 'summary.txt'
foreach ($cfg in $configs) {
    if ($Only -ne '' -and $cfg.label -ne $Only) { continue }
    $done = Join-Path $outDir ($cfg.label + '.done')
    if (Test-Path $done) { continue }
    $lines = @("===== $($cfg.label) ($(Get-Date -Format 'MM-dd HH:mm'))", "args: $($cfg.args)")
    for ($run = 1; $run -le $Runs; $run++) {
        $wvdir = Join-Path $root ('work\webview2-90-' + $cfg.label)
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
            if ($run -eq 1) {
                $cmd = (Tree $app.Id | Where-Object { $_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -notmatch '--type=' } | Select-Object -First 1).CommandLine
                $lines += "browser command line: $cmd"
            }
            $ws = New-Object Net.WebSockets.ClientWebSocket
            $none = [Threading.CancellationToken]::None
            $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $none).Wait()
            $script:msgId = 0
            function Cdp([string]$method, $params) {
                $script:msgId++
                $json = @{ id = $script:msgId; method = $method; params = $params } | ConvertTo-Json -Depth 6 -Compress
                $bytes = [Text.Encoding]::UTF8.GetBytes($json)
                $ws.SendAsync((New-Object ArraySegment[byte] (, $bytes)), [Net.WebSockets.WebSocketMessageType]::Text, $true, $none).Wait()
                while ($true) {
                    $ms = New-Object IO.MemoryStream
                    do {
                        $buf = New-Object byte[] 262144
                        $r = $ws.ReceiveAsync((New-Object ArraySegment[byte] (, $buf)), $none).Result
                        $ms.Write($buf, 0, $r.Count)
                    } while (-not $r.EndOfMessage)
                    $text = [Text.Encoding]::UTF8.GetString($ms.ToArray())
                    if ($text -match ('^\{"id":' + $script:msgId + '[,}]')) { return $text }
                }
            }
            function Js([string]$expression) {
                $raw = Cdp 'Runtime.evaluate' @{ expression = $expression; awaitPromise = $true; returnByValue = $true }
                $obj = $raw | ConvertFrom-Json
                if ($obj.result.exceptionDetails) { return "JS ERROR: $($obj.result.exceptionDetails.exception.description)" }
                $obj.result.result.value
            }
            Start-Sleep -Seconds 20
            $m1 = MemSnap $app.Id
            $queryJson = $query | ConvertTo-Json
            $search = "new Promise(async (done) => { const input = document.querySelector('form.search input'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(input, $queryJson); input.dispatchEvent(new Event('input', { bubbles: true })); await new Promise(r => setTimeout(r, 200)); document.querySelector('form.search').requestSubmit(); const t0 = performance.now(); for (let i = 0; i < 900; i++) { await new Promise(r => setTimeout(r, 100)); if (document.querySelector('ol.results') || document.querySelector('.error')) break; } await new Promise(r => setTimeout(r, 1500)); done(Math.round(performance.now() - t0 - 1500) + ' ms; results ' + document.querySelectorAll('ol.results > li').length + '; images ' + document.querySelectorAll('img.page-image').length); })"
            $res = Js $search
            $m2 = MemSnap $app.Id
            if ($run -eq 1) {
                $raw = Cdp 'Page.captureScreenshot' @{ format = 'png' }
                if ($raw -match '"data":"([^"]+)"') { [IO.File]::WriteAllBytes((Join-Path $outDir ($cfg.label + '.png')), [Convert]::FromBase64String($Matches[1])) }
            }
            Start-Sleep -Seconds 20
            $m3 = MemSnap $app.Id
            # minimize (the app hides / lowers / suspends the webview), wait, then restore and check the page still works
            $app.Refresh()
            [void][W.U]::ShowWindow($app.MainWindowHandle, 6)
            Start-Sleep -Seconds 20
            $m4 = MemSnap $app.Id
            [void][W.U]::ShowWindow($app.MainWindowHandle, 9)
            Start-Sleep -Seconds 5
            $after = Js "String(document.querySelectorAll('ol.results > li').length) + ' results still listed; visible ' + document.visibilityState"
            $m5 = MemSnap $app.Id
            $ws.Dispose()
            $lines += "run $run search: $res | after restore: $after"
            foreach ($pair in @(@('idle', $m1), @('search', $m2), @('later', $m3), @('min', $m4), @('back', $m5))) {
                $m = $pair[1]
                $detail = ($m.groups.Keys | ForEach-Object { '{0}={1:F0}' -f $_, ($m.groups[$_] / 1MB) }) -join ', '
                $lines += ('run {0} {1,-6} webview {2,5:F0} MB in {3} processes | {4}' -f $run, $pair[0], ($m.webview / 1MB), $m.count, $detail)
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
