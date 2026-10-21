# Experiment 99: Nemotron 3.5 Parse on the 36 real scanned pages of experiment 87 (same 110 dpi page images).
# Writes results/99/nemotron-<device>/NNN.raw.txt (model output) and NNN.md (official postprocessing -> Markdown),
# and times.csv. Pages already done are skipped, so the run can be stopped anytime.
# Usage (in rag-ocr-box): /opt/venv/bin/python experiments/99_nemotron_ocr.py cuda [limit]
#                         /opt/venv/bin/python experiments/99_nemotron_ocr.py cpu 3
import json
import os
import sys
import time

import torch
from PIL import Image

MODEL = "/rag/models/ocr/nemotron-3.5-parse"
sys.path.insert(0, MODEL)
from transformers import AutoModel, AutoProcessor, GenerationConfig  # noqa: E402
from postprocessing import postprocess_document  # noqa: E402

device = sys.argv[1]
limit = int(sys.argv[2]) if len(sys.argv) > 2 else None
out = f"/rag/results/99/nemotron-{device}"
os.makedirs(out, exist_ok=True)
pages = json.load(open("/rag/work/87_scan_pages.json", encoding="utf-8"))[:limit]

dtype = torch.bfloat16 if device == "cuda" else torch.float32
if device == "cpu":
    torch.set_num_threads(6)
t0 = time.time()
model = AutoModel.from_pretrained(MODEL, trust_remote_code=True, dtype=dtype).to(device).eval()
processor = AutoProcessor.from_pretrained(MODEL, trust_remote_code=True)
config = GenerationConfig.from_pretrained(MODEL)
config.max_new_tokens = 8744
print(f"load {time.time() - t0:.1f} s", flush=True)
prompt = "</s><s><predict_bbox><predict_classes><output_markdown><predict_text_in_pic>"

times = open(f"{out}/times.csv", "a", encoding="utf-8")
for n, page in enumerate(pages):
    if os.path.exists(f"{out}/{n:03d}.md"):
        continue
    image = Image.open(page["image"]).convert("RGB")
    t = time.time()
    inputs = processor(images=[image], text=prompt, return_tensors="pt", add_special_tokens=False).to(device)
    with torch.inference_mode():
        output = model.generate(**inputs, generation_config=config)
    seconds = time.time() - t
    raw = processor.batch_decode(output, skip_special_tokens=False)[0]
    markdown = postprocess_document(raw, image_size=image.size)["markdown"]
    open(f"{out}/{n:03d}.raw.txt", "w", encoding="utf-8").write(raw)
    open(f"{out}/{n:03d}.md", "w", encoding="utf-8").write(markdown)
    tokens = output.shape[-1]
    times.write(f"{n},{image.size[0]}x{image.size[1]},{tokens},{seconds:.2f}\n")
    times.flush()
    print(f"{n:03d} {image.size} {tokens} tokens {seconds:.1f} s {len(markdown)} chars", flush=True)
