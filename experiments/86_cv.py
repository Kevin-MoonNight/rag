"""實驗 86：合併規則的參數有沒有過度貼合這 128 題（交叉驗證＋自助抽樣）。

輸入是 `rag eval --pages ... --lists --out x.jsonl` 的細節檔（每題附三份原始名單），
每筆是 [段落編號, 分數, 標註頁序號(-1 無), 是不是答案頁, 是不是正確檔案]。
做法：
1. 每個候選設定（舊規則、向量合一的補償值 × 圖片保底、RRF）先算出每題的指標。
2. 重複把題目依類別分層、隨機分成兩半：用一半挑「頁前 5 名最多」的設定（同分看頁 MRR），在另一半上量。
   挑的那一半和量的那一半的差距，就是「在同一組題目上挑參數」高估了多少。
3. 新舊規則逐題配對的自助抽樣，看差距的 95% 區間。
用法：python3 -I 86_cv.py <細節檔.jsonl> <頁標註.jsonl>
"""
import json, math, random, sys
from collections import Counter

rows = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
nlab = {json.loads(l)["n"]: len(json.loads(l)["pages"]) for l in open(sys.argv[2], encoding="utf-8")}
LIMIT = 20


def push_into(out, seen, h, src):
    if h[0] not in seen:
        seen.add(h[0]); out.append((h, src))


def take_turns(t, i, k, offset):
    out, seen = [], set()
    for r in range(max(len(t), len(i), len(k))):
        tt = t[r] if r < len(t) else None
        ii = i[r] if r < len(i) else None
        if tt and ii and ii[1] + offset > tt[1]:
            push_into(out, seen, ii, "i"); push_into(out, seen, tt, "t")
        else:
            if tt: push_into(out, seen, tt, "t")
            if ii: push_into(out, seen, ii, "i")
        if r < len(k): push_into(out, seen, k[r], "k")
        if len(out) >= LIMIT: break
    return out[:LIMIT]


def vec_then_kw(t, i, k, offset, floor):
    vec = sorted([(h, "t", h[1]) for h in t] + [(h, "i", h[1] + offset) for h in i], key=lambda x: -x[2])
    out, seen = [], set()
    for r in range(max(len(vec), len(k))):
        if r < len(vec): push_into(out, seen, vec[r][0], vec[r][1])
        if r < len(k): push_into(out, seen, k[r], "k")
        if len(out) >= max(LIMIT, floor or 0): break
    if floor and i:
        top = i[0][0]
        at = next((n for n, (h, _) in enumerate(out) if h[0] == top), None)
        if at is None or at >= floor:
            out = [x for x in out if x[0][0] != top]
            out.insert(min(floor - 1, len(out)), (i[0], "i"))
    return out[:LIMIT]


def rrf(t, i, k, kc):
    score, first = {}, {}
    for lst, src in ((t, "t"), (i, "i"), (k, "k")):
        for r, h in enumerate(lst):
            score[h[0]] = score.get(h[0], 0) + 1 / (kc + r + 1)
            first.setdefault(h[0], (h, src))
    return [first[x] for x in sorted(score, key=lambda x: -score[x])[:LIMIT]]


configs = {}
for off in (0.0, 0.04, 0.08, 0.11):
    configs[f"輪流 +{off:.2f}"] = lambda t, i, k, o=off: take_turns(t, i, k, o)
for off in [round(0.05 + 0.01 * n, 2) for n in range(12)]:
    for floor in (None, 3, 5, 10):
        name = f"向量合一 +{off:.2f}" + (f" 保底{floor}" if floor else "")
        configs[name] = lambda t, i, k, o=off, f=floor: vec_then_kw(t, i, k, o, f)
for kc in (10, 60):
    configs[f"RRF k={kc}"] = lambda t, i, k, c=kc: rrf(t, i, k, c)

NEW, OLD = "向量合一 +0.11 保底10", "輪流 +0.08"


def per_question(merged, n, row):
    first = next((r + 1 for r, (h, _) in enumerate(merged) if h[3]), None)
    ffirst = next((r + 1 for r, (h, _) in enumerate(merged) if h[4]), None)
    seen, dcg = set(), 0.0
    for r, (h, _) in enumerate(merged[:10]):
        if h[2] >= 0 and h[2] not in seen:
            seen.add(h[2]); dcg += 1 / math.log2(r + 2)
    ideal = sum(1 / math.log2(r + 2) for r in range(min(nlab.get(n, 1), 10)))
    return {"p1": bool(first and first <= 1), "p5": bool(first and first <= 5), "mrr": 1 / first if first else 0.0,
            "f5": bool(ffirst and ffirst <= 5), "ndcg": dcg / ideal}


res = {name: [per_question(fn(r["lists"]["text"], r["lists"]["image"], r["lists"]["keyword"]), n, r)
              for n, r in enumerate(rows)] for name, fn in configs.items()}
N = len(rows)
scan = [r["tag"] == "掃描" for r in rows]


def mean(name, idx, key):
    return sum(res[name][j][key] for j in idx) / len(idx)


def objective(name, idx):
    return (mean(name, idx, "p5"), mean(name, idx, "mrr"))


