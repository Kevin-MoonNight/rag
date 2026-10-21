#!/usr/bin/env bash
# 從 Hugging Face 下載指定檔案到 /rag/models/<子目錄>/。
# 用法：bash hf_download.sh <repo id> <子目錄> <檔名> [檔名...]
REPO=$1; DIR=/rag/models/$2; shift 2
mkdir -p "$DIR"
for f in "$@"; do
    echo "downloading $REPO/$f"
    curl -sSfL --retry 5 -C - -o "$DIR/$f" "https://huggingface.co/$REPO/resolve/main/$f" || echo "FAILED: $f"
done
ls -l "$DIR"
