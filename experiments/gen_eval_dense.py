#!/usr/bin/env python3
"""產生密集頁面版的檢索評測集：把 eval60 的 60 份文件每 N 份排在同一頁（在 rag-ref 容器內執行）。

目的是測「一頁的字變多、字變小之後，280 個 token 的圖片向量還找不找得到頁面裡某一段的內容」。
內容和查詢沿用 eval60（gen_eval_set.py），只改版面：
  n3：每頁 3 份文件（約 600 到 800 字），共 20 頁。
  n6：每頁 6 份文件、六類各一份（約 1,400 字），共 10 頁。
同一頁的幾份文件來自不同類別；其他頁面有同類別、不同主題的文件，所以查詢仍然要靠主題名稱才分得出是哪一頁。
查詢的正確答案是「包含那份文件的那一頁」。

輸出到 /rag/testdata/eval60_dense_n3/ 和 eval60_dense_n6/，格式和 eval60 相同（texts.json、pages/），
queries 多一個 pos 欄位（那份文件在頁面裡由上往下第幾段，0 起算）。用法：python gen_eval_dense.py
"""
import json
import os
import random
import sys

from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, "/rag/experiments")
from gen_testdata import FONT, FONT_BOLD, TC_INDEX, scan_like, wrap  # noqa: E402

SRC = "/rag/testdata/eval60/texts.json"
W, H, MARGIN = 1240, 1754, 100  # A4 @ 150 dpi


def layout(sections: list[dict], size: int, draw: ImageDraw.ImageDraw):
    """回傳 (要畫的東西, 內容底端的 y)。"""
    f_title = ImageFont.truetype(FONT_BOLD, int(size * 1.25), index=TC_INDEX)
    f_body = ImageFont.truetype(FONT, size, index=TC_INDEX)
    items, y = [], 90
    for sec in sections:
        title, *body = sec["text"].split("\n")
        items.append((y, title, f_title))
        y += int(size * 1.25 * 1.6)
        for para in body:
            for line in wrap(draw, para, f_body, W - 2 * MARGIN):
                items.append((y, line, f_body))
                y += int(size * 1.45)
            y += int(size * 0.35)
        y += int(size * 1.1)
    return items, y


def render(sections: list[dict], path: str, start_size: int) -> int:
    img = Image.new("RGB", (W, H), "white")
    d = ImageDraw.Draw(img)
    size = start_size
    while True:  # 放不下就縮小字
        items, bottom = layout(sections, size, d)
        if bottom <= H - 120 or size <= 12:
            break
        size -= 1
    for y, text, font in items:
        d.text((MARGIN, y), text, font=font, fill="black")
    f_small = ImageFont.truetype(FONT, 18, index=TC_INDEX)
    d.text((MARGIN, H - 80), "內部文件　請勿外流", font=f_small, fill=(90, 90, 90))
    img.save(path)
    return size


def build(name: str, groups: list[list[str]], start_size: int, data: dict) -> None:
    out = f"/rag/testdata/eval60_dense_{name}"
    os.makedirs(f"{out}/pages", exist_ok=True)
    rnd = random.Random(20261013 + len(groups))
    by_family: dict[str, list[int]] = {}
    for i, doc in enumerate(data["docs"]):
        by_family.setdefault(doc["family"], []).append(i)
    for ids in by_family.values():
        rnd.shuffle(ids)
    pages, doc_page, doc_pos, sizes = [], {}, {}, []
    for gi, group in enumerate(groups):
        for j in range(10):
            ids = [by_family[f][j] for f in group]
            rnd.shuffle(ids)
            p = len(pages)
            for pos, i in enumerate(ids):
                doc_page[i], doc_pos[i] = p, pos
            path = f"{out}/pages/page_{p:03d}.png"
            sizes.append(render([data["docs"][i] for i in ids], path, start_size))
            scan_like(path, f"{out}/pages/page_{p:03d}_scan.jpg", seed=2000 + p)
            pages.append({"text": "\n\n".join(data["docs"][i]["text"] for i in ids), "family": f"group{gi}", "docs": ids})
    queries = [{"text": q["text"], "doc": doc_page[q["doc"]], "pos": doc_pos[q["doc"]], "src_doc": q["doc"]} for q in data["queries"]]
    with open(f"{out}/texts.json", "w", encoding="utf-8") as f:
        json.dump({"docs": pages, "queries": queries}, f, ensure_ascii=False, indent=1)
    chars = [len(p["text"]) for p in pages]
    print(f"{name}: {len(pages)} pages, {len(queries)} queries, {min(chars)} to {max(chars)} chars per page, "
          f"body font {min(sizes)} to {max(sizes)} px ({min(sizes) * 72 / 150:.1f} to {max(sizes) * 72 / 150:.1f} pt) -> {out}")


def main() -> None:
    data = json.load(open(SRC, encoding="utf-8"))
    build("n3", [["loan", "ward", "incident"], ["retention", "clinic", "access"]], 25, data)
    build("n6", [["loan", "retention", "ward", "clinic", "incident", "access"]], 22, data)


if __name__ == "__main__":
    main()
