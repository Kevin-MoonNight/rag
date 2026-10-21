#!/bin/bash
# Index the real PDF attachments with the Rust program (Linux build, inside the container).
# Usage: bash 54_index_real.sh <tag> [extra rag index args, e.g. --max-tokens 128]
# Index goes to /rag/work/index-<tag>, log to /rag/results/54_index_<tag>.txt
tag=$1; shift
RAG=/rag/bin/app-linux/rag
out=/rag/results/54_index_$tag.txt
echo "args: $*" > "$out"
"$RAG" index /rag/testdata/realdocs/pdf --root /rag --data "/rag/work/index-$tag" --quiet "$@" >> "$out" 2>&1
echo "exit=$?" >> "$out"
"$RAG" stats --root /rag --data "/rag/work/index-$tag" "$@" >> "$out" 2>&1
du -sh "/rag/work/index-$tag" "/rag/work/index-$tag"/* >> "$out" 2>&1
tail -40 "$out"
