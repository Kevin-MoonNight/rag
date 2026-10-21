#!/bin/bash
# Convert a handful of real PDFs and show the intermediate format (LiteParse on Chinese PDFs).
# Usage: bash 53_convert_samples.sh [extra rag convert args]
RAG=/rag/bin/app-linux/rag
D=/rag/testdata/realdocs/pdf
n=0
find "$D" -iname "*.pdf" | sort | awk 'NR % 40 == 7' | while read -r f; do
  echo "=================================================================="
  echo "FILE: $f"
  "$RAG" convert "$f" --image-dir /rag/work/convert-pages "$@" 2>&1 | head -${LINES_PER_FILE:-45} | cut -c1-${COLS:-400}
done
