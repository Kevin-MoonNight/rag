#!/usr/bin/env bash
# 環境檢查：掛載方式、透過掛載讀模型檔的速度、CPU 指令集。
cd /rag/bin/linux-b11567/llama-b11567
export LD_LIBRARY_PATH=$PWD
./llama-server --version 2>&1 | head -3
echo "--- mount"
mount | grep ' /rag '
F=/rag/models/gemma-4-e2b/gemma-4-E2B-it-mmproj.gguf
echo "--- read 987MB (1st)"
( time dd if=$F of=/dev/null bs=4M 2>&1 | tail -1 ) 2>&1
echo "--- read 987MB (2nd)"
( time dd if=$F of=/dev/null bs=4M 2>&1 | tail -1 ) 2>&1
echo "--- cpu"
lscpu | grep -E 'Model name|^CPU\(s\)|Thread|Core'
lscpu | grep -oE 'avx[^ ]*|fma|f16c|bmi2|sse4_2' | sort -u | tr '\n' ' '
echo
free -m
