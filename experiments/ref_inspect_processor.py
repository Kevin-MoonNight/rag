#!/usr/bin/env python3
"""（在 rag-ref 容器內執行）看官方圖片處理器實際怎麼縮圖、切 patch。

印出：處理器類別與設定、原始碼裡和縮圖有關的函式、
以及測試頁經處理後的 patch 格點大小與邊緣像素值（判斷有沒有補邊）。
"""
import inspect
import sys

import numpy as np
from PIL import Image
from sentence_transformers import SentenceTransformer

MODEL = "/rag/models/ref/embeddinggemma-2"
model = SentenceTransformer(MODEL, device="cpu")
proc = model[0].processor
ip = proc.image_processor
print("processor:", type(proc).__name__, "| image processor:", type(ip).__name__)
print("image processor file:", inspect.getsourcefile(type(ip)))
print("config:", {k: v for k, v in ip.to_dict().items() if not k.startswith("_")})

src = inspect.getsource(sys.modules[type(ip).__module__])
print("---- image processor source ----")
print(src)

img = Image.open("/rag/testdata/pages/page_00.png").convert("RGB")
out = ip(images=img, return_tensors="np")
for k, v in out.items():
    print("out", k, getattr(v, "shape", None), getattr(v, "dtype", None))
pv = out["pixel_values"][0]
pos = out["image_position_ids"][0] if "image_position_ids" in out else None
if pos is not None:
    valid = (pos[:, 0] >= 0)
    print("valid patches:", int(valid.sum()), "grid x max", int(pos[valid, 0].max()) + 1, "grid y max", int(pos[valid, 1].max()) + 1)
    print("pixel value range:", float(pv[valid].min()), float(pv[valid].max()), "mean", float(pv[valid].mean()))
    # 第一欄（最左邊）和中間一欄 patch 的平均值，補黑邊的話最左欄會明顯偏暗
    gx = pos[:, 0]
    for col in (0, 1, int(pos[valid, 0].max()) // 2, int(pos[valid, 0].max())):
        m = valid & (gx == col)
        print(f"  column {col}: mean {float(pv[m].mean()):.4f} min {float(pv[m].min()):.4f}")
    np.savez("/rag/results/ref_pixels_page00.npz", pixel_values=pv, pos=pos)
