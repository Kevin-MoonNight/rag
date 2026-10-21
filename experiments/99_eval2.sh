#!/bin/bash
# 實驗 99：用 27 題「掃描細節」題（work/99_scan_queries.jsonl，答案頁看原圖決定）評估 99_chain.sh 建好的每份索引。
# 用法：bash 99_eval2.sh [系統…]   結果：/rag/results/99/eval2_<系統>.txt/.jsonl
cd /rag
for s in ${@:-paddlevl nemotron ppv5s ppv6m ppv6s ppv6m_t ppv6s_t ppv6m_vt ppv6s_vt}; do
    [ -d work/index-99-$s ] || continue
    ./bin/app-linux/rag eval work/99_scan_queries.jsonl --root /rag --data work/index-99-$s --threads 6 --out results/99/eval2_$s.jsonl \
        --text-override work/99_md/$s > results/99/eval2_$s.txt 2>&1
    echo "$s $(grep -m1 '^合併' results/99/eval2_$s.txt)"
done
