#!/usr/bin/env python3
"""Regenerate RemoteX icons from docs/logo.svg (single source of truth)."""

import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent
desktop = root.parents[1]
repo = root.parents[3]
logo = repo / "docs" / "logo.svg"

subprocess.run(
    ["npm", "run", "tauri", "--", "icon", str(logo), "-o", str(root)],
    cwd=desktop,
    check=True,
)
subprocess.run(
    ["sips", "-z", "64", "64", str(root / "icon.png"), "--out", str(root / "tray.png")],
    check=True,
)
subprocess.run(["cp", str(root / "icon.png"), str(repo / "docs" / "icon.png")], check=True)
print("wrote src-tauri/icons/*, tray.png, docs/icon.png")
