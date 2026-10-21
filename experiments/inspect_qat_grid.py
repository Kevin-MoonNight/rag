#!/usr/bin/env python3
"""看 Google 未量化 QAT 權重在每個 32 值區塊裡實際落在什麼格點上。

對每個區塊估計格距 s（相鄰相異值的最小差），再看 x / s 是不是整數、範圍多大。
用法：python3 -I inspect_qat_grid.py <未量化張量目錄> <張量名稱> [要印出的區塊數]
"""
import json
import os
import sys

import numpy as np


def main() -> None:
    ref_dir, name = sys.argv[1:3]
    n_show = int(sys.argv[3]) if len(sys.argv) > 3 else 6
    header = json.load(open(os.path.join(ref_dir, "header.json"), "rb"))
    info = header[name]
    raw = np.fromfile(os.path.join(ref_dir, name + ".bin"), dtype=np.uint16)
    x = (raw.astype(np.uint32) << 16).view(np.float32).reshape(info["shape"])
    rows = np.linspace(0, x.shape[0] - 1, 512).astype(np.int64)
    blocks = x[rows].reshape(-1, 32).astype(np.float64)

    stats = {"kmin": [], "kmax": [], "frac_err": [], "levels": [], "ext_ratio": []}
    shown = 0
    for b in blocks:
        u = np.unique(b)
        if len(u) < 3:
            continue
        s = np.min(np.diff(u))
        k = b / s
        err = np.max(np.abs(k - np.round(k)))
        stats["kmin"].append(np.round(k).min())
        stats["kmax"].append(np.round(k).max())
        stats["frac_err"].append(err)
        stats["levels"].append(len(u))
        stats["ext_ratio"].append(np.max(np.abs(b)) / s)
        if shown < n_show:
            shown += 1
            print(f"block: s={s:.6g} levels={len(u)} k={np.array2string(np.round(k, 2), max_line_width=200)}")
    for key, vals in stats.items():
        v = np.asarray(vals)
        print(f"{key:9s}: min {v.min():.3f}  median {np.median(v):.3f}  max {v.max():.3f}")
    k_min = np.asarray(stats["kmin"])
    k_max = np.asarray(stats["kmax"])
    span = k_max - k_min
    print(f"blocks: {len(span)}; span (kmax-kmin) <= 15 in {100.0 * np.mean(span <= 15):.2f}%"
          f"; within [-8, 7] in {100.0 * np.mean((k_min >= -8) & (k_max <= 7)):.2f}%"
          f"; within [-7, 8] in {100.0 * np.mean((k_min >= -7) & (k_max <= 8)):.2f}%"
          f"; max |x/s - round| < 0.02 in {100.0 * np.mean(np.asarray(stats['frac_err']) < 0.02):.2f}%")


if __name__ == "__main__":
    main()
