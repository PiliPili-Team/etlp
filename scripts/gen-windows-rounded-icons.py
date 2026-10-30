#!/usr/bin/env python3
"""Generate macOS-style rounded (squircle) Windows icons from the artwork.

Windows does not round app icons itself, and the shipped `icon.ico` / tray PNG
are hard-cornered squares of the full-bleed artwork. This bakes the same
superellipse "squircle" used for the macOS build into:

* `icon.ico` — the Windows app/taskbar icon (multi-resolution: 16, 24, 32, 48,
  64, 128, 256). Uses standard Windows hybrid ICO format: uncompressed 32-bit
  DIB for frames < 256, PNG compression for 256.
* `icon-windows-rounded.png` — 256x256 high-resolution rounded PNG set on the
  Win32 window at runtime so the taskbar button is crystal clear at all DPIs.
* `tray-icon-windows.png` — the system-tray icon. The tray cannot reuse
  `32x32.png` because that file is also a shared bundle-icon source; a dedicated
  file keeps the rounding off the other platforms.

The app icon uses a small (~3.5%) transparent margin so it reads as a rounded
tile while still filling its slot; the tiny tray icon is rounded almost edge to
edge.

Usage:
    python3 scripts/gen-windows-rounded-icons.py

Output:
    etlp-gui/src-tauri/icons/icon.ico
    etlp-gui/src-tauri/icons/icon-windows-rounded.png
    etlp-gui/src-tauri/icons/tray-icon-windows.png
"""

from __future__ import annotations

import io
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# Superellipse |x|^N + |y|^N ≤ 1 with N ≈ 5 approximates the Apple squircle.
SUPERELLIPSE_N = 5.0
# Multi-resolution frames written into the .ico (including Windows DPI 24x24).
ICO_SIZES = [256, 128, 64, 48, 32, 24, 16]


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


def rounded_master(source: Path, canvas: int, body: int) -> Image.Image:
    """Squircle-mask the artwork and centre it on a transparent `canvas`.

    `body` is the size of the rounded artwork; `canvas - body` is the total
    transparent margin. The mask is rendered at the body size (4× supersampled
    by the caller's choice of `body`) for smooth edges.
    """
    art = Image.open(source).convert("RGBA").resize((body, body), Image.LANCZOS)
    mask = squircle_mask(body)
    # Intersect the artwork's own alpha with the squircle.
    alpha = Image.composite(
        art.getchannel("A"), Image.new("L", (body, body), 0), mask
    )
    art.putalpha(alpha)

    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    offset = (canvas - body) // 2
    out.paste(art, (offset, offset), art)
    return out


def save_standard_ico(
    master: Image.Image, sizes: list[int], out_path: Path
) -> None:
    """Save multi-resolution ICO using standard Windows Vista+ format.

    Sizes < 256 are stored as uncompressed 32-bit BGRA DIB with doubled biHeight
    and a 1-bit monochrome AND mask, ensuring full compatibility with classic
    Win32 LoadImageW and ExtractIconEx. 256 is stored as PNG.
    """
    frames = [master.resize((s, s), Image.LANCZOS) for s in sorted(sizes)]
    count = len(frames)
    header = struct.pack("<HHH", 0, 1, count)
    entries: list[bytes] = []
    data_blobs: list[bytes] = []
    offset = 6 + count * 16

    for f in frames:
        w, h = f.size
        bio = io.BytesIO()
        if w >= 256 and h >= 256:
            f.save(bio, "PNG")
            raw = bio.getvalue()
        else:
            f.save(bio, "DIB")
            dib = bio.getvalue()
            # DIB in ICO requires biHeight = 2*h for XOR+AND mask
            raw = dib[:8] + struct.pack("<i", h * 2) + dib[12:]
            # 1-bit monochrome AND mask (all 0 for 32-bit alpha DIB)
            row_bytes = ((w + 31) // 32) * 4
            and_mask = b"\x00" * (row_bytes * h)
            raw += and_mask

        b_w = 0 if w >= 256 else w
        b_h = 0 if h >= 256 else h
        size = len(raw)
        entries.append(
            struct.pack("<BBBBHHII", b_w, b_h, 0, 0, 1, 32, size, offset)
        )
        data_blobs.append(raw)
        offset += size

    with open(out_path, "wb") as fp:
        fp.write(header)
        for e in entries:
            fp.write(e)
        for d in data_blobs:
            fp.write(d)


def main() -> int:
    icons = repo_root() / "etlp-gui" / "src-tauri" / "icons"
    source = icons / "source.png"
    if not source.exists():
        print(f"error: source artwork not found: {source}", file=sys.stderr)
        return 1

    # App icon: ~3.5% margin per side, rendered large then saved as multi-size.
    app_master = rounded_master(source, canvas=1024, body=952)
    ico_path = icons / "icon.ico"
    save_standard_ico(app_master, ICO_SIZES, ico_path)
    print(f"    Generated {ico_path.relative_to(repo_root())}")

    # 256x256 rounded app icon for runtime Win32 window icon (crisp taskbar)
    win_icon_path = icons / "icon-windows-rounded.png"
    app_master.resize((256, 256), Image.LANCZOS).save(win_icon_path)
    print(f"    Generated {win_icon_path.relative_to(repo_root())}")

    # Tray icon: rounded almost edge to edge, downsampled to 32 px.
    tray_master = rounded_master(source, canvas=256, body=252)
    tray_path = icons / "tray-icon-windows.png"
    tray_master.resize((32, 32), Image.LANCZOS).save(tray_path)
    print(f"    Generated {tray_path.relative_to(repo_root())}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
