# ASCII-only (see note in win_server_test.ps1).
# Experiment 27 on Windows: modification 6 (BF16 weights x F32 activations), 6 and 12 threads.
# Usage (from the rag directory): powershell -File experiments\win_embed_p3.ps1
$here = $PSScriptRoot
$bf16 = 'models\embeddinggemma-2\embeddinggemma-2-BF16.gguf'
$mmBf16 = 'models\derived\mmproj-embeddinggemma-2-vision-BF16.gguf'
$slim = '-lm none -lzm on --cache-ram 0'
& "$here\win_server_test.ps1" -Bin win-p3-noomp -Kind embed -Model $bf16                 -Tag embed_p3_bf16_text       -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p3-noomp -Kind embed -Model $bf16 -Mmproj $mmBf16 -Tag embed_p3_bf16_vision     -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p3-noomp -Kind embed -Model $bf16 -Mmproj $mmBf16 -Tag embed_p3_bf16_vision_12t -ServerArgs $slim -Threads 12
