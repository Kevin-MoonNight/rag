# ASCII-only. Experiment 96: unload-when-idle vs keep-resident-at-low-memory-priority vs preload.
# Runs llama-server with the same flags the desktop app uses (bin\win-p5-noomp, MTP on) for the answer model
# and the query embedder, and times, for each scenario, the start (until /health) and the first request:
#   cold-cache     fresh start, model file still in the file cache (just used)
#   cold-flushed   fresh start after memory pressure pushed the model file out of the file cache
#   hot            resident process, nothing trimmed
#   trimmed        resident process after EmptyWorkingSet (pages on the standby list, soft faults)
#   lowprio-press  resident process at very low memory priority, then memory pressure (hard faults)
#   both-cold      both models started at the same time (preload when the user starts typing)
# Memory pressure = allocate and touch all available memory except 0.5 GB for about 10 s, then free it.
# Results are appended to results\win\96\summary.txt; each scenario is one line, so the run can be stopped anytime.
# Usage (from the rag directory): powershell -File experiments\win_96_reload.ps1 [-Reps 3]
param([int]$Reps = 3)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$outDir = Join-Path $root 'results\win\96'
New-Item -ItemType Directory -Force $outDir | Out-Null
$log = Join-Path $outDir 'summary.txt'
$server = Join-Path $root 'bin\win-p5-noomp\llama-server.exe'
$prompt = [IO.File]::ReadAllText((Join-Path $root 'work\win-args\96_prompt.json'), [Text.Encoding]::UTF8) | ConvertFrom-Json
$query = ([IO.File]::ReadAllLines((Join-Path $root 'work\win-args\desktop-query.txt'), [Text.Encoding]::UTF8))[0]

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Mem {
    [StructLayout(LayoutKind.Sequential)]
    public struct Status { public uint Length; public uint Load; public ulong TotalPhys; public ulong AvailPhys; public ulong TotalPage; public ulong AvailPage; public ulong TotalVirtual; public ulong AvailVirtual; public ulong AvailExtended; }
    [DllImport("kernel32.dll")] static extern bool GlobalMemoryStatusEx(ref Status s);
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] static extern bool K32EmptyWorkingSet(IntPtr h);
    [DllImport("kernel32.dll")] static extern bool SetProcessInformation(IntPtr h, int cls, ref uint info, uint size);
    [DllImport("kernel32.dll")] static extern IntPtr VirtualAlloc(IntPtr addr, UIntPtr size, uint type, uint protect);
    [DllImport("kernel32.dll")] static extern bool VirtualFree(IntPtr addr, UIntPtr size, uint type);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr CreateFileW(string name, uint access, uint share, IntPtr sec, uint disposition, uint flags, IntPtr template);
    // Opening a file without buffering makes NTFS flush and purge its cached pages (when nobody has it mapped).
    public static bool Purge(string path) { IntPtr h = CreateFileW(path, 0x80000000, 7, IntPtr.Zero, 3, 0x20000000, IntPtr.Zero); if (h == new IntPtr(-1)) return false; CloseHandle(h); return true; }
    public static ulong Available() { Status s = new Status(); s.Length = (uint)Marshal.SizeOf(s); GlobalMemoryStatusEx(ref s); return s.AvailPhys; }
    public static bool Trim(int pid) { IntPtr h = OpenProcess(0x1500, false, pid); bool ok = K32EmptyWorkingSet(h); CloseHandle(h); return ok; }
    // ProcessMemoryPriority = 0; MEMORY_PRIORITY_VERY_LOW = 1
    public static bool LowPriority(int pid) { IntPtr h = OpenProcess(0x1600, false, pid); uint p = 1; bool ok = SetProcessInformation(h, 0, ref p, 4); CloseHandle(h); return ok; }
    // Allocate and touch all available memory except `keep` bytes, 1 GB at a time; hold, then free. Returns GB taken.
    public static double Press(ulong keep, int holdMs) {
        var blocks = new System.Collections.Generic.List<IntPtr>();
        ulong chunk = 1UL << 30;
        while (Available() > keep + chunk) {
            IntPtr p = VirtualAlloc(IntPtr.Zero, (UIntPtr)chunk, 0x3000, 0x04);
            if (p == IntPtr.Zero) break;
            for (ulong o = 0; o < chunk; o += 4096) Marshal.WriteByte(p, (int)o, 1);
            blocks.Add(p);
        }
        System.Threading.Thread.Sleep(holdMs);
        foreach (IntPtr p in blocks) VirtualFree(p, UIntPtr.Zero, 0x8000);
        return blocks.Count;
    }
}
'@

$common = @('-t', '6', '-c', '8192', '-lm', 'none', '-lzm', 'on', '--cache-ram', '0', '-ngl', '0', '--no-webui', '--parallel', '1', '--host', '127.0.0.1')
$models = @{
    answer = @{ port = 9361; args = @('-m', (Join-Path $root 'models\derived\e2b-q4_0-exact-head-zhtw-en.gguf'), '--reasoning', 'off', '-md', (Join-Path $root 'models\gemma-4-e2b\mtp-gemma-4-E2B-it-Q4_0.gguf'), '--spec-type', 'draft-mtp') }
    query = @{ port = 9362; args = @('-m', (Join-Path $root 'models\embeddinggemma-2\embeddinggemma-2-BF16.gguf'), '--embedding', '-b', '1024', '-ub', '1024') }
}

