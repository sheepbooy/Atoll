---
name: atoll-release
description: >-
  Push Atoll commits and trigger a GitHub Release build (version bump, tag,
  CI). Use when the user asks to release, publish, push and build a new version,
  trigger release workflow, or run ./scripts/release.sh.
---

# Atoll Release

## Quick checklist

```
- [ ] On branch main, working tree clean (or commit WIP first)
- [ ] CHANGELOG.md has ## [X.Y.Z] section for the new version
- [ ] Next version = current package.json patch + 1 (unless user specifies)
- [ ] ./scripts/release.sh X.Y.Z
- [ ] Confirm Release workflow succeeded on tag branch
```

## Workflow

### 1. Inspect state

```bash
git fetch origin
git status -sb
git log --oneline origin/main..HEAD
git log --oneline v$(node -p "require('./package.json').version")..HEAD
grep '"version"' package.json
```

- **Uncommitted changes** → commit them before releasing (user often forgets).
- **Commits not on origin** → `release.sh` pushes `HEAD:main` at the end.
- **Already released** → check `gh release list --limit 3` and latest `v*` tag.

### 2. Write release notes

Add a section at the top of `CHANGELOG.md` (Keep a Changelog, Chinese prose):

```markdown
## [0.1.45] - 2026-07-09

### 修复
- **Feature name**：what changed and why

### 新增
- ...

### 改进
- ...
```

Group multiple commits by theme; `scripts/sync-release-notes.py` extracts this into `scripts/release-notes/vX.Y.Z.md` during release.

### 3. Run one-command release

```bash
bash scripts/release.sh 0.1.45
```

`release.sh` will:

1. `scripts/sync-version.sh` → `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
2. Sync release notes from CHANGELOG
3. Commit `chore: release vX.Y.Z` if version/notes changed
4. `git push origin HEAD:main`
5. `git tag vX.Y.Z && git push origin vX.Y.Z`
6. `gh run watch` on the Release workflow (if `gh` is installed)

**Do not** use `trigger-release.yml` from local unless `RELEASE_PAT` is configured. Prefer `release.sh` with the user's git credentials so the tag triggers `release.yml`.

### 4. Verify

```bash
gh run list --workflow=release.yml --branch=vX.Y.Z --limit 1
gh release view vX.Y.Z --json url,isDraft
```

Success URL: `https://github.com/sheepbooy/Atoll/releases/tag/vX.Y.Z`

## Project conventions

| Item | Value |
|------|-------|
| Repo | `sheepbooy/Atoll` |
| Release trigger | Push tag `v*` → `.github/workflows/release.yml` |
| Version files | `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` |
| Release script | `scripts/release.sh` |
| Commit style | `chore: release vX.Y.Z` for version bumps |

## Common pitfalls

### Workflow YAML invalid → 0s failure, no jobs

In `release.yml`, `Fix-Atoll.command` heredoc body **must stay indented** inside the `run: |` block. Strip padding after write:

```bash
sed -i 's/^          //' release/Fix-Atoll.command
```

Never leave unindented lines inside the YAML literal block.

### Tag pushed by GITHUB_TOKEN does not trigger Release

Tags created inside Actions with the default token won't start `release.yml`. Local `release.sh` uses the developer's credentials — that's intentional.

### release.sh waits for workflow

If interrupted, check Actions manually; re-run with `gh run rerun` or delete/recreate the tag on a fixed commit.

## When user says "push and build"

1. Commit any local WIP if needed
2. Update CHANGELOG for next patch version
3. Run `bash scripts/release.sh <version>`
4. Report release URL and whether CI passed
