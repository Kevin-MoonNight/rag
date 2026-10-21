"""Compare two evaluation outputs question by question: where did the rank of the right file change?

Usage: python3 -I eval_diff.py <a.jsonl> <b.jsonl> [list name: merged|text|keyword|image]
"""
import json
import sys

a = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
b = [json.loads(l) for l in open(sys.argv[2], encoding="utf-8")]
which = sys.argv[3] if len(sys.argv) > 3 else "merged"
if len(sys.argv) > 4:
    print(json.dumps(a[0], ensure_ascii=False)[:1500])


def rank(row, name):
    v = row.get(name)
    if isinstance(v, dict):
        return v.get("file_rank")
    return v


for ra, rb in zip(a, b):
    x, y = rank(ra, which), rank(rb, which)
    if x != y:
        print(f"[{ra['tag']}] {which}: {x} -> {y}   {ra['q']}")
