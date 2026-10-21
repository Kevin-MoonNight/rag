#!/bin/bash
# 實驗 97：簡體中文的文件和問題。先跑 experiments/97_simplified.py 產生 work/97/{trad,simp}（同一批頁面文字，
# 只差繁簡字形）。兩份各建一個索引（--text-override），再用繁體題目和簡體題目各評估一次，共 4 組。
# 每一步做完留下記號，中斷後再跑會接著做。
cd /rag
mkdir -p results/97
for kind in trad simp; do
  data=work/index-97-$kind
  if [ ! -f $data/.done ]; then
    rm -rf $data
    ./bin/app-linux/rag index testdata/meddocs --root /rag --data $data --text-override work/97/$kind --quiet \
      > results/97/index-$kind.txt 2>&1 || exit 1
    touch $data/.done
  fi
done
for kind in trad simp; do
  for q in trad simp; do
    out=results/97/docs-$kind.q-$q
    [ -f $out.eval.txt ] && grep -q '^exit=0' $out.eval.txt && continue
    queries=testdata/meddocs/queries.jsonl
    [ $q = simp ] && queries=work/97/queries_simp.jsonl
    ./bin/app-linux/rag eval $queries --root /rag --data work/index-97-$kind \
      --pages testdata/meddocs/page_labels.jsonl --threads 8 --out $out.eval.jsonl > $out.eval.txt 2>&1
    echo "exit=$?" >> $out.eval.txt
  done
done
