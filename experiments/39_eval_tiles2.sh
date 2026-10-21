#!/usr/bin/env bash
# 實驗 39：密集頁面沿空白間隔切塊、滑動視窗切塊後的檢索準確度（接續 38，沿用 37 存下的查詢向量）。
# 用法：bash 39_eval_tiles2.sh [bin 名稱]
# 只看準確度，不是計時實驗。結果寫到 results/39_eval_tiles2.txt
BIN=${1:-linux-p4}
PORT=8109
MODEL=/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf
MMPROJ=/rag/models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval-tiles2.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
{
echo "== n3 (scan images)"
(cd /tmp && python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60_dense_n3 /rag/results/eval60_dense_n3.npz gaps:3 slide:0.34:0.165)
echo "== n6 (scan images)"
(cd /tmp && python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60_dense_n6 /rag/results/eval60_dense_n6.npz gaps:6 gaps:3)
} 2>&1 | tee /rag/results/39_eval_tiles2.txt
bash /rag/experiments/server.sh stop $PORT
