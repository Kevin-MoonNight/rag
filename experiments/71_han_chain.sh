#!/bin/bash
# 實驗 71：相容漢字（U+F900 區等）換成一般漢字之後重建 d256，重跑檢索評測、生成評測和 1 位元量化實驗。
# 舊的索引改名為 index-d256-prehan，舊的結果加上 _prehan 保留。Log：/rag/results/71_chain.log
log=/rag/results/71_chain.log
step() { echo "$(date +%T) $*" >> "$log"; }
: > "$log"
step "test rag_core rag_convert"
bash /rag/experiments/build.sh test -p rag_core -p rag_convert > /dev/null || { step "test failed"; exit 1; }
step "build linux"
bash /rag/experiments/build.sh linux > /dev/null || { step "build failed"; exit 1; }
cd /rag/results
for f in 54_index_d256.txt 59_eval_d256.txt 59_eval_d256.jsonl 67_ask_eval.jsonl 67_ask_eval.txt 67_ask_unanswerable.jsonl 67_ask_unanswerable.txt 67_chain.log; do
  [ -f "$f" ] && [ ! -f "${f%.*}_prehan.${f##*.}" ] && mv "$f" "${f%.*}_prehan.${f##*.}"
done
[ -d /rag/work/index-d256-prehan ] || mv /rag/work/index-d256 /rag/work/index-d256-prehan
rm -rf /rag/work/index-d256
step "index d256"
bash /rag/experiments/54_index_real.sh d256 > /dev/null
step "eval d256"
bash /rag/experiments/59_eval_one.sh d256 - > /dev/null
step "yin check"
YIN_MODELS=derived/e2b-q4_0-exact-head-zhtw-en.gguf YIN_SUFFIX=_after bash /rag/experiments/70_yin_check.sh
step "ask eval"
bash /rag/experiments/67_ask_eval.sh
step "quant"
bash /rag/experiments/69_quant.sh
step "done"
