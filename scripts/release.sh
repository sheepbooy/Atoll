#!/usr/bin/env bash
# Start the release pipeline for a new version.
# Usage:
#   ./scripts/release.sh 0.2.0
#
# With GitHub CLI (recommended): triggers Trigger Release workflow in Actions.
# Without gh: creates and pushes a git tag, which triggers the Release workflow.

set -euo pipefail

VERSION="${1:?Usage: ./scripts/release.sh 0.2.0}"
VERSION="${VERSION#v}"
TAG="v${VERSION}"
REPO="${ATOLL_REPO:-sheepbooy/Atoll}"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version must look like 0.2.0" >&2
  exit 1
fi

if command -v gh >/dev/null 2>&1; then
  echo "Starting Trigger Release workflow for ${TAG}..."
  gh workflow run trigger-release.yml \
    --repo "$REPO" \
    -f "version=${VERSION}"
  echo
  echo "Pipeline started. Track progress:"
  echo "https://github.com/${REPO}/actions/workflows/trigger-release.yml"
  exit 0
fi

echo "gh not found; falling back to git tag ${TAG}"
git tag "$TAG"
git push origin "$TAG"
echo
echo "Tag pushed. Track progress:"
echo "https://github.com/${REPO}/actions/workflows/release.yml"
