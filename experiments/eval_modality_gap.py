#!/usr/bin/env python3
"""文字向量和圖片向量放在同一個索引時，掃描頁會不會被別份文件的文字壓在後面。

用 eval_retrieval.py 存下的向量做三個情境（每份文件在索引裡只有一種形式）：
  A. 正確答案那一份是掃描圖，其他 59 份都是文字。
  B. 隨機一半文件是掃描圖、一半是文字（重複多次取平均）。
  C. 同 B，但圖片的相似度先加上一個固定的補償值再一起排（補償值 = 文字與圖片「正確配對相似度」的平均差）。
並印出相似度的分布，說明差距從哪裡來。

用法：python3 -I eval_modality_gap.py <評測集目錄> <eval.npz>
"""
import json
import sys

import numpy as np

root, path = sys.argv[1:3]
data = json.load(open(f"{root}/texts.json", encoding="utf-8"))
gold = np.array([q["doc"] for q in data["queries"]])
z = np.load(path)
q, d, scan = z["q"], z["d"], z["scan"]
n_q, n_d = len(gold), len(d)
st, si = q @ d.T, q @ scan.T
idx = np.arange(n_q)

gold_t, gold_i = st[idx, gold], si[idx, gold]
mask = np.ones_like(st, dtype=bool)
mask[idx, gold] = False
best_other_t = np.where(mask, st, -1).max(axis=1)
best_other_i = np.where(mask, si, -1).max(axis=1)
print(f"similarity to the correct doc : text {gold_t.mean():.3f} (min {gold_t.min():.3f}), scan image {gold_i.mean():.3f} (min {gold_i.min():.3f})")
print(f"best similarity to a wrong doc: text {best_other_t.mean():.3f} (max {best_other_t.max():.3f}), scan image {best_other_i.mean():.3f} (max {best_other_i.max():.3f})")
gap = float(gold_t.mean() - gold_i.mean())
print(f"modality gap (text minus image, correct pairs): {gap:.3f}")

# A：正確答案是掃描圖，其他都是文字
rank_a = np.array([1 + int(np.sum(np.where(mask[i], st[i], -1) > gold_i[i])) for i in range(n_q)])
print(f"A. gold is a scan, all other docs are text : top-1 {100 * np.mean(rank_a == 1):.1f}%, top-3 {100 * np.mean(rank_a <= 3):.1f}%, "
      f"top-10 {100 * np.mean(rank_a <= 10):.1f}%, median rank {int(np.median(rank_a))}")


def mixed(offset: float, trials: int = 200) -> tuple[float, float, float]:
    rnd = np.random.default_rng(1)
    hit_all, hit_scan, hit_text = [], [], []
    for _ in range(trials):
        is_scan = rnd.permutation(n_d) < n_d // 2
        s = np.where(is_scan[None, :], si + offset, st)
        hit = np.argmax(s, axis=1) == gold
        hit_all.append(hit.mean())
        hit_scan.append(hit[is_scan[gold]].mean())
        hit_text.append(hit[~is_scan[gold]].mean())
    return float(np.mean(hit_all)), float(np.mean(hit_scan)), float(np.mean(hit_text))


for name, off in (("B. half scans, half text, no correction      ", 0.0), (f"C. same, image similarity + {gap:.3f}           ", gap)):
    a, s, t = mixed(off)
    print(f"{name}: top-1 {100 * a:.1f}% (gold is a scan {100 * s:.1f}%, gold is text {100 * t:.1f}%)")

# 補償值敏感度
print("top-1 for other offsets: " + ", ".join(f"{o:+.2f} -> {100 * mixed(o, 50)[0]:.1f}%" for o in (0.02, 0.04, 0.06, 0.08, 0.10, 0.12, 0.15)))
# 另一種不需要補償值的做法：文字和圖片各自排名，再用名次合併（RRF）
rnd = np.random.default_rng(2)
hits = []
for _ in range(200):
    is_scan = rnd.permutation(n_d) < n_d // 2
    score = np.zeros((n_q, n_d))
    for part, sims in ((is_scan, si), (~is_scan, st)):
        cols = np.where(part)[0]
        order = np.argsort(-sims[:, cols], axis=1)
        ranks = np.empty_like(order)
        ranks[np.arange(n_q)[:, None], order] = np.arange(len(cols))[None, :]
        score[:, cols] = 1.0 / (60 + ranks)
    # 名次相同（兩邊的第一名）時，用原始相似度決定先後
    tie = np.where(is_scan[None, :], si, st) * 1e-6
    hits.append(np.mean(np.argmax(score + tie, axis=1) == gold))
print(f"D. half scans, half text, rank each kind separately and interleave by rank: top-1 {100 * np.mean(hits):.1f}% (top-2 would contain both first places)")


def unit(x: np.ndarray) -> np.ndarray:
    return x / np.linalg.norm(x, axis=1, keepdims=True)


