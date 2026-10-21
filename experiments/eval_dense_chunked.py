#!/usr/bin/env python3
"""密集頁面版評測的對照：文字照計畫切成 chunk（這裡是一份原始文件一個 chunk），一頁的分數取各 chunk 的最大值。

向量沿用 eval60 的結果（密集頁面的每一段就是 eval60 的一份文件）。
用法：python3 -I eval_dense_chunked.py <eval60.npz> <密集評測集目錄> [...]
"""
import json
import sys

import numpy as np

z = np.load(sys.argv[1])
sims = z["q"] @ z["d"].T
for root in sys.argv[2:]:
    data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
    gold = np.array([q["doc"] for q in data["queries"]])
    page = np.stack([sims[:, p["docs"]].max(axis=1) for p in data["docs"]], axis=1)
    rank = 1 + np.sum(page > page[np.arange(len(gold)), gold][:, None], axis=1)
    print(f"{root.split('/')[-1]}: text chunked per section, max over a page's chunks: top-1 {np.sum(rank == 1)}/{len(gold)} "
          f"({100 * np.mean(rank == 1):.1f}%), top-3 {100 * np.mean(rank <= 3):.1f}%")
