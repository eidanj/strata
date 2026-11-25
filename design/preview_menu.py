# Approximate preview of the in-game Strata menu, using the same layout and
# background math as StrataTitleScreen / StrataBackground. Text uses a stand-in
# pixel font, so letter shapes differ from Minecraft's.
import math, sys
from PIL import Image, ImageDraw, ImageFont

W, H, SCALE = 427, 240, 3
img = Image.new("RGBA", (W, H), (0, 0, 0, 255))
d = ImageDraw.Draw(img)

def argb(c):
    return ((c >> 16) & 255, (c >> 8) & 255, c & 255, (c >> 24) & 255)

def fill(x0, y0, x1, y1, c):
    if x1 <= x0 or y1 <= y0:
        return
    layer = Image.new("RGBA", (x1 - x0, y1 - y0), argb(c))
    img.alpha_composite(layer, (x0, y0))

def lerp(a, b, t):
    out = 0
    for sh in (24, 16, 8, 0):
        ca, cb = (a >> sh) & 255, (b >> sh) & 255
        out |= int(ca + (cb - ca) * t) << sh
    return out

COLORS = [0xFF6A7652, 0xFFA88A5B, 0xFF3A424C, 0xFF8C5B46, 0xFF587080, 0xFF2E353D, 0xFF20252B]
THICK = [0.035, 0.075, 0.11, 0.03, 0.09, 0.12, 0.6]
seconds, mouse_x = 3.2, W * 0.62
horizon = round(H * 0.46)
for y in range(horizon + 40):  # sky gradient
    t = y / (horizon + 40)
    fill(0, y, W, y + 1, lerp(0xFF0E1115, 0xFF1F252C, t))
par = (mouse_x - W / 2) / W
for x in range(0, W, 2):
    nx = x / W
    y = horizon + (nx - 0.5) * H * 0.07
    for i, col in enumerate(COLORS):
        depth = i / len(COLORS)
        wave = math.sin(x * (0.018 - depth * 0.008) + seconds * (0.25 - depth * 0.15) + i * 1.7) * (3.5 - depth * 2.5) \
            + math.sin(x * 0.051 + seconds * 0.11 + i) * 1.2
        top = y + wave + par * (12 - depth * 10)
        y0 = round(top)
        if y0 >= H:
            break
        fill(x, y0, x + 2, H, col)
        fill(x, y0, x + 2, y0 + 1, lerp(col, 0xFFFFFFFF, 0.14))
        y = top + THICK[i] * H

PANEL = 170
fill(0, 0, PANEL, H, 0xEE161A1F)
fill(PANEL, 0, PANEL + 1, H, 0x809A86B5)

# mark
bands = [0xFF2A3038, 0xFF5F7A8C, 0xFF3A424C, 0xFFD3AE72, 0xFFA0674F, 0xFF2A3038, 0xFF7F8F64]
weights = [1.2, .6, 1.5, .8, .5, 1.1, .8]
cy, size, mx, my = 20, 18, 18, 20
for b, w in zip(bands, weights):
    y0 = round(cy); cy += w / sum(weights) * size
    fill(mx, y0, mx + size, round(cy), b)

font = ImageFont.truetype("DejaVuSans.ttf", 8) if False else None
try:
    font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 8)
    big = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 16)
except OSError:
    font = big = ImageFont.load_default()
d = ImageDraw.Draw(img)
d.fontmode = "1"

def text(x, y, s, c, f=None, shadow=True):
    f = f or font
    if shadow:
        d.text((x + 1, y + 1), s, font=f, fill=argb(lerp(c, 0xFF000000, 0.75)))
    d.text((x, y), s, font=f, fill=argb(c))

text(44, 19, "S T R A T A", 0xFFE6E3DC, big)
text(18, 44, "Minecraft 26.3 with Fabric", 0xFF9AA3AD, shadow=False)

labels = ["Singleplayer", "Multiplayer", "Options", "Crosshair", "Quit game"]
top, bottom, stack = 62, H - 40, 22 * 4 + 4 + 20
y = top + max(0, (bottom - top - stack) // 2)
for i, label in enumerate(labels):
    if i == 4:
        y += 4
    hot = i == 1
    fill(18, y, 18 + PANEL - 36, y + 20, 0x24FFFFFF if hot else 0x0AFFFFFF)
    fill(18, y, 18 + (4 if hot else 2), y + 20, 0xFF9A86B5 if hot else 0x559A86B5)
    text(28 + (3 if hot else 0), y + 6, label, 0xFFFFFFFF if hot else 0xFFD5D2CB)
    y += 22

text(18, H - 30, "Signed in as", 0xFF9AA3AD, shadow=False)
text(18 + d.textlength("Signed in as ", font=font), H - 30, "Eidan", 0xFFE6E3DC)
text(18, H - 16, "Use classic menu", 0xFF9AA3AD, shadow=False)
cr = "Copyright Mojang AB. Do not distribute!"
text(W - d.textlength(cr, font=font) - 6, H - 12, cr, 0xB3E6E3DC, shadow=False)
st = "Strata 0.1.0"
text(W - d.textlength(st, font=font) - 6, H - 23, st, 0x809AA3AD, shadow=False)

img.convert("RGB").resize((W * SCALE, H * SCALE), Image.NEAREST).save(sys.argv[1])
