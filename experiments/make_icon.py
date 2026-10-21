"""Draw a simple application icon (magnifying glass on a blue square) and save it as .ico and .png.

Run inside the rag-ref container (has Pillow):
  docker run --rm -v "${PWD}:/rag" -w /tmp rag-ref python /rag/experiments/make_icon.py
"""
import os

from PIL import Image, ImageDraw

OUT = "/rag/app/desktop/src-tauri/icons"
os.makedirs(OUT, exist_ok=True)

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle([40, 40, S - 40, S - 40], radius=190, fill=(31, 95, 191, 255))
# page
d.rounded_rectangle([250, 190, 700, 800], radius=36, fill=(255, 255, 255, 255))
for i, y in enumerate(range(300, 700, 80)):
    d.rounded_rectangle([320, y, 630 - (i % 2) * 90, y + 28], radius=14, fill=(190, 205, 228, 255))
# magnifying glass
cx, cy, r = 640, 640, 150
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(255, 255, 255, 255), outline=(20, 50, 110, 255), width=46)
d.line([cx + 105, cy + 105, cx + 230, cy + 230], fill=(20, 50, 110, 255), width=64)
d.ellipse([cx + 198, cy + 198, cx + 262, cy + 262], fill=(20, 50, 110, 255))

img.save(f"{OUT}/icon.png")
img.save(f"{OUT}/icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
for size in (32, 128):
    img.resize((size, size), Image.LANCZOS).save(f"{OUT}/{size}x{size}.png")
print(sorted(os.listdir(OUT)))
