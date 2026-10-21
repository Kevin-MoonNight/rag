#!/usr/bin/env bash
# 實驗 40（對照）：密集頁面整頁一個向量，但圖片 token 從 280 加到 560、1120，看「只抓得到頁面頂端」是不是解析度不夠造成的。
# 用法：bash 40_eval_tokens.sh [bin 名稱]
# 只看準確度，不是計時實驗。結果寫到 results/40_eval_tokens.txt
BIN=${1:-linux-p4}
PORT=8109
MODEL=/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf
MMPROJ=/rag/models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf
rm -f /rag/results/40_eval_tokens.txt
for TOK in 560 1120; do
    bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval-tokens.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
        --image-min-tokens $TOK --image-max-tokens $TOK -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
    for N in n3 n6; do
        echo "== $N (scan images), whole page, $TOK image tokens"
        (cd /tmp && python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60_dense_$N /rag/results/eval60_dense_$N.npz slide:1:1)
    done 2>&1 | tee -a /rag/results/40_eval_tokens.txt
    bash /rag/experiments/server.sh stop $PORT
done
