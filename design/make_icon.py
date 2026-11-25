# Generates the Strata app icon: a rounded slab cut through layered strata.
import sys
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([64, 64, S - 64, S - 64], radius=196, fill=255)

slab = Image.new("RGBA", (S, S), (22, 26, 31, 255))
d = ImageDraw.Draw(slab)
# Bands from top to bottom: (thickness, colour). Irregular on purpose.
bands = [
    (150, (42, 48, 56)),     # schist
    (70, (95, 122, 140)),    # shale blue
    (190, (58, 66, 76)),     # slate
    (96, (211, 174, 114)),   # sandstone (accent)
    (60, (160, 103, 79)),    # oxide
    (130, (42, 48, 56)),
    (90, (127, 143, 100)),   # lichen
]
y = 150
for i, (h, col) in enumerate(bands):
    # each layer dips slightly to the right, like tilted sediment
    tilt = 26 if i % 2 == 0 else 14
    d.polygon([(0, y), (S, y + tilt), (S, y + tilt + h), (0, y + h)], fill=col + (255,))
    y += h
img.paste(slab, (0, 0), mask)
img.save(sys.argv[1])
