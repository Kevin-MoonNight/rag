#!/usr/bin/env python3
"""印出 Hugging Face 模型庫標示的授權。用法：python3 -I hf_license.py <repo> [...]"""
import json
import sys
import urllib.request

for repo in sys.argv[1:]:
    try:
        d = json.load(urllib.request.urlopen(f"https://huggingface.co/api/models/{repo}", timeout=30))
        card = d.get("cardData") or {}
        print(repo, "| tags:", [t for t in d.get("tags", []) if t.startswith("license")], "| license:", card.get("license"), "| link:", card.get("license_link"))
    except Exception as e:  # noqa: BLE001
        print(repo, "| failed:", repr(e))
