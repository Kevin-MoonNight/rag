# ASCII-only (Windows PowerShell 5.1 reads a .ps1 without BOM in the system code page).
#
# Runs bin\app-win\rag.exe natively on Windows with the given arguments plus "--hold", waits until
# the program prints its HOLD line, then reports the memory of the program and of its llama-server
# child processes. Only reads/writes files inside the rag directory.
#
# Usage (from the rag directory):
#   powershell -File experiments\win_app_run.ps1 -Tag search1 -ArgsFile work\win-args\search1.txt
# The args file is UTF-8, one argument per line (so Chinese queries survive), without the exe name.
param(
    [Parameter(Mandatory = $true)][string]$Tag,
    [Parameter(Mandatory = $true)][string]$ArgsFile,
    [int]$Hold = 15,
    [int]$TimeoutSec = 7200
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'bin\app-win\rag.exe'
$outDir = Join-Path $root 'results\win'
New-Item -ItemType Directory -Force $outDir | Out-Null
$out = Join-Path $outDir "$Tag.out.txt"
$err = Join-Path $outDir "$Tag.err.txt"

$lines = [IO.File]::ReadAllLines((Join-Path $root $ArgsFile), [Text.Encoding]::UTF8) | Where-Object { $_ -ne '' }
$quoted = $lines | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }
$argLine = ($quoted -join ' ') + " --hold $Hold"

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
$proc = New-Object Diagnostics.Process
$proc.StartInfo = $psi
$sw = [Diagnostics.Stopwatch]::StartNew()
[void]$proc.Start()
$errTask = $proc.StandardError.ReadToEndAsync()

function Mem([int]$id, [string]$label) {
    try {
        $p = Get-Process -Id $id -ErrorAction Stop
        '{0,-18}: private {1,6:N0} MB   working set {2,6:N0} MB   peak working set {3,6:N0} MB' -f $label, ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB), ($p.PeakWorkingSet64 / 1MB)
    } catch { '{0,-18}: not running' -f $label }
}

$sb = New-Object Text.StringBuilder
$held = $false
# While the program runs, sample the private memory of the program and its children about once a
# second and keep the highest value seen per process name (the index model only lives during indexing).
$peaks = @{}
function Sample {
    $ids = @(@{ Id = $proc.Id; Name = 'rag.exe' })
    foreach ($c in (Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)")) { $ids += @{ Id = $c.ProcessId; Name = $c.Name } }
    $total = 0
    foreach ($i in $ids) {
        try {
            $mb = (Get-Process -Id $i.Id -ErrorAction Stop).PrivateMemorySize64 / 1MB
            $total += $mb
            if (-not $peaks.ContainsKey($i.Name) -or $peaks[$i.Name] -lt $mb) { $peaks[$i.Name] = $mb }
        } catch { }
    }
    if (-not $peaks.ContainsKey('TOTAL') -or $peaks['TOTAL'] -lt $total) { $peaks['TOTAL'] = $total }
}
$task = $proc.StandardOutput.ReadLineAsync()
while ($true) {
    if ($task.Wait(1000)) {
        $line = $task.Result
        if ($null -eq $line) { break }
        [void]$sb.AppendLine($line)
        if ($line -like 'HOLD pid=*') {
            $held = $true
            Start-Sleep -Seconds 3
            $report = @("== $Tag at HOLD (elapsed {0:N1} s)" -f $sw.Elapsed.TotalSeconds)
            $report += Mem $proc.Id 'rag.exe'
            $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)"
            foreach ($c in $children) { $report += Mem $c.ProcessId $c.Name }
            $report | ForEach-Object { [void]$sb.AppendLine($_) }
        }
        $task = $proc.StandardOutput.ReadLineAsync()
    } else {
        Sample
    }
    if ($sw.Elapsed.TotalSeconds -gt $TimeoutSec) { $proc.Kill(); break }
}
$proc.WaitForExit()
[void]$sb.AppendLine("== $Tag peak private memory while running (sampled every second)")
foreach ($k in ($peaks.Keys | Sort-Object)) { [void]$sb.AppendLine(('{0,-18}: {1,6:N0} MB' -f $k, $peaks[$k])) }
[IO.File]::WriteAllText($out, $sb.ToString(), [Text.Encoding]::UTF8)
[IO.File]::WriteAllText($err, $errTask.Result, [Text.Encoding]::UTF8)
"exit code $($proc.ExitCode), total {0:N1} s, output in results\win\$Tag.out.txt" -f $sw.Elapsed.TotalSeconds
if (-not $held) { 'WARNING: no HOLD line seen' }
