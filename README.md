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

## Architecture

- `src/` contains the React floating island UI.
- `src/tauri.ts` contains the frontend bridge to Tauri commands and events.
- `src-tauri/src/lib.rs` contains the Rust core, tray menu, and simulated approval requests.

Future agent adapters should publish events into the Rust core instead of coupling UI components directly to a specific CLI.
