#!/usr/bin/env python3
"""整頁向量和各塊向量的分數怎麼合起來，才能讓「一頁一份文件」和「一頁多份文件」兩種頁面都不吃虧。

輸入是 eval_retrieval.py 的 npz（查詢向量 q、整頁掃描圖向量 scan）和 eval_tiles2.py save= 存下的各塊向量。
比較的計分方式（一頁一個分數，再排名）：
  whole          只用整頁向量（目前的設計）
  tiles          各塊取最高分
  max            整頁和各塊一起取最高分
  sum(w)         整頁分數 + w x 各塊最高分
  two lists      整頁和各塊各排一份名單，輪流各取一筆（看前 2、前 3 名）
用法：python3 -I eval_tile_scoring.py <名稱>=<評測集目錄>:<eval.npz>:<tiles.npy> [...]
"""
import json
import sys

import numpy as np

sets = {}
for arg in sys.argv[1:]:
    name, rest = arg.split("=")
    root, npz, tiles = rest.split(":")
    data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
    z = np.load(npz)
    t = np.load(tiles)
    sets[name] = dict(gold=np.array([x["doc"] for x in data["queries"]]), whole=z["q"] @ z["scan"].T,
                      tile=np.einsum("qd,ptd->qpt", z["q"], t).max(axis=2), n_tiles=t.shape[1])


def rank_of(score: np.ndarray, gold: np.ndarray) -> np.ndarray:
    return 1 + np.sum(score > score[np.arange(len(gold)), gold][:, None], axis=1)


def line(name: str, fn) -> None:
    cells = []
    for s in sets.values():
        r = fn(s)
        cells.append(f"{100 * np.mean(r == 1):5.1f}% / {100 * np.mean(r <= 3):5.1f}%")
    print(f"{name:28s} " + "   ".join(cells))


print(f"{'top-1 / top-3':28s} " + "   ".join(f"{n + ' (' + str(s['n_tiles']) + ' tiles)':15s}" for n, s in sets.items()))
line("whole page only", lambda s: rank_of(s["whole"], s["gold"]))
line("tiles only (max)", lambda s: rank_of(s["tile"], s["gold"]))
line("max(whole, tiles)", lambda s: rank_of(np.maximum(s["whole"], s["tile"]), s["gold"]))
for w in (0.25, 0.5, 1.0, 2.0, 4.0):
    line(f"whole + {w} x best tile", lambda s, w=w: rank_of(s["whole"] + w * s["tile"], s["gold"]))


def two_lists(s) -> np.ndarray:
    """兩份名單輪流各取一筆（整頁先），重複的跳過。回傳正確那一頁的名次。"""
    out = []
    for i, g in enumerate(s["gold"]):
        a, b = list(np.argsort(-s["whole"][i])), list(np.argsort(-s["tile"][i]))
        merged, seen = [], set()
        for x, y in zip(a, b):
            for c in (x, y):
                if c not in seen:
                    seen.add(c)
                    merged.append(c)
        out.append(merged.index(g) + 1)
    return np.array(out)


line("two lists, take turns", two_lists)
for s_name, s in sets.items():
    r = two_lists(s)
    print(f"    {s_name}: two lists top-2 {100 * np.mean(r <= 2):.1f}%")

# 分數的範圍：整頁和各塊最高分的平均值（看兩者能不能直接相加、相比）
for s_name, s in sets.items():
    idx = np.arange(len(s["gold"]))
    print(f"    {s_name}: gold page mean similarity whole {s['whole'][idx, s['gold']].mean():.3f}, best tile {s['tile'][idx, s['gold']].mean():.3f}; "
          f"all pages mean whole {s['whole'].mean():.3f}, best tile {s['tile'].mean():.3f}")
