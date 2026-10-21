#!/usr/bin/env bash
# 用詳細紀錄模式跑一張頁面圖片的向量，看時間花在哪裡。
# 用法：bash trace_image.sh <bin 名稱> <model.gguf> <mmproj.gguf> [threads] [額外 llama-server 參數]
BIN=$1; MODEL=$2; MMPROJ=$3; T=${4:-6}
shift 4 || shift $#
bash /rag/experiments/server.sh start "$BIN" 8102 /tmp/trace.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "$T" -c 8192 -b 1024 -ub 1024 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -ngl 0 --no-webui --parallel 1 -lv 4 "$@" > /dev/null
bash /rag/experiments/server.sh mem 8102
cd /tmp && python3 -I /rag/experiments/time_image.py 8102 3
bash /rag/experiments/server.sh mem 8102
grep -iE 'encod|preproc|image.*ms| ms$|ms\)|alloc|MiB|n_tokens|batch' /tmp/trace.log | grep -viE 'kv cache|n_past|memory_seq' | tail -${TAIL:-40}
bash /rag/experiments/server.sh stop 8102
