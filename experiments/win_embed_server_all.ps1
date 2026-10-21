# ASCII-only (see note in win_server_test.ps1).
# Runs the EmbeddingGemma 2 server measurements on Windows.
# Usage (from the rag directory): powershell -File experiments\win_embed_server_all.ps1
$here = $PSScriptRoot
$bf16 = 'models\embeddinggemma-2\embeddinggemma-2-BF16.gguf'
$q8 = 'models\embeddinggemma-2\embeddinggemma-2-Q8_0.gguf'
$mmFull = 'models\embeddinggemma-2\mmproj-embeddinggemma-2-BF16.gguf'
$mmBf16 = 'models\derived\mmproj-embeddinggemma-2-vision-BF16.gguf'
$mmQ8 = 'models\derived\mmproj-embeddinggemma-2-vision-Q8_0.gguf'
$slim = '-lm none -lzm on --cache-ram 0'
& "$here\win_server_test.ps1" -Bin win-b11567   -Kind embed -Model $bf16 -Mmproj $mmFull -Tag embed_official_bf16_full -ServerArgs ''
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $bf16                 -Tag embed_p2_bf16_text      -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $q8                   -Tag embed_p2_q8_text        -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $bf16 -Mmproj $mmBf16 -Tag embed_p2_bf16_vision    -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $q8   -Mmproj $mmQ8   -Tag embed_p2_q8_vision      -ServerArgs $slim
