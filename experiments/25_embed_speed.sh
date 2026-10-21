#!/usr/bin/env bash
# 實驗 25：EmbeddingGemma 2 權重存成 BF16／F16／F32／Q8_0 時，圖片與文字轉向量的速度、記憶體，以及和官方向量的差異。
# 用法：bash 25_embed_speed.sh <bin 名稱> <結果標籤> <model.gguf> <mmproj.gguf>
# 計時用，執行時不要有其他吃 CPU 的工作。
BIN=$1; TAG=$2; MODEL=$3; MMPROJ=$4
PORT=8107
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/speed-$TAG.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "${T:-6}" -c 8192 -b 1024 -ub 1024 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -ngl 0 --no-webui --parallel 1 || exit 1
echo "== $TAG: model=$(basename "$MODEL") mmproj=$(basename "$MMPROJ") threads=${T:-6}"
cd /tmp
python3 -I /rag/experiments/time_image.py $PORT 4
python3 -I /rag/experiments/embed_testdata.py $PORT "/rag/results/llama_embeddings_$TAG.npz" | grep -vE '^    '
echo -n "mem  : "; bash /rag/experiments/server.sh mem $PORT
bash /rag/experiments/server.sh stop $PORT
sleep 1
python3 -I /rag/experiments/compare_npz.py /rag/results/ref_embeddings.npz "/rag/results/llama_embeddings_$TAG.npz"
