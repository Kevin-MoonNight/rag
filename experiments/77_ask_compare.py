"""比較兩次 ask-eval 的結果（例如相容漢字修正前後、合併規則改前改後）。

用法：python3 -I 77_ask_compare.py <頁標註.jsonl> <a.jsonl> <b.jsonl> [...]
每個檔案印一行統計；最後列出兩者答案不同的題目裡值得看的差異（找不到、截斷）。
"""
import json, sys, unicodedata

labels = {}
for l in open(sys.argv[1], encoding="utf-8"):
    j = json.loads(l)
    labels[j["n"]] = j["pages"]


def page_hit(src, labs):
    name = src["file"].rsplit("/", 1)[-1]
    return any(src["page"] == p and code in src["file"] and frag in name for code, frag, p in labs)


def odd_chars(s):
    # 相容漢字、康熙部首、替代字元、私用區
    return sum(1 for c in s if 0xF900 <= ord(c) <= 0xFAFF or 0x2E80 <= ord(c) <= 0x2FDF or 0x2F800 <= ord(c) <= 0x2FA1F
               or c == "�" or unicodedata.category(c) == "Co")


runs = {}
for f in sys.argv[2:]:
    rows = [json.loads(l) for l in open(f, encoding="utf-8")]
    runs[f] = rows
    n = len(rows)
    nf = sum(r["not_found"] for r in rows)
    trunc = sum(r["finish"] != "stop" for r in rows)
    gis = sum(r["gold_in_sources"] for r in rows)
    gc = sum(r["gold_cited"] for r in rows)
    lab = [(i, r) for i, r in enumerate(rows) if i in labels]
    pin = sum(any(page_hit(s, labels[i]) for s in r["sources"]) for i, r in lab)
    pcited = sum(any(page_hit(s, labels[i]) for s in r["sources"] if s["n"] in r["cited"]) for i, r in lab)
    odd = sum(odd_chars(r["answer"]) > 0 for r in rows)
    toks = sorted(r["answer_tokens"] for r in rows)
    tot = sorted(r["total_s"] for r in rows)
    print(f"{f}\n  題數 {n}  找不到 {nf}  截斷 {trunc}  正確檔案在資料裡 {gis}  被引用 {gc}  "
          f"答案頁在資料裡 {pin}/{len(lab)}  答案頁被引用 {pcited}/{len(lab)}  回答含怪字 {odd}  "
          f"回答 token 中位數 {toks[n//2]}  全部時間中位數 {tot[n//2]:.1f} 秒")

files = list(runs)
if len(files) >= 2:
    a, b = runs[files[0]], runs[files[1]]
    print("\n兩次結果不同的題目（找不到或截斷有變化）：")
    for i, (x, y) in enumerate(zip(a, b)):
        if (x["not_found"], x["finish"]) != (y["not_found"], y["finish"]):
            print(f"  Q{i} [{x['tag']}] {x['q'][:40]}  找不到 {x['not_found']}→{y['not_found']}  結束 {x['finish']}→{y['finish']}")
