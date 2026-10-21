#!/usr/bin/env python3
"""把 ocr_dense_pages.py 辨識出來的文字切成 chunk、算文字向量，看密集頁面的檢索準確度（對照用）。

切法不看段落，只把辨識結果的每一行依序裝進 chunk，裝到超過字數上限就換下一個（接近計畫裡的遞迴切分）。
一頁的分數取各 chunk 的最高分。查詢向量沿用 eval_retrieval.py 的 npz。
用法：python3 -I eval_ocr_chunks.py <port> <評測集目錄> <eval.npz> <ocr.json> [字數上限 ...]
"""
import json
import sys

import numpy as np

sys.path.insert(0, "/rag/experiments")
from embed_client import DOC_PREFIX, embed_texts  # noqa: E402

port, root, src, ocr = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
limits = [int(x) for x in sys.argv[5:]] or [200, 400]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
pos = np.array([q.get("pos", 0) for q in data["queries"]])
q = np.load(src)["q"]
pages = json.load(open(ocr, encoding="utf-8"))
idx = np.arange(len(gold))


def chunks(text: str, limit: int) -> list[str]:
    out, cur = [], ""
    for line in (x.strip() for x in text.splitlines()):
        if not line:
            continue
        if cur and len(cur) + len(line) > limit:
            out.append(cur)
            cur = ""
        cur = (cur + "\n" + line) if cur else line
    if cur:
        out.append(cur)
    return out or [""]


for limit in limits:
    sims, counts = [], []
    for page in pages:
        parts = chunks(page["text"], limit)
        vecs = np.stack([embed_texts(port, [DOC_PREFIX + c])[0] for c in parts])
        sims.append((q @ vecs.T).max(axis=1))
        counts.append(len(parts))
    sims = np.stack(sims, axis=1)
    rank = 1 + np.sum(sims > sims[idx, gold][:, None], axis=1)
    by_pos = ", ".join(f"#{p + 1} {int(np.sum(rank[pos == p] == 1))}/{int(np.sum(pos == p))}" for p in sorted(set(pos)))
    print(f"OCR text, chunks of up to {limit} chars ({np.mean(counts):.1f} chunks per page): top-1 {np.sum(rank == 1)}/{len(gold)} "
          f"({100 * np.mean(rank == 1):.1f}%), top-3 {100 * np.mean(rank <= 3):.1f}% | by position: {by_pos}", flush=True)
