# ASCII-only. Checks that llama-server.exe dies with rag.exe when rag.exe is killed (Windows Job Object).
# Usage (from the rag directory): powershell -File experiments\win_kill_test.ps1 -Data work\index-win-mini
param([Parameter(Mandatory = $true)][string]$Data)
$root = Split-Path -Parent $PSScriptRoot
$env:RAG_ROOT = $root
$exe = Join-Path $root 'bin\app-win\rag.exe'
$p = Start-Process -FilePath $exe -ArgumentList @('search', 'test', '--data', (Join-Path $root $Data), '--hold', '60') -PassThru -WindowStyle Hidden
$child = $null
for ($i = 0; $i -lt 60 -and -not $child; $i++) {
    Start-Sleep -Milliseconds 500
    $child = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id) AND Name='llama-server.exe'"
}
if (-not $child) { 'FAIL: llama-server.exe never started'; if (-not $p.HasExited) { $p.Kill() }; exit 1 }
Start-Sleep -Seconds 3
"rag.exe pid $($p.Id), llama-server.exe pid $($child.ProcessId): killing rag.exe"
Stop-Process -Id $p.Id -Force
Start-Sleep -Seconds 2
$still = Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
if ($still) { 'FAIL: llama-server.exe is still running (orphan); stopping it'; Stop-Process -Id $child.ProcessId -Force } else { 'OK: llama-server.exe ended together with rag.exe' }
