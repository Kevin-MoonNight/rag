"""Experiment 93: compare answers before / after prompt rule 6 (no LaTeX).

Usage (in the container): python3 /rag/experiments/93_compare.py <before.jsonl> <after.jsonl>
Prints LaTeX counts, gold-in-sources / cited / not-found counts, timing medians, and every question whose
answer contained LaTeX before or after, plus the questions whose not-found status changed.
"""
import json
import re
import statistics
import sys

LATEX = re.compile(r"\$[^$\n]+\$|\\(text|times|frac|mathrm|cdot|le|ge|leq|geq|approx|mu|sim)\b|\^\{|_\{")


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def summary(name, rows):
    latex = sum(1 for r in rows if LATEX.search(r["answer"]))
    first = statistics.median(r["first_s"] for r in rows)
    total = statistics.median(r["total_s"] for r in rows)
    tokens = statistics.median(r["answer_tokens"] for r in rows)
    print(f"{name}: n={len(rows)} LaTeX {latex} | gold in sources {sum(r['gold_in_sources'] for r in rows)}"
          f" cited {sum(r['gold_cited'] for r in rows)} not found {sum(r['not_found'] for r in rows)}"
          f" | first {first:.1f} s total {total:.1f} s answer tokens {tokens:.0f}")


before, after = load(sys.argv[1]), load(sys.argv[2])
summary("before", before)
summary("after ", after)
by_q = {r["q"]: r for r in before}
same = sum(1 for r in after if r["q"] in by_q and by_q[r["q"]]["answer"] == r["answer"])
print(f"identical answers: {same}/{len(after)}")
for r in after:
    b = by_q.get(r["q"])
    if not b:
        continue
    if LATEX.search(b["answer"]) or LATEX.search(r["answer"]) or b["not_found"] != r["not_found"] or b["gold_cited"] != r["gold_cited"]:
        print("\n## " + r["q"])
        print("before:", b["answer"].replace("\n", " ")[:400])
        print("after: ", r["answer"].replace("\n", " ")[:400])
