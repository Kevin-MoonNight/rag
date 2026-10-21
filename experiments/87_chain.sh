#!/bin/bash
# 實驗 87：掃描頁加上 OCR 文字後重建索引，量檢索和掃描題的回答。可以中斷後重跑（做完的步驟有 .done 記號）。
# 用法：bash 87_chain.sh    Log：/rag/results/87_chain.log
R=/rag/results
D=/rag/work/87_done
OV=/rag/work/87_ocr_md
log=$R/87_chain.log
mkdir -p "$D"
step() { # step <名稱> <指令…>
    local name=$1; shift
    [ -f "$D/$name" ] && { echo "$(date +%T) skip $name" >> "$log"; return 0; }
    echo "$(date +%T) start $name" >> "$log"
    if "$@"; then touch "$D/$name"; echo "$(date +%T) done $name" >> "$log"; else echo "$(date +%T) FAILED $name" >> "$log"; exit 1; fi
}
build() { bash /rag/experiments/build.sh test > $R/87_build_test.log 2>&1 && bash /rag/experiments/build.sh linux > $R/87_build_linux.log 2>&1; }
md() { rm -rf "$OV"; cd /tmp && python3 -I /rag/experiments/87_ocr_md.py "$OV" > $R/87_ocr_md.txt 2>&1; }
index() { rm -rf /rag/work/index-ocr87; EVAL_THREADS=6 bash /rag/experiments/54_index_real.sh ocr87 --text-override "$OV" > /dev/null; grep -q '^exit=0' $R/54_index_ocr87.txt; }
evalp() { EVAL_THREADS=6 bash /rag/experiments/59_eval_one.sh ocr87 _pages87 --pages /rag/testdata/realdocs/page_labels.jsonl --lists --text-override "$OV" > /dev/null; grep -q '^exit=0' $R/59_eval_ocr87_pages87.txt; }
ask() { /rag/bin/app-linux/rag ask-eval /rag/work/87_scan_queries.jsonl --root /rag --data /rag/work/index-ocr87 --text-override "$OV" \
          --out $R/87_ask_scan.jsonl > $R/87_ask_scan.txt 2>&1; }
step build build
step md md
step index index
step eval evalp
step ask ask
echo "$(date +%T) all done" >> "$log"
