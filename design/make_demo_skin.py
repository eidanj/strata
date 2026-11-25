# An original 64x64 demo skin used only by the browser preview (mock data).
from PIL import Image
im = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
px = im.load()
def rect(x, y, w, h, c):
    for i in range(x, x + w):
        for j in range(y, y + h):
            px[i, j] = c
skin, hair, eye, white = (214, 170, 138, 255), (58, 40, 30, 255), (40, 52, 70, 255), (236, 233, 226, 255)
hood, stripe, pants, shoe = (44, 52, 61, 255), (211, 174, 114, 255), (36, 40, 46, 255), (220, 218, 210, 255)
# head front
rect(8, 8, 8, 8, skin); rect(8, 8, 8, 2, hair); rect(8, 10, 1, 2, hair); rect(15, 10, 1, 2, hair)
rect(10, 12, 1, 1, white); rect(11, 12, 1, 1, eye); rect(13, 12, 1, 1, eye); rect(14, 12, 1, 1, white)
rect(11, 14, 3, 1, (176, 120, 100, 255))
# body front: hoodie with a sandstone band
rect(20, 20, 8, 12, hood); rect(20, 25, 8, 2, stripe); rect(23, 20, 2, 3, (30, 36, 42, 255))
# arms front (right at 44,20; left at 36,52)
for x, y in ((44, 20), (36, 52)):
    rect(x, y, 4, 12, hood); rect(x, y + 5, 4, 2, stripe); rect(x, y + 10, 4, 2, skin)
# legs front (right at 4,20; left at 20,52)
for x, y in ((4, 20), (20, 52)):
    rect(x, y, 4, 12, pants); rect(x, y + 10, 4, 2, shoe)
im.save("demo-skin.png")
