# ASCII-only. Experiment 88: peak private memory of the generation llama-server with and without the MTP draft.
# Usage (from the rag directory): powershell -File experiments\win_88_mem.ps1
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
foreach ($d in @('', 'Q4_0')) {
    $extra = @('ask-eval', 'testdata\realdocs\queries.jsonl', '--root', $root, '--data', 'work\index-d256', '--limit', '5',
               '--out', 'results\win\88_mem.jsonl', '--server', 'bin\win-p5-noomp\llama-server.exe')
    if ($d -ne '') { $extra += @('--gen-draft', ("models\gemma-4-e2b\mtp-gemma-4-E2B-it-" + $d + ".gguf")) }
    $p = Start-Process -FilePath 'bin\app-win\rag.exe' -ArgumentList $extra -PassThru -WindowStyle Hidden -RedirectStandardOutput 'results\win\88_mem_out.txt'
    $peak = @{}
    while (-not $p.HasExited) {
        foreach ($s in Get-Process llama-server -ErrorAction SilentlyContinue) {
            $cmd = (Get-CimInstance Win32_Process -Filter ("ProcessId=" + $s.Id)).CommandLine
            $kind = 'embed'; if ($cmd -match 'e2b') { $kind = 'gen' }
            $mb = [math]::Round($s.PagedMemorySize64 / 1MB)
            if (-not $peak.ContainsKey($kind) -or $peak[$kind] -lt $mb) { $peak[$kind] = $mb }
        }
        Start-Sleep -Milliseconds 200
    }
    $label = 'none'; if ($d -ne '') { $label = $d }
    ("draft=" + $label + " peak private MB: gen=" + $peak['gen'] + " embed=" + $peak['embed'])
}
