#!/usr/bin/env python3
import math
import struct
import zlib
from pathlib import Path


def png(size, pixel):
    rows = []
    for y in range(size):
        row = [0]
        for x in range(size):
            row.extend(pixel(x, y, size))
        rows.append(bytes(row))
    raw = b"".join(rows)
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", ihdr),
            chunk(b"IDAT", zlib.compress(raw, 9)),
            chunk(b"IEND", b""),
        ]
    )


def dist(x, y, x2, y2):
    return math.hypot(x - x2, y - y2)


def line_dist(px, py, x1, y1, x2, y2):
    vx, vy = x2 - x1, y2 - y1
    mag = vx * vx + vy * vy
    t = 0 if mag == 0 else max(0, min(1, ((px - x1) * vx + (py - y1) * vy) / mag))
    return math.hypot(px - (x1 + t * vx), py - (y1 + t * vy))


def icon_pixel(x, y, size):
    p = size * 0.12
    r = size * 0.22
    inset = False
    xx, yy = x + 0.5, y + 0.5
    if p + r <= xx <= size - p - r or p + r <= yy <= size - p - r:
        if p <= xx <= size - p and p <= yy <= size - p:
            inset = True
    else:
        corners = [
            (p + r, p + r),
            (size - p - r, p + r),
            (p + r, size - p - r),
            (size - p - r, size - p - r),
        ]
        inset = any(dist(xx, yy, cx, cy) <= r for cx, cy in corners) and (
            p <= xx <= size - p and p <= yy <= size - p
        )
    if not inset:
        return (0, 0, 0, 0)
    stroke = size * 0.045
    pad = size * 0.30
    on_x = min(
        line_dist(xx, yy, pad, pad, size - pad, size - pad),
        line_dist(xx, yy, size - pad, pad, pad, size - pad),
    ) <= stroke
    if on_x:
        return (250, 250, 250, 255)
    return (17, 17, 19, 255)


def tray_pixel(x, y, size):
    xx, yy = x + 0.5, y + 0.5
    stroke = size * 0.11
    pad = size * 0.18
    on_x = min(
        line_dist(xx, yy, pad, pad, size - pad, size - pad),
        line_dist(xx, yy, size - pad, pad, pad, size - pad),
    ) <= stroke
    if on_x:
        return (32, 32, 36, 255)
    return (0, 0, 0, 0)


root = Path("/Users/a503/Downloads/Mac-soft/RemoteX/apps/desktop/src-tauri/icons")
root.mkdir(parents=True, exist_ok=True)
(root / "icon.png").write_bytes(png(1024, icon_pixel))
(root / "32x32.png").write_bytes(png(32, icon_pixel))
(root / "128x128.png").write_bytes(png(128, icon_pixel))
(root / "128x128@2x.png").write_bytes(png(256, icon_pixel))
(root / "tray.png").write_bytes(png(32, tray_pixel))
print("icons written", root)
