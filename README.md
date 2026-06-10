# Atoll

Atoll is a cross-platform floating approval island for local coding agents.

The first version focuses on the desktop shell:

- Tauri v2 window, tray, and Rust command core.
- React + TypeScript island UI for pending approvals.
- Demo event flow that mirrors Claude/Codex permission requests.

## Development

Install dependencies:

```bash
npm install
```

Run the frontend preview:

```bash
npm run dev
```

Run the desktop app after installing Rust:

```bash
npm run tauri dev
```

## Claude Code hook smoke test

Atoll currently exposes a local hook bridge on `127.0.0.1:47777`.
Run Atoll first, then launch Claude Code with temporary hooks:

```bash
claude --settings '{
  "hooks": {
    "PermissionRequest": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "node /Users/yangshuai/Documents/Atoll/scripts/atoll-claude-hook.mjs",
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
            "command": "node /Users/yangshuai/Documents/Atoll/scripts/atoll-claude-hook.mjs",
            "timeout": 30
          }
        ]
      }
    ]
  }
}'
```

This does not persist anything into Claude Code settings. The hook forwards Claude
permission requests into Atoll, waits for approval or denial, and uses
`PostToolUse` events to clear requests that were handled from Claude itself.

For global capture from any Claude Code working directory, add the same
`PermissionRequest` and `PostToolUse` hook command entries to
`~/.claude/settings.json`.

## Architecture

- `src/` contains the React floating island UI.
- `src/tauri.ts` contains the frontend bridge to Tauri commands and events.
- `src-tauri/src/lib.rs` contains the Rust core, tray menu, and request state.
- `src-tauri/src/hook_bridge.rs` contains the Claude Code hook bridge.
- `scripts/atoll-claude-hook.mjs` is the Claude hook command shim.

Future agent adapters should publish events into the Rust core instead of coupling UI components directly to a specific CLI.
