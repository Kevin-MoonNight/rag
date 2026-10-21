#!/usr/bin/env python3
"""Google QAT 權重的格點還原：把落在 16 階格點上的浮點權重，無損地編碼成 ggml 的 Q4_0。

觀察到的規則（inspect_qat_grid.py）：每 32 個值一組，數值都是 k × s，k 是 -8 到 7 的整數，
而且每一組絕對值最大的那個值不是 -8 × s 就是 +7 × s。
Q4_0 的還原公式是 (q - 8) × d，q 是 0 到 15，所以取 d = s、q = k + 8 就能原樣表示。

ggml 內建的 Q4_0 量化器取 d = 最大值 / -8：最大值是負的時候等於 s（無損），
最大值是正的時候變成 -7s/8，整組被重新取整，這就是官方 GGUF 和訓練數值的差異來源。
"""
import numpy as np


def exact_q4_0(x: np.ndarray) -> tuple[np.ndarray, np.ndarray, dict]:
    """x: float32，元素數是 32 的倍數。回傳 (d: float16 [n_blocks], q: uint8 [n_blocks, 32], 統計)。"""
    b = x.reshape(-1, 32).astype(np.float64)
    amax = np.max(np.abs(b), axis=1)
    ext = b[np.arange(b.shape[0]), np.argmax(np.abs(b), axis=1)]
    # 最大值是 8 格（-8s）還是 7 格（±7s）兩種都試，取比較貼近格點的那一個。
    # 不能只看正負號：同時有 -7s 和 +7s 的區塊，BF16 捨入後哪一個比較大是隨機的。
    best_k = None
    best_resid = None
    for steps in (8.0, 7.0):
        s = amax / steps
        kf = b / np.where(s > 0, s, 1.0)[:, None]
        k = np.clip(np.round(kf), -8, 7)
        r = np.abs(kf - k)
        if best_k is None:
            best_k, best_resid = k, r
        else:
            pick = r.max(axis=1) < best_resid.max(axis=1)
            best_k[pick] = k[pick]
            best_resid[pick] = r[pick]
    k, resid = best_k, best_resid
    # 重新用最小平方法估格距，抵銷 BF16 捨入帶進來的誤差
    denom = np.sum(k * k, axis=1)
    s_fit = np.where(denom > 0, np.sum(b * k, axis=1) / np.where(denom > 0, denom, 1.0), 0.0)
    d = s_fit.astype(np.float16)
    q = (k + 8).astype(np.uint8)
    back = (q.astype(np.float32) - 8.0) * d.astype(np.float32)[:, None]
    stats = {
        "blocks": int(b.shape[0]),
        "max_resid": float(resid.max()),
        "blocks_off_grid": int(np.sum(resid.max(axis=1) > 0.1)),
        "rel_rms": float(np.sqrt(np.mean((back - b) ** 2) / max(np.mean(b ** 2), 1e-30))),
        "neg_ext": float(np.mean(ext < 0)),
    }
    return d, q, stats


def pack_q4_0(d: np.ndarray, q: np.ndarray) -> np.ndarray:
    """組成 ggml block_q4_0 的位元組：2 位元組 d（fp16），接 16 位元組，第 j 個位元組低 4 位是 q[j]、高 4 位是 q[j+16]。"""
    n = d.shape[0]
    out = np.empty((n, 18), dtype=np.uint8)
    out[:, 0:2] = d.astype("<f2").view(np.uint8).reshape(n, 2)
    out[:, 2:18] = q[:, :16] | (q[:, 16:] << 4)
    return out


def ggml_q4_0_roundtrip(x: np.ndarray) -> np.ndarray:
    """ggml 內建 Q4_0 量化再還原（和 quantize_row_q4_0_ref 相同）。"""
    b = x.reshape(-1, 32)
    idx = np.argmax(np.abs(b), axis=1)
    vmax = b[np.arange(b.shape[0]), idx]
    d = vmax / -8.0
    with np.errstate(divide="ignore"):
        inv = np.where(d != 0, 1.0 / d, 0.0).astype(np.float32)
    q = np.minimum(15, np.floor(b * inv[:, None] + 8.5)).astype(np.int8)
    d16 = d.astype(np.float16).astype(np.float32)
    return ((q - 8).astype(np.float32) * d16[:, None]).reshape(x.shape)
