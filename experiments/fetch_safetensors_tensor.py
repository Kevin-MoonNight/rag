#!/usr/bin/env python3
"""只下載 Hugging Face 上某個 safetensors 檔裡的指定張量（HTTP 範圍請求），不必抓整個檔案。

用法：python3 -I fetch_safetensors_tensor.py <repo id> <檔名> <輸出目錄> [張量名稱的正規表示式]
  不給正規表示式時只列出檔案裡所有張量（名稱、型別、形狀），不下載資料。
  輸出：<輸出目錄>/header.json（完整表頭）、<輸出目錄>/<張量名稱>.bin（原始位元組）。
"""
import json
import os
import re
import struct
import subprocess
import sys


def curl_range(url: str, start: int, end: int, out: str) -> None:
    subprocess.run(["curl", "-sSfL", "--retry", "5", "-r", f"{start}-{end}", "-o", out, url], check=True)


def main() -> None:
    repo, fname, out_dir = sys.argv[1:4]
    pattern = sys.argv[4] if len(sys.argv) > 4 else None
    os.makedirs(out_dir, exist_ok=True)
    url = f"https://huggingface.co/{repo}/resolve/main/{fname}"

    head_path = os.path.join(out_dir, "header.json")
    if not os.path.exists(head_path):
        tmp = os.path.join(out_dir, "header.len")
        curl_range(url, 0, 7, tmp)
        (n,) = struct.unpack("<Q", open(tmp, "rb").read())
        os.remove(tmp)
        curl_range(url, 8, 8 + n - 1, head_path)
    raw = open(head_path, "rb").read()
    header = json.loads(raw)
    base = 8 + len(raw)

    names = [k for k in header if k != "__metadata__"]
    if pattern is None:
        for k in sorted(names, key=lambda k: header[k]["data_offsets"][0]):
            t = header[k]
            print(f"{k:90s} {t['dtype']:5s} {t['shape']}")
        return
    for k in names:
        if not re.search(pattern, k):
            continue
        t = header[k]
        start, end = t["data_offsets"]
        out = os.path.join(out_dir, k + ".bin")
        print(f"{k}: {t['dtype']} {t['shape']}, {(end - start) / 1e6:.1f} MB -> {out}", flush=True)
        if os.path.exists(out) and os.path.getsize(out) == end - start:
            print("  already downloaded")
            continue
        curl_range(url, base + start, base + end - 1, out)
        assert os.path.getsize(out) == end - start, "size mismatch"


if __name__ == "__main__":
    main()
