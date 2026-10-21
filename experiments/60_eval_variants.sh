#!/bin/bash
# Run the search-option comparisons on one index, strictly one after another
# (two processes cannot open the same index at once).
# Usage: bash 60_eval_variants.sh <index tag>
# Waits until no index run is active, then writes /rag/results/59_eval_<tag><suffix>.txt
tag=$1
while pgrep -f 54_index_real.sh > /dev/null; do sleep 5; done
run() { bash /rag/experiments/59_eval_one.sh "$tag" "$@" > /dev/null; }
run -                               # defaults (agreement depth 10)
run _agree0 --agree 0               # plain take-turns (the earlier behaviour)
run _agree3 --agree 3
run _agree5 --agree 5
run _agree20 --agree 20
run _offset0 --image-offset 0
run _offset004 --image-offset 0.04
run _offset012 --image-offset 0.12
run _nokeywords --no-keywords
run _nokeywords_agree0 --no-keywords --agree 0
grep -H -E "^合併|exit=" /rag/results/59_eval_"$tag"*.txt | grep -v "exit=0"
echo "variants done"
