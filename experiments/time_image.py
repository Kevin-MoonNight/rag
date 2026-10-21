#!/usr/bin/env python3
"""重複送同一張頁面圖片取向量，印出每次耗時。用法：python3 -I time_image.py <port> [次數] [圖片路徑]"""
import sys
import time

sys.path.insert(0, "/rag/experiments")
from embed_client import embed_image  # noqa: E402

port = int(sys.argv[1])
n = int(sys.argv[2]) if len(sys.argv) > 2 else 3
path = sys.argv[3] if len(sys.argv) > 3 else "/rag/testdata/pages/page_00.png"
data = open(path, "rb").read()
for _ in range(n):
    t = time.time()
    embed_image(port, data)
    print(f"image embedding: {int((time.time() - t) * 1000)} ms")
