#!/usr/bin/env python3
"""Generate the macOS and Linux tray icons from the Liyue emblem artwork.

The shared source is a square, transparent-background PNG of the emblem in
white (`tray-source.png`). Two platform variants are produced from it:

* macOS (`tray-icon.png`): a *template* image — pure black pixels carrying the
  emblem's alpha. macOS recolours template images to match the light/dark menu
  bar, so only the alpha mask matters and black is the convention.
* Linux (`tray-icon-linux.png`): the emblem kept white, since Linux tray areas
  do not recolour icons and a white glyph stays visible on the common dark
  panel.

Both are rendered onto a 44x44 canvas (the @2x size of a 22pt menu-bar slot)
with a small transparent margin so the glyph is not flush against the edges.

Windows intentionally keeps the full-colour app logo (`32x32.png`) and is not
touched here.

Usage:
    python3 scripts/gen-tray-icons.py

Output:
    etlp-gui/src-tauri/icons/tray-icon.png        (macOS template)
    etlp-gui/src-tauri/icons/tray-icon-linux.png  (Linux, white)
"""

from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

# Final canvas size: @2x of a 22pt macOS menu-bar slot.
CANVAS = 44
# Glyph box inside the canvas, leaving a ~3px transparent margin per side.
GLYPH = 38


def repo_root() -> Path:
    """Return the repository root (the parent of this script's directory)."""
    return Path(__file__).resolve().parent.parent


def fit_glyph(source: Image.Image) -> Image.Image:
    """Scale `source` to fit GLYPH and centre it on a transparent CANVAS."""
    glyph = source.convert("RGBA")
    glyph.thumbnail((GLYPH, GLYPH), Image.LANCZOS)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    offset = ((CANVAS - glyph.width) // 2, (CANVAS - glyph.height) // 2)
    canvas.alpha_composite(glyph, offset)
    return canvas


def to_template(image: Image.Image) -> Image.Image:
    """Recolour every pixel to black while preserving the alpha channel."""
    r, g, b, a = image.split()
    black = Image.new("L", image.size, 0)
    return Image.merge("RGBA", (black, black, black, a))


def main() -> int:
    icons = repo_root() / "etlp-gui" / "src-tauri" / "icons"
    source_path = icons / "tray-source.png"
    if not source_path.exists():
        print(f"error: missing source artwork {source_path}", file=sys.stderr)
        return 1

    source = Image.open(source_path)
    glyph = fit_glyph(source)

    # Linux: keep the white emblem as-is.
    linux_path = icons / "tray-icon-linux.png"
    glyph.save(linux_path)
    print(f"    Generated {linux_path.relative_to(repo_root())}")

    # macOS: black template carrying the emblem's alpha.
    macos_path = icons / "tray-icon.png"
    to_template(glyph).save(macos_path)
    print(f"    Generated {macos_path.relative_to(repo_root())}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
