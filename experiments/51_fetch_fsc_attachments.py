"""Download PDF attachments of FSC regulations (open data, Government Open Data Licence v1).

Usage: python3 -I 51_fetch_fsc_attachments.py <max_files> [bureau ...]
Files go to /rag/testdata/realdocs/pdf/<law id>/<original name>.pdf, manifest to manifest.json.
At most one request per second.
"""
import json
import os
import re
import sys
import time
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

RAW = "/rag/testdata/realdocs/raw"
OUT = "/rag/testdata/realdocs/pdf"
MANIFEST = "/rag/testdata/realdocs/manifest.json"
UA = "Mozilla/5.0 (X11; Linux x86_64) rag-eval-fetch"
MAX_BYTES = 15 * 1024 * 1024


def clean(s):
    return re.sub(r"\s+", " ", (s or "").strip())


def candidates(bureaus):
    for bureau in bureaus:
        root = ET.parse(f"{RAW}/fsc_{bureau}.xml").getroot()
        for law in root.findall("Law"):
            if clean(law.findtext("異動性質")) in ("廢止", "停止適用"):
                continue
            for field in ("圖表附件", "立法理由"):
                node = law.find(field)
                if node is None:
                    continue
                for f in node.findall("檔案"):
                    name = clean(f.findtext("檔案名稱"))
                    url = clean(f.findtext("下載網址"))
                    if not name.lower().endswith(".pdf") or not url:
                        continue
                    q = urllib.parse.parse_qs(urllib.parse.urlparse(url).query)
                    yield {
                        "bureau": bureau,
                        "law_id": q.get("id", ["x"])[0],
                        "file_id": q.get("FileID", ["x"])[0],
                        "field": field,
                        "name": name,
                        "url": url,
                        "law_name": clean(law.findtext("法規名稱")),
                        "subject": clean(law.findtext("主旨")),
                        "doc_no": clean(law.findtext("發文字號")),
                        "category": clean(law.findtext("法規體系")),
                        "kind": clean(law.findtext("法規類別")),
                        "amended": clean(law.findtext("修正日期")),
                    }


def main():
    limit = int(sys.argv[1])
    bureaus = sys.argv[2:] or ["銀行局", "本會", "檢查局"]
    items = list(candidates(bureaus))
    print("pdf attachments in force:", len(items), flush=True)
    manifest = json.load(open(MANIFEST, encoding="utf-8")) if os.path.exists(MANIFEST) else []
    done = {m["url"] for m in manifest}
    got = len(manifest)
    for item in items:
        if got >= limit:
            break
        if item["url"] in done:
            continue
        folder = f"{OUT}/{item['law_id']}"
        os.makedirs(folder, exist_ok=True)
        safe = re.sub(r'[\\/:*?"<>|]', "_", item["name"])
        path = f"{folder}/{safe}"
        if os.path.exists(path):
            path = f"{folder}/{item['file_id']}_{safe}"
        try:
            req = urllib.request.Request(item["url"], headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=120) as resp:
                data = resp.read(MAX_BYTES + 1)
        except Exception as e:  # noqa: BLE001
            print("fail", item["url"], repr(e)[:120], flush=True)
            time.sleep(2)
            continue
        time.sleep(1.0)
        if len(data) > MAX_BYTES or not data.startswith(b"%PDF"):
            print("skip", item["name"], len(data), data[:8], flush=True)
            continue
        with open(path, "wb") as fh:
            fh.write(data)
        item["path"] = os.path.relpath(path, "/rag/testdata/realdocs")
        item["bytes"] = len(data)
        manifest.append(item)
        done.add(item["url"])
        got += 1
        if got % 20 == 0:
            print("downloaded", got, flush=True)
            json.dump(manifest, open(MANIFEST, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    json.dump(manifest, open(MANIFEST, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print("total files:", len(manifest), "bytes:", sum(m["bytes"] for m in manifest))


main()
