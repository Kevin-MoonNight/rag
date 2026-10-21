#!/bin/bash
# 實驗 92b：查詢含代號（code）或像關鍵詞（style）時，合併改成關鍵字先。可續跑。
cd /rag
mkdir -p results/92
for mode in code style; do
  for w in 1 3; do
  for set in med real; do
    out=results/92/${set}_first-${mode}_w${w}
    [ -s "$out.txt" ] && grep -q '^exit=0' "$out.txt" && continue
    if [ $set = med ]; then q=testdata/meddocs; idx=work/index-med; else q=testdata/realdocs; idx=work/index-win-1bit; fi
    RAG_KW_FIRST=$mode RAG_KW_WORD_W=$w ./bin/app-linux/rag eval $q/queries.jsonl --root /rag --data $idx \
      --pages $q/page_labels.jsonl --threads 8 --out "$out.jsonl" > "$out.txt" 2>&1
    echo "exit=$?" >> "$out.txt"
    echo "$set $mode w=$w: $(grep -A4 '== 全部' "$out.txt" | grep '合併' | cut -c8-200)"
  done; done
done
