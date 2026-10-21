#!/usr/bin/env bash
# 實驗 38：密集頁面切成橫帶後的檢索準確度（先跑過 37_eval_dense.sh，沿用它存下的查詢向量）。
# 用法：bash 38_eval_tiles.sh [bin 名稱]
# 只看準確度，不是計時實驗。結果寫到 results/38_eval_tiles.txt
BIN=${1:-linux-p4}
PORT=8109
MODEL=/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf
MMPROJ=/rag/models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval-tiles.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
for N in n3 n6; do
    echo "== $N (scan images)"
    (cd /tmp && python3 -I /rag/experiments/eval_tiles.py $PORT /rag/testdata/eval60_dense_$N "/rag/results/eval60_dense_$N.npz" "/rag/results/eval60_dense_${N}_tiles.npz" 2 3 4)
done 2>&1 | tee /rag/results/38_eval_tiles.txt
bash /rag/experiments/server.sh stop $PORT
