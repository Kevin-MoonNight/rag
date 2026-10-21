#!/usr/bin/env bash
# 實驗 34 的全部組合：BF16 與 Q8_0 各跑一次 60 份文件的檢索評測，再比較兩者。
D=/rag/models/derived
E=/rag/models/embeddinggemma-2
rm -f /rag/results/34_eval60.txt
bash /rag/experiments/34_eval60.sh linux-p4 bf16 $E/embeddinggemma-2-BF16.gguf $D/mmproj-embeddinggemma-2-vision-BF16.gguf
bash /rag/experiments/34_eval60.sh linux-p4 q8   $E/embeddinggemma-2-Q8_0.gguf $D/mmproj-embeddinggemma-2-vision-Q8_0.gguf
cd /tmp && python3 -I /rag/experiments/eval_retrieval_compare.py /rag/testdata/eval60 /rag/results/eval60_bf16.npz /rag/results/eval60_q8.npz 2>&1 | tee -a /rag/results/34_eval60.txt
