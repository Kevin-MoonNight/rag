"""Compare ways of estimating the token count of a chunk with the model's real tokenizer.

Reads the text chunks of an index, asks a llama-server (/tokenize) for the real count, and reports
how far each estimate is off. Used to pick the estimate in rag_core/src/text.rs.

Usage: python3 -I 62_token_estimate.py <data_dir> [port]
"""
import json
import math
import sqlite3
import subprocess
import sys
import time
import urllib.request

data = sys.argv[1]
port = int(sys.argv[2]) if len(sys.argv) > 2 else 18555
MODEL = "/rag/models/embeddinggemma-2/embeddinggemma-2-BF16.gguf"
SERVER = "/rag/bin/linux-p4/llama-server"


def is_cjk(c):
    o = ord(c)
    return 0x4E00 <= o <= 0x9FFF or 0x3400 <= o <= 0x4DBF or 0xF900 <= o <= 0xFAFF or 0x20000 <= o <= 0x2FA1F


def classes(s):
    cjk = letters = digits = other = 0
    for c in s:
        if is_cjk(c):
            cjk += 1
        elif c.isspace():
            pass
        elif c.isascii() and c.isalpha():
            letters += 1
        elif c.isdigit():
            digits += 1
        else:
            other += 1
    return cjk, letters, digits, other


ESTIMATES = {
    "now: cjk + (rest)/4": lambda k, l, d, o: k + math.ceil((l + d + o) / 4),
    "cjk + letters/4 + digits + other": lambda k, l, d, o: k + math.ceil(l / 4) + d + o,
    "cjk + letters/4 + digits/2 + other": lambda k, l, d, o: k + math.ceil(l / 4) + math.ceil(d / 2) + o,
    "cjk + letters/3 + digits + other": lambda k, l, d, o: k + math.ceil(l / 3) + d + o,
}

proc = subprocess.Popen(
    [SERVER, "--host", "127.0.0.1", "--port", str(port), "-m", MODEL, "--embedding", "-t", "1", "-lm", "none", "-lzm", "on",
     "--cache-ram", "0", "-ngl", "0", "--no-webui"],
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(300):
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2).status == 200:
                break
        except Exception:
            time.sleep(0.5)

    def real(text):
        req = urllib.request.Request(f"http://127.0.0.1:{port}/tokenize", json.dumps({"content": text}).encode(),
                                     {"Content-Type": "application/json"})
        return len(json.loads(urllib.request.urlopen(req, timeout=60).read())["tokens"])

    db = sqlite3.connect(f"file:{data}/index.sqlite?mode=ro", uri=True)
    rows = db.execute("SELECT text FROM chunks WHERE kind = 0 AND text IS NOT NULL").fetchall()
    samples = []
    for (text,) in rows:
        samples.append((real(text), classes(text), text))
    print(f"chunks: {len(samples)}")
    tot = [sum(s[1][i] for s in samples) for i in range(4)]
    print(f"characters: cjk {tot[0]}, ascii letters {tot[1]}, digits {tot[2]}, other non-space {tot[3]}; real tokens {sum(s[0] for s in samples)}")

    def pct(v, p):
        v = sorted(v)
        return v[min(len(v) - 1, int(len(v) * p))]

    for name, f in ESTIMATES.items():
        ratios = [r / max(1, f(*c)) for r, c, _ in samples]
        under = sum(1 for x in ratios if x > 1.0)
        print(f"\n{name}\n  real/estimate: median {pct(ratios, 0.5):.2f}  p90 {pct(ratios, 0.9):.2f}  p99 {pct(ratios, 0.99):.2f}  max {max(ratios):.2f}"
              f"   underestimated: {under} ({100 * under / len(ratios):.1f}%)")
    worst = sorted(samples, key=lambda s: -s[0])[:3]
    for r, c, text in worst:
        print(f"\nlargest: real {r}, classes(cjk, letters, digits, other) {c}\n  {text[:200]!r}")
finally:
    proc.kill()
