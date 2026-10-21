#!/usr/bin/env python3
"""比較兩組向量（embed_testdata.py / ref_embed.py 的輸出）。

用法：python3 -I compare_npz.py <a.npz> <b.npz>
印出每一類（q 查詢、d 文件文字、img 頁面圖、scan 掃描圖）逐筆的餘弦相似度，
以及兩組各自的檢索排名是否一致。
"""
import json
import sys

import numpy as np


def main() -> None:
    a = np.load(sys.argv[1])
    b = np.load(sys.argv[2])
    gold = [q["doc"] for q in json.load(open("/rag/testdata/texts.json", encoding="utf-8"))["queries"]]
    for key in ("q", "d", "img", "scan"):
        if key not in a or key not in b:
            continue
        cos = np.sum(a[key] * b[key], axis=1) / (np.linalg.norm(a[key], axis=1) * np.linalg.norm(b[key], axis=1))
        print(f"{key:5s} cosine: min {cos.min():.5f}  mean {cos.mean():.5f}   [{' '.join(f'{c:.4f}' for c in cos)}]")
    for key in ("d", "img", "scan"):
        if key not in a or key not in b:
            continue
        sa, sb = a["q"] @ a[key].T, b["q"] @ b[key].T
        same_top = int(np.sum(np.argmax(sa, 1) == np.argmax(sb, 1)))
        same_order = int(np.sum(np.all(np.argsort(-sa, 1) == np.argsort(-sb, 1), axis=1)))
        hit_a = int(np.sum(np.argmax(sa, 1) == np.array(gold)))
        hit_b = int(np.sum(np.argmax(sb, 1) == np.array(gold)))
        print(f"q->{key:5s} top-1 hits: a {hit_a}/{len(gold)}, b {hit_b}/{len(gold)}; same top-1 {same_top}/{len(gold)}; same full ranking {same_order}/{len(gold)}; max |sim diff| {np.abs(sa-sb).max():.4f}")


if __name__ == "__main__":
    main()
