#!/usr/bin/env bash
# 實驗 41：切塊會不會讓「一頁一份文件」的頁面變差（testdata/eval60 的掃描圖，沿空白切成 3 塊）。
# 用法：bash 41_eval_tiles_sparse.sh [bin 名稱]
# 只看準確度，不是計時實驗。結果寫到 results/41_eval_tiles_sparse.txt
BIN=${1:-linux-p4}
PORT=8109
MODEL=/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf
MMPROJ=/rag/models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval-tiles3.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
{
echo "== eval60 (one document per page, scan images)"
(cd /tmp && python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60 /rag/results/eval60_bf16.npz gaps:3)
} 2>&1 | tee /rag/results/41_eval_tiles_sparse.txt
bash /rag/experiments/server.sh stop $PORT
