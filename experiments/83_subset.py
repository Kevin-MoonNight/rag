# 實驗 83：把 rag eval 的逐題結果分成「答案頁在條文對照表」和「其他」兩組，各算合併名單的命中率和 nDCG@10。
# 用法：python3 83_subset.py <page_labels.jsonl> <eval 結果 .jsonl> [更多 .jsonl ...]
import json
import sys

labels = {}
for line in open(sys.argv[1], encoding="utf-8"):
    if line.strip():
        d = json.loads(line)
        labels[d["n"]] = any("對照表" in p[1] for p in d["pages"])


def hit(rank, k):
    return rank is not None and rank <= k


for path in sys.argv[2:]:
    rows = [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]
    print(f"== {path}")
    for name, want in (("對照表", True), ("其他", False)):
        sel = [r for i, r in enumerate(rows) if i in labels and labels[i] == want]
        n = len(sel)
        if not n:
            continue
        f1 = sum(hit(r["merged"][0], 1) for r in sel) / n
        f5 = sum(hit(r["merged"][0], 5) for r in sel) / n
        p1 = sum(hit(r["merged"][1], 1) for r in sel) / n
        p5 = sum(hit(r["merged"][1], 5) for r in sel) / n
        nd = sum(r["ndcg"][3] for r in sel) / n
        print(f"  {name}（{n} 題）：檔案 前1 {f1:.1%} 前5 {f5:.1%} | 頁 前1 {p1:.1%} 前5 {p5:.1%} | nDCG@10 {nd:.3f}")
