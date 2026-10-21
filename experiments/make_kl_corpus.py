#!/usr/bin/env python3
"""組一份比較輸出機率分布用的文字：自己寫的繁體中文與英文測試文件，加上 llama.cpp 文件裡的一段英文。

用法：python3 -I make_kl_corpus.py <輸出.txt>
"""
import json
import sys

parts = []
for path in ("/rag/testdata/texts.json", "/rag/testdata/dense/texts.json"):
    data = json.load(open(path, encoding="utf-8"))
    for doc in data["docs"]:
        parts.append(doc["text"])
    for q in data.get("queries", []):
        parts.append(q["text"] if isinstance(q, dict) and "text" in q else str(q))
readme = open("/rag/vendor/llama.cpp-b11567/docs/build.md", encoding="utf-8").read()
parts.append(readme[:12000])
text = "\n\n".join(parts)
open(sys.argv[1], "w", encoding="utf-8").write(text)
print(f"{len(text)} characters, {sum(1 for c in text if ord(c) > 0x2E80)} of them CJK")
