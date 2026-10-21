#!/bin/bash
# 實驗 69：1 位元量化會不會漏掉結果。把 d256 的向量原樣複製成 1 位元量化的索引（不重算向量），
# 用同樣的 128 題評測，和 int8 比較。合併一律用舊規則（--take-turns），int8 的基準是 results/76_pages_d256_turns.txt。Log：/rag/results/69_chain.log
log=/rag/results/69_chain.log
P="--take-turns --pages /rag/testdata/realdocs/page_labels.jsonl"
: > "$log"
step() { echo "$(date +%T) $1" >> "$log"; }
step "build linux"
bash /rag/experiments/build.sh linux >> "$log" 2>&1
for q in 1bit 1bit-q8; do
  step "requantize $q"
  rm -rf "/rag/work/index-d256-$q"
  /rag/bin/app-linux/rag requantize --data /rag/work/index-d256 --out "/rag/work/index-d256-$q" --quant "$q" >> "$log" 2>&1
done
step "eval 1bit (oversampling 2)"
bash /rag/experiments/59_eval_one.sh d256-1bit - $P > /dev/null
step "eval 1bit (oversampling 4)"
bash /rag/experiments/59_eval_one.sh d256-1bit -os4 --oversampling 4 $P > /dev/null
step "eval 1bit-q8 (oversampling 2)"
bash /rag/experiments/59_eval_one.sh d256-1bit-q8 - $P > /dev/null
step "eval int8 (oversampling 4)"
bash /rag/experiments/59_eval_one.sh d256 -os4 --oversampling 4 $P > /dev/null
step "sizes"
du -sm /rag/work/index-d256/vectors-text /rag/work/index-d256-1bit/vectors-text /rag/work/index-d256-1bit-q8/vectors-text >> "$log" 2>&1
step "done"
