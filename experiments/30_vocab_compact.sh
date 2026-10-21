#!/usr/bin/env bash
# 實驗 30：修改七（BPE 合併規則表改用 token 編號當鍵）前後，分詞結果是否完全相同、只載入詞彙表時用多少記憶體。
# 用法：bash 30_vocab_compact.sh <修改前 bin> <修改後 bin> <model.gguf> [...更多 model.gguf]
OLD=$1; NEW=$2; shift 2
cd /tmp
# 測試文字：自己寫的中英文測試文件、llama.cpp 的英文文件，再加上原始碼（符號、縮排、罕見字元多）
cat /rag/testdata/kl_corpus.txt /rag/vendor/llama.cpp-b11567/README.md /rag/vendor/llama.cpp-b11567/src/llama-vocab.cpp \
    /rag/vendor/llama.cpp-b11567/src/unicode.cpp > /tmp/tok_corpus.txt
echo "test text: $(wc -c < /tmp/tok_corpus.txt) bytes"
for m in "$@"; do
    echo "== $(basename "$m")"
    for b in $OLD $NEW; do
        /usr/bin/time -v /rag/bin/$b/llama-tokenize -m "$m" -f /tmp/tok_corpus.txt --ids --log-disable > /tmp/tok_$b.txt 2> /tmp/tok_$b.time
        echo "  $b: $(tr ',' '\n' < /tmp/tok_$b.txt | wc -l) tokens, peak RSS $(grep 'Maximum resident' /tmp/tok_$b.time | awk '{printf "%.0f MB", $NF/1024}'), $(grep 'Elapsed' /tmp/tok_$b.time | awk '{print $NF}') elapsed"
    done
    if cmp -s /tmp/tok_$OLD.txt /tmp/tok_$NEW.txt; then echo "  token ids: identical"; else echo "  token ids: DIFFERENT"; fi
done
/rag/bin/$NEW/llama-tokenize -m "$1" -p "x" 2>&1 | grep -E 'bpe merges|n_merges' | cut -c1-120
