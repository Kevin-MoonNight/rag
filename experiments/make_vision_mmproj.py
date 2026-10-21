#!/usr/bin/env python3
"""從圖片加聲音的編碼器檔（mmproj GGUF）產生只含圖片編碼器的版本。

聲音編碼器的張量（a.*、mm.a.*）和中繼資料（clip.audio.*）整個拿掉，
圖片編碼器的張量原樣逐位元組複製，數值完全不變。

用法：python3 -I make_vision_mmproj.py <src.gguf> <dst.gguf>
"""
import sys

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
import gguf  # noqa: E402


def is_audio(name: str) -> bool:
    return name.startswith("a.") or name.startswith("mm.a.")


def main() -> None:
    src, dst = sys.argv[1:3]
    reader = gguf.GGUFReader(src)
    arch = reader.get_field(gguf.Keys.General.ARCHITECTURE).contents()
    writer = gguf.GGUFWriter(dst, arch=arch, endianess=reader.endianess)

    alignment = reader.get_field(gguf.Keys.General.ALIGNMENT)
    if alignment is not None:
        writer.data_alignment = alignment.contents()

    for field in reader.fields.values():
        if field.name == gguf.Keys.General.ARCHITECTURE or field.name.startswith("GGUF."):
            continue
        if field.name.startswith("clip.audio."):
            continue
        if field.name == "clip.has_audio_encoder":
            writer.add_bool(field.name, False)
            continue
        val_type = field.types[0]
        sub_type = field.types[-1] if val_type == gguf.GGUFValueType.ARRAY else None
        writer.add_key_value(field.name, field.contents(), val_type, sub_type=sub_type)

    keep = [t for t in reader.tensors if not is_audio(t.name)]
    drop = [t for t in reader.tensors if is_audio(t.name)]
    print(f"keep {len(keep)} tensors ({sum(int(t.n_bytes) for t in keep)/1e6:.1f} MB), "
          f"drop {len(drop)} audio tensors ({sum(int(t.n_bytes) for t in drop)/1e6:.1f} MB)")

    for t in keep:
        writer.add_tensor_info(t.name, t.data.shape, t.data.dtype, t.data.nbytes, t.tensor_type)
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_ti_data_to_file()
    for t in keep:
        writer.write_tensor_data(t.data, tensor_endianess=reader.endianess)
    writer.close()
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
