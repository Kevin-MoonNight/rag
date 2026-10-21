#!/usr/bin/env python3
"""比對兩個向量檔是否逐位元相同。用法：python3 -I npz_maxdiff.py <a.npz> <b.npz>"""
import sys

import numpy as np

a = np.load(sys.argv[1])
b = np.load(sys.argv[2])
for key in a.files:
    if key not in b.files or a[key].dtype.kind != "f":
        continue
    diff = float(np.max(np.abs(a[key].astype(np.float64) - b[key].astype(np.float64))))
    print(f"{key:6s} max |a - b| = {diff:.3e}  {'bit-identical' if np.array_equal(a[key], b[key]) else 'differs'}")
