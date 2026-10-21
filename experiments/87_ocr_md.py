"""實驗 87：把掃描頁的 OCR 結果組成 `--text-override` 用的 Markdown。

輸入：work/87_scan_pages.json（d256 索引裡的掃描頁）、results/win/87_ocr/NNN.txt（win_87_ocr.ps1 的輸出）。
輸出：<輸出資料夾>/<PDF 所在資料夾名>/<PDF 檔名去掉副檔名>.md，頁和頁之間用 `<!-- page break -->`；
有文字層的頁留空（照舊用 LiteParse），掃描頁放 OCR 的文字。同時印出每頁的字數和疑似重複輸出。
用法：python3 -I 87_ocr_md.py <輸出資料夾>
"""
import json, os, re, sys

root = "/rag"
out_dir = sys.argv[1]
pages = json.load(open(f"{root}/work/87_scan_pages.json", encoding="utf-8"))
docs = {}
for n, p in enumerate(pages):
    f = f"{root}/results/win/87_ocr/{n:03d}.txt"
    if not os.path.exists(f):
        print(f"缺少 {f}"); continue
    text = open(f, encoding="utf-8").read().strip()
    # PaddleOCR-VL 把畫底線的字寫成 \(\underline{\text{…}}\)：只留文字
    prev = None
    while prev != text:
        prev = text
        text = re.sub(r"\\(?:underline|text|textbf|mathrm|mathbf)\{([^{}]*)\}", r"\1", text)
    text = text.replace("\\(", "").replace("\\)", "")
    chars = len(re.sub(r"\s+", "", text))
    # 同一行連續出現很多次：模型陷入重複
    lines = [l for l in text.splitlines() if l.strip()]
    rep = max((lines.count(l) for l in set(lines)), default=0)
    print(f"{n:3d} {os.path.basename(p['path'])[:40]} 第 {p['page']} 頁：{chars} 字" + (f"，同一行重複 {rep} 次" if rep >= 5 else ""))
    docs.setdefault(p["path"], (p["pages"], {}))[1][p["page"]] = text
for path, (total, texts) in docs.items():
    folder = os.path.basename(os.path.dirname(path))
    stem = os.path.splitext(os.path.basename(path))[0]
    os.makedirs(f"{out_dir}/{folder}", exist_ok=True)
    body = "\n<!-- page break -->\n".join(texts.get(i, "") for i in range(1, total + 1))
    open(f"{out_dir}/{folder}/{stem}.md", "w", encoding="utf-8").write(body)
print(f"{len(docs)} 份 PDF 寫到 {out_dir}")
