#!/bin/bash
# 實驗 24：把 EmbeddingGemma 2 的 BF16 權重另存成 F32、F16，用來比較 CPU 上的運算速度。
set -e
cd /tmp
D=/rag/models/derived
S=/rag/experiments/convert_bf16.py
rm -f $D/mmproj-embeddinggemma-2-vision-F32.gguf $D/mmproj-embeddinggemma-2-vision-F16.gguf $D/embeddinggemma-2-F16.gguf
python3 -I $S $D/mmproj-embeddinggemma-2-vision-BF16.gguf $D/mmproj-embeddinggemma-2-vision-F32.gguf f32
python3 -I $S $D/mmproj-embeddinggemma-2-vision-BF16.gguf $D/mmproj-embeddinggemma-2-vision-F16.gguf f16
python3 -I $S /rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf $D/embeddinggemma-2-F32.gguf f32
python3 -I $S /rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf $D/embeddinggemma-2-F16.gguf f16
ls -l $D | awk '{printf "%12d  %s\n", $5, $9}'