ALL = list(range(N))
print(f"題數 {N}（掃描 {sum(scan)}）\n")
print("== 全部 128 題（這就是先前定參數時看的數字）")
for name in (OLD, "向量合一 +0.08", "向量合一 +0.11", NEW, "RRF k=60"):
    txt = [j for j in ALL if not scan[j]]; sc = [j for j in ALL if scan[j]]
    print(f"  {name:<20} 頁前1 {100*mean(name, ALL, 'p1'):5.1f}% 頁前5 {100*mean(name, ALL, 'p5'):5.1f}%"
          f" 檔案前5 {100*mean(name, ALL, 'f5'):5.1f}% 文字題nDCG {mean(name, txt, 'ndcg'):.3f}"
          f" 掃描頁前5 {100*mean(name, sc, 'p5'):5.1f}%")
best_all = max(configs, key=lambda c: objective(c, ALL))
print(f"  全部題目上最好的設定：{best_all}，頁前5 {100*mean(best_all, ALL, 'p5'):.1f}%")

print("\n== 補償值的敏感度（保底10）：頁前5 全部／文字題／掃描題，文字題 nDCG")
for off in [round(0.05 + 0.01 * n, 2) for n in range(12)]:
    name = f"向量合一 +{off:.2f} 保底10"
    txt = [j for j in ALL if not scan[j]]; sc = [j for j in ALL if scan[j]]
    print(f"  +{off:.2f}  {100*mean(name, ALL, 'p5'):5.1f}%  {100*mean(name, txt, 'p5'):5.1f}%  {100*mean(name, sc, 'p5'):5.1f}%  {mean(name, txt, 'ndcg'):.3f}")

# 交叉驗證：依類別分層，隨機對半
REPS = 1000
rng = random.Random(86)
by_tag = {}
for j, r in enumerate(rows):
    by_tag.setdefault(r["tag"], []).append(j)
sel_train, sel_test, new_test, old_test, picks = [], [], [], [], Counter()
pick_off, pick_floor = Counter(), Counter()
for _ in range(REPS):
    a, b = [], []
    for tag, idx in by_tag.items():
        idx = idx[:]; rng.shuffle(idx); h = len(idx) // 2 + rng.randint(0, len(idx) % 2)
        a += idx[:h]; b += idx[h:]
    for train, test in ((a, b), (b, a)):
        best = max(configs, key=lambda c: objective(c, train))
        picks[best] += 1
        if best.startswith("向量合一"):
            pick_off[best.split()[1]] += 1
            pick_floor[best.split()[2] if len(best.split()) > 2 else "無保底"] += 1
        sel_train.append(mean(best, train, "p5")); sel_test.append(mean(best, test, "p5"))
        new_test.append(mean(NEW, test, "p5")); old_test.append(mean(OLD, test, "p5"))
k = len(sel_test)
avg = lambda xs: 100 * sum(xs) / len(xs)
print(f"\n== 交叉驗證（分層對半 × {REPS} 次，共 {k} 次挑選），指標：頁前 5 名")
print(f"  挑參數的那一半上看到的成績       {avg(sel_train):5.1f}%")
print(f"  同一個設定在另一半上的成績       {avg(sel_test):5.1f}%   ← 誠實的估計")
print(f"  高估了                           {avg(sel_train) - avg(sel_test):5.1f} 個百分點")
print(f"  （參考）舊規則在另一半上         {avg(old_test):5.1f}%")
print(f"  （參考）現在的設定在另一半上     {avg(new_test):5.1f}%   ← 用全部題目定的，有偷看")
better = sum(s > o for s, o in zip(sel_test, old_test)); worse = sum(s < o for s, o in zip(sel_test, old_test))
print(f"  「挑出來的設定」在另一半上比舊規則好／一樣／差：{better}／{k - better - worse}／{worse} 次")
print("  最常被挑中的設定：" + "、".join(f"{c}（{100*v/k:.0f}%）" for c, v in picks.most_common(6)))
print("  補償值被挑中的比例：" + "、".join(f"{o}（{100*v/k:.0f}%）" for o, v in sorted(pick_off.items())))
print("  保底被挑中的比例：" + "、".join(f"{f}（{100*v/k:.0f}%）" for f, v in pick_floor.most_common()))

# 自助抽樣：新舊規則逐題配對
print("\n== 新規則減舊規則，逐題配對自助抽樣 5,000 次（95% 區間）")
for key, label in (("p5", "頁前5"), ("p1", "頁前1"), ("mrr", "頁MRR"), ("f5", "檔案前5")):
    for grp, idx in (("全部", ALL), ("文字題", [j for j in ALL if not scan[j]]), ("掃描題", [j for j in ALL if scan[j]])):
        d = [res[NEW][j][key] - res[OLD][j][key] for j in idx]
        boots = sorted(sum(d[rng.randrange(len(d))] for _ in d) / len(d) for _ in range(5000))
        lo, hi = boots[125], boots[4874]
        m = sum(d) / len(d)
        unit = 1 if key == "mrr" else 100
        fmt = (lambda x: f"{x:+.3f}") if key == "mrr" else (lambda x: f"{100*x:+.1f}")
        print(f"  {label:<6} {grp:<4} {fmt(m)}  [{fmt(lo)}, {fmt(hi)}]" + ("  ← 區間不含 0" if lo > 0 or hi < 0 else ""))
