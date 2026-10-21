# NOTE: keep this file ASCII-only. Windows PowerShell 5.1 reads a .ps1 without BOM in the system
# code page, so non-ASCII text here breaks parsing. Chinese prompt text lives in testdata\*.txt.
#
# Runs llama-server.exe natively on Windows and measures load time, memory and speed.
# Only reads/writes files inside the rag directory; the server listens on 127.0.0.1 only.
#
# Usage (from the rag directory):
#   powershell -File experiments\win_server_test.ps1 -Bin win-p2-noomp -Kind embed -Model models\... -Mmproj models\... -Tag xxx -ServerArgs "-lzm on --cache-ram 0"
#   -Kind embed : EmbeddingGemma 2 (queries, documents, page images)
#   -Kind chat  : E2B (text prompt built from the test documents, 64 generated tokens)
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [Parameter(Mandatory = $true)][ValidateSet('embed', 'chat')][string]$Kind,
    [Parameter(Mandatory = $true)][string]$Model,
    [string]$Mmproj = '',
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$ServerArgs = '',
    [int]$Threads = 6,
    [int]$Port = 8201
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "bin\$Bin\llama-server.exe"
$logDir = Join-Path $root 'results\win'
New-Item -ItemType Directory -Force $logDir | Out-Null
$log = Join-Path $logDir "$Tag.server.log"

$argList = "--host 127.0.0.1 --port $Port -m `"$(Join-Path $root $Model)`" -t $Threads -c 8192 -ngl 0 --no-webui --parallel 1 $ServerArgs"
if ($Mmproj -ne '') { $argList += " --mmproj `"$(Join-Path $root $Mmproj)`" --image-min-tokens 280 --image-max-tokens 280" }
if ($Kind -eq 'embed') { $argList += ' --embedding -b 1024 -ub 1024' }

$env:LLAMA_MEDIA_MARKER = '<__media__>'
$sw = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $exe -ArgumentList $argList -PassThru -WindowStyle Hidden -RedirectStandardError $log -RedirectStandardOutput "$log.out"

function Get-Mem([string]$label) {
    $p = Get-Process -Id $proc.Id
    '{0,-22}: private {1,6:N0} MB   working set {2,6:N0} MB   peak working set {3,6:N0} MB' -f $label, ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB), ($p.PeakWorkingSet64 / 1MB)
}
function Post([string]$path, $payload) {
    $json = $payload | ConvertTo-Json -Depth 8 -Compress
    $bytes = [Text.Encoding]::UTF8.GetBytes($json)
    Invoke-RestMethod -Uri "http://127.0.0.1:$Port$path" -Method Post -Body $bytes -ContentType 'application/json; charset=utf-8' -TimeoutSec 900
}

try {
    $ready = $false
    while ($sw.Elapsed.TotalSeconds -lt 300) {
        if ($proc.HasExited) { throw "server exited with code $($proc.ExitCode); see $log" }
        try {
            $r = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/health" -UseBasicParsing -TimeoutSec 2
            if ($r.StatusCode -eq 200) { $ready = $true; break }
        } catch { }
        Start-Sleep -Milliseconds 200
    }
    if (-not $ready) { throw 'server did not become ready in 300 s' }
    "== $Tag : bin=$Bin kind=$Kind threads=$Threads args=$ServerArgs"
    'load time             : {0:N1} s' -f $sw.Elapsed.TotalSeconds
    Get-Mem 'idle'

    $data = Get-Content -Raw -Encoding UTF8 (Join-Path $root 'testdata\texts.json') | ConvertFrom-Json
    if ($Kind -eq 'embed') {
        $t = [Diagnostics.Stopwatch]::StartNew()
        foreach ($q in $data.queries) { Post '/embedding' @{ content = @('task: search result | query: ' + $q.text) } | Out-Null }
        'queries               : {0:N0} ms each' -f ($t.Elapsed.TotalMilliseconds / $data.queries.Count)
        Get-Mem 'after queries'
        $t.Restart()
        foreach ($d in $data.docs) { Post '/embedding' @{ content = @('title: none | text: ' + $d.text) } | Out-Null }
        'documents             : {0:N0} ms each' -f ($t.Elapsed.TotalMilliseconds / $data.docs.Count)
        Get-Mem 'after documents'
        if ($Mmproj -ne '') {
            $pages = Get-ChildItem (Join-Path $root 'testdata\pages') -Filter 'page_??.png' | Sort-Object Name
            $t.Restart()
            foreach ($pg in $pages) {
                $b64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($pg.FullName))
                Post '/embedding' @{ content = @{ prompt_string = '<__media__>'; multimodal_data = @($b64) } } | Out-Null
            }
            'page images           : {0:N0} ms each' -f ($t.Elapsed.TotalMilliseconds / $pages.Count)
            Get-Mem 'after page images'
        }
    } else {
        $nl = [string][char]10
        $ctx = (($data.docs | ForEach-Object { $_.text }) -join ($nl + $nl))
        $template = Get-Content -Raw -Encoding UTF8 (Join-Path $root 'testdata\chat_prompt_template.txt')
        $prompt = $template.Replace('{CTX}', $ctx)
        foreach ($round in 1..2) {
            $res = Post '/v1/chat/completions' @{ messages = @(@{ role = 'user'; content = $prompt }); temperature = 0; max_tokens = 64; cache_prompt = $false }
            $tm = $res.timings
            'round {0}: prompt {1} tok at {2:N1} tok/s ({3:N1} s), generate {4} tok at {5:N1} tok/s' -f $round, $tm.prompt_n, $tm.prompt_per_second, ($tm.prompt_ms / 1000), $tm.predicted_n, $tm.predicted_per_second
            if ($round -eq 1) {
                [IO.File]::WriteAllText((Join-Path $logDir "$Tag.answer.txt"), $res.choices[0].message.content, [Text.Encoding]::UTF8)
            }
            Get-Mem "after round $round"
        }
    }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
