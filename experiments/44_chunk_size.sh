#!/usr/bin/env bash
# 實驗 44：文字 chunk 的大小對檢索的影響（密集頁面評測集的原文，不經過辨識）。
# 切法和 eval_ocr_chunks.py 相同：不看段落，逐行裝進 chunk，超過字數上限就換下一個。
# 用法：bash 44_chunk_size.sh [bin 名稱]    結果寫到 results/44_chunk_size.txt
BIN=${1:-linux-p4}
R=/rag/results
bash /rag/experiments/server.sh start "$BIN" 8109 /tmp/chunk-size.log -m /rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf --embedding -t "${T:-6}" -c 8192 -b 2048 -ub 2048 \
    -lm none -lzm on --cache-ram 0 -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
for N in n3 n6; do
    echo "== $N: original page text, blind chunking by size"
    python3 -I -c "
import json
d = json.load(open('/rag/testdata/eval60_dense_$N/texts.json', encoding='utf-8'))
json.dump([{'text': p['text']} for p in d['docs']], open('/tmp/pages_$N.json', 'w', encoding='utf-8'), ensure_ascii=False)
"
    (cd /tmp && python3 -I /rag/experiments/eval_ocr_chunks.py 8109 /rag/testdata/eval60_dense_$N $R/eval60_dense_$N.npz /tmp/pages_$N.json 100 200 300 400 600 1000)
done 2>&1 | sed 's/^OCR text, //' | tee $R/44_chunk_size.txt
bash /rag/experiments/server.sh stop 8109
