#!/usr/bin/env bash
# Sync project version across package metadata files.
# Usage: ./scripts/sync-version.sh 0.2.0

set -euo pipefail

VERSION="${1:?version required}"
VERSION="${VERSION#v}"

TMP_FILES=()
cleanup() {
  if ((${#TMP_FILES[@]})); then
    rm -f "${TMP_FILES[@]}"
  fi
}
trap cleanup EXIT

package_tmp="$(mktemp "${TMPDIR:-/tmp}/atoll-package.XXXXXX")"
tauri_tmp="$(mktemp "${TMPDIR:-/tmp}/atoll-tauri-conf.XXXXXX")"
TMP_FILES+=("$package_tmp" "$tauri_tmp")

jq --arg v "$VERSION" '.version = $v' package.json > "$package_tmp"
mv "$package_tmp" package.json

jq --arg v "$VERSION" '.version = $v' src-tauri/tauri.conf.json > "$tauri_tmp"
mv "$tauri_tmp" src-tauri/tauri.conf.json

if [[ "$(uname -s)" == "Darwin" ]]; then
  sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" src-tauri/Cargo.toml
else
  sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" src-tauri/Cargo.toml
fi

echo "Synced version to ${VERSION}"
