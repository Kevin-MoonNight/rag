#!/bin/bash
# After removing the Traditional/Simplified folding of keywords (2026-10-14, user decision):
# rebuild the Linux binary, rebuild the 400-document index with the default settings and evaluate it.
# Index tag d256.  Progress in /rag/results/65_chain.log
log=/rag/results/65_chain.log
step() { echo "$(date +%H:%M:%S) $*" >> "$log"; }
: > "$log"
step "build linux"
bash /rag/experiments/build.sh linux > /dev/null || { step "build failed"; exit 1; }
step "index d256"
bash /rag/experiments/54_index_real.sh d256 > /dev/null
step "eval d256"
bash /rag/experiments/59_eval_one.sh d256 - > /dev/null
step "done"
