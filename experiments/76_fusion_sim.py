"""實驗 76：離線比較三份名單的合併規則。

輸入是 `rag eval --pages ... --lists --out x.jsonl` 的細節檔（每題附三份原始名單）。
每筆是 [段落編號, 分數, 標註頁序號(-1 無), 是不是答案頁, 是不是正確檔案]。
用法：python3 -I 76_fusion_sim.py <細節檔.jsonl> [image_offset]
"""
import json, math, sys

rows = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
OFFSET = float(sys.argv[2]) if len(sys.argv) > 2 else 0.08
LIMIT = 20


def take_turns(t, i, k, offset=OFFSET):
    out, seen = [], set()
    def push(h, src):
        if h[0] not in seen:
            seen.add(h[0]); out.append((h, src))
    for r in range(max(len(t), len(i), len(k))):
        tt = t[r] if r < len(t) else None
        ii = i[r] if r < len(i) else None
        if tt and ii and ii[1] + offset > tt[1]:
            push(ii, "i"); push(tt, "t")
        else:
            if tt: push(tt, "t")
            if ii: push(ii, "i")
        if r < len(k): push(k[r], "k")
        if len(out) >= LIMIT: break
    return out[:LIMIT]


def vec_then_kw(t, i, k, offset=OFFSET, kw_first=False):
    """文字和圖片依（圖片加補償後的）相似度排成一份向量名單，再和關鍵字輪流。"""
    vec = sorted([(h, "t", h[1]) for h in t] + [(h, "i", h[1] + offset) for h in i], key=lambda x: -x[2])
    out, seen = [], set()
    def push(h, src):
        if h[0] not in seen:
            seen.add(h[0]); out.append((h, src))
    for r in range(max(len(vec), len(k))):
        a = (vec[r][0], vec[r][1]) if r < len(vec) else None
        b = (k[r], "k") if r < len(k) else None
        for x in ([b, a] if kw_first else [a, b]):
            if x: push(*x)
        if len(out) >= LIMIT: break
    return out[:LIMIT]


def rrf(t, i, k, kc=60, w=(1.0, 1.0, 1.0)):
    score, first = {}, {}
    for lst, src, wt in ((t, "t", w[0]), (i, "i", w[1]), (k, "k", w[2])):
        for r, h in enumerate(lst):
            score[h[0]] = score.get(h[0], 0) + wt / (kc + r + 1)
            first.setdefault(h[0], (h, src))
    order = sorted(score, key=lambda x: -score[x])[:LIMIT]
    return [first[x] for x in order]


def gated_turns(t, i, k, offset=OFFSET, gap=0.0):
    """輪流，但圖片只有在（加補償後）不比同輪文字低超過 gap 時才放進來；沒放的圖片之後也不放。"""
    out, seen = [], set()
    def push(h, src):
        if h[0] not in seen:
            seen.add(h[0]); out.append((h, src))
    top_t = t[0][1] if t else 0
    for r in range(max(len(t), len(i), len(k))):
        tt = t[r] if r < len(t) else None
        ii = i[r] if r < len(i) else None
        ok = ii and (not tt or ii[1] + offset > tt[1] - gap)
        if tt and ok and ii[1] + offset > tt[1]:
            push(ii, "i"); push(tt, "t")
        else:
            if tt: push(tt, "t")
            if ok: push(ii, "i")
        if r < len(k): push(k[r], "k")
        if len(out) >= LIMIT: break
    return out[:LIMIT]


def metrics(merged, row):
    labels = None
    # 標註頁數：從三份名單裡看得到的最大序號推不出來，用細節檔的 ndcg 是否為 None 判斷有沒有標註
    hits = [h[3] for h, _ in merged]
    first = next((n + 1 for n, x in enumerate(hits) if x), None)
    # 交給生成模型的是前 5 段文字段落（圖片不交）
    text_only = [h for h, s in merged if s != "i" or True]
    return first


