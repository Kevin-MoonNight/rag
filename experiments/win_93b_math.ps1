# ASCII-only. Experiment 93b: prompt without rule 6; LaTeX in answers is converted to plain text by rag_core::math.
# Re-runs the medical 93 questions with the new bin\app-win\rag.exe; compare with results\win\91_ask.jsonl.
# Resumable: each set leaves results\win\93b_<set>.done.
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$log = Join-Path $root 'results\win\93b_math.txt'
$sets = @(
    @{ tag = 'med';  q = 'testdata\meddocs\queries.jsonl';  data = 'work\index-med';  limit = 93 }
)
foreach ($s in $sets) {
    $done = Join-Path $root ('results\win\93b_' + $s.tag + '.done')
    if (Test-Path $done) { continue }
    ("===== " + $s.tag + " (" + (Get-Date -Format 'MM-dd HH:mm') + ")") | Add-Content -Encoding utf8 $log
    & (Join-Path $root 'bin\app-win\rag.exe') ask-eval $s.q --root $root --data $s.data --limit $s.limit `
        --out ('results\win\93b_' + $s.tag + '.jsonl') --server bin\win-p4-noomp\llama-server.exe 2>&1 | Select-Object -Last 8 | Add-Content -Encoding utf8 $log
    ("exit=" + $LASTEXITCODE) | Add-Content -Encoding utf8 $log
    if ($LASTEXITCODE -eq 0) { '' | Set-Content $done }
}
