#!/usr/bin/env python3
"""實驗 28 的除錯：印出不符合「最大值是 -8s 或 +7s」規則的區塊，看實際的格點長什麼樣。

用法：python3 -I 28_offgrid_blocks.py <未量化張量目錄> <張量名稱> [印出幾個]
"""
import json
import os
import sys

import numpy as np

ref_dir, name = sys.argv[1:3]
n_show = int(sys.argv[3]) if len(sys.argv) > 3 else 8
header = json.load(open(os.path.join(ref_dir, "header.json"), "rb"))
raw = np.fromfile(os.path.join(ref_dir, name + ".bin"), dtype=np.uint16)
x = (raw.astype(np.uint32) << 16).view(np.float32).reshape(header[name]["shape"])
b = x[: min(x.shape[0], 2048)].reshape(-1, 32).astype(np.float64)

idx = np.argmax(np.abs(b), axis=1)
ext = b[np.arange(b.shape[0]), idx]
s_rule = np.where(ext < 0, -ext / 8.0, ext / 7.0)
kf = b / s_rule[:, None]
bad = np.where(np.abs(kf - np.round(kf)).max(axis=1) > 0.1)[0]
print(f"{len(bad)} of {b.shape[0]} blocks off the rule; of these the largest value is positive in "
      f"{100 * np.mean(ext[bad] > 0):.1f}%")

hist = {}
for i in bad:
    u = np.unique(b[i])
    s = np.min(np.diff(u))
    # 用最小差當初值，再用最小平方法修正
    k = np.round(b[i] / s)
    s = np.sum(b[i] * k) / np.sum(k * k)
    k = np.round(b[i] / s)
    key = (int(k.min()), int(k.max()))
    hist[key] = hist.get(key, 0) + 1
    if n_show > 0:
        n_show -= 1
        print(f"  ext={ext[i]:+.5f} s={s:.6g} kmin={int(k.min())} kmax={int(k.max())} resid={np.abs(b[i] / s - k).max():.3f}")
        print("    k =", " ".join(f"{int(v):d}" for v in k))
print("(kmin, kmax) of the off-rule blocks:")
for key, count in sorted(hist.items(), key=lambda kv: -kv[1])[:12]:
    print(f"  {key}: {count}")
