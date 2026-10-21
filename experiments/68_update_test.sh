#!/bin/bash
# 檔案來源與更新的端到端測試：新增、改名搬移、刪除、隱藏檔和暫存檔、壞檔不重試、外接磁碟離線、取消來源。
# 用開發版執行檔（66_dev_build.sh 編出的 /build/target/release/rag），資料都放在 /rag/work/upd。
# Log：/rag/results/68_update_test.log
set -u
RAG=/build/target/release/rag
W=/rag/work/upd
P=/rag/testdata/realdocs/pdf
out=/rag/results/68_update_test.log
: > "$out"
rm -rf "$W"; mkdir -p "$W/src/sub" "$W/ext"

step() { echo "=== $1" | tee -a "$out"; }
run() {
  "$RAG" "$@" --root /rag --data "$W/data" --quiet > "$W/last.txt" 2>&1
  local code=$?
  cat "$W/last.txt" >> "$out"
  # 只印出摘要裡有意義的數字
  grep -E '"(files|indexed|duplicates|unchanged|failed|still_failed|removed|moved|offline_sources|skipped_hidden|unsupported|reembedded|estimate_files)"' "$W/last.txt" | tr -d ' \n'; echo " (exit=$code)"
}

cp "$P/FL005713/信用合作社授信審議委員會組織準則 條文對照表（107.11.08）.pdf" "$W/src/a.pdf"
cp "$P/FL005713/信用合作社授信審議委員會組織準則 總說明（107.11.08）.pdf" "$W/src/b.pdf"
cp "$P/FE220738/台財融（五）字第0925000075號令.pdf" "$W/src/sub/c.pdf"
cp "$W/src/a.pdf" "$W/src/~\$a.pdf"
cp "$W/src/a.pdf" "$W/src/.hidden.pdf"
echo "not a docx" > "$W/src/note.docx"
cp "$P/FL006416/國際金融業務條例施行細則 總說明（114.12.12）.pdf" "$W/ext/d.pdf"

step "1 首次建立：預期 indexed=3、skipped_hidden=2、unsupported=1"
run index "$W/src"

step "2 沒有變動：預期 unchanged=3"
run index

step "3 a.pdf 改名搬到 sub/a2.pdf、刪掉 b.pdf：預期 moved=1、removed=1、unchanged=2"
mv "$W/src/a.pdf" "$W/src/sub/a2.pdf"
rm "$W/src/b.pdf"
run index
"$RAG" search "授信審議委員會開會出席" --root /rag --data "$W/data" --show 3 2>&1 | grep -E "a2.pdf|a.pdf" | head -3 | tee -a "$out"

step "4 壞掉的 PDF：預期 failed=1"
echo "broken" > "$W/src/bad.pdf"
run index

step "5 再更新一次，壞檔沒變：預期 still_failed=1、failed=0"
run index

step "6 修改壞檔（內容變了）：預期會重試 failed=1"
echo "broken again" > "$W/src/bad.pdf"
run index

step "7 加入外接磁碟資料夾 ext：預期 indexed=1"
run index "$W/ext"

step "8 外接磁碟拔掉（ext 改名）：預期 offline_sources=1、removed=0"
mv "$W/ext" "$W/ext-unplugged"
run index
"$RAG" stats --root /rag --data "$W/data" 2>&1 | head -5 | tee -a "$out"

step "9 接回來：預期 offline_sources=0、unchanged 含 d.pdf"
mv "$W/ext-unplugged" "$W/ext"
run index

step "10 取消來源 ext：預期拿掉 1 個檔案，頁面影像跟著刪掉"
ls "$W/data/pages" | tee -a "$out"
"$RAG" remove-source "$W/ext" --root /rag --data "$W/data" 2>&1 | tee -a "$out"
echo "剩下的頁面影像：$(ls "$W/data/pages" | wc -l)" | tee -a "$out"
run index

step "11 換了切分設定、沒加 --rebuild：預期拒絕"
run index --max-tokens 128

step "12 換了切分設定、加 --rebuild：預期 2 個檔案整份重做（indexed=2、reembedded=0）"
run index --max-tokens 128 --rebuild
grep -E '"reembedded"' "$W/last.txt" | tee -a "$out"

step "13 換了向量模型（同一個模型換檔名）：預期只重算向量（reembedded=2）"
M=$(ls /rag/models/embeddinggemma-2-BF16.gguf /rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf 2>/dev/null | head -1)
ln -sf "$M" "$W/renamed-model.gguf" 2>/dev/null || cp "$M" "$W/renamed-model.gguf"
run index --max-tokens 128 --rebuild --model "$W/renamed-model.gguf"
grep -E '"reembedded"' "$W/last.txt" | tee -a "$out"
"$RAG" search "授信審議委員會開會出席" --root /rag --data "$W/data" --max-tokens 128 --model "$W/renamed-model.gguf" --show 2 2>&1 | grep -E "^#" | head -2 | tee -a "$out"

step "14 再更新一次：預期全部 unchanged"
run index --max-tokens 128 --model "$W/renamed-model.gguf"

step "15 刪掉有頁面影像的掃描檔 c.pdf：預期 removed=1，頁面影像剩 0 張"
rm "$W/src/sub/c.pdf"
run index --max-tokens 128 --model "$W/renamed-model.gguf"
echo "剩下的頁面影像：$(ls "$W/data/pages" | wc -l)" | tee -a "$out"

echo done | tee -a "$out"
