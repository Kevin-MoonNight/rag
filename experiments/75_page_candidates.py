#!/usr/bin/env python3
"""實驗 75 的準備：替每一題列出正確檔案裡最可能有答案的幾頁，供人工標出答案所在的頁。

每頁的分數 = 問題裡的中文雙字詞和英數詞，在這頁出現了幾個（不重複）。每題列前 4 頁，
每頁附上命中詞最密集的一段（約 360 字）。
用法：python3 -I 75_page_candidates.py <index.sqlite> <queries.jsonl> <輸出.txt>
"""
import json
import re
import sqlite3
import sys


def terms(q: str) -> set:
    out = set()
    for run in re.findall(r"[一-鿿]+", q):
        out.update(run[i:i + 2] for i in range(len(run) - 1))
    out.update(w.lower() for w in re.findall(r"[A-Za-z0-9]{2,}", q))
    return out


def best_window(text: str, ts: set, width: int = 360) -> str:
    low = text.lower()
    best, best_i = -1, 0
    for i in range(0, max(1, len(text) - width), 40):
        w = low[i:i + width]
        n = sum(1 for t in ts if t in w)
        if n > best:
            best, best_i = n, i
    return text[best_i:best_i + width].replace("\n", " / ")


def main() -> None:
    db_path, q_path, out_path = sys.argv[1:4]
    db = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    files = list(db.execute("select id, path from files"))
    qs = [json.loads(l) for l in open(q_path, encoding="utf-8") if l.strip()]
    with open(out_path, "w", encoding="utf-8") as out:
        for n, q in enumerate(qs):
            golds = q["file"] if isinstance(q["file"], list) else [q["file"]]
            ts = terms(q["q"])
            pages = []
            for g in golds:
                for fid, path in files:
                    if g not in path:
                        continue
                    by_page = {}
                    for page, text in db.execute("select page, text from chunks where file_id=? and kind=0 order by chunk_index", (fid,)):
                        by_page.setdefault(page, []).append(text or "")
                    for page, texts in by_page.items():
                        text = "".join(texts)
                        score = sum(1 for t in ts if t in text.lower())
                        pages.append((score, path.rsplit("/", 1)[-1], page, text))
            pages.sort(key=lambda p: -p[0])
            total = len(pages)
            out.write(f"\n### Q{n} [{q['tag']}] {q['q']}  (gold={';'.join(golds)}, 頁數 {total})\n")
            for score, name, page, text in pages[:4]:
                out.write(f"  - {name} p{page} 分數 {score}: {best_window(text, ts)}\n")


if __name__ == "__main__":
    main()
