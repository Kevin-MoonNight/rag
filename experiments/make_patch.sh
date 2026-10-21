#!/usr/bin/env bash
# 把 work/llama.cpp 相對於 vendor/llama.cpp-b11567 的所有修改輸出成一個 patch 檔。
# 套用方式：在乾淨的 llama.cpp b11567 原始碼目錄下執行 patch -p1 < llama.cpp-b11567-rag.patch
cd /rag
diff -ruN --exclude='*.orig' --exclude='*.rej' --exclude='__pycache__' vendor/llama.cpp-b11567 work/llama.cpp \
    | sed -e 's#^--- vendor/llama.cpp-b11567/#--- a/#' -e 's#^+++ work/llama.cpp/#+++ b/#' \
          -e 's#^diff -ruN .* vendor/llama.cpp-b11567/\(.*\) work/llama.cpp/.*#diff -ruN a/\1 b/\1#' \
    > patches/llama.cpp-b11567-rag.patch
grep -c '^diff ' patches/llama.cpp-b11567-rag.patch
grep '^diff ' patches/llama.cpp-b11567-rag.patch
wc -l patches/llama.cpp-b11567-rag.patch
