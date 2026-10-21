#!/usr/bin/env bash
# 用 strace 找出 llama-server 處理文件時配置大塊記憶體的呼叫堆疊。
# 用法：bash trace_mmap.sh <bin 名稱> <model.gguf> [最小位元組數，預設 100 MB]
BIN=$1; MODEL=$2; MIN=${3:-100000000}
PORT=8105
command -v strace > /dev/null || { apt-get update -qq > /dev/null && apt-get install -y -qq strace > /dev/null; }
export LLAMA_MEDIA_MARKER='<__media__>'
strace -f -k -e trace=mmap,mremap,brk -o /tmp/strace.out /rag/bin/$BIN/llama-server --host 127.0.0.1 --port $PORT -m "$MODEL" --embedding -t 6 -c 8192 -b 512 -ub 512 \
    -lm none -ngl 0 --no-webui --parallel 1 -lzm on --cache-ram 0 > /tmp/strace-server.log 2>&1 &
SPID=$!
for i in $(seq 1 240); do curl -sf "http://127.0.0.1:$PORT/health" > /dev/null 2>&1 && break; sleep 0.5; done
echo "---- marker: server ready" >> /tmp/strace.out
cd /tmp && python3 -I /rag/experiments/mem_probe_client.py $PORT query
echo "---- marker: after query" >> /tmp/strace.out
python3 -I /rag/experiments/mem_probe_client.py $PORT docs
pkill -f "llama-server --host 127.0.0.1 --port $PORT"; wait $SPID 2>/dev/null
# 印出「server ready」之後、長度超過門檻的 mmap 以及它的呼叫堆疊
awk -v min="$MIN" '
    /marker: server ready/ {on=1}
    on && /mmap\(NULL, [0-9]+/ { match($0, /mmap\(NULL, [0-9]+/); len=substr($0, RSTART+11, RLENGTH-11)+0; show=(len>=min); if (show) print }
    on && show && /^ > / { print }
    on && !/^ > / && !/mmap\(NULL/ { show=0 }
' /tmp/strace.out | cut -c1-200 | head -${HEAD:-60}
