# Atoll

Atoll is a floating approval island for local coding agents. It sits in your
macOS menu bar / notch and surfaces pending permission requests from agents like
Claude Code, so you can approve or deny them without leaving your editor.

> Status: early desktop shell. The first version focuses on the macOS floating
> island + Claude Code hook bridge.

## Features

- **Notch-hugging floating island** — a compact pill that lives in the menu bar
  / notch area, expands into a panel when an approval is pending, and collapses
  back when idle.
- **Live permission flow** — Claude Code `PermissionRequest` hooks are forwarded
  into Atoll in real time over a local HTTP bridge.
- **Token usage** — per-session token counters roll over across sessions.
- **Tray menu** — quit, show, and status from the menu bar tray.
- **No cloud** — everything runs locally; the hook bridge binds to
  `127.0.0.1:47777`.

## Installation

Atoll is not notarized yet (no Apple Developer account), so a browser-downloaded
`.dmg` may show macOS "damaged" or "unverified developer" warnings. Use one of
the options below.

### Option 1: One-line install (recommended)

Downloads the latest release, installs to `/Applications`, and clears quarantine
for you:

```bash
curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.sh | bash
```

Pin a specific version:

```bash
ATOLL_VERSION=0.1.0 curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/install.sh | bash
```

If macOS still blocks the first launch, right-click **Atoll** in Applications and
choose **Open** once.

### Option 2: Homebrew

```bash
brew tap sheepbooy/tap
brew install --cask --no-quarantine atoll
```

To upgrade later:

```bash
brew upgrade --cask --no-quarantine atoll
```

### Option 3: Manual `.dmg` download

1. Download `Atoll-aarch64.dmg` from
   [Releases](https://github.com/sheepbooy/Atoll/releases).
2. Drag **Atoll.app** into **Applications**.
3. Clear quarantine once:

```bash
sudo xattr -cr /Applications/Atoll.app
```

Or run the `Fix-Atoll.command` script from the same release page (GUI prompt).

If the app still will not open, right-click **Atoll** in Applications and
choose **Open** once.

## Connecting Claude Code

Atoll listens on `127.0.0.1:47777` and ships with a one-click hook installer —
**no manual editing of Claude Code settings is needed**.

1. Open Atoll and click the menu (tray / island menu).
2. Click **Install hooks**.

Atoll writes the hook configuration into `~/.claude/settings.json` for you,
pointing at the hook shim bundled inside the app (`node <app-bundle>/…/atoll-claude-hook.mjs`).
It registers `PermissionRequest`, `PostToolUse`, `Stop`, and `SubagentStop`
hooks so permission requests from any Claude Code working directory are
forwarded into the floating island in real time.

To disconnect later, open the same menu and click **Uninstall hooks** — Atoll
removes the hooks entry from `~/.claude/settings.json`.

> The hook bridge runs entirely locally; nothing leaves your machine. Override
> the bridge URL with the `ATOLL_HOOK_URL` environment variable if you ever
> change the port.

## Development

Install dependencies:

```bash
npm install
```

Run the frontend preview:

```bash
npm run dev
```

Run the desktop app (requires [Rust](https://rustup.rs)):

```bash
npm run tauri dev
```

Run tests:

```bash
npm test
```

Build a production bundle:

```bash
npm run tauri build
```

## Architecture

- `src/` — React + TypeScript floating island UI.
  - `src/App.tsx` — island presentation and layout.
  - `src/tauri.ts` — frontend bridge to Tauri commands and events.
  - `src/TokenCounter.tsx` — per-session token usage display.
- `src-tauri/src/lib.rs` — Rust core: tray menu, window geometry, request state.
- `src-tauri/src/hook_bridge.rs` — the Claude Code hook bridge (local HTTP server
  on `127.0.0.1:47777`).
- `scripts/atoll-claude-hook.mjs` — the Claude hook command shim, bundled into the
  app and registered into `~/.claude/settings.json` by the one-click installer.

Future agent adapters should publish events into the Rust core instead of
coupling UI components directly to a specific CLI.

## Releases

Releases are built automatically by GitHub Actions. You do **not** need to
manually edit version numbers in multiple files — the pipeline injects the
version from the release tag.

### Option 1: One-command release (recommended)

From `main`, with your changes committed or ready to ship:

```bash
./scripts/release.sh 0.2.0
```

This script:

1. Syncs version across `package.json`, `tauri.conf.json`, and `Cargo.toml`
2. Commits the bump to `main` and pushes
3. Creates and pushes tag `v0.2.0` from your machine
4. Waits for the **Release** workflow to build and publish artifacts

Requires [GitHub CLI](https://cli.github.com) for the auto-watch step. Without `gh`,
the script still releases; open Actions manually to track the build.

### Option 2: Git tag only

If you already pushed your changes to `main`:

```bash
git tag v0.2.0
git push origin v0.2.0
```

The **Release** workflow reads the version from the tag and builds automatically.
Use this when version files on `main` are already at the target version.

### Option 3: GitHub Actions UI (version bump only)

[Actions → Trigger Release](https://github.com/sheepbooy/Atoll/actions/workflows/trigger-release.yml)
can bump versions and create a tag, but tags pushed by `GITHUB_TOKEN` do **not**
start the Release build. After it finishes, run locally:

```bash
git fetch --tags
git push origin v0.2.0
```

Each release ships:

- `Atoll-aarch64.dmg` — Apple Silicon (M1/M2/M3/M4)
- `Atoll-aarch64.dmg.sha256` — checksum for verification
- `install.sh` — one-line installer script
- `Fix-Atoll.command` — manual quarantine repair script

Intel (`x86_64`) builds are not published yet.

See [Releases](https://github.com/sheepbooy/Atoll/releases).

## License

MIT
