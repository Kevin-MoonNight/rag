"""實驗 87：圖片縮小後再 OCR，和原尺寸的辨識結果比（沒有人工標的正確答案，以原尺寸的結果當基準）。

用法：python3 -I 87_scale_cmp.py <後綴> [<後綴> ...]   例如 _s0.75 _s0.5
"""
import csv, difflib, os, re, sys

W = "/rag/results/win"


def clean(t):
    prev = None
    while prev != t:
        prev = t
        t = re.sub(r"\\(?:underline|text|textbf|mathrm|mathbf)\{([^{}]*)\}", r"\1", t)
    return re.sub(r"\s+", "", t.replace("\\(", "").replace("\\)", ""))


def times(suffix):
    with open(f"{W}/87_ocr{suffix}_times.csv") as f:
        return {int(r["n"]): r for r in csv.DictReader(f)}


base_t = times("")
for suffix in sys.argv[1:]:
    t = times(suffix)
    tot_ref = tot_match = 0
    rows = []
    for n in sorted(t):
        ref = clean(open(f"{W}/87_ocr/{n:03d}.txt", encoding="utf-8").read())
        got = clean(open(f"{W}/87_ocr{suffix}/{n:03d}.txt", encoding="utf-8").read())
        m = sum(b.size for b in difflib.SequenceMatcher(None, ref, got, autojunk=False).get_matching_blocks())
        tot_ref += len(ref); tot_match += m
        rows.append((n, len(ref), len(got), 100 * m / max(len(ref), 1), float(base_t[n]["seconds"]), float(t[n]["seconds"]),
                     int(base_t[n]["prompt_tokens"]), int(t[n]["prompt_tokens"])))
    print(f"== {suffix}：原尺寸文字被保留 {100 * tot_match / tot_ref:.1f}%")
    print(f"   每頁秒數 {sum(r[4] for r in rows) / len(rows):.1f} → {sum(r[5] for r in rows) / len(rows):.1f}，"
          f"圖片 token {sum(r[6] for r in rows) / len(rows):.0f} → {sum(r[7] for r in rows) / len(rows):.0f}")
    for r in rows:
        print(f"   頁 {r[0]:2d}：原 {r[1]} 字、這次 {r[2]} 字，保留 {r[3]:5.1f}%，{r[4]:.0f} → {r[5]:.0f} 秒")
