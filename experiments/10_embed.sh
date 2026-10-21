#!/usr/bin/env bash
# EmbeddingGemma 2：啟動 llama-server、算測試資料的向量、量記憶體，並和官方實作的向量比對。
# 用法：bash 10_embed.sh <bin 名稱> <model.gguf> <mmproj.gguf> <結果標籤> [額外 llama-server 參數]
BIN=$1; MODEL=$2; MMPROJ=$3; TAG=$4; shift 4
PORT=8101
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/embed-$TAG.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 1024 -ub 1024 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -ngl 0 --no-webui --parallel 1 "$@" || exit 1
echo "== $TAG: bin=$BIN model=$(basename "$MODEL") mmproj=$(basename "$MMPROJ") extra=$*"
echo -n "idle : "; bash /rag/experiments/server.sh mem $PORT
cd /tmp && python3 -I /rag/experiments/embed_testdata.py $PORT "/rag/results/llama_embeddings_$TAG.npz" | grep -vE '^    '
echo -n "after: "; bash /rag/experiments/server.sh mem $PORT
grep -iE 'lazy read|prompt cache|audio' /tmp/embed-$TAG.log | cut -c1-160 | head -8
bash /rag/experiments/server.sh stop $PORT
sleep 1
if [ -f /rag/results/ref_embeddings.npz ]; then
    echo "-- official implementation (a) vs this run (b)"
    python3 -I /rag/experiments/compare_npz.py /rag/results/ref_embeddings.npz "/rag/results/llama_embeddings_$TAG.npz"
fi
