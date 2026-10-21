#!/bin/bash
# 實驗 99 的產品驗證：rag index --ocr（程式內建的 PP-OCRv6 small＋直式重排＋轉繁）建的索引，
# 檢索結果應該和 99_chain.sh 的 ppv6s_vt（外部辨識結果、--text-override）相同。
# 在 CUDA 容器裡執行（向量用 GPU，只為省時間；辨識用 CPU）：docker exec rag-ocr-box bash /rag/experiments/99_product_ocr.sh
cd /rag
IX=work/index-99-product
R=results/99
rm -rf $IX
t0=$(date +%s)
./bin/app-linux/rag index testdata/realdocs/pdf --root /rag --data $IX --server bin/linux-p5-cuda/llama-server-gpu --ocr > $R/product_index.txt 2>&1
echo "exit=$? seconds=$(( $(date +%s) - t0 ))" >> $R/product_index.txt
./bin/app-linux/rag eval testdata/realdocs/queries.jsonl --root /rag --data $IX --threads 6 --out $R/product_eval.jsonl \
    --pages testdata/realdocs/page_labels.jsonl --lists --ocr > $R/product_eval.txt 2>&1
./bin/app-linux/rag eval work/99_scan_queries.jsonl --root /rag --data $IX --threads 6 --out $R/product_eval2.jsonl --ocr > $R/product_eval2.txt 2>&1
for f in eval eval2; do
    a=$R/product_$f.txt; b=$R/${f}_ppv6s_vt.txt
    diff <(grep -Ev "搜尋時間|載入模型" $a) <(grep -Ev "搜尋時間|載入模型" $b) > /dev/null && echo "$f: same as ppv6s_vt" || echo "$f: DIFFERENT"
done
tail -3 $R/product_index.txt
