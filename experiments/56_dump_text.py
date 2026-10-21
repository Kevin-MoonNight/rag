#!/usr/bin/env python3
"""Convert every real PDF to text with the Rust converter and write a digest for writing questions.

Usage: python3 56_dump_text.py [excerpt_chars]
Output: /rag/work/text/<law_id>/<file>.txt (full text, one block per paragraph, [p.N] page marks)
        /rag/work/text/digest.txt (per law folder: files, pages, scanned pages, an excerpt)
"""
import json, os, subprocess, sys

ROOT = "/rag/testdata/realdocs"
OUT = "/rag/work/text"
RAG = "/rag/bin/app-linux/rag"
excerpt = int(sys.argv[1]) if len(sys.argv) > 1 else 260

manifest = json.load(open(f"{ROOT}/manifest.json", encoding="utf-8"))
by_law = {}
for it in manifest:
    by_law.setdefault(it["law_id"], []).append(it)

digest = []
for law_id in sorted(by_law):
    items = by_law[law_id]
    first = items[0]
    digest.append(f"\n=== {law_id} [{first['category']}/{first['kind']}] {first['law_name'] or first['subject'][:60]} | {first['doc_no']}")
    os.makedirs(f"{OUT}/{law_id}", exist_ok=True)
    for it in items:
        path = f"{ROOT}/{it['path']}"
        r = subprocess.run([RAG, "convert", path, "--json", "--image-dir", "/rag/work/convert-pages"],
                           capture_output=True, text=True)
        try:
            blocks = json.loads(r.stdout[r.stdout.index("["):])
        except Exception:
            digest.append(f"  - {it['name']}: CONVERT FAILED {r.stderr[-120:]!r}")
            continue
        lines, page, images, pages = [], None, [], 0
        for b in blocks:
            pages = max(pages, b.get("page", 0))
            if str(b.get("kind", "")).lower() == "image":
                images.append(b.get("page"))
                continue
            if b.get("page") != page:
                page = b.get("page")
                lines.append(f"[p.{page}]")
            mark = "# " if b.get("heading_level") else ""
            lines.append(mark + b.get("text", ""))
        text = "\n".join(lines)
        with open(f"{OUT}/{law_id}/{it['name']}.txt", "w", encoding="utf-8") as f:
            f.write(text)
        flat = text.replace("\n", " / ")
        start = min(len(flat) // 3, max(0, len(flat) - excerpt))
        digest.append(f"  - {it['name']} ({it['field']}) pages={pages} scanned={images} chars={len(text)}")
        digest.append(f"      HEAD: {flat[:120]}")
        digest.append(f"      MID:  {flat[start:start + excerpt]}")

with open(f"{OUT}/digest.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(digest))
print(len(by_law), "laws,", len(manifest), "files ->", f"{OUT}/digest.txt")
