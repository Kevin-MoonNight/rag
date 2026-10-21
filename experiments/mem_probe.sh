#!/usr/bin/env bash
# 分階段量 EmbeddingGemma 2 伺服器的記憶體，並列出最大的幾塊匿名記憶體。
# 用法：bash mem_probe.sh <bin 名稱> <model.gguf> <mmproj.gguf 或 none> [額外 llama-server 參數]
BIN=$1; MODEL=$2; MMPROJ=$3; shift 3
PORT=8104
MM=(); [ "$MMPROJ" != none ] && MM=(--mmproj "$MMPROJ" --image-min-tokens 280 --image-max-tokens 280)
bash /rag/experiments/server.sh start "$BIN" $PORT /tmp/probe.log -m "$MODEL" "${MM[@]}" --embedding -t "${T:-6}" -c "${CTX:-8192}" -b "${NB:-1024}" -ub "${NB:-1024}" \
    -lm none -ngl 0 --no-webui --parallel 1 -lzm on --cache-ram 0 "$@" > /dev/null || exit 1
PID=$(cat /tmp/llama-server-$PORT.pid)
anon() { awk '/RssAnon/ {printf "%d MB", $2/1024}' /proc/$PID/status; }
top_maps() {
    # 每個匿名對應區：大小與實際常駐量（MB），列出常駐量最大的幾塊
    awk '/^[0-9a-f]+-[0-9a-f]+ / {name=$6; if (name=="") name="[anon]"} /^Size:/ {size=$2} /^Rss:/ {if (name=="[anon]" || name=="[heap]") printf "%8.1f MB resident of %8.1f MB  %s\n", $2/1024, size/1024, name}' /proc/$PID/smaps | sort -rn | head -${TOP:-8}
}
echo "== bin=$BIN model=$(basename "$MODEL") mmproj=$(basename "$MMPROJ") extra=$*"
echo "idle                : $(anon)"
cd /tmp
python3 -I /rag/experiments/mem_probe_client.py $PORT query;  echo "after 1 short query : $(anon)"
python3 -I /rag/experiments/mem_probe_client.py $PORT docs;   echo "after 6 documents   : $(anon)"
if [ "$MMPROJ" != none ]; then
    python3 -I /rag/experiments/mem_probe_client.py $PORT image;  echo "after 1 page image  : $(anon)"
    python3 -I /rag/experiments/mem_probe_client.py $PORT images; echo "after 6 page images : $(anon)"
fi
echo "peak (VmHWM)        : $(awk '/VmHWM/ {printf "%d MB", $2/1024}' /proc/$PID/status)   threads: $(ls /proc/$PID/task | wc -l)"
top_maps
grep -E 'compute buffer size|model buffer size|worst-case' /tmp/probe.log | sed -E 's/^[0-9.]+ I //' | cut -c1-120
bash /rag/experiments/server.sh stop $PORT
sleep 1
