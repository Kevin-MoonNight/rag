#!/usr/bin/env python3
"""查 crates.io 上套件的最新版本、更新日期、說明與原始碼位置。用法：python3 -I crates_info.py <crate> [...]"""
import json
import sys
import urllib.request

for name in sys.argv[1:]:
    try:
        req = urllib.request.Request(f"https://crates.io/api/v1/crates/{name}", headers={"User-Agent": "rag-dev (local development check)"})
        c = json.load(urllib.request.urlopen(req, timeout=30))["crate"]
        print(f"{name}: {c.get('max_stable_version') or c.get('max_version')} | updated {c.get('updated_at', '')[:10]} | downloads {c.get('downloads')} | "
              f"{(c.get('description') or '').strip()[:110]} | {c.get('repository')}")
    except Exception as e:  # noqa: BLE001
        print(f"{name}: not found or failed ({e!r})")
