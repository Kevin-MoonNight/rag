#!/usr/bin/env bash
# 實驗 26：取樣分析一頁圖片轉向量的時間花在哪些函式。
# 用法：bash 26_profile_image.sh <bin 名稱> <model.gguf> <mmproj.gguf> [threads]
# 先試 perf（cpu-clock 取樣），容器不允許時改用 gdb 定時抓堆疊。
BIN=$1; MODEL=$2; MMPROJ=$3; T=${4:-6}
PORT=8108
command -v perf > /dev/null || { apt-get install -y -qq linux-perf > /dev/null 2>&1 || apt-get install -y -qq linux-tools-generic > /dev/null 2>&1; }
command -v gdb > /dev/null || apt-get install -y -qq gdb > /dev/null 2>&1
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/prof.log -m "$MODEL" --mmproj "$MMPROJ" --embedding -t "$T" -c 8192 -b 1024 -ub 1024 \
    --image-min-tokens 280 --image-max-tokens 280 -lm none -ngl 0 --no-webui --parallel 1 > /dev/null || exit 1
PID=$(pgrep -n llama-server)
cd /tmp
python3 -I /rag/experiments/time_image.py $PORT 1 > /dev/null   # 暖機
python3 -I /rag/experiments/time_image.py $PORT 5 &
CLIENT=$!
sleep 1
PERF=$(command -v perf || ls /usr/lib/linux-tools/*/perf 2> /dev/null | head -1)
if [ -n "$PERF" ] && $PERF record -e cpu-clock -F 499 -g -p "$PID" -o /tmp/perf.data -- sleep 15 > /tmp/perf.log 2>&1; then
    echo "== perf, self time by function"
    $PERF report -i /tmp/perf.data --no-children --sort symbol --stdio 2> /dev/null | grep -E '^ +[0-9]' | head -40
else
    echo "== perf not usable ($(tail -1 /tmp/perf.log 2> /dev/null | cut -c1-100)); sampling with gdb"
    : > /tmp/stacks.txt
    for i in $(seq 1 60); do
        gdb -p "$PID" -batch -ex "thread apply all bt 8" 2> /dev/null >> /tmp/stacks.txt
        sleep 0.15
    done
    python3 -I /rag/experiments/stack_summary.py /tmp/stacks.txt
fi
wait $CLIENT
bash /rag/experiments/server.sh stop $PORT
