#!/bin/bash
# 實驗 99：每個 OCR 系統的掃描頁文字各建一份索引（GPU 建索引，只為省時間），再量 128 題的頁命中和 12 題掃描題的回答。
# 可以中斷後重跑（做完的步驟有 .done 記號）。PaddleOCR-VL 基準也用同一套流程重建，和其他系統比較才公平。
# 在 CUDA 容器裡執行：docker exec rag-ocr-box bash /rag/experiments/99_chain.sh [系統…]
# Log：/rag/results/99/chain.log
cd /rag
R=/rag/results/99
D=/rag/work/99_done
log=$R/chain.log
mkdir -p "$D"
declare -A SRC=(
    [paddlevl]="results/win/87_ocr/{n:03d}.txt"
    [nemotron]="results/99/nemotron-cuda/{n:03d}.md"
    [ppv5s]="results/99/ppocrv5-server/{n:03d}.txt"
    [ppv6m]="results/99/ppocrv6-medium/{n:03d}.txt"
    [ppv6s]="results/99/ppocrv6-small/{n:03d}.txt"
    [ppv6m_t]="results/99/ppocrv6-medium-t/{n:03d}.txt"
    [ppv6s_t]="results/99/ppocrv6-small-t/{n:03d}.txt"
    [ppv6m_vt]="results/99/ppocrv6-medium-vt/{n:03d}.txt"
    [ppv6s_vt]="results/99/ppocrv6-small-vt/{n:03d}.txt"
)
step() { # step <名稱> <指令…>
    local name=$1; shift
    [ -f "$D/$name" ] && { echo "$(date +%T) skip $name" >> "$log"; return 0; }
    echo "$(date +%T) start $name" >> "$log"
    if "$@"; then touch "$D/$name"; echo "$(date +%T) done $name" >> "$log"; else echo "$(date +%T) FAILED $name" >> "$log"; exit 1; fi
}
md() { rm -rf "$OV"; (cd /tmp && python3 -I /rag/experiments/99_ocr_md.py "$OV" "${SRC[$s]}") > $R/md_$s.txt 2>&1; }
index() { rm -rf "$IX"; ./bin/app-linux/rag index testdata/realdocs/pdf --root /rag --data "$IX" --server bin/linux-p5-cuda/llama-server-gpu \
            --quiet --text-override "$OV" > $R/index_$s.txt 2>&1; }
evalp() { nice -n 19 ./bin/app-linux/rag eval testdata/realdocs/queries.jsonl --root /rag --data "$IX" --threads 6 --out $R/eval_$s.jsonl \
            --pages testdata/realdocs/page_labels.jsonl --lists --text-override "$OV" > $R/eval_$s.txt 2>&1; }
ask() { ./bin/app-linux/rag ask-eval work/87_scan_queries.jsonl --root /rag --data "$IX" --text-override "$OV" \
          --out $R/ask_$s.jsonl > $R/ask_$s.txt 2>&1; }
for s in ${@:-paddlevl nemotron ppv5s ppv6m ppv6s ppv6m_t ppv6s_t ppv6m_vt ppv6s_vt}; do
    OV=/rag/work/99_md/$s
    IX=/rag/work/index-99-$s
    step md_$s md
    step index_$s index
    step eval_$s evalp
    step ask_$s ask
done
echo "$(date +%T) all done" >> "$log"
