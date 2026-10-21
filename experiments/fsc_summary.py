"""Summarise the FSC open-data XML files: record counts, text lengths, attachment fields."""
import glob
import statistics
import sys
import xml.etree.ElementTree as ET

RAW = "/rag/testdata/realdocs/raw"


def main():
    show = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    for path in sorted(glob.glob(f"{RAW}/fsc_*.xml")):
        root = ET.parse(path).getroot()
        laws = root.findall("Law")
        print(f"== {path.split('/')[-1]}  records={len(laws)}")
        fields = {}
        for law in laws:
            for child in law:
                fields.setdefault(child.tag, 0)
                if (child.text or "").strip() or len(child):
                    fields[child.tag] += 1
        print("   fields(non-empty):", fields)
        lens = [len((law.findtext("法規內容") or "")) for law in laws]
        if lens:
            lens.sort()
            print("   content chars: median", statistics.median(lens), "p90", lens[int(len(lens) * 0.9)], "max", lens[-1], "total", sum(lens))
        kinds = {}
        status = {}
        for law in laws:
            kinds[law.findtext("法規類別")] = kinds.get(law.findtext("法規類別"), 0) + 1
            status[law.findtext("生效狀態")] = status.get(law.findtext("生效狀態"), 0) + 1
        print("   kinds:", kinds, " status:", status)
        for law in laws[:show]:
            print("   ---")
            for child in law:
                text = (child.text or "").strip().replace("\r", "")
                sub = [(c.tag, (c.text or "")[:100]) for c in child]
                print(f"   {child.tag}: {text[:260]!r} {sub if sub else ''}")


main()
