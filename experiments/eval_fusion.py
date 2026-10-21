#!/usr/bin/env python3
"""向量加關鍵字的融合，遇到「有些文件進不了關鍵字名單」時會不會把它們壓到後面。

進不了關鍵字名單的有兩種：掃描頁（不做 OCR，沒有文字可以建關鍵字索引），以及和查詢不同語言的文件
（中文查詢找英文文件，沒有共同的詞）。

情境：一半文件是掃描圖（只有圖片向量），一半是文字（文字向量加 BM25）。隨機分 200 次取平均。
每個查詢有三份名單：文字向量、圖片向量、BM25（分數是 0 的文件不在名單裡）。比較幾種合併方式：
  1. 三份名單直接做 RRF（計畫原本寫的做法）。
  2. 文字這一類先在內部做 RRF（文字向量加 BM25），圖片這一類只有圖片向量；兩類再交錯合併。
  3. 三份名單輪流各取一筆（重複的跳過）：每一輪先放文字向量和圖片向量的那一筆（誰先誰後用
     「圖片相似度加補償值」比），再放 BM25 的那一筆。
  4. 不用關鍵字，只有兩種向量交錯合併（實驗 35 的做法，當對照）。

BM25 的切詞：中文用單字加雙字，英文用小寫單字（和計畫裡的做法相同）。
用法：python3 -I eval_fusion.py <評測集目錄> <eval.npz>
"""
import json
import math
import re
import sys
from collections import Counter

import numpy as np

root, path = sys.argv[1:3]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
z = np.load(path)
q, d, scan = z["q"], z["d"], z["scan"]
n_q, n_d = len(gold), len(d)
st, si = q @ d.T, q @ scan.T
K = 60          # RRF 的常數
OFFSET = 0.08   # 實驗 35 量到的圖片補償值
TRIALS = 200


def tokens(s: str) -> list[str]:
    out = [w.lower() for w in re.findall(r"[A-Za-z0-9]+", s)]
    for run in re.findall(r"[一-鿿]+", s):
        out += list(run) + [run[i:i + 2] for i in range(len(run) - 1)]
    return out


doc_tf = [Counter(tokens(x["title"] + " " + x["text"])) for x in data["docs"]]
doc_len = np.array([sum(c.values()) for c in doc_tf])
df = Counter(t for c in doc_tf for t in c)


def bm25(query: str, k1: float = 1.2, b: float = 0.75) -> np.ndarray:
    s = np.zeros(n_d)
    for t in set(tokens(query)):
        if t not in df:
            continue
        idf = math.log(1 + (n_d - df[t] + 0.5) / (df[t] + 0.5))
        tf = np.array([c[t] for c in doc_tf])
        s += idf * tf * (k1 + 1) / (tf + k1 * (1 - b + b * doc_len / doc_len.mean()))
    return s


sb = np.stack([bm25(x["text"]) for x in data["queries"]])
# 查詢和正確文件不同語言（關鍵字一定找不到）
cross = np.array([sb[i][gold[i]] == 0 for i in range(n_q)])


def ordered(scores: np.ndarray, cols: np.ndarray, drop_zero: bool = False) -> list[int]:
    cols = [c for c in cols if not (drop_zero and scores[c] == 0)]
    return sorted(cols, key=lambda c: -scores[c])


def rrf_merge(lists: list[list[int]], tie: np.ndarray) -> list[int]:
    score: dict[int, float] = {}
    for lst in lists:
        for r, c in enumerate(lst):
            score[c] = score.get(c, 0.0) + 1.0 / (K + 1 + r)
    return sorted(score, key=lambda c: (-score[c], -tie[c]))


def interleave(text_list: list[int], img_list: list[int], i: int, kw_list: list[int] | None = None) -> list[int]:
    out, seen = [], set()
    for r in range(max(len(text_list), len(img_list), len(kw_list or []))):
        pair = []
        if r < len(text_list):
            pair.append((st[i][text_list[r]], text_list[r]))
        if r < len(img_list):
            pair.append((si[i][img_list[r]] + OFFSET, img_list[r]))
        row = [c for _, c in sorted(pair, reverse=True)]
        if kw_list and r < len(kw_list):
            row.append(kw_list[r])
        for c in row:
            if c not in seen:
                seen.add(c)
                out.append(c)
    return out


