#!/usr/bin/env bash
# One-time setup for Tauri updater signing keys.
# Generates keys locally and prints GitHub Secrets instructions.
set -euo pipefail

KEY_DIR="src-tauri/.tauri-keys"
KEY_PATH="${KEY_DIR}/atoll.key"

mkdir -p "$KEY_DIR"

if [[ ! -f "$KEY_PATH" ]]; then
  CI=true npx tauri signer generate --write-keys "$KEY_PATH" --password "${ATOLL_SIGNING_PASSWORD:-}" --force
fi

PUBKEY=$(cat "${KEY_PATH}.pub")

echo ""
echo "==> Public key (already in src-tauri/tauri.conf.json plugins.updater.pubkey):"
echo "$PUBKEY"
echo ""
echo "==> Add these GitHub repository secrets (Settings → Secrets → Actions):"
echo "    TAURI_SIGNING_PRIVATE_KEY  = contents of ${KEY_PATH}"
echo "    TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (empty unless you set ATOLL_SIGNING_PASSWORD)"
echo ""
echo "Private key path: ${KEY_PATH} (gitignored — keep it safe!)"
