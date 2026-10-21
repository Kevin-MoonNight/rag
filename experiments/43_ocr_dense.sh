#!/usr/bin/env bash
# 實驗 43（對照，產品的決定仍然是不做 OCR）：密集掃描頁用 PaddleOCR-VL 轉成文字、切 chunk 後的檢索準確度與每頁時間。
# 用法：bash 43_ocr_dense.sh [bin 名稱]
# 結果寫到 results/43_ocr_dense.txt；辨識文字在 results/43_ocr_n3.json、43_ocr_n6.json
BIN=${1:-linux-p4}
O=/rag/models/ocr/paddleocr-vl-1.6
R=/rag/results
rm -f $R/43_ocr_dense.txt
bash /rag/experiments/server.sh start "$BIN" 8106 /tmp/ocr-dense.log -m $O/PaddleOCR-VL-1.6-GGUF.gguf --mmproj $O/PaddleOCR-VL-1.6-GGUF-mmproj.gguf \
    -t "${T:-6}" -c 8192 -lm none -ngl 0 --no-webui --parallel 1 --cache-ram 0 > /dev/null || exit 1
for N in n3 n6; do
    echo "== $N: PaddleOCR-VL 1.6 on the scan images"
    (cd /tmp && python3 -I /rag/experiments/ocr_dense_pages.py 8106 /rag/testdata/eval60_dense_$N $R/43_ocr_$N.json)
done 2>&1 | tee -a $R/43_ocr_dense.txt
bash /rag/experiments/server.sh stop 8106
bash /rag/experiments/server.sh start "$BIN" 8109 /tmp/ocr-embed.log -m /rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
for N in n3 n6; do
    echo "== $N: retrieval over the OCR text"
    (cd /tmp && python3 -I /rag/experiments/eval_ocr_chunks.py 8109 /rag/testdata/eval60_dense_$N $R/eval60_dense_$N.npz $R/43_ocr_$N.json 200 400)
done 2>&1 | tee -a $R/43_ocr_dense.txt
bash /rag/experiments/server.sh stop 8109
