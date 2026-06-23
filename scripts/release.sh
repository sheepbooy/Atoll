#!/usr/bin/env bash
# One-command release: sync version, push main, push tag, wait for CI.
# Usage:
#   ./scripts/release.sh 0.2.0
#
# Pushes tag with your local git credentials so the Release workflow starts.
# (Tags pushed by GITHUB_TOKEN inside Actions do not trigger other workflows.)

set -euo pipefail

VERSION="${1:?Usage: ./scripts/release.sh 0.2.0}"
VERSION="${VERSION#v}"
TAG="v${VERSION}"
REPO="${ATOLL_REPO:-sheepbooy/Atoll}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION_FILES=(
  package.json
  src-tauri/tauri.conf.json
  src-tauri/Cargo.toml
)

die() {
  echo "error: $*" >&2
  exit 1
}

info() {
  echo "==> $*"
}

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  die "version must look like 0.2.0"
fi

cd "$ROOT"
git rev-parse --git-dir >/dev/null 2>&1 || die "not a git repository"

branch="$(git branch --show-current)"
if [[ "$branch" != "main" ]]; then
  die "switch to main before releasing (current: ${branch})"
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  die "tag ${TAG} already exists locally"
fi

if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then
  die "tag ${TAG} already exists on origin"
fi

info "Syncing version files to ${VERSION}..."
bash scripts/sync-version.sh "$VERSION"

RELEASE_NOTE_FILES=()

if [[ -f CHANGELOG.md ]]; then
  if ! python3 scripts/sync-release-notes.py; then
    die "sync-release-notes.py failed; update CHANGELOG.md before releasing"
  fi
  notes_file="scripts/release-notes/${TAG}.md"
  if [[ ! -f "$notes_file" ]]; then
    die "missing release notes: ${notes_file} (add a ## [${VERSION}] section to CHANGELOG.md)"
  fi
  RELEASE_NOTE_FILES+=(CHANGELOG.md scripts/release-notes/"${TAG}.md")
fi

if ! git diff --quiet -- "${VERSION_FILES[@]}" "${RELEASE_NOTE_FILES[@]}"; then
  info "Committing version bump..."
  git add "${VERSION_FILES[@]}" "${RELEASE_NOTE_FILES[@]}"
  git commit -m "chore: release ${TAG}"
fi

info "Pushing main..."
git push origin HEAD:main

info "Creating and pushing tag ${TAG}..."
git tag "$TAG"
git push origin "$TAG"

echo
info "Release build triggered for ${TAG}"
echo "Actions: https://github.com/${REPO}/actions/workflows/release.yml"
echo "Release: https://github.com/${REPO}/releases/tag/${TAG}"

if ! command -v gh >/dev/null 2>&1; then
  echo
  echo "Install GitHub CLI (gh) to auto-watch the build, or open Actions manually."
  exit 0
fi

info "Waiting for Release workflow to start..."
run_id=""
for _ in $(seq 1 30); do
  run_id="$(
    gh run list \
      --repo "$REPO" \
      --workflow release.yml \
      --branch "$TAG" \
      --limit 1 \
      --json databaseId \
      -q '.[0].databaseId' 2>/dev/null || true
  )"
  if [[ -n "$run_id" ]]; then
    break
  fi
  sleep 2
done

if [[ -z "$run_id" ]]; then
  die "Release workflow did not start; check Actions manually"
fi

gh run watch "$run_id" --repo "$REPO" --exit-status
info "Release published: https://github.com/${REPO}/releases/tag/${TAG}"
