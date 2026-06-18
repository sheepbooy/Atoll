#!/usr/bin/env bash
# Sync project version across package metadata files.
# Usage: ./scripts/sync-version.sh 0.2.0

set -euo pipefail

VERSION="${1:?version required}"
VERSION="${VERSION#v}"

jq --arg v "$VERSION" '.version = $v' package.json > package.json.tmp
mv package.json.tmp package.json

jq --arg v "$VERSION" '.version = $v' src-tauri/tauri.conf.json > tauri.conf.json.tmp
mv tauri.conf.json.tmp src-tauri/tauri.conf.json

if [[ "$(uname -s)" == "Darwin" ]]; then
  sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" src-tauri/Cargo.toml
else
  sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" src-tauri/Cargo.toml
fi

echo "Synced version to ${VERSION}"
