#!/usr/bin/env python3
"""Measure the MacBook camera-housing (notch) width/height from a screenshot.

Feeds the notch metrics used by src-tauri/src/platform/macos.rs
(detect_notch_metrics slop, FALLBACK_NOTCH_WIDTH/HEIGHT) and the TS mirrors
in src/islandLayout.ts. Apple does not publish these values; run on your own
machine to calibrate:

    # Terminal needs Screen Recording permission (System Settings → Privacy
    # & Security → Screen Recording). Quit Atoll first so the bare notch is
    # visible, then:
    python3 scripts/calibrate_notch_metrics.py

Requires Pillow (pip3 install Pillow). Output is in logical points,
screen-scale aware (Retina screenshots are captured at 2x).

How it works:
  1. Capture the top strip of the main display.
  2. Find the notch: columns whose pixels are pure black from y=0 downward
     (the menu bar background never reaches true black).
  3. Width  = run of fully-black columns at mid-notch height.
     Height = deepest black column from y=0 at the notch center.
     Offset = distance between the notch center and the screen center.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("Pillow is required: pip3 install Pillow")

BLACK_THRESHOLD = 8  # notch pixels are (0,0,0); menu bar bg is not
TOP_STRIP_HEIGHT = 300  # physical px captured from the top of the screen


def capture_top_strip() -> tuple[Image.Image, int]:
    """Return (grayscale image, logical width of the captured region)."""
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "notch.png"
        # Capture the full logical width so the screen center is measurable.
        subprocess.run(
            ["screencapture", "-x", "-R", f"0,0,99999,{TOP_STRIP_HEIGHT}", str(path)],
            check=True,
        )
        return Image.open(path).convert("L"), 0


def detect_scale(img: Image.Image) -> tuple[float, int]:
    """Return (scale factor, logical screen width) via system_profiler."""
    out = subprocess.run(
        ["system_profiler", "SPDisplaysDataType"],
        capture_output=True,
        text=True,
    ).stdout
    import re

    # Main display resolution is the first listed for the built-in panel.
    widths = [int(w) for w in re.findall(r"(\d+)\s*x\s*\d+", out)]
    if widths and img.width:
        for logical in widths:
            ratio = img.width / logical
            if 1 <= ratio <= 3:
                return ratio, logical
    return 2.0, img.width // 2  # sensible Retina default


def notch_span(img: Image.Image) -> tuple[int, int, int]:
    """Return (left, right, bottom) of the notch black region in pixels.

    Unlike calibrate_notch_radius.py this scans the full column span at
    several probe rows and takes the widest, so a menu bar item that dips
    to true black cannot inflate the result.
    """
    w, _ = img.size
    px = img.load()
    center_x = w // 2

    # Bottom: last row that is still black at the notch center.
    bottom = 0
    for y in range(img.height):
        if px[center_x, y] > BLACK_THRESHOLD:
            break
        bottom = y
    if bottom < 8:
        sys.exit("No notch found: is this a notched MacBook display?")

    # Span: probe rows at 25/50/75% of notch height, take the widest run of
    # consecutive black columns around the center.
    best_left, best_right = center_x, center_x
    for frac in (0.25, 0.5, 0.75):
        probe_y = max(1, int(bottom * frac))
        left = center_x
        while left > 0 and px[left - 1, probe_y] <= BLACK_THRESHOLD:
            left -= 1
        right = center_x
        while right < w - 1 and px[right + 1, probe_y] <= BLACK_THRESHOLD:
            right += 1
        if right - left > best_right - best_left:
            best_left, best_right = left, right
    return best_left, best_right, bottom


def main() -> None:
    img, _ = capture_top_strip()
    scale, logical_width = detect_scale(img)
    left, right, bottom = notch_span(img)

    width_px = right - left + 1
    height_px = bottom + 1
    center_px = (left + right) / 2
    screen_center_px = img.width / 2
    offset_pt = (center_px - screen_center_px) / scale

    width_pt = width_px / scale
    height_pt = height_px / scale

    print(f"screen: {img.width}px captured, logical width {logical_width}pt, scale {scale}x")
    print(f"housing: {width_px}px × {height_px}px  →  {width_pt:.1f}pt × {height_pt:.1f}pt")
    print(f"housing center offset from screen center: {offset_pt:+.1f}pt")
    print()
    print("Suggested constants (logical pt):")
    print(f"  measured notch width          = {width_pt:.1f}")
    print(f"  measured notch height         = {height_pt:.1f}")
    print(f"  aux-gap slop to fully cover   = ceil(width) - (frame - auxL - auxR);"
          f" current code adds +4 and floors at 200")
    print()
    print("Update targets if these differ from the runtime detection:")
    print("  - src-tauri/src/platform/macos.rs  notch_logical_width() slop / floor")
    print("  - src-tauri/src/state.rs           FALLBACK_NOTCH_HEIGHT, NOTCH_COVER_PADDING")
    print("  - src/islandLayout.ts              mirrored constants")


if __name__ == "__main__":
    main()
