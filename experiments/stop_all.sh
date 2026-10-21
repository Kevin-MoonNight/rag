#!/bin/bash
# Pause everything CPU-heavy inside the container: the evaluation chain, index runs, model servers, builds.
# Index runs can be continued later (files already done are skipped).
pkill -f 59_eval_chain.sh 2>/dev/null
pkill -f 60_eval_variants.sh 2>/dev/null
pkill -f 61_chunk_variants.sh 2>/dev/null
pkill -f 64_final_check.sh 2>/dev/null
pkill -f 65_nofold.sh 2>/dev/null
pkill -f 67_ask_eval.sh 2>/dev/null
pkill -f 68_update_test.sh 2>/dev/null
pkill -f 69_quant.sh 2>/dev/null
pkill -f 70_yin_check.sh 2>/dev/null
pkill -f 71_han_chain.sh 2>/dev/null
pkill -f "target/release/rag " 2>/dev/null
pkill -f 59_eval_one.sh 2>/dev/null
pkill -f 55_rebuild_index.sh 2>/dev/null
pkill -f 54_index_real.sh 2>/dev/null
pkill -f "app-linux/rag " 2>/dev/null
sleep 1
pkill -f "linux-p4/llama-server" 2>/dev/null
pkill -f "experiments/build.sh" 2>/dev/null
pkill -f 58_build_win_all.sh 2>/dev/null
pkill -x cargo 2>/dev/null
pkill -x rustc 2>/dev/null
sleep 1
ps -eo etime,pcpu,args --sort=-pcpu | grep -v -E "ps -eo|sleep infinity|stop_all|defunct" | head -6 | cut -c1-110
echo "stopped"
