#!/usr/bin/env python3
"""用 Google 未量化 QAT 權重，把 E2B 的 GGUF 重新編碼成「和訓練時格點完全一致」的 Q4_0。

輸入：
  official.gguf : Google 發布的 Q4_0 GGUF（中繼資料、張量清單、非量化張量都從這裡原樣複製）
  ref.gguf      : 用 convert_hf_to_gguf.py --outtype bf16 從未量化 QAT 權重轉出來的 GGUF
輸出：
  dst.gguf      : official 裡 Q4_0 的張量、以及 Q6_K 的兩張詞向量表，都改用 ref 的數值做無損 Q4_0 編碼。
                  某個張量如果不在格點上（有任何區塊離格點超過 0.1 格），就保留 official 的資料並印出警告。

用法：python3 -I make_e2b_exact.py <official.gguf> <ref.gguf> <dst.gguf>
"""
import sys

import numpy as np

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
sys.path.insert(0, "/rag/experiments")
import gguf  # noqa: E402
from qat_grid import exact_q4_0, pack_q4_0  # noqa: E402

Q = gguf.GGMLQuantizationType
CHUNK_VALUES = 32 * 1024 * 1024  # 每次處理的元素數，控制暫存記憶體


def ref_to_f32(t) -> np.ndarray:
    if t.tensor_type == Q.BF16:
        raw = np.ascontiguousarray(t.data).view(np.uint16)
        return raw  # 延後轉換，逐塊處理
    raise ValueError(f"unexpected reference type {t.tensor_type.name} for {t.name}")


def encode(raw16: np.ndarray, n_rows: int, n_cols: int) -> tuple[np.ndarray, dict]:
    flat = raw16.reshape(-1)
    out = np.empty((flat.size // 32, 18), dtype=np.uint8)
    total = {"blocks": 0, "max_resid": 0.0, "blocks_off_grid": 0, "sq_err": 0.0, "sq": 0.0}
    for start in range(0, flat.size, CHUNK_VALUES):
        part = (flat[start:start + CHUNK_VALUES].astype(np.uint32) << 16).view(np.float32)
        d, q, st = exact_q4_0(part)
        out[start // 32:start // 32 + d.shape[0]] = pack_q4_0(d, q)
        total["blocks"] += st["blocks"]
        total["max_resid"] = max(total["max_resid"], st["max_resid"])
        total["blocks_off_grid"] += st["blocks_off_grid"]
        p64 = part.astype(np.float64)
        total["sq"] += float(np.sum(p64 ** 2))
        total["sq_err"] += st["rel_rms"] ** 2 * float(np.sum(p64 ** 2))
    total["rel_rms"] = (total["sq_err"] / max(total["sq"], 1e-30)) ** 0.5
    return out.reshape(n_rows, n_cols // 32 * 18), total


def main() -> None:
    official_path, ref_path, dst = sys.argv[1:4]
    official = gguf.GGUFReader(official_path)
    ref = {t.name: t for t in gguf.GGUFReader(ref_path).tensors}

    arch = official.get_field(gguf.Keys.General.ARCHITECTURE).contents()
    writer = gguf.GGUFWriter(dst, arch=arch, endianess=official.endianess)
    alignment = official.get_field(gguf.Keys.General.ALIGNMENT)
    if alignment is not None:
        writer.data_alignment = alignment.contents()
    for field in official.fields.values():
        if field.name == gguf.Keys.General.ARCHITECTURE or field.name.startswith("GGUF."):
            continue
        val_type = field.types[0]
        sub_type = field.types[-1] if val_type == gguf.GGUFValueType.ARRAY else None
        writer.add_key_value(field.name, field.contents(), val_type, sub_type=sub_type)

    out = []
    n_exact = n_kept = 0
    worst = 0.0
    for t in official.tensors:
        if t.tensor_type not in (Q.Q4_0, Q.Q6_K):
            out.append((t.name, t.data, t.tensor_type))
            continue
        r = ref.get(t.name)
        shape = [int(v) for v in t.shape]  # ggml 順序：shape[0] 是每一列的長度
        if r is None or [int(v) for v in r.shape] != shape or len(shape) != 2 or shape[0] % 32 != 0:
            print(f"KEEP   {t.name}: no usable reference tensor")
            out.append((t.name, t.data, t.tensor_type))
            n_kept += 1
            continue
        data, st = encode(ref_to_f32(r), shape[1], shape[0])
        if st["blocks_off_grid"] > 0:
            print(f"KEEP   {t.name}: {st['blocks_off_grid']} of {st['blocks']} blocks are not on the grid")
            out.append((t.name, t.data, t.tensor_type))
            n_kept += 1
            continue
        worst = max(worst, st["max_resid"])
        n_exact += 1
        if t.tensor_type == Q.Q6_K or n_exact <= 3:
            print(f"EXACT  {t.name}: was {t.tensor_type.name}, {st['blocks']} blocks, max distance {st['max_resid']:.3f} step, "
                  f"relative RMS vs reference {st['rel_rms']:.5f}", flush=True)
        out.append((t.name, data, Q.Q4_0))
    print(f"{n_exact} tensors re-encoded exactly, {n_kept} kept as released; largest distance from the grid {worst:.3f} step")

    for name, data, qtype in out:
        writer.add_tensor_info(name, data.shape, data.dtype, data.nbytes, qtype)
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    for _, data, _ in out:
        writer.write_tensor_data(data, tensor_endianess=official.endianess)
    writer.close()
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
