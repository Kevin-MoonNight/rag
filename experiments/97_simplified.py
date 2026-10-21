# Experiment 97: simplified-Chinese documents and questions.
# Converts every PDF of testdata/meddocs with `rag convert --json`, rebuilds each page as Markdown, and writes two
# text-override folders with the same pages: work/97/trad (as converted) and work/97/simp (OpenCC t2s).
# Indexing with --text-override <folder> then gives two indexes that differ only in the script of the text.
# Also writes work/97/queries_simp.jsonl (the 93 questions in simplified characters).
# Usage (in the container, from /rag): python3 experiments/97_simplified.py
import glob
import json
import os
import subprocess

import opencc

t2s = opencc.OpenCC("t2s")
out = "work/97"
pdfs = sorted(glob.glob("testdata/meddocs/*/*.pdf"))
for n, pdf in enumerate(pdfs, 1):
    raw = subprocess.run(["bin/app-linux/rag", "convert", pdf, "--json", "--image-dir", "/tmp/97-pages"],
                         capture_output=True, text=True, check=True).stdout
    pages = {}
    for b in json.loads(raw):
        if b.get("image_path"):
            continue
        level = b.get("heading_level")
        text = ("#" * level + " " + b["text"]) if level else b["text"]
        pages.setdefault(b["page"], []).append(text)
    last = max(pages) if pages else 0
    trad = "\n\n<!-- page break -->\n\n".join("\n\n".join(pages.get(p, [])) for p in range(1, last + 1))
    folder = os.path.basename(os.path.dirname(pdf))
    stem = os.path.splitext(os.path.basename(pdf))[0]
    for kind, text in (("trad", trad), ("simp", t2s.convert(trad))):
        os.makedirs(f"{out}/{kind}/{folder}", exist_ok=True)
        with open(f"{out}/{kind}/{folder}/{stem}.md", "w", encoding="utf-8") as f:
            f.write(text)
    print(n, len(pdfs), folder, last, "pages", flush=True)

with open("testdata/meddocs/queries.jsonl", encoding="utf-8") as src, open(f"{out}/queries_simp.jsonl", "w", encoding="utf-8") as dst:
    for line in src:
        q = json.loads(line)
        q["q"] = t2s.convert(q["q"])
        dst.write(json.dumps(q, ensure_ascii=False) + "\n")
