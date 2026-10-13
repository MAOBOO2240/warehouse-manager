"""產生應用程式圖示（貨架造型）到 src-tauri/icons/。打包前執行一次即可：python scripts/make_icons.py"""
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
out.mkdir(parents=True, exist_ok=True)
S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle([40, 40, S - 40, S - 40], radius=180, fill=(29, 42, 51, 255))
d.rectangle([200, 220, 250, 820], fill=(120, 140, 155, 255))
d.rectangle([S - 250, 220, S - 200, 820], fill=(120, 140, 155, 255))
for y in (470, 800):
    d.rectangle([200, y, S - 200, y + 40], fill=(242, 183, 5, 255))
box = (230, 236, 240, 255)
for r in ([290, 300, 480, 470], [520, 350, 700, 470], [330, 640, 560, 800], [600, 680, 740, 800]):
    d.rectangle(r, fill=box)
img.save(out / "icon.png")
for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
    img.resize((size, size), Image.LANCZOS).save(out / name)
img.save(out / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("icons written to", out)
