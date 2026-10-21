# ASCII-only (see note in win_server_test.ps1).
# Experiment 32 on Windows: llama-bench for the official, exact-grid and exact-grid + pruned head model files.
# Usage (from the rag directory): powershell -File experiments\win_bench_exact.ps1
$here = $PSScriptRoot
$models = @(
    'models\gemma-4-e2b\gemma-4-E2B_q4_0-it.gguf',
    'models\derived\e2b-q4_0-exact.gguf',
    'models\derived\e2b-q4_0-exact-head-zhtw-en.gguf'
)
foreach ($m in $models) {
    "== $m"
    & "$here\win_bench.ps1" -Bins 'win-p4-noomp' -Threads '4,6' -Model $m
}
