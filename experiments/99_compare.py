# Experiment 99: compare OCR outputs on the 36 scanned pages without ground truth.
# Text is normalized (NFKC, keep only letters/digits/CJK, drop Markdown/HTML/LaTeX markup), then each pair of systems
# gets a character error rate (edit distance / reference length). Also dumps per-page disagreements between two systems.
# Usage: python3 experiments/99_compare.py                -> pairwise CER table
#        python3 experiments/99_compare.py diff A B [page] -> disagreement spans between systems A and B
import difflib, os, re, sys, unicodedata

SYSTEMS = {
    "paddlevl": "results/win/87_ocr/{n:03d}.txt",
    "nemotron": "results/99/nemotron-cuda/{n:03d}.md",
    "ppv5s": "results/99/ppocrv5-server/{n:03d}.txt",
    "ppv6m": "results/99/ppocrv6-medium/{n:03d}.txt",
    "ppv6s": "results/99/ppocrv6-small/{n:03d}.txt",
    "ppv6m_t": "results/99/ppocrv6-medium-t/{n:03d}.txt",
    "ppv6s_t": "results/99/ppocrv6-small-t/{n:03d}.txt",
    "ppv6m_vt": "results/99/ppocrv6-medium-vt/{n:03d}.txt",
    "ppv6s_vt": "results/99/ppocrv6-small-vt/{n:03d}.txt",
}

def norm(text):
    text = re.sub(r"<[^>]+>", " ", text)            # HTML tags (tables)
    text = re.sub(r"\[a-zA-Z]+", " ", text)         # LaTeX commands
    text = unicodedata.normalize("NFKC", text)
    return "".join(c for c in text if unicodedata.category(c)[0] in "LN")

def load(system, n):
    path = SYSTEMS[system].format(n=n)
    return norm(open(path, encoding="utf-8").read()) if os.path.exists(path) else None

def distance(a, b):
    # Levenshtein, O(len(a)*len(b)) with one row
    row = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        prev, row[0] = row[0], i
        for j, cb in enumerate(b, 1):
            cur = min(row[j] + 1, row[j - 1] + 1, prev + (ca != cb))
            prev, row[j] = row[j], cur
    return row[-1]

if len(sys.argv) > 1 and sys.argv[1] == "diff":
    a, b = sys.argv[2], sys.argv[3]
    pages = [int(sys.argv[4])] if len(sys.argv) > 4 else range(36)
    for n in pages:
        x, y = load(a, n), load(b, n)
        if x is None or y is None:
            continue
        ops = [op for op in difflib.SequenceMatcher(None, x, y, autojunk=False).get_opcodes() if op[0] != "equal"]
        print(f"## {n:03d} {a}={len(x)} {b}={len(y)} spans={len(ops)}")
        for tag, i1, i2, j1, j2 in ops:
            print(f"  {x[max(0, i1 - 4):i1]}[{x[i1:i2]}|{y[j1:j2]}]{x[i2:i2 + 4]}")
    sys.exit()
def table():
    names = [s for s in SYSTEMS if load(s, 0) is not None]
    texts = {s: [load(s, n) for n in range(36)] for s in names}
    print("chars:", {s: sum(len(t or "") for t in texts[s]) for s in names})
    print("CER of column vs row as reference (summed over pages both have):")
    print(r"ref\hyp  " + " ".join(f"{s:>9}" for s in names))
    for r in names:
        cells = []
        for h in names:
            pairs = [(x, y) for x, y in zip(texts[r], texts[h]) if x is not None and y is not None]
            total = sum(len(x) for x, _ in pairs)
            cells.append(f"{100 * sum(distance(x, y) for x, y in pairs) / max(total, 1):8.2f}%")
        print(f"{r:9}" + " ".join(cells))

if __name__ == "__main__":
    table()
