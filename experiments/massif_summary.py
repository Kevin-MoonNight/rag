#!/usr/bin/env python3
"""整理 valgrind massif 的輸出：列出最後一個詳細快照裡，各個配置來源佔多少堆積記憶體。

每個來源取呼叫堆疊裡第一個看起來屬於 llama.cpp 的函式（跳過 std:: 與配置器本身）。
用法：python3 -I massif_summary.py <massif.out>
"""
import re
import sys

snapshots = open(sys.argv[1], encoding="utf-8", errors="replace").read().split("#-----------\nsnapshot=")
detailed = [s for s in snapshots if "heap_tree=detailed" in s or "heap_tree=peak" in s]
last = detailed[-1]
heap = int(re.search(r"mem_heap_B=(\d+)", last).group(1))
extra = int(re.search(r"mem_heap_extra_B=(\d+)", last).group(1))
print(f"snapshot {last.split()[0]}: heap {heap / 1e6:.1f} MB + allocator overhead {extra / 1e6:.1f} MB")

SKIP = ("std::", "operator new", "__gnu_cxx", "malloc", "void std", "nlohmann", "_M_")
totals: dict[str, int] = {}
stack: list[tuple[int, str]] = []
lines = [ln for ln in last.splitlines() if re.match(r"^\s*n\d+: ", ln)]
for i, ln in enumerate(lines):
    depth = len(ln) - len(ln.lstrip(" "))
    m = re.match(r"\s*n(\d+): (\d+) (.*)", ln)
    n_children, size, rest = int(m.group(1)), int(m.group(2)), m.group(3)
    fn = re.sub(r"^0x[0-9A-F]+: ", "", rest)
    fn = re.sub(r" \(.*?\)$", "", fn)
    while stack and stack[-1][0] >= depth:
        stack.pop()
    stack.append((depth, fn))
    if n_children == 0:
        owner = next((f for _, f in stack if not f.startswith(SKIP) and "in " not in f[:3]), stack[-1][1])
        owner = re.sub(r"\(.*", "", owner)[:90]
        totals[owner] = totals.get(owner, 0) + size
for owner, size in sorted(totals.items(), key=lambda kv: -kv[1])[:14]:
    print(f"  {size / 1e6:7.1f} MB  {owner}")
