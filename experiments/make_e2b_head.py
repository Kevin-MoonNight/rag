#!/usr/bin/env python3
"""幫 Gemma 4 E2B 的 GGUF 加上「只含指定 token 的輸出層」。

原檔的輸入詞向量表和輸出層是同一個張量（token_embd.weight）。這支腳本：
  1. 原樣複製原檔的所有中繼資料和張量。
  2. 從 token_embd.weight 取出要保留的 token 那幾列，另存成 output.weight。
     每一列剛好是整數個量化區塊，所以是逐位元組複製，不重新量化，數值完全不變。
  3. 寫入 rag.output_token_ids（保留的 token id，遞增）。

修改後的 llama.cpp 讀到 rag.output_token_ids 時，輸出層改用 output.weight，
token_embd.weight 只用來查表（可留在 SSD），沒保留的 token 的 logit 一律是 -inf。

用法：python3 -I make_e2b_head.py <src.gguf> <dst.gguf> <classes.tsv> <class,class,...>
  classes.tsv 由 vocab_stats.py --dump 產生。
"""
import sys

import numpy as np

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
import gguf  # noqa: E402


def main() -> None:
    src, dst, cls_path, keep_arg = sys.argv[1:5]
    keep_classes = set(keep_arg.split(","))

    keep_ids = []
    with open(cls_path, encoding="utf-8") as f:
        for line in f:
            tid, c = line.rstrip("\n").split("\t")
            if c in keep_classes:
                keep_ids.append(int(tid))
    keep_ids.sort()
    print(f"keep {len(keep_ids)} tokens ({', '.join(sorted(keep_classes))})")

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

    writer.add_key_value("rag.output_token_ids", keep_ids, gguf.GGUFValueType.ARRAY, sub_type=gguf.GGUFValueType.INT32)

    tok = next(t for t in reader.tensors if t.name == "token_embd.weight")
    n_vocab = int(tok.shape[1])
    assert tok.data.shape[0] == n_vocab, f"unexpected token_embd data shape {tok.data.shape}"
    assert keep_ids[-1] < n_vocab
    head = np.ascontiguousarray(tok.data[np.asarray(keep_ids, dtype=np.int64)])
    print(f"token_embd.weight: {tok.tensor_type.name} {tuple(map(int, tok.shape))}, {tok.n_bytes/1e6:.1f} MB")
    print(f"output.weight    : {tok.tensor_type.name} ({int(tok.shape[0])}, {len(keep_ids)}), {head.nbytes/1e6:.1f} MB")

    for t in reader.tensors:
        writer.add_tensor_info(t.name, t.data.shape, t.data.dtype, t.data.nbytes, t.tensor_type)
    writer.add_tensor_info("output.weight", head.shape, head.dtype, head.nbytes, tok.tensor_type)

    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    for t in reader.tensors:
        writer.write_tensor_data(t.data, tensor_endianess=reader.endianess)
    writer.write_tensor_data(head, tensor_endianess=reader.endianess)
    writer.close()
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
