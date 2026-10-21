# ASCII-only (see note in win_server_test.ps1).
# Experiment 32 on Windows: E2B with the exact-grid Q4_0 model files, patched build p4.
# Usage (from the rag directory): powershell -File experiments\win_e2b_exact.ps1
$here = $PSScriptRoot
$e2b = 'models\gemma-4-e2b\gemma-4-E2B_q4_0-it.gguf'
$oldHead = 'models\derived\e2b-q4_0-head-zhtw-en.gguf'
$exact = 'models\derived\e2b-q4_0-exact.gguf'
$exactHead = 'models\derived\e2b-q4_0-exact-head-zhtw-en.gguf'
$slim = '-lm none -lzm on --cache-ram 0 --reasoning off'
& "$here\win_server_test.ps1" -Bin win-p4-noomp -Kind chat -Model $e2b       -Tag chat_p4_official   -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p4-noomp -Kind chat -Model $oldHead   -Tag chat_p4_head       -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p4-noomp -Kind chat -Model $exact     -Tag chat_p4_exact      -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p4-noomp -Kind chat -Model $exactHead -Tag chat_p4_exact_head -ServerArgs $slim
