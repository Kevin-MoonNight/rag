#!/bin/bash
# 生成回答的評測：128 題（有答案）和 15 題無答案題，用 d256 索引。
# 用 /rag/bin/app-linux/rag（build.sh linux 的產物）。Log：/rag/results/67_chain.log
RAG=/rag/bin/app-linux/rag
log=/rag/results/67_chain.log
: > "$log"
echo "$(date +%T) answerable" >> "$log"
"$RAG" ask-eval /rag/testdata/realdocs/queries.jsonl --root /rag --data /rag/work/index-d256 \
  --out /rag/results/67_ask_eval.jsonl > /rag/results/67_ask_eval.txt 2>&1
echo "$(date +%T) unanswerable exit=$?" >> "$log"
"$RAG" ask-eval /rag/testdata/realdocs/unanswerable.jsonl --root /rag --data /rag/work/index-d256 \
  --out /rag/results/67_ask_unanswerable.jsonl > /rag/results/67_ask_unanswerable.txt 2>&1
echo "$(date +%T) done exit=$?" >> "$log"
