#!/usr/bin/env bash
# 實驗 33：無損 Q4_0 版 E2B 的後續檢查。
#   1. 重量「無損版加精簡輸出層」的輸出層耗時（實驗 32 那次被別的工作干擾）。
#   2. rag-check（修正過 -inf 的比較）確認提示階段跳層在無損版與精簡輸出層上都正確。
#   3. 用無損版重跑「E2B 讀掃描頁」：中文抄錄錯字數有沒有因為模型更接近原始訓練結果而改善。
# 結果寫到 results/33_exact_followup.txt
D=/rag/models/derived
cd /tmp
{
echo "== headcost: e2b-q4_0-exact-head-zhtw-en.gguf"
for t in 4 6; do /rag/bin/linux-p4/rag-headcost $D/e2b-q4_0-exact-head-zhtw-en.gguf $t 64 2>&1 | grep -E 'threads|per token|output head'; done
for m in e2b-q4_0-exact.gguf e2b-q4_0-exact-head-zhtw-en.gguf; do
    echo "== rag-check: $m"
    /rag/bin/linux-p4/rag-check $D/$m 6 8 2>&1 | grep -E '^ +[0-9]+ +[0-9]+ \||max NMSE|RESULT'
done
for tok in 280 560; do
    MODEL=$D/e2b-q4_0-exact.gguf SKIP_TEXT_QA=1 bash /rag/experiments/12_e2b_read.sh linux-p4 $tok 'page_??_scan.jpg' exact_scan_$tok 6 2>&1 | grep -vE '^Vm|server ready'
done
python3 -I /rag/experiments/show_transcripts.py /rag/results/12_e2b_read_exact_scan_280.json /rag/results/12_e2b_read_exact_scan_560.json | grep -E '^==|合計|原文'
} 2>&1 | tee /rag/results/33_exact_followup.txt
