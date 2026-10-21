#!/usr/bin/env bash
# 實驗 34：60 份相似文件的檢索評測（testdata/eval60，由 gen_eval_set.py 產生）。
# 用法：bash 34_eval60.sh <bin 名稱> <標籤> <model.gguf> <mmproj.gguf>
# 只看準確度，不是計時實驗。結果附加到 results/34_eval60.txt
BIN=$1; TAG=$2; MODEL=$3; MMPROJ=$4
PORT=8109
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/eval60-$TAG.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 1024 -ub 1024 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
{
echo "== $TAG: bin=$BIN model=$(basename "$MODEL") mmproj=$(basename "$MMPROJ")"
cd /tmp && python3 -I /rag/experiments/eval_retrieval.py $PORT /rag/testdata/eval60 "/rag/results/eval60_$TAG.npz"
} 2>&1 | tee -a /rag/results/34_eval60.txt
bash /rag/experiments/server.sh stop $PORT
