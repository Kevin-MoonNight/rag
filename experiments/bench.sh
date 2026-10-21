#!/usr/bin/env bash
# 通用速度量測。用法：bash bench.sh <bin 名稱(linux-xxx)> <threads> [額外 llama-bench 參數]
#   MODEL 環境變數可換模型檔。模型一律讀進記憶體（-lm none），逐層詞向量表留在磁碟（-lzm on）。
set -u
BIN=/rag/bin/$1
THREADS=$2
shift 2
MODEL=${MODEL:-/rag/models/gemma-4-e2b/gemma-4-E2B_q4_0-it.gguf}
$BIN/llama-bench -m "$MODEL" -ngl 0 -t "$THREADS" -r 3 -lm none -lzm on "$@" 2>&1 | grep -vE '^load_backend|^$'
