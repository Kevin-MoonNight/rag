#!/usr/bin/env python3
"""用 Google 官方實作（sentence-transformers）算參考向量（在 rag-ref 容器內執行）。

用法：python ref_embed.py [float32|bfloat16]
讀 /rag/testdata/，寫 /rag/results/ref_embeddings.npz：
  q, d        查詢、文件的文字向量
  img, scan   乾淨頁面圖、模擬掃描圖的圖片向量
同時印出 tokenizer / processor 實際送進模型的 token，供比對 llama.cpp 的輸入格式。
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
DOC_PREFIX = "title: none | text: "


def main() -> None:
    dtype = torch.bfloat16 if (len(sys.argv) > 1 and sys.argv[1] == "bfloat16") else torch.float32
    torch.set_num_threads(6)
    t0 = time.time()
    model = SentenceTransformer(MODEL, device="cpu", model_kwargs={"torch_dtype": dtype})
    print(f"loaded in {time.time()-t0:.1f}s, dtype={dtype}")
    print("modules:", [type(m).__name__ for m in model])
    print("prompts:", json.dumps(model.prompts, ensure_ascii=False)[:600])

    data = json.load(open("/rag/testdata/texts.json", encoding="utf-8"))
    queries = [q["text"] for q in data["queries"]]
    docs = [d["text"] for d in data["docs"]]

    # 實際送進模型的 token（看有沒有 BOS / EOS、前綴怎麼接）
    tok = model.tokenizer
    ids = tok(QUERY_PREFIX + queries[0])["input_ids"]
    print("text token ids:", ids)
    print("text tokens   :", tok.convert_ids_to_tokens(ids))

    t0 = time.time()
    q = model.encode([QUERY_PREFIX + s for s in queries], normalize_embeddings=True, batch_size=4)
    d = model.encode([DOC_PREFIX + s for s in docs], normalize_embeddings=True, batch_size=2)
    print(f"text: {len(queries)+len(docs)} texts in {time.time()-t0:.1f}s, dim={q.shape[1]}")

    imgs = [Image.open(p).convert("RGB") for p in sorted(glob.glob("/rag/testdata/pages/page_??.png"))]
    scans = [Image.open(p).convert("RGB") for p in sorted(glob.glob("/rag/testdata/pages/page_??_scan.jpg"))]

    # 圖片實際展開成什麼 token
    try:
        proc = model[0].processor
        feats = proc(text="<|image|>", images=imgs[0], return_tensors="pt")
        ids = feats["input_ids"][0].tolist()
        print("image input: n_tokens =", len(ids), "pixel_values shape =", tuple(feats["pixel_values"].shape) if "pixel_values" in feats else None)
        toks = tok.convert_ids_to_tokens(ids)
        print("image tokens head:", toks[:6], "... tail:", toks[-6:])
        print("image token id counts:", {t: toks.count(t) for t in sorted(set(toks))})
        for k, v in feats.items():
            print("  feat", k, tuple(v.shape) if hasattr(v, "shape") else type(v))
    except Exception as e:  # noqa: BLE001
        print("processor introspection failed:", repr(e))

    t0 = time.time()
    img = model.encode(imgs, normalize_embeddings=True, batch_size=1)
    print(f"images: {len(imgs)} in {time.time()-t0:.1f}s")
    scan = model.encode(scans, normalize_embeddings=True, batch_size=1)

    np.savez("/rag/results/ref_embeddings.npz", q=q, d=d, img=img, scan=scan)

    def report(name, a, b):
        sims = a @ b.T
        hits = sum(int(np.argmax(sims[i]) == data["queries"][i]["doc"]) for i in range(len(queries)))
        print(f"{name}: top-1 {hits}/{len(queries)}")
        for i in range(len(queries)):
            print("   ", " ".join(f"{s:.3f}" for s in sims[i]), "| gold", data["queries"][i]["doc"])

    report("query -> text doc ", q, d)
    report("query -> page img ", q, img)
    report("query -> scan img ", q, scan)


if __name__ == "__main__":
    main()
