# 實驗 91：由 work/med/q91_src.jsonl（題目＋證據短句）產生醫療題目檔與頁標註。
# 頁碼不手填：在轉檔結果裡找含證據短句的頁（去掉空白後比對）。
import json, os, re, subprocess, sys, glob
root = "/rag"
cache = "/rag/work/med/conv"
os.makedirs(cache, exist_ok=True)
norm = lambda s: re.sub(r"\s+", "", s)
pages = {}
def doc_pages(mid):
    if mid in pages: return pages[mid]
    out = f"{cache}/{mid}.json"
    if not os.path.exists(out):
        pdf = glob.glob(f"{root}/testdata/meddocs/{mid}/*.pdf")[0]
        with open(out, "w") as f:
            subprocess.run([f"{root}/bin/app-linux/rag", "convert", pdf, "--json"], stdout=f, stderr=subprocess.DEVNULL, check=True)
    p = {}
    for b in json.load(open(out)):
        if b.get("text"): p[b["page"]] = p.get(b["page"], "") + norm(b["text"])
    pages[mid] = p
    return p
qs, labels, bad = [], [], 0
for n, line in enumerate(open(f"{root}/work/med/q91_src.jsonl", encoding="utf-8")):
    src = json.loads(line)
    files, lab = [], []
    for mid, phrase in src["ev"]:
        hit = [pg for pg, t in sorted(doc_pages(mid).items()) if norm(phrase) in t]
        if not hit or len(hit) > 4:
            print(f"!! {n} {mid} 「{phrase}」 命中 {hit}"); bad += 1
        if mid not in files: files.append(mid)
        for pg in hit[:4]:
            if [mid, ".pdf", pg] not in lab: lab.append([mid, ".pdf", pg])
    qs.append({"q": src["q"], "file": files if len(files) > 1 else files[0], "tag": src["tag"]})
    labels.append({"n": n, "pages": lab})
    print(n, files, [l[2] for l in lab])
with open(f"{root}/testdata/meddocs/queries.jsonl", "w", encoding="utf-8") as f:
    for q in qs: f.write(json.dumps(q, ensure_ascii=False) + "\n")
with open(f"{root}/testdata/meddocs/page_labels.jsonl", "w", encoding="utf-8") as f:
    for l in labels: f.write(json.dumps(l, ensure_ascii=False) + "\n")
print(f"題數 {len(qs)}，有問題 {bad}")
