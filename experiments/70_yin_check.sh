#!/bin/bash
# 實驗 70：公允價值那題的回答把「參數、資料、波動率、參與者」寫成「引」。
# 同一題（temperature 0）換三個 E2B 模型檔比較，找出是哪一步造成的。Log：/rag/results/70_yin.txt
RAG=/rag/bin/app-linux/rag
out=/rag/results/70_yin${YIN_SUFFIX}.txt
: > "$out"
Q="金融商品的公允價值分成幾個等級，怎麼判斷屬於哪一級"
for m in ${YIN_MODELS:-derived/e2b-q4_0-exact-head-zhtw-en.gguf derived/e2b-q4_0-exact.gguf derived/e2b-q4_0-head-zhtw-en.gguf gemma-4-e2b/gemma-4-E2B_q4_0-it.gguf}; do
  echo "===== $m" >> "$out"
  "$RAG" ask "$Q" --root /rag --data /rag/work/index-d256 --gen-model "/rag/models/$m" >> "$out" 2>&1
done
echo "===== done" >> "$out"
