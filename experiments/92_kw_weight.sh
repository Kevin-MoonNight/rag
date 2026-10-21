#!/bin/bash
# 實驗 92：關鍵字查詢端的詞權重（英數詞 RAG_KW_WORD_W、中文單字 RAG_KW_UNI_W）。
# 醫療 93 題（work/index-med）＋銀行 128 題（work/index-win-1bit）。可續跑：已有結果檔的組合跳過。
cd /rag
mkdir -p results/92
for cfg in 1:1 2:1 3:1 4:1 1:0.5 2:0.5 3:0.5 1:0.3 3:0.3; do
  w=${cfg%:*}; u=${cfg#*:}
  for set in med real; do
    out=results/92/${set}_w${w}_u${u}
    [ -s "$out.txt" ] && grep -q '^exit=0' "$out.txt" && continue
    if [ $set = med ]; then q=testdata/meddocs; idx=work/index-med; else q=testdata/realdocs; idx=work/index-win-1bit; fi
    RAG_KW_WORD_W=$w RAG_KW_UNI_W=$u ./bin/app-linux/rag eval $q/queries.jsonl --root /rag --data $idx \
      --pages $q/page_labels.jsonl --threads 8 --out "$out.jsonl" > "$out.txt" 2>&1
    echo "exit=$?" >> "$out.txt"
    echo "$set w=$w u=$u: $(grep -A4 '全部' "$out.txt" | grep '合併')"
  done
done
