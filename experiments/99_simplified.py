# Experiment 99: count common simplified-only characters leaking into each OCR system's output (documents are Traditional).
import glob, collections
S = set("别页员监带这们会过还与发问题门关间见说话认识样实应经进现将时动处为业务银构机个书条规关单费额币据报证务亿万计审议决复询询银财税输贷险价质类级织续统权违获组织给续让务细终")
for d in ["results/win/87_ocr/*.txt", "results/99/nemotron-cuda/*.md", "results/99/ppocrv5-server/*.txt",
          "results/99/ppocrv6-medium/*.txt", "results/99/ppocrv6-small/*.txt", "results/99/ppocrv6-medium-t/*.txt", "results/99/ppocrv6-small-t/*.txt"]:
    c = collections.Counter(ch for f in sorted(glob.glob(d)) for ch in open(f, encoding="utf-8").read() if ch in S)
    print(f"{d:40} {sum(c.values()):4} {''.join(f'{k}{v} ' for k, v in c.most_common(15))}")
