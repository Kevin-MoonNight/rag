#!/usr/bin/env python3
"""檢索評測：用 llama-server 算評測集的向量，報告文字查詢對三種文件表示法（文字、乾淨頁面圖、模擬掃描圖）的命中率。

用法：python3 -I eval_retrieval.py <port> <評測集目錄> <輸出.npz>
  評測集目錄裡要有 texts.json 和 pages/（gen_eval_set.py 產生）。
指標：第一名命中率、前三名命中率、MRR；沒命中的查詢裡，第一名落在「同一類文件」（句型相同、只差主題）的比例。
輸出的 npz 可以再用 eval_retrieval_compare.py 比較兩個模型。
"""
import glob
import json
import sys
import time

import numpy as np

sys.path.insert(0, "/rag/experiments")
from embed_client import DOC_PREFIX, QUERY_PREFIX, embed_image, embed_texts  # noqa: E402

POS: list[int] = []  # 查詢的 pos 欄位（只有密集頁面版的評測集有）


def report(name: str, q: np.ndarray, d: np.ndarray, gold: list[int], family: list[str]) -> None:
    sims = q @ d.T
    order = np.argsort(-sims, axis=1)
    rank = np.array([int(np.where(order[i] == gold[i])[0][0]) for i in range(len(gold))])
    miss = np.where(rank > 0)[0]
    same_family = sum(1 for i in miss if family[order[i][0]] == family[gold[i]])
    margin = np.array([sims[i][gold[i]] - np.delete(sims[i], gold[i]).max() for i in range(len(gold))])
    print(f"{name}: top-1 {np.sum(rank == 0)}/{len(gold)} ({100 * np.mean(rank == 0):.1f}%), top-3 {100 * np.mean(rank < 3):.1f}%, "
          f"MRR {np.mean(1.0 / (rank + 1)):.3f}, misses in the same family {same_family}/{len(miss)}, "
          f"median margin {np.median(margin):+.4f}")
    by_family: dict[str, list[int]] = {}
    for i in range(len(gold)):
        by_family.setdefault(family[gold[i]], []).append(int(rank[i] == 0))
    print("    top-1 by family: " + ", ".join(f"{k} {sum(v)}/{len(v)}" for k, v in by_family.items()))
    if POS:  # 密集頁面版：依那一段在頁面裡的位置（由上往下）分開看
        by_pos: dict[int, list[int]] = {}
        for i in range(len(gold)):
            by_pos.setdefault(POS[i], []).append(int(rank[i] == 0))
        print("    top-1 by position on the page: " + ", ".join(f"#{k + 1} {sum(v)}/{len(v)}" for k, v in sorted(by_pos.items())))


def main() -> None:
    port, root, out = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
    queries = [q["text"] for q in data["queries"]]
    gold = [q["doc"] for q in data["queries"]]
    docs = [d["text"] for d in data["docs"]]
    family = [d["family"] for d in data["docs"]]
    POS.extend(q["pos"] for q in data["queries"] if "pos" in q)

    t0 = time.time()
    q = np.stack([embed_texts(port, [QUERY_PREFIX + s])[0] for s in queries])
    t_q = (time.time() - t0) / len(queries)
    t0 = time.time()
    d = np.stack([embed_texts(port, [DOC_PREFIX + s])[0] for s in docs])
    t_d = (time.time() - t0) / len(docs)

    def images(pattern: str):
        paths = sorted(glob.glob(pattern))
        assert len(paths) == len(docs), f"{pattern}: {len(paths)} images for {len(docs)} docs"
        t0 = time.time()
        vecs = [embed_image(port, open(p, "rb").read(), "<__media__>") for p in paths]
        return np.stack(vecs), (time.time() - t0) / len(paths)

    img, t_img = images(f"{root}/pages/page_???.png")
    scan, t_scan = images(f"{root}/pages/page_???_scan.jpg")
    np.savez(out, q=q, d=d, img=img, scan=scan)

    print(f"{len(docs)} docs, {len(queries)} queries; avg time: query {t_q * 1000:.0f} ms, text doc {t_d * 1000:.0f} ms, "
          f"page image {t_img * 1000:.0f} ms, scan image {t_scan * 1000:.0f} ms")
    report("query -> text doc ", q, d, gold, family)
    report("query -> page img ", q, img, gold, family)
    report("query -> scan img ", q, scan, gold, family)
    # 文字和圖片放在同一個索引裡一起排（產品實際的情況：一份文件不是文字就是圖片，這裡是最壞情況的混排）
    both = np.concatenate([d, scan])
    order = np.argsort(-(q @ both.T), axis=1)
    hit = np.mean([order[i][0] % len(docs) == gold[i] for i in range(len(gold))])
    text_first = np.mean([order[i][0] < len(docs) for i in range(len(gold))])
    print(f"mixed index (text + scan image of every doc): top-1 {100 * hit:.1f}%, the top hit is the text version in {100 * text_first:.1f}% of queries")


if __name__ == "__main__":
    main()