function Start-Model([string]$name) {
    $m = $models[$name]
    $argList = $m.args + $common + @('--port', [string]$m.port)
    $quoted = ($argList | ForEach-Object { if ($_ -match ' ') { '"' + $_ + '"' } else { $_ } }) -join ' '
    $p = Start-Process -FilePath $server -ArgumentList $quoted -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $outDir "$name.log")
    @{ name = $name; proc = $p; port = $m.port; t0 = [Diagnostics.Stopwatch]::StartNew() }
}
function Wait-Ready($s) {
    while ($true) {
        if ($s.proc.HasExited) { throw "$($s.name) exited" }
        try { $r = Invoke-WebRequest "http://127.0.0.1:$($s.port)/health" -UseBasicParsing -TimeoutSec 2; if ($r.StatusCode -eq 200) { break } } catch { }
        if ($s.t0.Elapsed.TotalSeconds -gt 180) { throw "$($s.name) not ready" }
        Start-Sleep -Milliseconds 100
    }
    [int]$s.t0.ElapsedMilliseconds
}
function Request($s) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    if ($s.name -eq 'answer') {
        $body = @{ messages = @(@{ role = 'system'; content = $prompt.system }, @{ role = 'user'; content = $prompt.user }); max_tokens = 64; temperature = 0; cache_prompt = $false } | ConvertTo-Json -Depth 5
        $r = Invoke-RestMethod "http://127.0.0.1:$($s.port)/v1/chat/completions" -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 300
        $extra = ' (prompt {0} tok {1:F0} ms, gen {2} tok {3:F0} ms)' -f $r.timings.prompt_n, $r.timings.prompt_ms, $r.timings.predicted_n, $r.timings.predicted_ms
    } else {
        $body = @{ content = 'task: search result | query: ' + $query } | ConvertTo-Json
        $null = Invoke-RestMethod "http://127.0.0.1:$($s.port)/embedding" -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 300
        $extra = ''
    }
    ('{0} ms{1}' -f [int]$sw.ElapsedMilliseconds, $extra)
}
function Footprint($s) {
    $s.proc.Refresh()
    $wsp = (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$($s.proc.Id)").WorkingSetPrivate
    'ws {0:F0} MB, private ws {1:F0} MB, private bytes {2:F0} MB' -f ($s.proc.WorkingSet64 / 1MB), ($wsp / 1MB), ($s.proc.PrivateMemorySize64 / 1MB)
}
function Stop-Model($s) { if (-not $s.proc.HasExited) { Stop-Process -Id $s.proc.Id -Force }; Start-Sleep -Seconds 2 }
function Press { $gb = [Mem]::Press(512MB, 8000); Start-Sleep -Seconds 2; "pressure took $gb GB" }
function Note([string]$line) { $line = (Get-Date -Format 'MM-dd HH:mm:ss') + ' ' + $line; Add-Content -Encoding utf8 $log $line; $line }

Note "start: available $([math]::Round([Mem]::Available() / 1GB, 1)) GB"
foreach ($name in @('query', 'answer')) {
    for ($rep = 1; $rep -le $Reps; $rep++) {
        # cold start, file cache warm
        $s = Start-Model $name; $load = Wait-Ready $s; $req = Request $s
        Note "$name rep $rep cold-cache: load $load ms, first request $req | $(Footprint $s)"
        # resident, hot
        Start-Sleep -Seconds 5
        Note "$name rep $rep hot: request $(Request $s)"
        # resident, working set emptied (soft faults)
        [void][Mem]::Trim($s.proc.Id); Start-Sleep -Seconds 2
        $after = Footprint $s
        Note "$name rep $rep trimmed: $after -> request $(Request $s)"
        # resident at very low memory priority, then memory pressure
        [void][Mem]::LowPriority($s.proc.Id)
        $press = Press
        $after = Footprint $s
        Note "$name rep $rep lowprio-press ($press): $after -> request $(Request $s)"
        Stop-Model $s
        # cold start after memory pressure pushed the model file out of the cache
        $press = Press
        $a = $models[$name].args
        $purged = @(for ($i = 0; $i -lt $a.Count - 1; $i++) { if ($a[$i] -in @('-m', '-md')) { [Mem]::Purge($a[$i + 1]) } })
        $s = Start-Model $name; $load = Wait-Ready $s; $req = Request $s
        Note "$name rep $rep cold-flushed ($press, purged $($purged -join ',')): load $load ms, first request $req"
        Stop-Model $s
    }
}
for ($rep = 1; $rep -le $Reps; $rep++) {
    # preload: both models start together (the moment the user focuses the window or starts typing)
    $q = Start-Model 'query'; $a = Start-Model 'answer'
    $lq = Wait-Ready $q; $la = Wait-Ready $a
    $rq = Request $q; $ra = Request $a
    Note "both rep $rep both-cold: query ready $lq ms, answer ready $la ms; query request $rq; answer request $ra"
    Stop-Model $q; Stop-Model $a
}
Note "done"
