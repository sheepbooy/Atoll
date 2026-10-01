# AGENTS.md

## Cursor Cloud specific instructions

Atoll is a **Tauri 2 desktop app** (Rust backend in `src-tauri/`, React + Vite frontend in `src/`) that targets **macOS / Windows**. On the Linux cloud VM the React frontend is the primary development surface, but the Rust backend and the real GUI can also be built and run (see below). Standard commands live in the README "开发" section and `package.json` scripts; only non-obvious caveats are noted here.

### Services & how to run them

- **Frontend dev server** — `npm run dev` (Vite, serves the island UI at `http://127.0.0.1:1420`). The port is `strictPort`, so only one instance can run at a time. `npm run tauri dev` starts its own Vite via `beforeDevCommand`, so do **not** keep a standalone `npm run dev` running when launching the full app.
- **Full desktop app** — `DISPLAY=:1 npm run tauri dev` launches the actual Tauri window on the VM's X display (xfce/xfwm4). It also starts the local **hook bridge** on `127.0.0.1:47777`.
  - `libEGL ... DRI3` warnings on startup are harmless (software rendering fallback).
  - End-to-end approval flow: POST a Claude `PreToolUse` payload to `http://127.0.0.1:47777/claude/pre-tool-use` (this request **blocks** until resolved). The island expands into the approval card; approving it returns `{"hookSpecificOutput":{"permissionDecision":"allow",...}}` to the blocked caller.
  - **Non-obvious (Linux only):** mouse clicks do **not** reach the transparent, borderless webview under xfwm4, so the Approve/Deny buttons can't be clicked with the mouse. The documented keyboard shortcuts do work once the window is focused (Approve = `Enter`, Deny = `Delete`, Always = `Shift`+`Enter`), e.g. `xdotool windowactivate <atoll_window_id>; xdotool key --window <atoll_window_id> Return`. This is a Linux windowing quirk only; the app ships for macOS/Windows.

### Lint / test / build

- **Typecheck (lint):** `npx tsc --noEmit` (also part of `npm run build`).
- **Frontend tests:** `npm test` (vitest). One test — `App.test.tsx > "automatically collapses after the final approval while still focused and hovered"` — is timing-sensitive and fails on this Linux VM. It is **pre-existing** (fails on a clean checkout, not caused by code changes).
- **Node hook script tests:** not run by `npm test`; run each directly, e.g. `node scripts/atoll-hook-bridge.test.mjs`. `atoll-claude-hook.test.mjs` has a `bridge.json` fallback assertion that assumes the Windows `LOCALAPPDATA` layout and therefore fails on Linux (on Linux `bridgeConfigPath()` resolves to `~/.local/share/Atoll/bridge.json` per XDG). The core `ATOLL_HOOK_URL` path and `atoll-codex-hook.test.mjs` pass.
- **Backend build/lint:** from `src-tauri/`, `cargo check` (or `cargo build`). Requires Rust ≥ 1.85 (a transitive dep needs `edition2024`) and the GTK3/WebKit2GTK dev libraries; both are already provisioned in the VM snapshot (`rustup default` is set to a newer stable, and the GTK/WebKit `-dev` packages are installed). The unused-variable warnings in `src/platform/mod.rs` / `src/lib.rs` are expected (Linux no-op fallbacks).
