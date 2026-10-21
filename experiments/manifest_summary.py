"""Summarise the downloaded attachment manifest."""
import collections
import json
import sys

m = json.load(open("/rag/testdata/realdocs/manifest.json", encoding="utf-8"))
print("files", len(m), "MB", round(sum(x["bytes"] for x in m) / 1e6, 1))
print("by bureau", collections.Counter(x["bureau"] for x in m))
print("by field", collections.Counter(x["field"] for x in m))
print("by category", collections.Counter(x["category"] for x in m).most_common(40))
sizes = sorted(x["bytes"] for x in m)
print("size median", sizes[len(sizes) // 2], "p90", sizes[int(len(sizes) * 0.9)], "max", sizes[-1])
n = int(sys.argv[1]) if len(sys.argv) > 1 else 30
for x in m[:: max(1, len(m) // n)]:
    print(f'{x["bytes"]:>9} {x["path"]} | {x["law_name"] or x["subject"][:40]}')
