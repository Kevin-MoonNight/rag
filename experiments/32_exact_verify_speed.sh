#!/usr/bin/env bash
# 實驗 32：無損 Q4_0 版 E2B 的驗證與速度。
#   1. rag-headcheck：精簡輸出層後，保留的 token 的 logit 是否和完整版逐位元相同。
#   2. rag-check：提示階段跳過上面 20 層（修改一）在這個模型檔上是否仍正確。
#   3. rag-headcost：輸出層佔每個 token 多少時間（官方檔、無損版、無損版加精簡輸出層）。
#   4. llama-bench：讀入提示與產生回答的速度。
# 計時用，執行時不要有其他吃 CPU 的工作。結果寫到 results/32_exact_verify_speed.txt
D=/rag/models/derived
OFF=/rag/models/gemma-4-e2b/gemma-4-E2B_q4_0-it.gguf
P2=/rag/bin/linux-p2
cd /tmp
{
echo "== headcheck: exact vs exact + pruned head"
$P2/rag-headcheck $D/e2b-q4_0-exact.gguf $D/e2b-q4_0-exact-head-zhtw-en.gguf 6 96 2>&1 | grep -vE '^(llama_|load|print_info|ggml|sched|init|common|\.|$)' | tail -8
echo "== rag-check: early crop on exact + pruned head"
$P2/rag-check $D/e2b-q4_0-exact-head-zhtw-en.gguf 6 8 2>&1 | grep -vE '^(llama_|load|print_info|ggml|sched|init|common|\.|$)' | tail -6
for m in $OFF $D/e2b-q4_0-head-zhtw-en.gguf $D/e2b-q4_0-exact.gguf $D/e2b-q4_0-exact-head-zhtw-en.gguf; do
    echo "== headcost: $(basename $m)"
    for t in 4 6; do $P2/rag-headcost $m $t 64 2>&1 | grep -E 'threads|per token|output head'; done
done
for m in $OFF $D/e2b-q4_0-exact.gguf $D/e2b-q4_0-exact-head-zhtw-en.gguf; do
    echo "== llama-bench: $(basename $m)"
    /rag/bin/linux-p4/llama-bench -m $m -t 4,6 -p 512 -n 64 -r 3 -ngl 0 2>&1 | grep -E '^\|' | cut -c1-160
done
} 2>&1 | tee /rag/results/32_exact_verify_speed.txt
