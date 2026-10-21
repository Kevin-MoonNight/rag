#!/bin/bash
# Evaluate one index with the question file, at low priority and with few threads (light on the CPU).
# Usage: bash 59_eval_one.sh <index tag> <result suffix or -> [search args]
# Result: /rag/results/59_eval_<tag><suffix>.txt and .jsonl
tag=$1; suffix=$2; shift 2
[ "$suffix" = "-" ] && suffix=""
out=/rag/results/59_eval_$tag$suffix
nice -n 19 /rag/bin/app-linux/rag eval /rag/testdata/realdocs/queries.jsonl --root /rag --data "/rag/work/index-$tag" \
    --threads "${EVAL_THREADS:-2}" --out "$out.jsonl" "$@" > "$out.txt" 2>&1
echo "exit=$?" >> "$out.txt"
cat "$out.txt"
