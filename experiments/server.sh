#!/usr/bin/env bash
# 在容器內啟動 / 停止 llama-server。
# 用法：bash server.sh start <bin 名稱> <port> <log 檔> [llama-server 參數...]
#       bash server.sh stop <port>
set -u
cmd=$1
case "$cmd" in
start)
    BIN=/rag/bin/$2; PORT=$3; LOG=$4; shift 4
    # llama-server 的圖片位置標記預設是每次啟動隨機產生，這裡固定下來
    export LLAMA_MEDIA_MARKER='<__media__>'
    nohup "$BIN/llama-server" --host 127.0.0.1 --port "$PORT" "$@" > "$LOG" 2>&1 &
    echo $! > /tmp/llama-server-$PORT.pid
    for i in $(seq 1 240); do
        if curl -sf "http://127.0.0.1:$PORT/health" > /dev/null 2>&1; then echo "server ready after ${i}x0.5s (pid $(cat /tmp/llama-server-$PORT.pid))"; exit 0; fi
        if ! kill -0 "$(cat /tmp/llama-server-$PORT.pid)" 2>/dev/null; then echo "server exited"; tail -30 "$LOG"; exit 1; fi
        sleep 0.5
    done
    echo "timeout"; tail -30 "$LOG"; exit 1
    ;;
stop)
    PORT=$2
    if [ -f /tmp/llama-server-$PORT.pid ]; then kill "$(cat /tmp/llama-server-$PORT.pid)" 2>/dev/null; rm -f /tmp/llama-server-$PORT.pid; fi
    ;;
mem)
    # 印出 server 行程的記憶體：常駐總量、匿名（私有）、檔案對應
    PORT=$2
    grep -E 'VmRSS|RssAnon|RssFile|RssShmem|VmHWM' /proc/"$(cat /tmp/llama-server-$PORT.pid)"/status | tr '\n' ' '; echo
    ;;
esac
