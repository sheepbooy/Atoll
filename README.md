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

Atoll is distributed via Homebrew. Because it is notarization-free (no Apple
Developer account), install with `--no-quarantine` so macOS does not flag it as
"damaged":

```bash
brew tap sheepbooy/tap
brew install --cask --no-quarantine atoll
```

To upgrade later:

```bash
brew upgrade --cask --no-quarantine atoll
```

> If you downloaded the `.dmg` directly from the
> [Releases](https://github.com/sheepbooy/Atoll/releases) page instead, run
> `sudo xattr -cr /Applications/Atoll.app` once after dragging it into
> Applications. Each release also ships a `Fix-Atoll-*.command` script that does
> the same thing with a GUI prompt.

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

Releases are built automatically by GitHub Actions when a `v*` tag is pushed.
Each release ships:

- `Atoll-aarch64.dmg` — Apple Silicon (M1/M2/M3/M4)
- `Atoll-x86_64.dmg` — Intel Mac
- `Fix-Atoll-*.command` — one-click "damaged app" repair script

See [Releases](https://github.com/sheepbooy/Atoll/releases).

## License

MIT
