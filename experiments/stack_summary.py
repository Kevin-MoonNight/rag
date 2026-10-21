#!/usr/bin/env python3
"""統計 gdb 「thread apply all bt」輸出裡，各執行緒停在哪個函式（最內層與往外第一個 ggml 運算）。

用法：python3 -I stack_summary.py <stacks.txt>
"""
import collections
import re
import sys

leaf = collections.Counter()
op = collections.Counter()
frames: list[str] = []


def flush() -> None:
    if not frames:
        return
    names = [re.sub(r"<.*", "", f) for f in frames]
    if any("futex" in n or "pthread_cond" in n or "poll" in n or "accept" in n or "nanosleep" in n for n in names[:3]):
        leaf["(waiting)"] += 1
    else:
        leaf[names[0]] += 1
        for n in names:
            if n.startswith("ggml_compute_forward") or "tinyBLAS" in n or n.startswith("llamafile"):
                op[n] += 1
                break
        else:
            op["(other) " + names[min(1, len(names) - 1)]] += 1
    frames.clear()


for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    if line.startswith("Thread "):
        flush()
        continue
    m = re.match(r"#\d+\s+(?:0x[0-9a-f]+ in )?([^\s(]+)", line)
    if m:
        frames.append(m.group(1))
flush()

for title, counter in (("innermost function", leaf), ("ggml operation", op)):
    busy = sum(v for k, v in counter.items() if k != "(waiting)")
    print(f"== {title} (busy samples: {busy})")
    for name, count in counter.most_common(25):
        if name == "(waiting)":
            continue
        print(f"  {100.0 * count / max(busy, 1):5.1f}%  {name}")
