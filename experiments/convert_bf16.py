#!/usr/bin/env python3
"""把 GGUF 裡 BF16 的張量改存成 F32 或 F16（其他張量原樣複製），用來比較 CPU 上的運算速度。

BF16 -> F32 是完全無損的（F32 的前 16 位元就是 BF16）。
BF16 -> F16 只有在數值落在 F16 能精確表示的範圍內才無損，腳本會統計有多少數值轉換後不相等。

用法：python3 -I convert_bf16.py <src.gguf> <dst.gguf> <f32|f16>
"""
import sys

import numpy as np

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
import gguf  # noqa: E402


def main() -> None:
    src, dst, target = sys.argv[1:4]
    reader = gguf.GGUFReader(src)
    arch = reader.get_field(gguf.Keys.General.ARCHITECTURE).contents()
    writer = gguf.GGUFWriter(dst, arch=arch, endianess=reader.endianess)
    alignment = reader.get_field(gguf.Keys.General.ALIGNMENT)
    if alignment is not None:
        writer.data_alignment = alignment.contents()
    for field in reader.fields.values():
        if field.name == gguf.Keys.General.ARCHITECTURE or field.name.startswith("GGUF."):
            continue
        val_type = field.types[0]
        sub_type = field.types[-1] if val_type == gguf.GGUFValueType.ARRAY else None
        writer.add_key_value(field.name, field.contents(), val_type, sub_type=sub_type)

    out = []
    n_total = n_inexact = 0
    for t in reader.tensors:
        if t.tensor_type == gguf.GGMLQuantizationType.BF16:
            # GGUFReader 把 BF16 當成位元組陣列回傳（最後一維是 2 倍），先還原成 16 位元整數
            raw = np.ascontiguousarray(t.data).view(np.uint16)
            f32 = (raw.astype(np.uint32) << 16).view(np.float32)
            if target == "f32":
                data, qtype = f32, gguf.GGMLQuantizationType.F32
            else:
                data, qtype = f32.astype(np.float16), gguf.GGMLQuantizationType.F16
                n_total += f32.size
                n_inexact += int(np.count_nonzero(data.astype(np.float32) != f32))
            out.append((t.name, np.ascontiguousarray(data), qtype))
        else:
            out.append((t.name, t.data, t.tensor_type))
    if target == "f16":
        print(f"BF16 -> F16: {n_inexact} of {n_total} values change ({100.0 * n_inexact / max(n_total, 1):.4f}%)")

    for name, data, qtype in out:
        writer.add_tensor_info(name, data.shape, data.dtype, data.nbytes, qtype)
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    for _, data, _ in out:
        writer.write_tensor_data(data, tensor_endianess=reader.endianess)
    writer.close()
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
