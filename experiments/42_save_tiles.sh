#!/usr/bin/env bash
# 實驗 42 的準備：沿空白切塊，把每一塊的向量存下來（三個評測集），給 eval_tile_scoring.py 離線比較各種計分方式。
# 用法：bash 42_save_tiles.sh [bin 名稱]
BIN=${1:-linux-p4}
PORT=8109
MODEL=/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf
MMPROJ=/rag/models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval-tiles4.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
R=/rag/results
cd /tmp
python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60_dense_n3 $R/eval60_dense_n3.npz gaps:3 save=$R/tiles_n3
python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60_dense_n6 $R/eval60_dense_n6.npz gaps:6 save=$R/tiles_n6
python3 -I /rag/experiments/eval_tiles2.py $PORT /rag/testdata/eval60 $R/eval60_bf16.npz gaps:3 save=$R/tiles_eval60
bash /rag/experiments/server.sh stop $PORT
