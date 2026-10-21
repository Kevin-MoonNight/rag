#!/bin/bash
# 實驗 94：段落記錄結束頁（page_end）後，重建醫療索引並重跑檢索評估（跨頁的段落，標註的頁在範圍內就算對）。
cd /rag
out=results/94_med
if [ ! -f work/index-med2/.done ]; then
  rm -rf work/index-med2
  ./bin/app-linux/rag index testdata/meddocs --root /rag --data work/index-med2 --quiet > $out.index.txt 2>&1 || exit 1
  touch work/index-med2/.done
fi
./bin/app-linux/rag eval testdata/meddocs/queries.jsonl --root /rag --data work/index-med2 \
  --pages testdata/meddocs/page_labels.jsonl --threads 8 --out $out.eval.jsonl > $out.eval.txt 2>&1
echo "exit=$?" >> $out.eval.txt
