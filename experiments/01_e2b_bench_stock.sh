#!/usr/bin/env bash
# 實驗 01：官方未修改的 llama.cpp b11567（Linux 執行檔）跑 Google 官方 E2B QAT 檔，純 CPU。
# 量讀入提示（pp）與產生（tg）的速度。用法：bash 01_e2b_bench_stock.sh [threads] [extra llama-bench args]
set -u
BIN=/rag/bin/linux-b11567/llama-b11567
export LD_LIBRARY_PATH=$BIN
MODEL=${MODEL:-/rag/models/gemma-4-e2b/gemma-4-E2B_q4_0-it.gguf}
THREADS=${1:-6}
shift || true
$BIN/llama-bench -m "$MODEL" -ngl 0 -t "$THREADS" -p 512 -n 64 -r 3 -lzm on "$@" 2>&1
