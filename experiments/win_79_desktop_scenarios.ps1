# ASCII-only strings (comments may be Chinese). Experiment 79: memory of the desktop app in the plan's scenarios
# (計畫「量測方式」：剛啟動、載入模型後、閒置、連續查詢、縮小視窗), on an existing index of a given size.
# No indexing here: the index is made beforehand (rag replicate, experiment 78).
#
# Usage (from the rag directory):
#   powershell -File experiments\win_79_desktop_scenarios.ps1 -Data work\scale-x20 -Name x20 [-IdleSeconds 340]
param(
    [Parameter(Mandatory = $true)][string]$Data,
    [Parameter(Mandatory = $true)][string]$Name,
    [int]$IdleSeconds = 340,
    [int]$Port = 9224
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win'
New-Item -ItemType Directory -Force $outDir | Out-Null
$report = Join-Path $outDir "79_desktop_$Name.txt"
"" | Set-Content -Encoding utf8 $report
function Log($s) { $s | Add-Content -Encoding utf8 $report }
$queries = [IO.File]::ReadAllLines((Join-Path $root 'testdata\realdocs\queries.jsonl'), [Text.Encoding]::UTF8) |
    Where-Object { $_.Trim() } | ForEach-Object { ($_ | ConvertFrom-Json).q }

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
'@

$psi = New-Object Diagnostics.ProcessStartInfo
$psi.FileName = Join-Path $root 'bin\app-win\rag-desktop.exe'
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.EnvironmentVariables['RAG_ROOT'] = $root
$psi.EnvironmentVariables['RAG_DATA'] = Join-Path $root $Data
$psi.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = Join-Path $root 'work\webview2-data'
$psi.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-port=$Port"
$app = [Diagnostics.Process]::Start($psi)
Log "data: $Data   started rag-desktop.exe pid $($app.Id) at $(Get-Date -Format HH:mm:ss)"

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
function Memory([string]$label) {
    $groups = @{}
    $groups['rag-desktop.exe'] = (Get-Process -Id $app.Id).PrivateMemorySize64
    $ws = (Get-Process -Id $app.Id).WorkingSet64
    foreach ($c in (Tree $app.Id)) {
        try { $p = Get-Process -Id $c.ProcessId -ErrorAction Stop } catch { continue }
        if ($groups.ContainsKey($c.Name)) { $groups[$c.Name] += $p.PrivateMemorySize64 } else { $groups[$c.Name] = $p.PrivateMemorySize64 }
        $ws += $p.WorkingSet64
    }
    $total = 0; $parts = @()
    foreach ($k in ($groups.Keys | Sort-Object)) { $parts += ('{0} {1:N0}' -f $k, ($groups[$k] / 1MB)); $total += $groups[$k] }
    Log ("== {0}: private TOTAL {1:N0} MB, working set TOTAL {2:N0} MB  [{3}]" -f $label, ($total / 1MB), ($ws / 1MB), ($parts -join ', '))
}

try {
    $target = $null
    for ($i = 0; $i -lt 60 -and -not $target; $i++) {
        Start-Sleep -Milliseconds 500
        try { $target = (Invoke-RestMethod "http://127.0.0.1:$Port/json" -TimeoutSec 2) | Where-Object { $_.type -eq 'page' } | Select-Object -First 1 } catch { }
    }
    if (-not $target) { throw 'the window did not open a debugging port' }
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
    # one search through the same command the search box uses; returns milliseconds and result count
    function Search([string]$q) {
        $qj = $q | ConvertTo-Json
        Js "(async () => { const t = performance.now(); const r = await window.__TAURI_INTERNALS__.invoke('search', { query: $qj, keywordOnly: false }); return Math.round(performance.now() - t) + ' ms, ' + r.results.length + ' results (engine ' + r.milliseconds + ' ms)'; })().catch(e => 'ERROR ' + e)"
    }

    Start-Sleep -Seconds 8
    Memory 'start (8 s, no model loaded)'
    Log ("first search (loads the query model): " + (Search $queries[0]))
    Memory 'after first search'
    $times = @()
    for ($i = 1; $i -le 20; $i++) {
        $r = Search $queries[$i]
        if ($r -match '^(\d+) ms') { $times += [int]$Matches[1] } else { Log "  search $i -> $r" }
    }
    $sorted = $times | Sort-Object
    if ($sorted.Count -gt 0) { Log ("20 searches: median {0} ms, max {1} ms" -f $sorted[[int][Math]::Floor($sorted.Count / 2)], $sorted[-1]) }
    Memory 'after 20 searches'

    $hwnd = $app.MainWindowHandle
    [void][Win]::ShowWindow($hwnd, 6)   # SW_MINIMIZE
    Start-Sleep -Seconds 6
    Memory 'minimized 6 s (models should be unloaded)'
    [void][Win]::ShowWindow($hwnd, 9)   # SW_RESTORE
    Start-Sleep -Seconds 2
    Log ("search after restore (reloads the model): " + (Search $queries[21]))
    Memory 'after restore + search'

    Log "idle wait $IdleSeconds s ..."
    Start-Sleep -Seconds $IdleSeconds
    Memory "idle $IdleSeconds s (idle unload at 300 s)"
    Log ("search after idle: " + (Search $queries[22]))
} catch {
    Log "ERROR: $_"
} finally {
    $children = @(Tree $app.Id | Where-Object { $_.Name -eq 'llama-server.exe' })
    if (-not $app.HasExited) { $app.Kill() }
    Start-Sleep -Seconds 2
    foreach ($c in $children) {
        if (Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue) { Log "ORPHAN llama-server.exe pid $($c.ProcessId)"; Stop-Process -Id $c.ProcessId -Force }
    }
    Log "done $(Get-Date -Format HH:mm:ss)"
}
