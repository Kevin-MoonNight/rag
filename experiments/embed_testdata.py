#!/usr/bin/env python3
"""用 llama-server 算測試資料的向量：查詢、文件文字、頁面圖片、模擬掃描圖片。

用法：python3 -I embed_testdata.py <port> <輸出.npz> [image_prompt]
  image_prompt 預設 "<__media__>"（只有圖片，沒有任何文字前綴）
印出文字查詢對三種文件表示法的第一名命中數，以及每類的平均耗時。
"""
import glob
import json
import sys
import time

import numpy as np

sys.path.insert(0, "/rag/experiments")
from embed_client import DOC_PREFIX, QUERY_PREFIX, embed_image, embed_texts  # noqa: E402


def report(name: str, q: np.ndarray, d: np.ndarray, gold: list[int]) -> None:
    sims = q @ d.T
    hits = sum(int(np.argmax(sims[i]) == gold[i]) for i in range(len(gold)))
    margins = []
    for i in range(len(gold)):
        others = np.delete(sims[i], gold[i])
        margins.append(sims[i][gold[i]] - others.max())
    print(f"{name}: top-1 {hits}/{len(gold)}, gold sim avg {np.mean([sims[i][gold[i]] for i in range(len(gold))]):.3f}, min margin {min(margins):+.3f}")
    for i in range(len(gold)):
        print("   ", " ".join(f"{s:.3f}" for s in sims[i]), "| gold", gold[i], "" if int(np.argmax(sims[i])) == gold[i] else "  <-- MISS")


def main() -> None:
    port = int(sys.argv[1])
    out = sys.argv[2]
    image_prompt = sys.argv[3] if len(sys.argv) > 3 else "<__media__>"

    data = json.load(open("/rag/testdata/texts.json", encoding="utf-8"))
    queries = [q["text"] for q in data["queries"]]
    gold = [q["doc"] for q in data["queries"]]
    docs = [d["text"] for d in data["docs"]]

    t0 = time.time()
    q = np.stack([embed_texts(port, [QUERY_PREFIX + s])[0] for s in queries])
    t_q = (time.time() - t0) / len(queries)
    t0 = time.time()
    d = np.stack([embed_texts(port, [DOC_PREFIX + s])[0] for s in docs])
    t_d = (time.time() - t0) / len(docs)

    def images(pattern: str):
        vecs, t0 = [], time.time()
        paths = sorted(glob.glob(pattern))
        for p in paths:
            vecs.append(embed_image(port, open(p, "rb").read(), image_prompt))
        return np.stack(vecs), (time.time() - t0) / len(paths)

    img, t_img = images("/rag/testdata/pages/page_??.png")
    scan, t_scan = images("/rag/testdata/pages/page_??_scan.jpg")

    np.savez(out, q=q, d=d, img=img, scan=scan)
    print(f"avg time: query {t_q*1000:.0f} ms, text doc {t_d*1000:.0f} ms, page image {t_img*1000:.0f} ms, scan image {t_scan*1000:.0f} ms")
    report("query -> text doc ", q, d, gold)
    report("query -> page img ", q, img, gold)
    report("query -> scan img ", q, scan, gold)


if __name__ == "__main__":
    main()
