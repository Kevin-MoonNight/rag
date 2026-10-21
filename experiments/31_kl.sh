#!/usr/bin/env bash
# 實驗 31：比較各個 E2B 模型檔和「未量化 QAT 權重」的輸出機率分布差多少（KL 散度）。
# 用法：
#   bash 31_kl.sh base <bin 名稱> <基準 model.gguf>          先用基準模型算一次，存下每個位置的機率分布
#   bash 31_kl.sh cmp  <bin 名稱> <標籤> <model.gguf>        拿另一個模型和基準比
# 文字是 testdata/kl_corpus.txt（make_kl_corpus.py 產生）。基準的機率分布檔很大，放在 work/tmp，用完可刪。
MODE=$1; BIN=$2
BASE=/rag/work/tmp/kl_base.bin
mkdir -p /rag/work/tmp
cd /tmp
if [ "$MODE" = base ]; then
    /rag/bin/$BIN/llama-perplexity -m "$3" -f /rag/testdata/kl_corpus.txt -c 512 -b 512 -t "${T:-6}" -ngl 0 \
        --kl-divergence-base $BASE 2>&1 | grep -E 'Final estimate|tokens|chunks' | cut -c1-160
    ls -l $BASE | awk '{printf "base file: %.0f MB\n", $5/1e6}'
else
    TAG=$3; MODEL=$4
    echo "== $TAG: $(basename "$MODEL")"
    /rag/bin/$BIN/llama-perplexity -m "$MODEL" -t "${T:-6}" -ngl 0 --kl-divergence-base $BASE --kl-divergence 2>&1 \
        | grep -E 'Mean PPL|Mean    KLD|Maximum KLD|99\.0%   KLD|Median  KLD|RMS Δp|Same top p' | cut -c1-120
fi
