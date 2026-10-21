#!/bin/bash
# After the code changes of 2026-10-14 (token estimate, garbled pages, script folding, small WAL files):
# rebuild the Linux binary, rebuild the 400-document index with the given chunk limits and evaluate each.
# Usage: bash 64_final_check.sh <max_tokens> [<max_tokens> ...]     -> index tags c<max_tokens>
# Progress in /rag/results/64_chain.log
log=/rag/results/64_chain.log
step() { echo "$(date +%H:%M:%S) $*" >> "$log"; }
: > "$log"
step "build linux"
bash /rag/experiments/build.sh linux > /dev/null || { step "build failed"; exit 1; }
RAG=/rag/bin/app-linux/rag
# the page whose text layer is garbled must now come out as a page image
"$RAG" convert "/rag/testdata/realdocs/pdf/GL002404/外國銀行在臺分行發行新臺幣金融債券辦法 條文對照表（112.04.26）.pdf" \
    --image-dir /rag/work/convert-pages 2>&1 | grep -E "^頁數|圖片" > /rag/results/64_garbled_convert.txt
# a chunk limit far above what the model accepts: the over-long chunks must be shortened, not fail the file
rm -rf /rag/work/index-toolong
"$RAG" index /rag/testdata/realdocs/mini --root /rag --data /rag/work/index-toolong --max-tokens 3000 --overlap 0 --no-heading-split > /rag/results/64_toolong.txt 2>&1
echo "exit=$?" >> /rag/results/64_toolong.txt
echo "requests refused as too large: $(grep -c 'too large to process' /rag/work/index-toolong/llama-index.log)" >> /rag/results/64_toolong.txt
grep -o "n_tokens = [0-9]*" /rag/work/index-toolong/llama-index.log | sort -t= -k2 -n | tail -3 >> /rag/results/64_toolong.txt
for n in "$@"; do
    tag=c$n
    step "index $tag"
    bash /rag/experiments/54_index_real.sh "$tag" --max-tokens "$n" > /dev/null
    grep -o "n_tokens = [0-9]*" "/rag/work/index-$tag/llama-index.log" | awk -v t="$tag" '{n=$3; c++; s+=n; if(n>m)m=n; if(n>256)a++; if(n>384)b++; if(n>512)d++} END {printf "%s: requests %d mean %.0f max %d  >256: %d  >384: %d  >512: %d\n", t,c,s/c,m,a,b,d}' >> "$log"
    step "eval $tag"
    bash /rag/experiments/59_eval_one.sh "$tag" - --max-tokens "$n" > /dev/null
done
step "done"
