#!/usr/bin/env python3
"""Render the RemoteX mark (red rounded square + disconnected white X)."""

from pathlib import Path

from PIL import Image, ImageDraw


C1 = (255, 59, 85, 255)   # #FF3B55
C2 = (155, 18, 48, 255)    # #9B1230
WHITE = (255, 255, 255, 255)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(4))


def gradient(size):
    img = Image.new("RGBA", (size, size))
    px = img.load()
    x1, y1 = 6 / 32 * size, 2 / 32 * size
    x2, y2 = 28 / 32 * size, 30 / 32 * size
    dx, dy = x2 - x1, y2 - y1
    mag = dx * dx + dy * dy
    for y in range(size):
        for x in range(size):
            t = ((x - x1) * dx + (y - y1) * dy) / mag
            t = 0 if t < 0 else 1 if t > 1 else t
            px[x, y] = lerp(C1, C2, t)
    return img


def render(size, supersample=4):
    big = size * supersample
    base = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    mask = Image.new("L", (big, big), 0)
    draw_mask = ImageDraw.Draw(mask)
    left = 1 / 32 * big
    right = 31 / 32 * big - 1
    radius = 8 / 32 * big
    draw_mask.rounded_rectangle([left, left, right, right], radius=radius, fill=255)
    base.paste(gradient(big), mask=mask)

    draw = ImageDraw.Draw(base)

    def xy(x, y):
        return x / 32 * big, y / 32 * big

    stroke = 2.7 / 32 * big
    cap = stroke / 2
    for x1, y1, x2, y2 in (
        (9.2, 9.2, 13.05, 13.05),
        (22.8, 9.2, 18.95, 13.05),
        (9.2, 22.8, 13.05, 18.95),
        (22.8, 22.8, 18.95, 18.95),
    ):
        p1, p2 = xy(x1, y1), xy(x2, y2)
        draw.line([p1, p2], fill=WHITE, width=max(1, round(stroke)))
        for px, py in (p1, p2):
            draw.ellipse([px - cap, py - cap, px + cap, py + cap], fill=WHITE)

    cx, cy = xy(16, 16)
    cr = 2.15 / 32 * big
    draw.ellipse([cx - cr, cy - cr, cx + cr, cy + cr], fill=WHITE)
    return base.resize((size, size), Image.Resampling.LANCZOS)


root = Path(__file__).resolve().parent
docs = root.parents[3] / "docs"
source = render(1024)
source.save(root / "icon.png", optimize=True)
render(64).save(root / "tray.png", optimize=True)
source.save(docs / "icon.png", optimize=True)
print("wrote icon.png, tray.png, docs/icon.png")
