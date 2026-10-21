# ASCII-only (see note in win_server_test.ps1).
# Runs the E2B server measurements on Windows for the official build and the patched build.
# Usage (from the rag directory): powershell -File experiments\win_e2b_server_all.ps1
$here = $PSScriptRoot
$e2b = 'models\gemma-4-e2b\gemma-4-E2B_q4_0-it.gguf'
$head = 'models\derived\e2b-q4_0-head-zhtw-en.gguf'
& "$here\win_server_test.ps1" -Bin win-b11567   -Kind chat -Model $e2b  -Tag chat_official_default -ServerArgs '--reasoning off'
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind chat -Model $e2b  -Tag chat_p2_mmap          -ServerArgs '-lzm on --cache-ram 0 --reasoning off'
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind chat -Model $e2b  -Tag chat_p2_none          -ServerArgs '-lm none -lzm on --cache-ram 0 --reasoning off'
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind chat -Model $head -Tag chat_p2_head_none     -ServerArgs '-lm none -lzm on --cache-ram 0 --reasoning off'
