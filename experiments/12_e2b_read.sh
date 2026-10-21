#!/usr/bin/env bash
# E2B 讀頁面圖片測試。
# 用法：bash 12_e2b_read.sh <bin 名稱> <圖片 token 數> <圖片 pattern> <結果標籤> [threads]
# 環境變數 MODEL 可以換成別的 E2B 模型檔（預設是 Google 發布的 Q4_0）。
BIN=$1; TOK=$2; PATTERN=$3; TAG=$4; T=${5:-6}
PORT=8103
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/e2b-$TAG.log \
    -m "${MODEL:-/rag/models/gemma-4-e2b/gemma-4-E2B_q4_0-it.gguf}" --mmproj /rag/models/gemma-4-e2b/gemma-4-E2B-it-mmproj.gguf \
    -t "$T" -c 8192 --image-min-tokens "$TOK" --image-max-tokens "$TOK" -lm none -lzm on -ngl 0 --no-webui --parallel 1 --cache-ram 0 --reasoning off || exit 1
echo "== $TAG: bin=$BIN image_tokens=$TOK pattern=$PATTERN threads=$T tiles=${TILES:-1} skip_text_qa=${SKIP_TEXT_QA:-0}"
bash /rag/experiments/server.sh mem $PORT
cd /tmp && python3 -I /rag/experiments/e2b_read_pages.py $PORT "/rag/results/12_e2b_read_$TAG.json" "$PATTERN"
bash /rag/experiments/server.sh mem $PORT
bash /rag/experiments/server.sh stop $PORT
sleep 1
