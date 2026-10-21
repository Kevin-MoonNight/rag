"""Find text chunks whose text layer is garbled (the PDF font maps glyphs to wrong characters).

For zh + en documents the letters should be Han or Latin. Counts, per chunk, the share of letters
from other scripts plus private-use and replacement characters.

Usage: python3 -I 63_garbled_scan.py <data_dir>
"""
import sqlite3
import sys
import unicodedata

db = sqlite3.connect(f"file:{sys.argv[1]}/index.sqlite?mode=ro", uri=True)


def expected_letter(c):
    o = ord(c)
    if o < 0x250:  # ASCII, Latin-1, Latin Extended
        return True
    if 0x370 <= o <= 0x3FF:  # Greek (formulas)
        return True
    if 0x2E80 <= o <= 0x9FFF or 0xF900 <= o <= 0xFAFF or 0xFE30 <= o <= 0xFE4F or 0xFF00 <= o <= 0xFFEF or o >= 0x20000:
        return True  # CJK radicals, kana, bopomofo, Han, compatibility and full-width forms
    return False


def score(text):
    letters = bad = 0
    for c in text:
        cat = unicodedata.category(c)
        if cat in ("Co", "Cn") or c == "�":
            letters += 1
            bad += 1
        elif cat[0] == "L" or cat[0] == "M":
            letters += 1
            if not expected_letter(c):
                bad += 1
    return letters, bad


rows = db.execute("SELECT c.id, c.page, c.text, f.path FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.kind = 0").fetchall()
buckets = [0] * 6
files = {}
for cid, page, text, path in rows:
    letters, bad = score(text or "")
    share = bad / letters if letters else 0.0
    b = 0 if bad == 0 else 1 if share < 0.02 else 2 if share < 0.1 else 3 if share < 0.3 else 4 if share < 0.6 else 5
    buckets[b] += 1
    if share >= 0.1:
        files.setdefault(path, []).append((page, round(share, 2), letters))
print(f"chunks {len(rows)}")
for name, n in zip(["none", "<2%", "2-10%", "10-30%", "30-60%", ">=60%"], buckets):
    print(f"  unexpected letters {name}: {n}")
print(f"files with a chunk at 10% or more: {len(files)}")
for path, items in files.items():
    pages = sorted(set(p for p, _, _ in items))
    print(f"  {path.split('/pdf/')[-1]}  chunks {len(items)}  pages {pages[:12]}  shares {sorted(set(s for _, s, _ in items))[:6]}")
# what the low-share chunks contain (to see which characters are normal in clean documents)
seen = {}
for cid, page, text, path in rows:
    letters, bad = score(text or "")
    if 0 < bad and bad / max(1, letters) < 0.1:
        for c in text:
            cat = unicodedata.category(c)
            if (cat[0] in "LM" and not expected_letter(c)) or cat in ("Co", "Cn"):
                seen[c] = seen.get(c, 0) + 1
print("characters counted as unexpected in otherwise clean chunks:",
      sorted(((f"U+{ord(c):04X}", n) for c, n in seen.items()), key=lambda x: -x[1])[:30])
