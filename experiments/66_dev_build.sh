#!/bin/bash
# Build the Linux CLI without copying it to /rag/bin/app-linux (a running index job may be using that copy).
# The binary stays at /build/target/release/rag.  Log: /rag/work/build-dev.log
cd /rag/app
export XDG_CACHE_HOME=/rag/work/cache
cargo build --release -p rag_cli > /rag/work/build-dev.log 2>&1
code=$?
echo "exit=$code" >> /rag/work/build-dev.log
grep -E "^(warning|error)" /rag/work/build-dev.log | sort | uniq -c | head -5
tail -2 /rag/work/build-dev.log
