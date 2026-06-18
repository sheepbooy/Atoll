#!/usr/bin/env bash
# Install Atoll from the latest GitHub Release (macOS only).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.sh | bash
# Pin a version:
#   ATOLL_VERSION=0.1.0 curl -fsSL ... | bash
# Private repo (or higher API rate limits):
#   GH_TOKEN=... ATOLL_VERSION=0.1.0 bash scripts/install.sh

set -euo pipefail

REPO="sheepbooy/Atoll"
APP_NAME="Atoll.app"
INSTALL_DIR="${ATOLL_INSTALL_DIR:-/Applications}"
TMP_DIR=""
MOUNT_DIR=""

die() {
  echo "error: $*" >&2
  exit 1
}

info() {
  echo "==> $*"
}

github_token() {
  printf '%s' "${GH_TOKEN:-${GITHUB_TOKEN:-}}"
}

github_api() {
  local path="$1"
  local curl_args=(-fsSL -H "Accept: application/vnd.github+json")
  local token
  token="$(github_token)"

  if [[ -n "$token" ]]; then
    curl_args+=(-H "Authorization: Bearer ${token}")
  fi

  curl "${curl_args[@]}" "https://api.github.com${path}"
}

release_asset_api_url() {
  local version="$1"
  local asset="$2"

  python3 - "$REPO" "$version" "$asset" <<'PY'
import json
import os
import sys
import urllib.error
import urllib.request

repo, version, asset = sys.argv[1:4]
token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
headers = {"Accept": "application/vnd.github+json"}
if token:
    headers["Authorization"] = f"Bearer {token}"

request = urllib.request.Request(
    f"https://api.github.com/repos/{repo}/releases/tags/v{version}",
    headers=headers,
)

try:
    with urllib.request.urlopen(request) as response:
        release = json.load(response)
except urllib.error.HTTPError as error:
    if error.code in {403, 404} and not token:
        sys.stderr.write(
            "error: could not access GitHub release metadata. "
            "If this repository is private, set GH_TOKEN or GITHUB_TOKEN.\n"
        )
    raise SystemExit(1) from error

for item in release.get("assets", []):
    if item.get("name") == asset:
        print(item["url"])
        break
else:
    sys.stderr.write(f"error: release asset not found: {asset}\n")
    raise SystemExit(1)
PY
}

require_macos() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    die "Atoll only supports macOS."
  fi
}

detect_arch() {
  case "$(uname -m)" in
    arm64)
      echo "aarch64"
      ;;
    x86_64)
      echo "x86_64"
      ;;
    *)
      die "unsupported CPU architecture: $(uname -m)"
      ;;
  esac
}

resolve_version() {
  if [[ -n "${ATOLL_VERSION:-}" ]]; then
    echo "${ATOLL_VERSION#v}"
    return
  fi

  local tag
  tag="$(
    github_api "/repos/${REPO}/releases/latest" \
      | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"].removeprefix("v"))'
  )"
  [[ -n "$tag" ]] || die "could not resolve latest release version"
  echo "$tag"
}

resolve_dmg_name() {
  local arch="$1"
  case "$arch" in
    aarch64)
      echo "Atoll-aarch64.dmg"
      ;;
    x86_64)
      die "Intel (x86_64) builds are not published yet. Use an Apple Silicon Mac or build from source."
      ;;
    *)
      die "unsupported architecture: $arch"
      ;;
  esac
}

download_release_asset() {
  local version="$1"
  local asset="$2"
  local dest="$3"
  local asset_api_url
  local curl_args=(-fsSL -L -H "Accept: application/octet-stream")
  local token

  asset_api_url="$(release_asset_api_url "$version" "$asset")"
  token="$(github_token)"
  if [[ -n "$token" ]]; then
    curl_args+=(-H "Authorization: Bearer ${token}")
  fi

  info "Downloading ${asset} (v${version})..."
  curl "${curl_args[@]}" -o "$dest" "$asset_api_url"
}

verify_checksum() {
  local version="$1"
  local dmg_name="$2"
  local dmg_path="$3"
  local checksum_name="${dmg_name}.sha256"
  local checksum_api_url expected actual
  local curl_args=(-fsSL -L -H "Accept: application/octet-stream")
  local token

  if ! checksum_api_url="$(release_asset_api_url "$version" "$checksum_name" 2>/dev/null)"; then
    info "No published sha256 file; skipping checksum verification."
    return 0
  fi

  token="$(github_token)"
  if [[ -n "$token" ]]; then
    curl_args+=(-H "Authorization: Bearer ${token}")
  fi

  expected="$(curl "${curl_args[@]}" "$checksum_api_url")"
  actual="$(shasum -a 256 "$dmg_path" | awk '{print $1}')"
  if [[ "$expected" != "$actual" ]]; then
    die "checksum mismatch for ${dmg_name}"
  fi

  info "Checksum verified."
}

mount_dmg() {
  local dmg_path="$1"
  local mount_dir

  mount_dir="$(
    hdiutil attach "$dmg_path" -nobrowse -readonly \
      | tail -n 1 \
      | awk '{$1=$2=""; sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, ""); print}'
  )"
  [[ -n "$mount_dir" && -d "$mount_dir" ]] || die "failed to mount ${dmg_path}"
  echo "$mount_dir"
}

run_privileged() {
  if [[ -w "$INSTALL_DIR" ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

install_app() {
  local mount_dir="$1"
  local source_app="${mount_dir}/${APP_NAME}"
  local target_app="${INSTALL_DIR}/${APP_NAME}"

  [[ -d "$source_app" ]] || die "could not find ${APP_NAME} inside the mounted disk image"

  if [[ -d "$target_app" ]]; then
    info "Replacing existing ${target_app}..."
    run_privileged rm -rf "$target_app"
  fi

  info "Installing to ${target_app}..."
  run_privileged ditto "$source_app" "$target_app"

  info "Clearing macOS quarantine attributes..."
  run_privileged xattr -cr "$target_app"
}

print_success() {
  cat <<EOF

Atoll is installed at ${INSTALL_DIR}/${APP_NAME}.

If macOS still blocks the first launch, right-click Atoll in Applications
and choose "Open", then confirm once in the dialog. After that, normal
double-click works.

Next steps:
  1. Open Atoll from Applications.
  2. Use the tray/island menu and click "Install hooks" to connect Claude Code.

EOF
}

cleanup() {
  if [[ -n "$MOUNT_DIR" ]]; then
    hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true
  fi
  if [[ -n "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}

main() {
  require_macos

  local arch version dmg_name dmg_path
  arch="$(detect_arch)"
  version="$(resolve_version)"
  dmg_name="$(resolve_dmg_name "$arch")"

  TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/atoll-install.XXXXXX")"
  trap cleanup EXIT

  dmg_path="${TMP_DIR}/${dmg_name}"
  download_release_asset "$version" "$dmg_name" "$dmg_path"
  verify_checksum "$version" "$dmg_name" "$dmg_path"

  MOUNT_DIR="$(mount_dmg "$dmg_path")"
  install_app "$MOUNT_DIR"

  hdiutil detach "$MOUNT_DIR" -quiet || true
  MOUNT_DIR=""

  print_success
}

main "$@"
