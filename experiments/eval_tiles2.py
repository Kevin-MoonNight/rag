#!/usr/bin/env python3
"""密集頁面切塊的兩種變化（接續 eval_tiles.py），每一塊仍然是 280 個 token 的圖片向量，一頁的分數取各塊最大值。

  gaps:K     沿著頁面上最寬的 K-1 道水平空白切成 K 塊（不看內容，只看每一列有多少黑點）。
  slide:H:S  滑動視窗：每塊高度是頁高的 H 倍，每次往下移頁高的 S 倍。

用法：python3 -I eval_tiles2.py <port> <評測集目錄> <eval.npz> <模式> [...]
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

port, root, src = int(sys.argv[1]), sys.argv[2], sys.argv[3]
modes = [m for m in sys.argv[4:] if not m.startswith("save=")]
SAVE = next((m[5:] for m in sys.argv[4:] if m.startswith("save=")), "")   # save=<路徑前綴>：把各塊的向量存下來
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
pos = np.array([q.get("pos", 0) for q in data["queries"]])
q, whole = np.load(src)["q"], np.load(src)["scan"]   # whole：280 token 的整頁向量
paths = sorted(glob.glob(f"{root}/pages/page_???_scan.jpg"))
idx = np.arange(len(gold))


def report(name: str, sims: np.ndarray) -> None:
    rank = 1 + np.sum(sims > sims[idx, gold][:, None], axis=1)
    by_pos = ", ".join(f"#{p + 1} {int(np.sum(rank[pos == p] == 1))}/{int(np.sum(pos == p))}" for p in sorted(set(pos)))
    print(f"{name}: top-1 {np.sum(rank == 1)}/{len(gold)} ({100 * np.mean(rank == 1):.1f}%), top-3 {100 * np.mean(rank <= 3):.1f}%, "
          f"MRR {np.mean(1.0 / rank):.3f} | by position: {by_pos}", flush=True)


def gap_cuts(img: Image.Image, k: int) -> list[tuple[int, int]]:
    """沿最寬的 k-1 道水平空白切。回傳每一塊的 (top, bottom)。"""
    g = np.asarray(img.convert("L"))
    ink = (g < 128).sum(axis=1).astype(float)
    ink = np.convolve(ink, np.ones(5) / 5, mode="same")
    rows = np.where(ink > 0.02 * g.shape[1])[0]              # 有字的列
    first, last = int(rows[0]), int(rows[-1])
    blank = ink < 0.004 * g.shape[1]                         # 幾乎沒有黑點的列（容許掃描雜點）
    runs, start = [], None
    for y in range(first, last + 1):
        if blank[y] and start is None:
            start = y
        elif not blank[y] and start is not None:
            runs.append((y - start, (start + y) // 2))
            start = None
    min_h = (last - first) / (k * 3)                         # 每塊至少這麼高，避免切出只有一行的小塊
    cuts = []
    for _, mid in sorted(runs, reverse=True):
        if all(abs(mid - c) >= min_h for c in cuts + [first, last]):
            cuts.append(mid)
        if len(cuts) == k - 1:
            break
    edges = [max(0, first - 30)] + sorted(cuts) + [min(g.shape[0], last + 30)]
    return [(edges[i], edges[i + 1]) for i in range(len(edges) - 1)]


def slide_cuts(img: Image.Image, h: float, s: float) -> list[tuple[int, int]]:
    out, top = [], 0.0
    while True:
        out.append((int(top * img.height), int(min(1.0, top + h) * img.height)))
        if top + h >= 1.0 - 1e-6:
            return out
        top += s


for mode in modes:
    kind, *args = mode.split(":")
    t0 = time.time()
    vecs, counts = [], []
    for p in paths:
        img = Image.open(p).convert("RGB")
        boxes = gap_cuts(img, int(args[0])) if kind == "gaps" else slide_cuts(img, float(args[0]), float(args[1]))
        row = []
        for top, bottom in boxes:
            buf = io.BytesIO()
            img.crop((0, top, img.width, bottom)).save(buf, format="PNG")
            row.append(embed_image(port, buf.getvalue(), "<__media__>"))
        vecs.append(np.stack(row))
        counts.append(len(boxes))
    sims = np.stack([(q @ v.T).max(axis=1) for v in vecs], axis=1)
    print(f"-- {mode}: {min(counts)} to {max(counts)} tiles per page, {(time.time() - t0) / len(paths):.1f} s per page")
    report(f"{mode:12s}", sims)
    report(f"{mode} + the whole page", np.maximum(sims, q @ whole.T))
    if SAVE and min(counts) == max(counts):
        np.save(f"{SAVE}_{mode.replace(':', '_')}.npy", np.stack(vecs))   # 頁 x 塊 x 維度，給 eval_tile_scoring.py 用
