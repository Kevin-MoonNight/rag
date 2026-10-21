# ASCII-only. End-to-end test of the desktop app (bin\app-win\rag-desktop.exe) on Windows.
#
# Starts the real window with the WebView2 remote-debugging port open on 127.0.0.1, then drives the
# page through that port: builds an index of a folder, types a query into the search box, reads the
# text on screen back and saves screenshots. The WebView2 profile folder and the index are kept inside
# the rag directory. The app is closed at the end.
#
# Usage (from the rag directory):
#   powershell -File experiments\win_desktop_test.ps1 -Folder testdata\realdocs\mini -QueryFile work\win-args\desktop-query.txt
# The query file is UTF-8 with the query on the first line.
param(
    [Parameter(Mandatory = $true)][string]$Folder,
    [Parameter(Mandatory = $true)][string]$QueryFile,
    [string]$Data = 'work\index-win-desktop',
    [int]$Port = 9223
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win'
New-Item -ItemType Directory -Force $outDir | Out-Null
$query = ([IO.File]::ReadAllLines((Join-Path $root $QueryFile), [Text.Encoding]::UTF8))[0]

$psi = New-Object Diagnostics.ProcessStartInfo
$psi.FileName = Join-Path $root 'bin\app-win\rag-desktop.exe'
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.EnvironmentVariables['RAG_ROOT'] = $root
$psi.EnvironmentVariables['RAG_DATA'] = Join-Path $root $Data
$psi.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = Join-Path $root 'work\webview2-data'
$psi.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$Port"
$app = [Diagnostics.Process]::Start($psi)
"started rag-desktop.exe pid $($app.Id)"

function Tree([int]$rootId) {
    # all processes below the app in the process tree
    $all = Get-CimInstance Win32_Process
    $found = @(); $queue = @($rootId)
    while ($queue.Count -gt 0) {
        $next = @()
        foreach ($p in $all) { if ($queue -contains $p.ParentProcessId) { $found += $p; $next += $p.ProcessId } }
        $queue = $next
    }
    $found
}
function Memory([string]$label) {
    $lines = @("== memory: $label")
    $groups = @{}
    $self = Get-Process -Id $app.Id
    $groups['rag-desktop.exe'] = $self.PrivateMemorySize64
    foreach ($c in (Tree $app.Id)) {
        try { $m = (Get-Process -Id $c.ProcessId -ErrorAction Stop).PrivateMemorySize64 } catch { continue }
        if ($groups.ContainsKey($c.Name)) { $groups[$c.Name] += $m } else { $groups[$c.Name] = $m }
    }
    $total = 0
    foreach ($k in ($groups.Keys | Sort-Object)) { $lines += ('{0,-22}: private {1,6:N0} MB' -f $k, ($groups[$k] / 1MB)); $total += $groups[$k] }
    $lines += ('{0,-22}: private {1,6:N0} MB' -f 'TOTAL', ($total / 1MB))
    $lines
}

try {
    $target = $null
    for ($i = 0; $i -lt 60 -and -not $target; $i++) {
        Start-Sleep -Milliseconds 500
        try { $target = (Invoke-RestMethod "http://127.0.0.1:$Port/json" -TimeoutSec 2) | Where-Object { $_.type -eq 'page' } | Select-Object -First 1 } catch { }
    }
    if (-not $target) { throw 'the window did not open a debugging port' }
    "page url: $($target.url)"

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
    function Shot([string]$name) {
        $raw = Cdp 'Page.captureScreenshot' @{ format = 'png' }
        if ($raw -match '"data":"([^"]+)"') {
            $file = Join-Path $outDir $name
            [IO.File]::WriteAllBytes($file, [Convert]::FromBase64String($Matches[1]))
            "screenshot: results\win\$name"
        }
    }
    $report = New-Object Text.StringBuilder
    function Note([string]$title, $text) { [void]$report.AppendLine("== $title"); [void]$report.AppendLine([string]$text); [void]$report.AppendLine('') }

    Start-Sleep -Seconds 2
    Note 'window text at start' (Js 'document.body.innerText')

    # build the index of the folder (the folder picker dialog cannot be driven, so call the same command the button calls)
    $folderJson = (Join-Path $root $Folder) | ConvertTo-Json
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Note 'start_index' (Js "window.__TAURI_INTERNALS__.invoke('start_index', { folder: $folderJson }).then(() => 'started').catch(e => 'ERROR ' + e)")
    Start-Sleep -Seconds 4
    Note 'window text while indexing' (Js 'document.body.innerText')
    Shot 'desktop-indexing.png'
    $peak = Memory 'while indexing'
    $wait = "new Promise(async (done) => { for (let i = 0; i < 900; i++) { const s = await window.__TAURI_INTERNALS__.invoke('status'); if (!s.progress.running) { done(JSON.stringify(s)); return; } await new Promise(r => setTimeout(r, 1000)); } done('TIMEOUT'); })"
    Note 'status after indexing' (Js $wait)
    Note 'indexing took (seconds)' ([Math]::Round($sw.Elapsed.TotalSeconds, 1))
    $peak | ForEach-Object { [void]$report.AppendLine($_) }

    # type the query into the search box and submit the form, like a user would
    $queryJson = $query | ConvertTo-Json
    $search = "new Promise(async (done) => { const input = document.querySelector('form.search input'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(input, $queryJson); input.dispatchEvent(new Event('input', { bubbles: true })); await new Promise(r => setTimeout(r, 200)); document.querySelector('form.search').requestSubmit(); const t0 = performance.now(); for (let i = 0; i < 600; i++) { await new Promise(r => setTimeout(r, 100)); if (document.querySelector('ol.results') || document.querySelector('.error')) break; } await new Promise(r => setTimeout(r, 1500)); done(Math.round(performance.now() - t0 - 1500) + ' ms until results were on screen'); })"
    Note 'search' (Js $search)
    Note 'window text after search' ((Js 'document.body.innerText').Substring(0, 1800))
    Note 'page images shown' (Js "String(document.querySelectorAll('img.page-image').length)")
    Shot 'desktop-search.png'
    (Memory 'after search (query model loaded)') | ForEach-Object { [void]$report.AppendLine($_) }

    [IO.File]::WriteAllText((Join-Path $outDir 'desktop-test.txt'), $report.ToString(), [Text.Encoding]::UTF8)
    'report: results\win\desktop-test.txt'
} finally {
    $children = @(Tree $app.Id | Where-Object { $_.Name -eq 'llama-server.exe' })
    if (-not $app.HasExited) { $app.Kill() }
    Start-Sleep -Seconds 2
    foreach ($c in $children) {
        if (Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue) { "ORPHAN llama-server.exe pid $($c.ProcessId) - stopping it"; Stop-Process -Id $c.ProcessId -Force }
        else { "llama-server.exe pid $($c.ProcessId) ended with the app" }
    }
}
