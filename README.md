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

Atoll listens on `127.0.0.1:47777`. To forward Claude Code permission requests
into Atoll, point Claude Code's hooks at the bundled shim script.

### 1. Get the hook shim

The shim is [`scripts/atoll-claude-hook.mjs`](scripts/atoll-claude-hook.mjs) in
this repo. Download it somewhere stable, e.g.:

```bash
mkdir -p ~/.atoll
curl -fsSL https://raw.githubusercontent.com/sheepbooy/Atoll/main/scripts/atoll-claude-hook.mjs \
  -o ~/.atoll/atoll-claude-hook.mjs
```

> If you cloned this repo, you can also point the hook command directly at your
> local checkout path.

### 2. Add the hooks to Claude Code

For a **one-off** test session, launch Claude Code with temporary hooks:

```bash
claude --settings '{
  "hooks": {
    "PermissionRequest": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "node ~/.atoll/atoll-claude-hook.mjs",
            "timeout": 1800
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "node ~/.atoll/atoll-claude-hook.mjs",
            "timeout": 30
          }
        ]
      }
    ]
  }
}'
```

This does not persist anything into Claude Code settings. The hook forwards
permission requests into Atoll, waits for approval or denial, and uses
`PostToolUse` events to clear requests that were handled from Claude itself.

For **global capture** from any Claude Code working directory, add the same
`PermissionRequest` and `PostToolUse` hook command entries to
`~/.claude/settings.json`.

> Override the bridge URL with the `ATOLL_HOOK_URL` environment variable if you
> ever change the port.

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
- `scripts/atoll-claude-hook.mjs` — the Claude hook command shim users configure
  in their Claude Code settings.

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
