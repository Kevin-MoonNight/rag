#!/usr/bin/env python3
"""列出 e2b_read_pages.py 結果裡每一頁抄錄和原文不一樣的地方。

用法：python3 -I show_transcripts.py <結果.json> [...更多結果.json]
每一處差異印成  原文片段 -> 模型抄錄的片段。
"""
import difflib
import json
import os
import re
import sys


def norm(s: str) -> str:
    return re.sub(r"[\s　*#>\-`|]+", "", s)


def main() -> None:
    docs = json.load(open(os.environ.get("TESTDIR", "/rag/testdata") + "/texts.json", encoding="utf-8"))["docs"]
    for path in sys.argv[1:]:
        res = json.load(open(path, encoding="utf-8"))
        print(f"== {path}")
        n_wrong_total = 0
        for r in res["transcribe"]:
            di = int(re.search(r"page_(\d+)", r["page"]).group(1))
            truth, got = norm(docs[di]["text"]), norm(r["text"])
            sm = difflib.SequenceMatcher(None, truth, got, autojunk=False)
            diffs = []
            n_wrong = 0
            for op, a0, a1, b0, b1 in sm.get_opcodes():
                if op == "equal":
                    continue
                if a0 >= len(truth) - 1 and op == "insert":
                    continue  # 頁尾（內部文件、頁碼）不在原文裡
                n_wrong += max(a1 - a0, 0)
                diffs.append(f"「{truth[max(0,a0-2):a0]}[{truth[a0:a1]}]」->「[{got[b0:b1][:20]}]」")
            n_wrong_total += n_wrong
            print(f"  {r['page'].split('/')[-1]}: 原文 {len(truth)} 字，錯或漏 {n_wrong} 字")
            for d in diffs:
                print("     ", d)
        print(f"  合計錯或漏 {n_wrong_total} 字\n")


if __name__ == "__main__":
    main()