def evaluate(fn, name):
    tot = {}
    for row in rows:
        L = row["lists"]
        merged = fn(L["text"], L["image"], L["keyword"])
        scan = row["tag"] == "掃描"
        group = "掃描" if scan else "文字"
        d = tot.setdefault(group, {"n": 0, "p1": 0, "p5": 0, "p10": 0, "mrr": 0.0, "ndcg": 0.0, "nd_n": 0, "gen5": 0, "img10": 0})
        d["n"] += 1
        hits = [h[3] for h, _ in merged]
        first = next((n + 1 for n, x in enumerate(hits) if x), None)
        if first:
            d["mrr"] += 1 / first
            d["p1"] += first <= 1; d["p5"] += first <= 5; d["p10"] += first <= 10
        d["img10"] += sum(1 for _, s in merged[:10] if s == "i")
        if not scan:
            # nDCG@10，每個標註頁只算一次
            nlab = row.get("_nlab")
            seen, dcg = set(), 0.0
            for n, (h, _) in enumerate(merged[:10]):
                if h[2] >= 0 and h[2] not in seen:
                    seen.add(h[2]); dcg += 1 / math.log2(n + 2)
            ideal = sum(1 / math.log2(n + 2) for n in range(min(nlab, 10)))
            d["ndcg"] += dcg / ideal; d["nd_n"] += 1
            # 生成：前 5 段文字段落（依合併名次，跳過圖片）裡有沒有答案頁
            texts = [h for h, s in merged if s != "i" and not is_image.get(h[0], False)][:5]
            d["gen5"] += any(h[3] for h in texts)
    parts = []
    for g in ("文字", "掃描"):
        d = tot[g]; n = d["n"]
        s = f"{g} 頁前1 {100*d['p1']/n:5.1f}% 前5 {100*d['p5']/n:5.1f}% 前10 {100*d['p10']/n:5.1f}% MRR {d['mrr']/n:.3f}"
        if d["nd_n"]:
            s += f" nDCG@10 {d['ndcg']/d['nd_n']:.3f} 生成前5段含答案 {100*d['gen5']/n:5.1f}%"
        s += f" 前10名平均圖片 {d['img10']/n:.1f}"
        parts.append(s)
    print(f"{name:<34} " + "\n" + "\n".join("    " + p for p in parts))


# 每題的標註頁數：名單裡出現過的最大序號 + 1 只是下限，改從頁標註檔讀
labfile = "/rag/testdata/realdocs/page_labels.jsonl"
nlab = {json.loads(l)["n"]: len(json.loads(l)["pages"]) for l in open(labfile, encoding="utf-8")}
for n, row in enumerate(rows):
    row["_nlab"] = nlab.get(n, 1)
# 圖片名單裡的段落都是圖片（掃描頁或頁面圖）；文字名單、關鍵字名單裡的都是文字
is_image = {}
for row in rows:
    for h in row["lists"]["image"]:
        is_image[h[0]] = True

evaluate(take_turns, "現行：三份輪流")
evaluate(lambda t, i, k: take_turns(t, i, k, 0.0), "三份輪流，補償 0")
evaluate(vec_then_kw, "向量合一（圖片+0.08）再和關鍵字輪流")
evaluate(lambda t, i, k: vec_then_kw(t, i, k, kw_first=True), "同上，關鍵字先")
evaluate(lambda t, i, k: vec_then_kw(t, i, k, offset=0.12), "向量合一（圖片+0.12）")
evaluate(lambda t, i, k: vec_then_kw(t, i, k, offset=0.16), "向量合一（圖片+0.16）")
for gap in (0.0, 0.05, 0.1):
    evaluate(lambda t, i, k, g=gap: gated_turns(t, i, k, gap=g), f"輪流但圖片要夠接近 gap={gap}")
evaluate(rrf, "RRF k=60")
evaluate(lambda t, i, k: rrf(t, i, k, w=(1, 0.5, 1)), "RRF 圖片權重 0.5")
evaluate(lambda t, i, k: rrf(t, i, k, kc=10), "RRF k=10")


def rrf_with_image_slots(t, i, k, offset=0.12, kc=60, kw_first=False):
    """圖片的位置照「向量合一再和關鍵字輪流」決定，其餘位置依序填文字向量+關鍵字的 RRF 名單。"""
    layout = vec_then_kw(t, i, k, offset=offset, kw_first=kw_first)
    texts = [x for x in rrf(t, [], k, kc=kc)]
    out, seen, ti = [], set(), 0
    for h, src in layout:
        if src == "i":
            if h[0] not in seen:
                seen.add(h[0]); out.append((h, "i"))
            continue
        while ti < len(texts) and texts[ti][0][0] in seen:
            ti += 1
        if ti < len(texts):
            seen.add(texts[ti][0][0]); out.append(texts[ti]); ti += 1
    return out[:LIMIT]


