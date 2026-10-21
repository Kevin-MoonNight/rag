#!/usr/bin/env bash
# 實驗 27：修改六（BF16 權重直接乘 F32 活化值）前後的速度，並確認結果和 F32 權重相同。
# 計時用，執行時不要有其他吃 CPU 的工作。結果寫到 results/27_bf16_f32_kernel.txt
D=/rag/models/derived
E=/rag/models/embeddinggemma-2
S=/rag/experiments/25_embed_speed.sh
{
bash $S linux-p2 k_p2_bf16 $E/embeddinggemma-2-BF16.gguf $D/mmproj-embeddinggemma-2-vision-BF16.gguf
bash $S linux-p3 k_p3_bf16 $E/embeddinggemma-2-BF16.gguf $D/mmproj-embeddinggemma-2-vision-BF16.gguf
bash $S linux-p3 k_p3_f32  $D/embeddinggemma-2-F32.gguf  $D/mmproj-embeddinggemma-2-vision-F32.gguf
echo "== p3 with BF16 weights (a) vs p3 with F32 weights (b)"
cd /tmp && python3 -I /rag/experiments/compare_npz.py /rag/results/llama_embeddings_k_p3_f32.npz /rag/results/llama_embeddings_k_p3_bf16.npz
python3 -I /rag/experiments/npz_maxdiff.py /rag/results/llama_embeddings_k_p3_f32.npz /rag/results/llama_embeddings_k_p3_bf16.npz
} 2>&1 | tee /rag/results/27_bf16_f32_kernel.txt
