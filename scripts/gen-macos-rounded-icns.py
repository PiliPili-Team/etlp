#!/usr/bin/env python3
"""Generate a macOS-style rounded (squircle) .icns from the source artwork.

macOS shows a third-party app's `.icns` content verbatim — it applies no
rounding of its own. The shipped `icon.icns` is a full-bleed square, so it
renders with hard corners. This script bakes the macOS "squircle" (a
superellipse, not a plain rounded rectangle) plus the standard ~10% transparent
margin into a separate `icon-macos-rounded.icns`, used only by the macOS x86
build so it matches Apple's icon grid.

Usage:
    python3 scripts/gen-macos-rounded-icns.py

Output:
    etlp-gui/src-tauri/icons/icon-macos-rounded.icns
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

# Apple's macOS icon grid (1024 px canvas): the rounded body is 824×824,
# centred, leaving a 100 px margin for the system drop shadow. The corner is a
# squircle approximated by a superellipse |x|^N + |y|^N ≤ 1 with N ≈ 5.
CANVAS = 1024
BODY = 824
MARGIN = (CANVAS - BODY) // 2
SUPERELLIPSE_N = 5.0
SUPERSAMPLE = 4  # render the mask at 4× then downsample for smooth edges

# iconutil .iconset members: (filename, pixel size).
ICONSET = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def squircle_mask(size: int) -> Image.Image:
    """Return an L-mode superellipse mask of the given square size."""
    coords = np.arange(size)
    axis = (size - 1) / 2.0
    norm = (coords - axis) / axis
    nx = np.abs(norm)[np.newaxis, :] ** SUPERELLIPSE_N
    ny = np.abs(norm)[:, np.newaxis] ** SUPERELLIPSE_N
    inside = (nx + ny) <= 1.0
    return Image.fromarray((inside * 255).astype(np.uint8))


def build_master(source: Path) -> Image.Image:
    """Return the 1024×1024 squircle-masked, margin-padded master image."""
    hi = BODY * SUPERSAMPLE
    art = Image.open(source).convert("RGBA").resize((hi, hi), Image.LANCZOS)
    mask = squircle_mask(hi)
    # Intersect the artwork's own alpha with the squircle so transparent source
    # pixels stay transparent.
    alpha = Image.composite(art.getchannel("A"), Image.new("L", (hi, hi), 0), mask)
    art.putalpha(alpha)

    body = art.resize((BODY, BODY), Image.LANCZOS)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    canvas.paste(body, (MARGIN, MARGIN), body)
    return canvas


def main() -> int:
    root = repo_root()
    icons = root / "etlp-gui" / "src-tauri" / "icons"
    source = icons / "source.png"
    if not source.exists():
        print(f"error: source artwork not found: {source}", file=sys.stderr)
        return 1

    master = build_master(source)

    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        iconset.mkdir()
        for name, size in ICONSET:
            master.resize((size, size), Image.LANCZOS).save(iconset / name)
        out = icons / "icon-macos-rounded.icns"
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(out)],
            check=True,
        )
        print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
