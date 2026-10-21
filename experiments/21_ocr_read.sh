#!/usr/bin/env bash
# 用專門的文件辨識模型抄錄測試頁，和原文逐字比對。
# 用法：TRANSCRIBE_PROMPT='OCR:' bash 21_ocr_read.sh <bin 名稱> <model.gguf> <mmproj.gguf> <結果標籤> [圖片 pattern] [額外 llama-server 參數]
BIN=$1; MODEL=$2; MMPROJ=$3; TAG=$4; PATTERN=${5:-page_??_scan.jpg}
shift 5 2>/dev/null || shift $#
PORT=8106
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/ocr-$TAG.log -m "$MODEL" --mmproj "$MMPROJ" -t "${T:-6}" -c 8192 \
    -lm none -ngl 0 --no-webui --parallel 1 --cache-ram 0 "$@" || exit 1
echo "== $TAG: bin=$BIN model=$(basename "$MODEL") prompt='${TRANSCRIBE_PROMPT:-}' pattern=$PATTERN extra=$*"
echo -n "idle : "; bash /rag/experiments/server.sh mem $PORT
cd /tmp && SKIP_IMAGE_QA=1 SKIP_TEXT_QA=1 python3 -I /rag/experiments/e2b_read_pages.py $PORT "/rag/results/21_ocr_$TAG.json" "$PATTERN"
echo -n "after: "; bash /rag/experiments/server.sh mem $PORT
bash /rag/experiments/server.sh stop $PORT
sleep 1
python3 -I /rag/experiments/show_transcripts.py "/rag/results/21_ocr_$TAG.json"