print("\n--- 混合：RRF 排文字段落、向量分數決定圖片位置")
for off in (0.08, 0.10, 0.12, 0.14):
    for kc in (10, 60):
        evaluate(lambda t, i, k, o=off, c=kc: rrf_with_image_slots(t, i, k, offset=o, kc=c), f"圖片+{off} RRF k={kc}")
print("\n--- 向量合一的補償值掃描")
for off in (0.09, 0.10, 0.11, 0.13, 0.14):
    evaluate(lambda t, i, k, o=off: vec_then_kw(t, i, k, offset=o), f"向量合一（圖片+{off}）")


def with_image_floor(fn, pos=3):
    """合併後，圖片名單的第 1 名如果不在前 pos 名，就插到第 pos 名（維持「每份名單的第 1 名在前 3 名」）。"""
    def go(t, i, k):
        out = fn(t, i, k)
        if not i:
            return out
        top = i[0][0]
        idx = next((n for n, (h, _) in enumerate(out) if h[0] == top), None)
        if idx is not None and idx < pos:
            return out
        out = [x for x in out if x[0][0] != top]
        out.insert(pos - 1, (i[0], "i"))
        return out[:LIMIT]
    return go


def by_tag(fn, name, tags=("英文", "編號", "關鍵詞", "改寫")):
    s = []
    for tag in tags:
        sub = [r for r in rows if r["tag"] == tag]
        p1 = p5 = 0
        for r in sub:
            L = r["lists"]
            m = fn(L["text"], L["image"], L["keyword"])
            first = next((n + 1 for n, (h, _) in enumerate(m) if h[3]), None)
            p1 += bool(first and first <= 1); p5 += bool(first and first <= 5)
        s.append(f"{tag} {p1}/{p5}/{len(sub)}")
    print(f"  {name:<30} 前1/前5/題數：" + "  ".join(s))


print("\n--- 圖片第 1 名保底在第 3 名")
for off in (0.08, 0.11, 0.12):
    evaluate(with_image_floor(lambda t, i, k, o=off: vec_then_kw(t, i, k, offset=o)), f"向量合一（圖片+{off}）+ 保底")
print("\n--- 各類別")
by_tag(take_turns, "現行")
by_tag(lambda t, i, k: vec_then_kw(t, i, k, offset=0.11), "向量合一 +0.11")
by_tag(with_image_floor(lambda t, i, k: vec_then_kw(t, i, k, offset=0.11)), "向量合一 +0.11 保底")
by_tag(lambda t, i, k: rrf_with_image_slots(t, i, k, offset=0.12, kc=10), "RRF k=10 圖片+0.12")
by_tag(rrf, "RRF k=60")

print("\n--- 向量合一 +0.11，圖片第 1 名保底在第 N 名")
for pos in (5, 8, 10):
    evaluate(with_image_floor(lambda t, i, k: vec_then_kw(t, i, k, offset=0.11), pos), f"保底第 {pos} 名")


def file_level(fn, name):
    out = []
    for g in ("文字", "掃描"):
        sub = [r for r in rows if (r["tag"] == "掃描") == (g == "掃描")]
        c = [0, 0, 0]
        for r in sub:
            L = r["lists"]; m = fn(L["text"], L["image"], L["keyword"])
            first = next((n + 1 for n, (h, _) in enumerate(m) if h[4]), None)
            for j, cut in enumerate((1, 5, 10)):
                c[j] += bool(first and first <= cut)
        out.append(f"{g} 檔案前1/5/10 " + "/".join(f"{100*x/len(sub):.1f}" for x in c))
    print(f"  {name:<24} " + "  ".join(out))


print("\n--- 檔案層級")
file_level(take_turns, "現行")
file_level(lambda t, i, k: vec_then_kw(t, i, k, offset=0.11), "向量合一 +0.11")
for pos in (5, 8, 10):
    file_level(with_image_floor(lambda t, i, k: vec_then_kw(t, i, k, offset=0.11), pos), f"+0.11 保底第 {pos} 名")
