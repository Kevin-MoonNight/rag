#!/usr/bin/env bash
# 實驗 29：只載入詞彙表（不載入權重）時行程用多少記憶體，用來確認閒置時那 121 MB 是不是詞彙表的資料結構。
# 用法：bash 29_vocab_mem.sh <bin 名稱> <model.gguf> [...更多 model.gguf]
BIN=$1; shift
command -v /usr/bin/time > /dev/null || apt-get install -y -qq time > /dev/null 2>&1
for m in "$@"; do
    /usr/bin/time -v /rag/bin/$BIN/llama-tokenize -m "$m" -p "測試 test" --log-disable > /dev/null 2> /tmp/vocab_time.txt
    echo "$(basename "$m"): vocab-only process peak RSS $(grep 'Maximum resident' /tmp/vocab_time.txt | awk '{printf "%.0f MB", $NF/1024}')"
done
