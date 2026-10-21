#!/usr/bin/env python3
"""實驗 28：驗證「每組最大值不是 -8s 就是 +7s」這條規則，並比較兩種 Q4_0 編碼和未量化權重的差距。

用法：python3 -I 28_exact_grid_check.py <未量化張量目錄>
"""
import json
import os
import sys

import numpy as np

sys.path.insert(0, "/rag/experiments")
from qat_grid import exact_q4_0, ggml_q4_0_roundtrip  # noqa: E402


def main() -> None:
    ref_dir = sys.argv[1]
    header = json.load(open(os.path.join(ref_dir, "header.json"), "rb"))
    for fname in sorted(os.listdir(ref_dir)):
        if not fname.endswith(".bin"):
            continue
        name = fname[:-4]
        info = header[name]
        raw = np.fromfile(os.path.join(ref_dir, fname), dtype=np.uint16)
        x = (raw.astype(np.uint32) << 16).view(np.float32).reshape(info["shape"])
        if x.shape[0] > 20000:
            x = np.ascontiguousarray(x[:: x.shape[0] // 20000])
        d, q, st = exact_q4_0(x)
        back_ggml = ggml_q4_0_roundtrip(x)
        rel_ggml = float(np.sqrt(np.mean((back_ggml.astype(np.float64) - x) ** 2) / np.mean(x.astype(np.float64) ** 2)))
        print(f"-- {name} {list(x.shape)}")
        print(f"   blocks {st['blocks']}, largest value is negative in {100 * st['neg_ext']:.1f}% of blocks")
        print(f"   distance from the grid (in steps): max {st['max_resid']:.3f}; blocks further than 0.1 step: {st['blocks_off_grid']}")
        print(f"   exact-grid Q4_0 vs unquantized : relative RMS error {st['rel_rms']:.5f}")
        print(f"   ggml Q4_0 vs unquantized       : relative RMS error {rel_ggml:.5f}")


if __name__ == "__main__":
    main()
