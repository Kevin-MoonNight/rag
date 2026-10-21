#!/usr/bin/env python3
"""檢查 Google 未量化 QAT 權重和發布的 GGUF 之間的關係，判斷輸出層能不能無損地改成 Q4_0。

對每個張量印出：
  - 未量化權重用 llama.cpp 的 Q4_0 演算法量化再還原後，相對誤差（RMS）是多少。
    接近 0 表示權重本來就落在 Q4_0 的格點上，改成 Q4_0 是無損的。
  - Google 發布的 GGUF 裡同一個張量（還原成浮點數後）和未量化權重的相對誤差。
  - GGUF 裡的張量和「我們自己把未量化權重量化成 Q4_0」的結果是否一致。

用法：python3 -I check_q4_grid.py <未量化張量目錄> <官方.gguf>
"""
import json
import os
import sys

import numpy as np

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
import gguf  # noqa: E402
from gguf import quants  # noqa: E402

PAIRS = [
    ("model.language_model.layers.0.self_attn.q_proj.weight", "blk.0.attn_q.weight"),
    ("model.language_model.layers.0.mlp.gate_proj.weight", "blk.0.ffn_gate.weight"),
    ("model.language_model.embed_tokens.weight", "token_embd.weight"),
]


def load_unquantized(ref_dir: str, name: str, header: dict) -> np.ndarray:
    info = header[name]
    raw = np.fromfile(os.path.join(ref_dir, name + ".bin"), dtype=np.uint16)
    assert info["dtype"] == "BF16"
    return (raw.astype(np.uint32) << 16).view(np.float32).reshape(info["shape"])


def q4_0_roundtrip(x: np.ndarray) -> np.ndarray:
    """和 ggml 的 quantize_row_q4_0_ref / dequantize_row_q4_0 相同的計算。"""
    b = x.reshape(-1, 32)
    idx = np.argmax(np.abs(b), axis=1)
    vmax = b[np.arange(b.shape[0]), idx]
    d = vmax / -8.0
    with np.errstate(divide="ignore"):
        inv = np.where(d != 0, 1.0 / d, 0.0).astype(np.float32)
    q = np.minimum(15, np.floor(b * inv[:, None] + 8.5)).astype(np.int8)
    d16 = d.astype(np.float16).astype(np.float32)
    return ((q - 8).astype(np.float32) * d16[:, None]).reshape(x.shape)


def rel_rms(a: np.ndarray, b: np.ndarray) -> float:
    a64 = a.astype(np.float64)
    return float(np.sqrt(np.mean((a64 - b) ** 2) / np.mean(a64 ** 2)))


def main() -> None:
    ref_dir, gguf_path = sys.argv[1:3]
    header = json.load(open(os.path.join(ref_dir, "header.json"), "rb"))
    reader = gguf.GGUFReader(gguf_path)
    tensors = {t.name: t for t in reader.tensors}
    for st_name, gg_name in PAIRS:
        if not os.path.exists(os.path.join(ref_dir, st_name + ".bin")):
            print(f"-- {st_name}: not downloaded, skipped")
            continue
        x = load_unquantized(ref_dir, st_name, header)
        t = tensors[gg_name]
        print(f"-- {st_name} {list(x.shape)}  <->  {gg_name} ({t.tensor_type.name})")
        rows = x.shape[0]
        step = max(1, rows // 4096)  # 大張量只取一部分列，夠判斷了
        sel = np.arange(0, rows, step)
        xs = np.ascontiguousarray(x[sel])
        rt = q4_0_roundtrip(xs)
        print(f"   unquantized -> Q4_0 -> back         : relative RMS error {rel_rms(xs, rt):.5f}"
              f", values unchanged {100.0 * np.mean(np.isclose(xs, rt, rtol=5e-3, atol=1e-7)):.2f}%")
        g = quants.dequantize(np.ascontiguousarray(t.data[sel]), t.tensor_type).reshape(xs.shape)
        print(f"   official GGUF ({t.tensor_type.name}) vs unquantized  : relative RMS error {rel_rms(xs, g):.5f}")
        print(f"   official GGUF vs our Q4_0 round trip : relative RMS error {rel_rms(g, rt):.5f}")
        levels = [len(np.unique(row[:32])) for row in xs[:200]]
        print(f"   distinct values in the first 32-value block of a row: median {int(np.median(levels))}, max {max(levels)}")


if __name__ == "__main__":
    main()
