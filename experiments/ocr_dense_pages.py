#!/usr/bin/env python3
"""用文件辨識模型把密集頁面評測集的模擬掃描圖轉成文字（對照用，產品的決定仍然是不做 OCR）。

用法：python3 -I ocr_dense_pages.py <port> <評測集目錄> <輸出.json>
輸出每一頁的辨識文字、讀入與輸出的時間，以及和原文比對的字元錯漏率。
"""
import base64
import difflib
import glob
import json
import re
import sys
import time
import urllib.request

port, root, out = int(sys.argv[1]), sys.argv[2], sys.argv[3]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
paths = sorted(glob.glob(f"{root}/pages/page_???_scan.jpg"))


def norm(s: str) -> str:
    return re.sub(r"\s+", "", s)


pages = []
for i, p in enumerate(paths):
    b64 = base64.b64encode(open(p, "rb").read()).decode("ascii")
    body = {"messages": [{"role": "user", "content": [{"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{b64}"}},
                                                      {"type": "text", "text": "OCR:"}]}],
            "temperature": 0, "max_tokens": 3000, "cache_prompt": False}
    t0 = time.time()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
    res = json.load(urllib.request.urlopen(req, timeout=1800))
    text, tm = res["choices"][0]["message"]["content"], res.get("timings", {})
    ref, got = norm(data["docs"][i]["text"]), norm(text)
    matched = sum(b.size for b in difflib.SequenceMatcher(None, ref, got, autojunk=False).get_matching_blocks())
    pages.append({"text": text, "seconds": time.time() - t0, "prompt_ms": tm.get("prompt_ms"), "predicted_n": tm.get("predicted_n"),
                  "ref_chars": len(ref), "missing_chars": len(ref) - matched})
    print(f"page {i:02d}: {time.time() - t0:5.1f} s, {tm.get('predicted_n')} tokens out, {len(ref) - matched}/{len(ref)} reference chars missing or wrong", flush=True)
    json.dump(pages, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
tot_ref, tot_miss = sum(p["ref_chars"] for p in pages), sum(p["missing_chars"] for p in pages)
print(f"total: {len(pages)} pages, {sum(p['seconds'] for p in pages) / len(pages):.1f} s per page, "
      f"{tot_miss}/{tot_ref} reference chars missing or wrong ({100 * tot_miss / tot_ref:.2f}%)")