def summarise(name: str, rank: np.ndarray, is_scan_gold: np.ndarray) -> None:
    def pct(m: np.ndarray, k: int) -> str:
        return f"{100 * np.mean(rank[m] <= k):.1f}%" if m.any() else "n/a"
    everything = np.ones_like(rank, dtype=bool)
    cross_all = np.tile(cross, TRIALS)
    print(f"{name}: top-1 {pct(everything, 1)}, top-3 {pct(everything, 3)}, top-5 {pct(everything, 5)} | "
          f"top-1 / top-5 when the answer is a scan {pct(is_scan_gold, 1)} / {pct(is_scan_gold, 5)}, "
          f"a text doc in the query's language {pct(~is_scan_gold & ~cross_all, 1)} / {pct(~is_scan_gold & ~cross_all, 5)}, "
          f"a text doc in the other language {pct(~is_scan_gold & cross_all, 1)} / {pct(~is_scan_gold & cross_all, 5)}")


def rank_of(lst: list[int], g: int) -> int:
    return lst.index(g) + 1 if g in lst else 10 ** 6


# 對照：全部文件都是文字
all_cols = np.arange(n_d)
ref = {"vector only": [], "BM25 only": [], "RRF of both": [], "take turns (vector, then BM25)": []}
for i in range(n_q):
    tv, kw = ordered(st[i], all_cols), ordered(sb[i], all_cols, drop_zero=True)
    ref["vector only"].append(rank_of(tv, gold[i]))
    ref["BM25 only"].append(rank_of(kw, gold[i]))
    ref["RRF of both"].append(rank_of(rrf_merge([tv, kw], st[i]), gold[i]))
    ref["take turns (vector, then BM25)"].append(rank_of(interleave(tv, [], i, kw), gold[i]))
print(f"{int(cross.sum())} of {n_q} queries are in a different language from their document")
for name, r in ref.items():
    r = np.array(r)
    print(f"all docs as text, {name:31s}: top-1 {100 * np.mean(r == 1):.1f}%, top-3 {100 * np.mean(r <= 3):.1f}%, top-5 {100 * np.mean(r <= 5):.1f}% | "
          f"same language top-1 {100 * np.mean(r[~cross] == 1):.1f}%, other language top-1 {100 * np.mean(r[cross] == 1):.1f}% top-5 {100 * np.mean(r[cross] <= 5):.1f}%")

res = {k: [] for k in ("1. RRF over the three lists           ", "2. RRF inside text, then interleave   ",
                       "3. take turns over the three lists    ", "4. vectors only, interleave two kinds ")}
gold_scan = []
rnd = np.random.default_rng(5)
for _ in range(TRIALS):
    is_scan = rnd.permutation(n_d) < n_d // 2
    c_scan, c_text = np.where(is_scan)[0], np.where(~is_scan)[0]
    gold_scan.append(is_scan[gold])
    names = list(res)
    for i in range(n_q):
        tv, iv, kw = ordered(st[i], c_text), ordered(si[i], c_scan), ordered(sb[i], c_text, drop_zero=True)
        tie = np.where(is_scan, si[i], st[i])
        res[names[0]].append(rank_of(rrf_merge([tv, iv, kw], tie), gold[i]))
        res[names[1]].append(rank_of(interleave(rrf_merge([tv, kw], st[i]), iv, i), gold[i]))
        res[names[2]].append(rank_of(interleave(tv, iv, i, kw), gold[i]))
        res[names[3]].append(rank_of(interleave(tv, iv, i), gold[i]))
gold_scan = np.concatenate(gold_scan)
print("half scans, half text:")
for name, r in res.items():
    summarise(name, np.array(r), gold_scan)
