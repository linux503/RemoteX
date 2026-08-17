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


def tray_pixel(x, y, size):
    xx, yy = x + 0.5, y + 0.5
    cx = cy = size / 2
    if dist(xx, yy, cx, cy) <= size * 0.13:
        return (255, 59, 85, 255)
    stroke = size * 0.09
    arms = [
        (size * 0.18, size * 0.18, cx - size * 0.12, cy - size * 0.12),
        (size * 0.82, size * 0.18, cx + size * 0.12, cy - size * 0.12),
        (size * 0.18, size * 0.82, cx - size * 0.12, cy + size * 0.12),
        (size * 0.82, size * 0.82, cx + size * 0.12, cy + size * 0.12),
    ]
    if any(line_dist(xx, yy, *arm) <= stroke for arm in arms):
        return (255, 59, 85, 255)
    return (0, 0, 0, 0)


root = Path("/Users/a503/Downloads/Mac-soft/RemoteX/apps/desktop/src-tauri/icons")
root.mkdir(parents=True, exist_ok=True)
(root / "tray.png").write_bytes(png(64, tray_pixel))
print("tray written")
