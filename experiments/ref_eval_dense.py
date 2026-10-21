#!/usr/bin/env python3
"""用 Google 官方實作（sentence-transformers）跑密集頁面的檢索評測（在 rag-ref 容器內執行）。

目的：確認「圖片向量只抓得到頁面最上面那一段」是模型本身的特性，還是 llama.cpp 圖片前處理造成的。
對每個評測集算查詢和模擬掃描圖的向量，印出命中率（依那一段在頁面裡的位置分開），
並和 llama.cpp 存下的向量（eval_retrieval.py 的 npz）比餘弦相似度。

用法：python ref_eval_dense.py <評測集目錄>:<llama 的 npz> [...]
"""
import glob
import json
import sys
import time

import numpy as np
import torch
from PIL import Image
from sentence_transformers import SentenceTransformer

MODEL = "/rag/models/ref/embeddinggemma-2"
QUERY_PREFIX = "task: search result | query: "


def report(name: str, sims: np.ndarray, gold: np.ndarray, pos: np.ndarray) -> None:
    rank = 1 + np.sum(sims > sims[np.arange(len(gold)), gold][:, None], axis=1)
    by_pos = ", ".join(f"#{p + 1} {int(np.sum(rank[pos == p] == 1))}/{int(np.sum(pos == p))}" for p in sorted(set(pos)))
    print(f"{name}: top-1 {np.sum(rank == 1)}/{len(gold)} ({100 * np.mean(rank == 1):.1f}%), top-3 {100 * np.mean(rank <= 3):.1f}% | by position: {by_pos}", flush=True)


def main() -> None:
    torch.set_num_threads(6)
    model = SentenceTransformer(MODEL, device="cpu", model_kwargs={"torch_dtype": torch.float32})
    for arg in sys.argv[1:]:
        root, llama_npz = arg.split(":")
        data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
        gold = np.array([q["doc"] for q in data["queries"]])
        pos = np.array([q.get("pos", 0) for q in data["queries"]])
        t0 = time.time()
        q = model.encode([QUERY_PREFIX + x["text"] for x in data["queries"]], normalize_embeddings=True, batch_size=4)
        scans = [Image.open(p).convert("RGB") for p in sorted(glob.glob(f"{root}/pages/page_???_scan.jpg"))]
        scan = model.encode(scans, normalize_embeddings=True, batch_size=1)
        print(f"== {root.split('/')[-1]}: {len(q)} queries, {len(scan)} scan images, {time.time() - t0:.0f} s")
        z = np.load(llama_npz)
        cq, cs = np.sum(q * z["q"], axis=1), np.sum(scan * z["scan"], axis=1)
        print(f"cosine official vs llama.cpp: queries min {cq.min():.4f} mean {cq.mean():.4f}; scan images min {cs.min():.4f} mean {cs.mean():.4f}")
        report("official implementation ", q @ scan.T, gold, pos)
        report("llama.cpp (modified)    ", z["q"] @ z["scan"].T, gold, pos)
        np.savez(llama_npz.replace(".npz", "_ref.npz"), q=q, scan=scan)


if __name__ == "__main__":
    main()
