#!/usr/bin/env python3
"""比較兩個模型在同一份評測集上的檢索結果（eval_retrieval.py 存下的 npz）。

用法：python3 -I eval_retrieval_compare.py <評測集目錄> <a.npz> <b.npz>
印出：兩者向量的相似度、第一名是否相同、各自命中而對方沒命中的查詢數。
"""
import json
import sys

import numpy as np

root, pa, pb = sys.argv[1:4]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
a, b = np.load(pa), np.load(pb)
print(f"== compare (a) {pa.split('/')[-1]} vs (b) {pb.split('/')[-1]}")
for key in ("q", "d", "img", "scan"):
    cos = np.sum(a[key] * b[key], axis=1)
    print(f"{key:5s} cosine between a and b: min {cos.min():.5f}  mean {cos.mean():.5f}")
for key, name in (("d", "text doc"), ("img", "page img"), ("scan", "scan img")):
    top_a = np.argmax(a["q"] @ a[key].T, axis=1)
    top_b = np.argmax(b["q"] @ b[key].T, axis=1)
    hit_a, hit_b = top_a == gold, top_b == gold
    print(f"query -> {name}: a {hit_a.sum()}/{len(gold)}, b {hit_b.sum()}/{len(gold)}; same top-1 {np.sum(top_a == top_b)}/{len(gold)}; "
          f"only a hits {np.sum(hit_a & ~hit_b)}, only b hits {np.sum(~hit_a & hit_b)}")
