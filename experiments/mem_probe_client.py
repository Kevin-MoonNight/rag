#!/usr/bin/env python3
"""mem_probe.sh 用的請求端。用法：python3 -I mem_probe_client.py <port> query|docs|image|images"""
import glob
import json
import sys

sys.path.insert(0, "/rag/experiments")
from embed_client import DOC_PREFIX, QUERY_PREFIX, embed_image, embed_texts  # noqa: E402

port, what = int(sys.argv[1]), sys.argv[2]
data = json.load(open("/rag/testdata/texts.json", encoding="utf-8"))
if what == "query":
    embed_texts(port, [QUERY_PREFIX + data["queries"][0]["text"]])
elif what == "docs":
    for d in data["docs"]:
        embed_texts(port, [DOC_PREFIX + d["text"]])
else:
    paths = sorted(glob.glob("/rag/testdata/pages/page_??.png"))
    for p in paths[: 1 if what == "image" else len(paths)]:
        embed_image(port, open(p, "rb").read())
