#!/bin/bash
# Evaluate the real-document index and its variants, one after another (inside the container).
# Waits for the running a256 index first. Results: /rag/results/59_eval_<tag>[_<option>].txt
RAG=/rag/bin/app-linux/rag
Q=/rag/testdata/realdocs/queries.jsonl

evaluate() {  # evaluate <index tag> <result suffix> [search args]
    local tag=$1 suffix=$2; shift 2
    "$RAG" eval "$Q" --root /rag --data "/rag/work/index-$tag" --out "/rag/results/59_eval_$tag$suffix.jsonl" "$@" \
        > "/rag/results/59_eval_$tag$suffix.txt" 2>&1
    echo "exit=$?" >> "/rag/results/59_eval_$tag$suffix.txt"
}

until grep -q "^exit=" /rag/results/54_index_a256.txt 2>/dev/null; do sleep 10; done
evaluate a256 ""
evaluate a256 _offset0 --image-offset 0
evaluate a256 _offset004 --image-offset 0.04
evaluate a256 _offset012 --image-offset 0.12
evaluate a256 _nokeywords --no-keywords
echo "STAGE a256 done" >> /rag/results/59_chain.log

bash /rag/experiments/54_index_real.sh a128 --max-tokens 128 > /dev/null 2>&1
evaluate a128 ""
echo "STAGE a128 done" >> /rag/results/59_chain.log

bash /rag/experiments/54_index_real.sh a512 --max-tokens 512 > /dev/null 2>&1
evaluate a512 ""
echo "STAGE a512 done" >> /rag/results/59_chain.log

bash /rag/experiments/54_index_real.sh nosplit --no-heading-split > /dev/null 2>&1
evaluate nosplit ""
echo "STAGE nosplit done" >> /rag/results/59_chain.log

bash /rag/experiments/54_index_real.sh plain --no-headings > /dev/null 2>&1
evaluate plain ""
echo "STAGE plain done" >> /rag/results/59_chain.log
echo "CHAIN finished" >> /rag/results/59_chain.log
