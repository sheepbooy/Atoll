#!/usr/bin/env bash
# Generate latest.json for Tauri updater from staged release artifacts.
# Usage: generate-latest-json.sh VERSION MACOS_STAGING WINDOWS_STAGING OUTPUT_JSON
set -euo pipefail

VERSION="${1:?version required}"
MACOS_DIR="${2:?macos staging dir required}"
WINDOWS_DIR="${3:?windows staging dir required}"
OUTPUT="${4:?output path required}"

REPO="${ATOLL_GITHUB_REPO:-sheepbooy/Atoll}"
TAG="v${VERSION}"
BASE="https://github.com/${REPO}/releases/download/${TAG}"

MACOS_TAR=$(find "$MACOS_DIR" -maxdepth 1 -name '*.tar.gz' ! -name '*.sig' | head -1)
MACOS_SIG=$(find "$MACOS_DIR" -maxdepth 1 -name '*.tar.gz.sig' | head -1)
WINDOWS_MSI=$(find "$WINDOWS_DIR" -maxdepth 1 -name '*.msi' ! -name '*.sig' | head -1)
WINDOWS_SIG=$(find "$WINDOWS_DIR" -maxdepth 1 -name '*.msi.sig' | head -1)

test -f "$MACOS_TAR" || { echo "error: macOS updater bundle (.tar.gz) not found in $MACOS_DIR" >&2; exit 1; }
test -f "$MACOS_SIG" || { echo "error: macOS .tar.gz.sig not found in $MACOS_DIR" >&2; exit 1; }
test -f "$WINDOWS_MSI" || { echo "error: Windows MSI not found in $WINDOWS_DIR" >&2; exit 1; }
test -f "$WINDOWS_SIG" || { echo "error: Windows .msi.sig not found in $WINDOWS_DIR" >&2; exit 1; }

MACOS_ASSET=$(basename "$MACOS_TAR")
WINDOWS_ASSET=$(basename "$WINDOWS_MSI")
MACOS_SIG_CONTENT=$(tr -d '\n' < "$MACOS_SIG")
WINDOWS_SIG_CONTENT=$(tr -d '\n' < "$WINDOWS_SIG")
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

NOTES_FILE="scripts/release-notes/${TAG}.md"
if [[ -f "$NOTES_FILE" ]]; then
  NOTES=$(head -c 500 "$NOTES_FILE" | tr '\n' ' ')
else
  NOTES="See https://github.com/${REPO}/releases/tag/${TAG}"
fi

jq -n \
  --arg version "$VERSION" \
  --arg notes "$NOTES" \
  --arg pub_date "$PUB_DATE" \
  --arg mac_url "${BASE}/${MACOS_ASSET}" \
  --arg mac_sig "$MACOS_SIG_CONTENT" \
  --arg win_url "${BASE}/${WINDOWS_ASSET}" \
  --arg win_sig "$WINDOWS_SIG_CONTENT" \
  '{
    version: $version,
    notes: $notes,
    pub_date: $pub_date,
    platforms: {
      "darwin-aarch64": { url: $mac_url, signature: $mac_sig },
      "windows-x86_64": { url: $win_url, signature: $win_sig }
    }
  }' > "$OUTPUT"

echo "Wrote $OUTPUT for version $VERSION"
jq -e '.version and .platforms["darwin-aarch64"].url and .platforms["darwin-aarch64"].signature and .platforms["windows-x86_64"].url and .platforms["windows-x86_64"].signature' "$OUTPUT" >/dev/null
