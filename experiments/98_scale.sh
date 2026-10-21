#!/bin/bash
# 實驗 98：真實文件的規模。同一個索引先放銀行 400 份＋醫療 86 份，評估兩組題目（基準）；
# 再加入行政院公報的干擾文件（work/scale/pdf，experiments 由 work/scale/crawl.sh 抓），重新評估，
# 看原本的答案會不會被擠下去，並記錄建索引時間、索引大小、搜尋時間。每一步做完留下記號，中斷後再跑會接著做。
# 用法：docker exec rag-rust-box bash /rag/experiments/98_scale.sh [base|scale]
cd /rag
RAG=./bin/app-linux/rag
data=work/index-98
out=results/98
mkdir -p $out
step() { # 名稱 指令…：做過就跳過，記下耗時
  local name=$1; shift
  [ -f $data/.done-$name ] && return 0
  local t0=$(date +%s)
  "$@" > $out/$name.txt 2>&1 || { echo "exit=$?" >> $out/$name.txt; return 1; }
  echo "exit=0 seconds=$(( $(date +%s) - t0 ))" >> $out/$name.txt
  mkdir -p $data; touch $data/.done-$name
}
evaluate() { # 標籤
  step eval-bank-$1 $RAG eval testdata/realdocs/queries.jsonl --root /rag --data $data --pages testdata/realdocs/page_labels.jsonl \
    --threads 8 --out $out/eval-bank-$1.jsonl || exit 1
  step eval-med-$1 $RAG eval testdata/meddocs/queries.jsonl --root /rag --data $data --pages testdata/meddocs/page_labels.jsonl \
    --threads 8 --out $out/eval-med-$1.jsonl || exit 1
  { $RAG stats --root /rag --data $data; du -sh $data $data/*; } > $out/stats-$1.txt 2>&1
}
step index-bank $RAG index testdata/realdocs/pdf --root /rag --data $data --quiet || exit 1
step index-med $RAG index testdata/meddocs --root /rag --data $data --quiet || exit 1
evaluate base
[ "$1" = base ] && exit 0
step index-scale $RAG index work/scale/pdf --root /rag --data $data --quiet || exit 1
evaluate scale
