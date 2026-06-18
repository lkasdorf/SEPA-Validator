"""Generate the SEPA Validator app icon source PNG (1024x1024).

Run from the repo root:  python scripts/gen_icon.py
Then:                     cd app && npx tauri icon src-tauri/icons/source.png
"""
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

# Background: rounded square, app blue
d.rounded_rectangle([40, 40, S - 40, S - 40], radius=180, fill=(10, 132, 255, 255))

# Document with a folded top-right corner
dx0, dy0, dx1, dy1 = 300, 230, 724, 770
fold = 120
d.polygon(
    [(dx0, dy0), (dx1 - fold, dy0), (dx1, dy0 + fold), (dx1, dy1), (dx0, dy1)],
    fill=(255, 255, 255, 255),
)
d.polygon(
    [(dx1 - fold, dy0), (dx1 - fold, dy0 + fold), (dx1, dy0 + fold)],
    fill=(205, 225, 250, 255),
)

# Text lines on the document
for i, y in enumerate(range(330, 620, 70)):
    w = 360 if i % 3 != 2 else 220
    d.rounded_rectangle([350, y, 350 + w, y + 26], radius=13, fill=(150, 175, 205, 255))

# Green check badge (bottom-right)
cx, cy, r = 690, 700, 120
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(22, 163, 74, 255))
d.line(
    [(cx - 52, cy + 4), (cx - 12, cy + 46), (cx + 58, cy - 42)],
    fill=(255, 255, 255, 255),
    width=26,
    joint="curve",
)

img.save("app/src-tauri/icons/source.png")
print("wrote app/src-tauri/icons/source.png")
