# ASCII-only (see note in win_server_test.ps1).
# Experiment 25 on Windows: vision encoder stored as F32 instead of BF16 (text model stays BF16).
# Usage (from the rag directory): powershell -File experiments\win_embed_f32.ps1
$here = $PSScriptRoot
$bf16 = 'models\embeddinggemma-2\embeddinggemma-2-BF16.gguf'
$mmBf16 = 'models\derived\mmproj-embeddinggemma-2-vision-BF16.gguf'
$mmF32 = 'models\derived\mmproj-embeddinggemma-2-vision-F32.gguf'
$slim = '-lm none -lzm on --cache-ram 0'
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $bf16 -Mmproj $mmBf16 -Tag embed_p2_bf16_vision_rerun -ServerArgs $slim
& "$here\win_server_test.ps1" -Bin win-p2-noomp -Kind embed -Model $bf16 -Mmproj $mmF32  -Tag embed_p2_bf16_visionF32    -ServerArgs $slim
