"""List attachment entries (圖表附件 / 立法理由 files) in the FSC open-data XML."""
import collections
import glob
import xml.etree.ElementTree as ET

RAW = "/rag/testdata/realdocs/raw"

ext_count = collections.Counter()
shown = 0
for path in sorted(glob.glob(f"{RAW}/fsc_*.xml")):
    root = ET.parse(path).getroot()
    n_files = 0
    for law in root.findall("Law"):
        for field in ("圖表附件", "立法理由"):
            node = law.find(field)
            if node is None:
                continue
            for f in node.findall("檔案"):
                n_files += 1
                parts = {c.tag: (c.text or "").strip() for c in f}
                name = " ".join(parts.values())
                ext = name.rsplit(".", 1)[-1].lower()[:5] if "." in name else "?"
                ext_count[(field, ext)] += 1
                if shown < 6:
                    shown += 1
                    print(path.split("/")[-1], field, parts, ET.tostring(f, encoding="unicode")[:300])
    print("==", path.split("/")[-1], "attachment files:", n_files)
print(sorted(ext_count.items(), key=lambda kv: -kv[1]))
