#!/usr/bin/env bash
# 實驗 29：用 valgrind massif 看「只載入詞彙表」時堆積區的記憶體是誰配置的。
# 用法：bash 29_vocab_massif.sh <bin 名稱> <model.gguf>
BIN=$1; MODEL=$2
command -v valgrind > /dev/null || apt-get install -y -qq valgrind > /dev/null 2>&1
cd /tmp
rm -f /tmp/massif.out
valgrind --tool=massif --massif-out-file=/tmp/massif.out --depth=12 --threshold=1.0 \
    /rag/bin/$BIN/llama-tokenize -m "$MODEL" -p "測試 test" --log-disable > /dev/null 2> /tmp/massif.err
# 取最後一個詳細快照（載入完成後的狀態）
ms_print /tmp/massif.out > /tmp/massif.txt
python3 -I /rag/experiments/massif_summary.py /tmp/massif.out
