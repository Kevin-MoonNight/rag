#!/usr/bin/env python3
"""Check the gold labels of the evaluation questions against the converted text.

For keyword / identifier questions: list every law folder that has a file containing all the terms,
so missing gold folders can be added. For all questions: check that the gold substring matches a file.
Usage: python3 57_check_gold.py
"""
import collections, json, os, re

ROOT = "/rag/testdata/realdocs"
TEXT = "/rag/work/text"
manifest = json.load(open(f"{ROOT}/manifest.json", encoding="utf-8"))
paths = [it["path"] for it in manifest]

texts = {}
for law in os.listdir(TEXT):
    d = f"{TEXT}/{law}"
    if not os.path.isdir(d):
        continue
    for f in os.listdir(d):
        texts[(law, f)] = re.sub(r"\s+", "", open(f"{d}/{f}", encoding="utf-8").read()).lower()

tags = collections.Counter()
for n, line in enumerate(open(f"{ROOT}/queries.jsonl", encoding="utf-8"), 1):
    if not line.strip():
        continue
    q = json.loads(line)
    tags[q["tag"]] += 1
    golds = q["file"] if isinstance(q["file"], list) else [q["file"]]
    for g in golds:
        hits = [p for p in paths if g in p]
        if not hits:
            print(f"line {n}: gold {g!r} matches no file")
        elif "page" in q and len(hits) != 1:
            print(f"line {n}: gold {g!r} matches {len(hits)} files (page question should match one)")
    if q["tag"] in ("關鍵詞", "編號"):
        terms = [t.lower() for t in q["q"].split()]
        found = sorted({law for (law, f), t in texts.items() if all(term in t for term in terms)})
        extra = [law for law in found if not any(g in law for g in golds)]
        missing = [g for g in golds if not any(g in law for law in found)]
        if extra or missing:
            print(f"line {n}: {q['q']!r} gold={golds} | also in: {extra} | gold without exact terms: {missing}")
print(dict(tags), "total", sum(tags.values()))
