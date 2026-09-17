#!/usr/bin/env python3
"""Measure the MacBook notch's bottom corner radius from a screenshot.

The result feeds `FALLBACK_NOTCH_CORNER_RADIUS` in src-tauri/src/state.rs
(and `cornerRadius` in NotchMetrics). Apple does not publish this value, so
the shipped 10pt default is a community-measured estimate. Run this on your
own machine to calibrate precisely:

    # Terminal needs Screen Recording permission (System Settings → Privacy
    # & Security → Screen Recording). Quit Atoll first so the bare notch is
    # visible, then:
    python3 scripts/calibrate_notch_radius.py

Requires Pillow (pip install Pillow). Output is in logical points,
screen-scale aware (Retina screenshots are captured at 2x).

How it works:
  1. Capture the top strip of the main display.
  2. Find the notch: columns whose pixels are pure black from y=0 downward
     (the menu bar background never reaches true black).
  3. For rows near the notch bottom, trace the left/right black edges —
     they curve inward at the rounded corners.
  4. Least-squares fit a circle through both corner boundary point sets and
     report the mean radius in points.
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


def capture_top_strip() -> Image.Image:
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "notch.png"
        subprocess.run(
            ["screencapture", "-x", "-R", f"0,0,99999,{TOP_STRIP_HEIGHT}", str(path)],
            check=True,
        )
        return Image.open(path).convert("L")


def detect_scale(img: Image.Image) -> float:
    # Compare against the logical resolution reported by the system.
    out = subprocess.run(
        ["system_profiler", "SPDisplaysDataType"],
        capture_output=True,
        text=True,
    ).stdout
    import re

    resolutions = [
        int(w)
        for w in re.findall(r"Resolution:\s+(\d+)", out)
    ]
    if resolutions and img.width:
        for logical in resolutions:
            ratio = img.width / logical
            if 1 <= ratio <= 3:
                return ratio
    return 2.0  # sensible Retina default


def notch_span(img: Image.Image) -> tuple[int, int, int]:
    """Return (left, right, bottom) of the notch black region in pixels."""
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

    # Span at mid-height (clear of the rounded corners).
    probe_y = bottom // 2
    left = center_x
    while left > 0 and px[left - 1, probe_y] <= BLACK_THRESHOLD:
        left -= 1
    right = center_x
    while right < w - 1 and px[right + 1, probe_y] <= BLACK_THRESHOLD:
        right += 1
    return left, right, bottom


def corner_points(
    img: Image.Image,
    anchor: int,
    bottom: int,
    side: str,
    scan: int = 40,
) -> list[tuple[float, float]]:
    """Boundary points of one bottom corner (moving up from the bottom)."""
    px = img.load()
    points: list[tuple[float, float]] = []
    for y in range(bottom - 1, bottom - scan, -1):
        row = [
            x
            for x in range(
                max(0, anchor - scan),
                min(img.width, anchor + scan),
            )
            if px[x, y] <= BLACK_THRESHOLD
        ]
        if not row:
            continue
        edge = min(row) if side == "left" else max(row)
        points.append((float(edge), float(y)))
    return points


def fit_circle(points: list[tuple[float, float]]) -> float:
    """Kasa algebraic circle fit; returns the radius."""
    n = len(points)
    if n < 8:
        sys.exit("Not enough boundary points for a circle fit.")
    sx = sum(p[0] for p in points)
    sy = sum(p[1] for p in points)
    sxx = sum(p[0] ** 2 for p in points)
    syy = sum(p[1] ** 2 for p in points)
    sxy = sum(p[0] * p[1] for p in points)
    sxz = sum(p[0] * (p[0] ** 2 + p[1] ** 2) for p in points)
    syz = sum(p[1] * (p[0] ** 2 + p[1] ** 2) for p in points)
    sz = sum(p[0] ** 2 + p[1] ** 2 for p in points)

    # Solve the 3x3 normal equations for x^2+y^2 + D x + E y + F = 0.
    a = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]]
    b = [sxz, syz, sz]
    import copy

    m = copy.deepcopy(a)
    for col in range(3):
        pivot = max(range(col, 3), key=lambda r: abs(m[r][col]))
        m[col], m[pivot] = m[pivot], m[col]
        b[col], b[pivot] = b[pivot], b[col]
        if abs(m[col][col]) < 1e-9:
            sys.exit("Degenerate circle fit.")
        for r in range(3):
            if r == col:
                continue
            factor = m[r][col] / m[col][col]
            for c in range(3):
                m[r][c] -= factor * m[col][c]
            b[r] -= factor * b[col]
    d = b[0] / m[0][0]
    e = b[1] / m[1][1]
    f = b[2] / m[2][2]
    center_x, center_y = -d / 2, -e / 2
    r2 = center_x**2 + center_y**2 - f
    if r2 <= 0:
        sys.exit("Degenerate circle fit (negative radius squared).")
    return r2**0.5


def main() -> None:
    img = capture_top_strip()
    scale = detect_scale(img)
    left, right, bottom = notch_span(img)

    left_pts = corner_points(img, left, bottom, "left")
    right_pts = corner_points(img, right, bottom, "right")
    r_left = fit_circle(left_pts)
    r_right = fit_circle(right_pts)
    r_px = (r_left + r_right) / 2
    r_pt = r_px / scale

    print(f"notch: {right - left + 1}px wide × {bottom + 1}px tall (scale {scale}x)")
    print(f"corner radius: left {r_left:.1f}px, right {r_right:.1f}px")
    print(f"→ FALLBACK_NOTCH_CORNER_RADIUS = {r_pt:.1f} pt")
    print(
        "Update src-tauri/src/state.rs (FALLBACK_NOTCH_CORNER_RADIUS) and the\n"
        "demo metrics in src/tauri/island.ts (cornerRadius) if this differs."
    )


if __name__ == "__main__":
    main()