def corrected(kind: str, held_out: bool, trials: int = 200) -> tuple[float, float, float, float]:
    """不需要調參數的修正。平均向量取自索引裡的文件（held_out=False），或取自不在該類型裡的另一半文件（True）。

    center : 各類型的向量減去該類型的平均向量後重新正規化，再算相似度。
    mean   : 相似度減去「查詢和該類型平均向量的相似度」（等於減去該查詢對該類型所有文件的平均相似度）。
    zscore : 每個查詢、每個類型各自把相似度換成標準分數（減平均、除以標準差）。
    """
    rnd = np.random.default_rng(3)
    hit_all, hit_scan, hit_text, top3 = [], [], [], []
    for _ in range(trials):
        is_scan = rnd.permutation(n_d) < n_d // 2
        ref_i = scan[~is_scan] if held_out else scan[is_scan]
        ref_t = d[is_scan] if held_out else d[~is_scan]
        mi, mt = ref_i.mean(axis=0), ref_t.mean(axis=0)
        if kind == "center":
            s = np.where(is_scan[None, :], q @ unit(scan - mi).T, q @ unit(d - mt).T)
        elif kind == "mean":
            s = np.where(is_scan[None, :], si - (q @ mi)[:, None], st - (q @ mt)[:, None])
        else:
            zi = (si - (q @ ref_i.T).mean(axis=1, keepdims=True)) / (q @ ref_i.T).std(axis=1, keepdims=True)
            zt = (st - (q @ ref_t.T).mean(axis=1, keepdims=True)) / (q @ ref_t.T).std(axis=1, keepdims=True)
            s = np.where(is_scan[None, :], zi, zt)
        rank = 1 + np.sum(s > s[idx, gold][:, None], axis=1)
        hit = rank == 1
        hit_all.append(hit.mean())
        top3.append(np.mean(rank <= 3))
        hit_scan.append(hit[is_scan[gold]].mean())
        hit_text.append(hit[~is_scan[gold]].mean())
    return float(np.mean(hit_all)), float(np.mean(hit_scan)), float(np.mean(hit_text)), float(np.mean(top3))


for kind, label in (("center", "E. subtract each kind's mean vector"), ("mean", "F. subtract the query's mean similarity to each kind"), ("zscore", "G. per-query z-score within each kind")):
    for held_out in (False, True):
        a, s, t, t3 = corrected(kind, held_out)
        src = "means from the other half" if held_out else "means from the indexed docs "
        print(f"{label:55s} ({src}): top-1 {100 * a:.1f}% (gold is a scan {100 * s:.1f}%, gold is text {100 * t:.1f}%), top-3 {100 * t3:.1f}%")

# 差距在不同文件類型、乾淨頁面和掃描頁之間是否一致（一致的話才能用一個固定的補償值）
fam = np.array([data["docs"][g]["family"] for g in gold])
clean_i = (q @ z["img"].T)[idx, gold]
print("gap by family (text minus scan / text minus clean page): " + ", ".join(
    f"{f} {np.mean(gold_t[fam == f] - gold_i[fam == f]):.3f}/{np.mean(gold_t[fam == f] - clean_i[fam == f]):.3f}" for f in dict.fromkeys(fam)))
# 不看正確答案也能估計的差距：每個查詢對全部文字文件的平均相似度，減去對全部圖片的平均相似度
print(f"gap estimated without labels (mean similarity to all text docs minus to all scans): {np.mean(st.mean(axis=1) - si.mean(axis=1)):.3f}; "
      f"same using the best match of each kind: {np.mean(st.max(axis=1) - si.max(axis=1)):.3f}")

# 極端情境：正確答案是唯一的掃描頁（A），或正確答案是文字而其他 59 份全是掃描頁（A'），加上補償後的結果
for off in (0.06, 0.08, gap, 0.12):
    ra = 1 + np.sum(np.where(mask, st, -9) > (gold_i + off)[:, None], axis=1)
    rb = 1 + np.sum(np.where(mask, si + off, -9) > gold_t[:, None], axis=1)
    print(f"offset {off:.3f}: A (gold is the only scan) top-1 {100 * np.mean(ra == 1):.1f}% top-3 {100 * np.mean(ra <= 3):.1f}%; "
          f"A' (gold is the only text doc) top-1 {100 * np.mean(rb == 1):.1f}% top-3 {100 * np.mean(rb <= 3):.1f}%")

# H：兩種各自排名後交錯合併（第 1、2 名是兩種各自的第一名，依此類推），同名次的先後用加上補償值的相似度決定
for off in (0.0, 0.08):
    rnd = np.random.default_rng(4)
    acc = np.zeros(4)
    for _ in range(200):
        is_scan = rnd.permutation(n_d) < n_d // 2
        s = np.where(is_scan[None, :], si + off, st)
        own = np.where(is_scan[gold][:, None] == is_scan[None, :], s, -9)       # 和正確答案同類型的文件
        other = np.where(is_scan[gold][:, None] != is_scan[None, :], s, -9)
        r_own = np.sum(own > s[idx, gold][:, None], axis=1)                     # 正確答案在自己類型裡的名次（0 起算）
        other_sorted = -np.sort(-other, axis=1)
        behind = other_sorted[idx, np.minimum(r_own, n_d - 1)] > s[idx, gold]   # 另一類型同名次的那一筆排在前面
        rank = 2 * r_own + 1 + behind
        acc += [np.mean(rank <= k) for k in (1, 2, 3, 5)]
    acc /= 200
    print(f"H. interleave the two kinds by rank, ties by similarity (image + {off:.2f}): top-1 {100 * acc[0]:.1f}%, top-2 {100 * acc[1]:.1f}%, top-3 {100 * acc[2]:.1f}%, top-5 {100 * acc[3]:.1f}%")

# 對照：全部文件都用同一種形式時的上限
print(f"reference: all docs as text {100 * np.mean(np.argmax(st, axis=1) == gold):.1f}%, all docs as scan images {100 * np.mean(np.argmax(si, axis=1) == gold):.1f}%")
