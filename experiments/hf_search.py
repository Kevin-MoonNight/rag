#!/usr/bin/env python3
"""查 Hugging Face 上的模型。用法：
  python3 -I hf_search.py search <關鍵字> [...]     列出符合的模型
  python3 -I hf_search.py files <repo id> [...]     列出模型庫裡的檔案與大小
"""
import json
import sys
import urllib.parse
import urllib.request


def get(url: str):
    with urllib.request.urlopen(url, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))


mode = sys.argv[1]
for arg in sys.argv[2:]:
    if mode == "search":
        res = get("https://huggingface.co/api/models?limit=30&sort=downloads&direction=-1&search=" + urllib.parse.quote(arg))
        print(f"== search: {arg} ({len(res)} results)")
        for m in res:
            print(f"  {m['id']:70s} downloads={m.get('downloads')} tags={','.join(t for t in m.get('tags', []) if t in ('gguf', 'image-text-to-text', 'ocr'))}")
    else:
        res = get(f"https://huggingface.co/api/models/{arg}/tree/main")
        print(f"== files: {arg}")
        for f in res:
            print(f"  {f['path']:70s} {f.get('size', 0)/1e6:10.1f} MB")
