#!/usr/bin/env python3
"""列出 GGUF 檔的重點中繼資料，以及依「張量類別 x 量化格式」彙總的大小。

用法：python3 -I gguf_summary.py <file.gguf> [--tensors]
"""
import re
import sys
from collections import defaultdict

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
from gguf import GGUFReader  # noqa: E402


def group_of(name: str) -> str:
    name = re.sub(r"\.\d+\.", ".N.", name)
    name = re.sub(r"\.\d+$", ".N", name)
    return name


def main() -> None:
    path = sys.argv[1]
    show_tensors = "--tensors" in sys.argv
    r = GGUFReader(path)

    print(f"== {path}")
    for key, field in r.fields.items():
        if key.startswith("tokenizer.") and key not in ("tokenizer.ggml.model", "tokenizer.ggml.pre"):
            continue
        if key.startswith("GGUF."):
            continue
        try:
            val = field.contents()
        except Exception:
            val = "?"
        s = str(val)
        if len(s) > 110:
            s = s[:110] + "..."
        print(f"  {key} = {s}")

    groups = defaultdict(lambda: [0, 0, 0])  # (group, type) -> [count, n_params, n_bytes]
    by_type = defaultdict(lambda: [0, 0])
    total_p = total_b = 0
    for t in r.tensors:
        n_params = 1
        for d in t.shape:
            n_params *= int(d)
        tname = t.tensor_type.name
        g = groups[(group_of(t.name), tname)]
        g[0] += 1
        g[1] += n_params
        g[2] += int(t.n_bytes)
        by_type[tname][0] += n_params
        by_type[tname][1] += int(t.n_bytes)
        total_p += n_params
        total_b += int(t.n_bytes)
        if show_tensors:
            print(f"    {t.name:60s} {tname:6s} {list(map(int, t.shape))} {int(t.n_bytes)}")

    print(f"\n  {'tensor group':58s} {'type':6s} {'n':>4s} {'params(M)':>10s} {'MB':>9s} {'bits/w':>6s}")
    for (g, tname), (cnt, n_params, n_bytes) in sorted(groups.items(), key=lambda kv: -kv[1][2]):
        if n_bytes < 1_000_000:
            continue
        print(f"  {g:58s} {tname:6s} {cnt:4d} {n_params/1e6:10.1f} {n_bytes/1e6:9.1f} {n_bytes*8/n_params:6.2f}")
    print("\n  by type:")
    for tname, (n_params, n_bytes) in sorted(by_type.items(), key=lambda kv: -kv[1][1]):
        print(f"    {tname:6s} {n_params/1e6:10.1f} M params {n_bytes/1e6:9.1f} MB")
    print(f"  TOTAL {total_p/1e6:.1f} M params, {total_b/1e6:.1f} MB tensor data\n")


if __name__ == "__main__":
    main()
