# Experiment 99: map simplified-only characters back to Traditional in an OCR system's output.
# Only OpenCC STCharacters entries with exactly one value that differs from the key are used (e.g. 额→額, 债→債);
# one-to-many entries (发→發/髮, 台→臺/檯/颱…) and characters that are also Traditional are left alone.
# Usage: python3 -I 99_s2t.py <input dir> <output dir>
import glob, os, sys
table = {}
for line in open("/rag/work/opencc/STCharacters.txt", encoding="utf-8"):
    if line.startswith("#") or "\t" not in line:
        continue
    key, values = line.rstrip("\n").split("\t")
    values = values.split()
    if len(values) == 1 and values[0] != key and len(key) == 1:
        table[ord(key)] = values[0]
# 别 is listed as 別/彆, but 彆 only appears in words like 彆扭, so it is fixed to 別.
table[ord("别")] = "別"
src, dst = sys.argv[1], sys.argv[2]
os.makedirs(dst, exist_ok=True)
changed = 0
for f in sorted(glob.glob(f"{src}/*.txt")):
    text = open(f, encoding="utf-8").read()
    out = text.translate(table)
    changed += sum(a != b for a, b in zip(text, out))
    open(f"{dst}/{os.path.basename(f)}", "w", encoding="utf-8").write(out)
print(f"{len(table)} mappings, {changed} characters changed")
