#!/usr/bin/env python3
"""Extract per-version release notes from CHANGELOG.md into scripts/release-notes/."""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHANGELOG = ROOT / "CHANGELOG.md"
OUT_DIR = ROOT / "scripts" / "release-notes"


def extract_notes(changelog_text: str) -> dict[str, str]:
    pattern = r"^## \[([^\]]+)\]"
    parts = re.split(pattern, changelog_text, flags=re.M)
    notes: dict[str, str] = {}

    for i in range(1, len(parts), 2):
        version = parts[i].strip()
        body = parts[i + 1].strip()
        lines = body.splitlines()
        content_lines: list[str] = []

        for line in lines:
            stripped = line.strip()
            if re.match(r"^\[\d", stripped):
                break
            if stripped.startswith("- ") and "releases/tag" in stripped:
                break
            content_lines.append(line)

        if content_lines and re.match(r"\d{4}-\d{2}-\d{2}", content_lines[0].strip().lstrip("- ")):
            content_lines = content_lines[1:]

        content = "\n".join(content_lines).strip()
        if content:
            notes[version] = content + "\n"

    return notes


def main() -> int:
    if not CHANGELOG.exists():
        print(f"error: {CHANGELOG} not found", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(exist_ok=True)
    notes = extract_notes(CHANGELOG.read_text())

    for version, content in notes.items():
        path = OUT_DIR / f"v{version}.md"
        path.write_text(content)
        print(f"written {path.relative_to(ROOT)}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
