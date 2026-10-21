#!/usr/bin/env bash
# 實驗 25 的全部組合，結果寫到 results/25_embed_speed.txt
D=/rag/models/derived
E=/rag/models/embeddinggemma-2
S=/rag/experiments/25_embed_speed.sh
{
bash $S linux-p2 speed_bf16 $E/embeddinggemma-2-BF16.gguf $D/mmproj-embeddinggemma-2-vision-BF16.gguf
bash $S linux-p2 speed_f16  $D/embeddinggemma-2-F16.gguf  $D/mmproj-embeddinggemma-2-vision-F16.gguf
bash $S linux-p2 speed_f32  $D/embeddinggemma-2-F32.gguf  $D/mmproj-embeddinggemma-2-vision-F32.gguf
bash $S linux-p2 speed_q8   $E/embeddinggemma-2-Q8_0.gguf $D/mmproj-embeddinggemma-2-vision-Q8_0.gguf
} 2>&1 | tee /rag/results/25_embed_speed.txt
