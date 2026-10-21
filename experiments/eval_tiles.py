#!/usr/bin/env python3
"""密集頁面切成幾條橫帶、每條各算一個圖片向量（仍然是每張 280 個 token），檢索準確度能救回多少。

對評測集的每張模擬掃描圖，切成 k 條等高的橫帶（相鄰兩條重疊 OVERLAP），每條送去算向量；
一頁的分數取各條的最大值。查詢向量沿用 eval_retrieval.py 存下的 npz（同一個模型）。
另外印「整頁向量和各條一起取最大值」的結果。

用法：python3 -I eval_tiles.py <port> <評測集目錄> <eval.npz> <輸出.npz> [k ...]
"""
import glob
import io
import json
import sys
import time

import numpy as np
from PIL import Image

sys.path.insert(0, "/rag/experiments")
from embed_client import embed_image  # noqa: E402

OVERLAP = 0.15  # 每條往上下各多取這個比例的帶高

port, root, src, out = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
ks = [int(x) for x in sys.argv[5:]] or [2, 3, 4]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
pos = np.array([q.get("pos", 0) for q in data["queries"]])
z = np.load(src)
q, whole = z["q"], z["scan"]
paths = sorted(glob.glob(f"{root}/pages/page_???_scan.jpg"))
assert len(paths) == len(whole)
idx = np.arange(len(gold))


def report(name: str, sims: np.ndarray) -> None:
    rank = 1 + np.sum(sims > sims[idx, gold][:, None], axis=1)
    by_pos = ", ".join(f"#{p + 1} {int(np.sum(rank[pos == p] == 1))}/{int(np.sum(pos == p))}" for p in sorted(set(pos)))
    print(f"{name}: top-1 {np.sum(rank == 1)}/{len(gold)} ({100 * np.mean(rank == 1):.1f}%), top-3 {100 * np.mean(rank <= 3):.1f}%, "
          f"MRR {np.mean(1.0 / rank):.3f} | by position: {by_pos}", flush=True)


report("whole page, 1 vector      ", q @ whole.T)
saved = {}
for k in ks:
    t0 = time.time()
    vecs = []
    for p in paths:
        img = Image.open(p).convert("RGB")
        band = img.height / k
        row = []
        for j in range(k):
            top = max(0, int((j - OVERLAP) * band))
            bottom = min(img.height, int((j + 1 + OVERLAP) * band))
            buf = io.BytesIO()
            img.crop((0, top, img.width, bottom)).save(buf, format="PNG")
            row.append(embed_image(port, buf.getvalue(), "<__media__>"))
        vecs.append(np.stack(row))
    tiles = np.stack(vecs)                                   # 頁 x 條 x 維度
    saved[f"k{k}"] = tiles
    sims = np.einsum("qd,ptd->qpt", q, tiles).max(axis=2)
    print(f"-- {k} bands per page ({(time.time() - t0) / len(paths):.1f} s per page)")
    report(f"{k} bands, max over bands   ", sims)
    report(f"{k} bands + the whole page  ", np.maximum(sims, q @ whole.T))
np.savez(out, **saved)
