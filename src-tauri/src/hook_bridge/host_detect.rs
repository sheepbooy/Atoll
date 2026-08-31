use std::net::TcpStream;

use serde_json::Value;

use super::*;

pub(crate) fn payload_transcript_path(payload: &Value) -> Option<String> {
    payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .or_else(|| payload.get("transcriptPath").and_then(Value::as_str))
        .filter(|s| !s.is_empty())
        .map(normalize_windows_path)
}

/// Strip URI-style leading `/` from Windows paths (e.g. `/C:/Users/…` → `C:/Users/…`).
#[cfg(windows)]
pub(crate) fn normalize_windows_path(path: &str) -> String {
    let stripped = if path.len() >= 3
        && path.starts_with('/')
        && path.as_bytes()[1].is_ascii_alphabetic()
        && path.as_bytes()[2] == b':'
    {
        &path[1..]
    } else {
        path
    };
    try_fix_gbk_mojibake(stripped)
}

#[cfg(not(windows))]
pub(crate) fn normalize_windows_path(path: &str) -> String {
    path.to_string()
}

/// Reverse GBK mojibake: when Cursor on Windows passes UTF-8 path bytes through a
/// pipeline that decodes them as GBK then re-encodes to UTF-8, Chinese characters
/// get garbled. E.g. `杨帅` (UTF-8 e6 9d a8 e5 b8 85) → GBK decode → `鏉ㄥ竻` →
/// UTF-8 re-encode → e9 8f 89 e3 84 a5 e7 ab bb. This function reverses the process.
#[cfg(windows)]
pub(crate) fn try_fix_gbk_mojibake(s: &str) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    let (gbk_bytes, _encoding_used, had_errors) = encoding_rs::GBK.encode(s);
    if had_errors {
        return s.to_string();
    }
    match std::str::from_utf8(&gbk_bytes) {
        Ok(fixed) => fixed.to_string(),
        Err(_) => s.to_string(),
    }
}

/// Resolve Cursor session cwd: prefer `workspace_roots[0]` over raw `cwd`
/// (which is often `"."` or the hook runner's working directory).
pub(crate) fn resolve_cursor_cwd(payload: &Value) -> String {
    if let Some(roots) = payload.get("workspace_roots").and_then(Value::as_array) {
        if let Some(first) = roots.first().and_then(Value::as_str) {
            if !first.is_empty() {
                return normalize_windows_path(first);
            }
        }
    }
    if let Some(roots) = payload.get("workspaceRoots").and_then(Value::as_array) {
        if let Some(first) = roots.first().and_then(Value::as_str) {
            if !first.is_empty() {
                return normalize_windows_path(first);
            }
        }
    }
    let from_payload = payload
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .to_string();
    let from_payload = normalize_windows_path(&from_payload);
    if !crate::is_unresolved_cursor_cwd(&from_payload) {
        return from_payload;
    }
    if let Some(lookup_id) = crate::payload_cursor_lookup_id(payload) {
        if let Some((_transcript, workspace)) = crate::discover_cursor_agent_transcript(lookup_id) {
            if !crate::is_unresolved_cursor_cwd(&workspace) {
                return workspace;
            }
        }
    }
    from_payload
}

/// Determine SessionHost for a Claude session.
///
/// Priority: peer process tree → transcript path → Claude Desktop running check → frontmost/cwd detection.
pub(crate) fn detect_host_for_claude_hook(
    state: &AppState,
    stream: &TcpStream,
    cwd: &str,
    transcript_path: &Option<String>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Claude session host detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}");
    eprintln!("[Atoll:host-detect] transcript_path={transcript_path:?}");

    let peer_host = hook_peer_session_host(stream);
    eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
    if peer_host != platform::SessionHost::Unknown {
        eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
        return peer_host;
    }

    if let Some(path) = transcript_path.as_deref() {
        if is_desktop_transcript_path(path) {
            eprintln!(
                "[Atoll:host-detect] RESULT: ClaudeDesktop (transcript path matched Desktop)"
            );
            return platform::SessionHost::ClaudeDesktop;
        }
        if is_cli_transcript_path(path) {
            if !is_claude_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: ClaudeCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::ClaudeCli;
            }
            eprintln!("[Atoll:host-detect] CLI-style path but Desktop IS running, need further signals...");
        }
    } else {
        eprintln!("[Atoll:host-detect] transcript_path is None");
    }

    let desktop_running = is_claude_desktop_app_running();
    eprintln!("[Atoll:host-detect] claude_desktop_running={desktop_running}");
    if desktop_running {
        let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
        eprintln!("[Atoll:host-detect] previous_app_pid={prev_pid:?}");
        let hint = prev_pid.map(|p| p as u32);
        if let Some(pid) = hint {
            let from_pid = platform::detect_session_host_from_peer_pid(pid);
            eprintln!("[Atoll:host-detect] detect_from_previous_pid({pid}) → {from_pid:?}");
            if from_pid == platform::SessionHost::ClaudeDesktop {
                eprintln!(
                    "[Atoll:host-detect] RESULT: ClaudeDesktop (previous_app_pid in Desktop tree)"
                );
                return platform::SessionHost::ClaudeDesktop;
            }
        }
        let terminal_front = is_any_terminal_frontmost();
        eprintln!("[Atoll:host-detect] terminal_frontmost={terminal_front}");
        if !terminal_front {
            eprintln!("[Atoll:host-detect] RESULT: ClaudeDesktop (Desktop running + no terminal frontmost)");
            return platform::SessionHost::ClaudeDesktop;
        }
    }

    let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
    let fallback = platform::detect_claude_session_host_at_hook(cwd, prev_pid);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (final fallback, prev_pid={prev_pid:?})");
    fallback
}

/// Determine SessionHost for a Codex session.
///
/// Priority: peer process tree → transcript path → Codex Desktop running check → frontmost/cwd detection.
pub(crate) fn detect_host_for_codex_hook(
    state: &AppState,
    stream: &TcpStream,
    cwd: &str,
    transcript_path: &Option<String>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Codex session host detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}");
    eprintln!("[Atoll:host-detect] transcript_path={transcript_path:?}");

    let peer_host = hook_peer_codex_session_host(stream);
    eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
    if peer_host != platform::SessionHost::Unknown {
        eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
        return peer_host;
    }

    if let Some(path) = transcript_path.as_deref() {
        if is_codex_desktop_transcript_path(path) {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (transcript path matched Desktop)");
            return platform::SessionHost::CodexDesktop;
        }
        if is_codex_cli_transcript_path(path) {
            if !is_codex_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: CodexCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::CodexCli;
            }
            eprintln!("[Atoll:host-detect] CLI-style path but Desktop IS running, need further signals...");
        }
    } else {
        eprintln!("[Atoll:host-detect] transcript_path is None");
    }

    let desktop_running = is_codex_desktop_app_running();
    eprintln!("[Atoll:host-detect] codex_desktop_running={desktop_running}");
    if desktop_running {
        let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
        eprintln!("[Atoll:host-detect] previous_app_pid={prev_pid:?}");
        if let Some(pid) = prev_pid.map(|p| p as u32) {
            let from_pid = platform::detect_codex_session_host_from_peer_pid(pid);
            eprintln!("[Atoll:host-detect] detect_from_previous_pid({pid}) → {from_pid:?}");
            if from_pid == platform::SessionHost::CodexDesktop {
                eprintln!(
                    "[Atoll:host-detect] RESULT: CodexDesktop (previous_app_pid in Desktop tree)"
                );
                return platform::SessionHost::CodexDesktop;
            }
        }
        let terminal_front = is_any_terminal_frontmost();
        eprintln!("[Atoll:host-detect] terminal_frontmost={terminal_front}");
        if !terminal_front {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (Desktop running + no terminal frontmost)");
            return platform::SessionHost::CodexDesktop;
        }
    }

    let prev_pid = state.previous_app_pid.lock().ok().and_then(|g| *g);
    let fallback = platform::detect_codex_session_host_at_hook(cwd, prev_pid);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (final fallback, prev_pid={prev_pid:?})");
    fallback
}

/// Identify the Claude session host by tracing the hook HTTP peer's process tree.
pub(crate) fn hook_peer_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("[Atoll:host-detect] peer_addr() failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };
    let port = peer.port();
    let own_pid = std::process::id();
    eprintln!("[Atoll:host-detect] peer port={port}, own_pid={own_pid}");

    let output = match platform::command_output_with_timeout(
        std::process::Command::new("lsof").args([
            "-i",
            &format!("TCP@127.0.0.1:{port}"),
            "-n",
            "-P",
            "-t",
        ]),
        Duration::from_secs(2),
    ) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[Atoll:host-detect] lsof exec failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    eprintln!("[Atoll:host-detect] lsof stdout={text:?}, stderr={stderr_text:?}");
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_session_host_from_peer_pid(pid);
                eprintln!("[Atoll:host-detect] peer_pid={pid} → {result:?}");
                return result;
            }
        }
    }
    platform::SessionHost::Unknown
}

pub(crate) fn is_cli_transcript_path(path: &str) -> bool {
    path.contains("/.claude/")
        || (path.contains("/claude/projects/") && !path.contains("/Application Support/"))
}

pub(crate) fn is_desktop_transcript_path(path: &str) -> bool {
    if path.contains("/Application Support/") && !path.contains("/.claude/") {
        return true;
    }
    path.contains("Claude-3p")
        || path.contains("local-agent-mode-sessions")
        || path.contains("com.anthropic.claude")
        || path.contains("agent-sessions")
}

pub(crate) fn is_claude_desktop_app_running() -> bool {
    platform::is_claude_desktop_app_running()
}

pub(crate) fn is_any_terminal_frontmost() -> bool {
    platform::frontmost_is_terminal()
}

/// Identify the Codex session host by tracing the hook HTTP peer's process tree.
pub(crate) fn hook_peer_codex_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("[Atoll:host-detect] peer_addr() failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };
    let port = peer.port();
    let own_pid = std::process::id();
    eprintln!("[Atoll:host-detect] peer port={port}, own_pid={own_pid}");

    let output = match platform::command_output_with_timeout(
        std::process::Command::new("lsof").args([
            "-i",
            &format!("TCP@127.0.0.1:{port}"),
            "-n",
            "-P",
            "-t",
        ]),
        Duration::from_secs(2),
    ) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[Atoll:host-detect] lsof exec failed: {e}");
            return platform::SessionHost::Unknown;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let stderr_text = String::from_utf8_lossy(&output.stderr);
    eprintln!("[Atoll:host-detect] lsof stdout={text:?}, stderr={stderr_text:?}");
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_codex_session_host_from_peer_pid(pid);
                eprintln!("[Atoll:host-detect] peer_pid={pid} → {result:?}");
                return result;
            }
        }
    }
    platform::SessionHost::Unknown
}

pub(crate) fn detect_host_for_cursor_hook(stream: &TcpStream) -> platform::SessionHost {
    let peer_host = hook_peer_cursor_session_host(stream);
    if peer_host != platform::SessionHost::Unknown {
        return peer_host;
    }
    if platform::is_cursor_app_running() {
        return platform::SessionHost::CursorIde;
    }
    platform::SessionHost::Unknown
}

pub(crate) fn detect_host_for_cursor_non_permission_hook(
    stream: Option<&TcpStream>,
) -> platform::SessionHost {
    if let Some(stream) = stream {
        let peer_host = hook_peer_cursor_session_host(stream);
        if peer_host != platform::SessionHost::Unknown {
            return peer_host;
        }
    }
    if platform::is_cursor_app_running() {
        return platform::SessionHost::CursorIde;
    }
    platform::SessionHost::Unknown
}

pub(crate) fn maybe_detect_and_store_cursor_host(
    state: &AppState,
    session_id: &str,
    stream: Option<&TcpStream>,
) {
    if get_stored_session_host(state, session_id) != platform::SessionHost::Unknown {
        return;
    }
    let host = detect_host_for_cursor_non_permission_hook(stream);
    if host != platform::SessionHost::Unknown {
        crate::store_session_host(state, session_id, host);
    }
}

/// Identify the Cursor session host by tracing the hook HTTP peer's process tree.
pub(crate) fn hook_peer_cursor_session_host(stream: &TcpStream) -> platform::SessionHost {
    let peer = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => return platform::SessionHost::Unknown,
    };
    let port = peer.port();
    let own_pid = std::process::id();

    let output = match platform::command_output_with_timeout(
        std::process::Command::new("lsof").args([
            "-i",
            &format!("TCP@127.0.0.1:{port}"),
            "-n",
            "-P",
            "-t",
        ]),
        Duration::from_secs(2),
    ) {
        Ok(o) => o,
        Err(_) => return platform::SessionHost::Unknown,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid != own_pid {
                let result = platform::detect_cursor_session_host_from_peer_pid(pid);
                if result != platform::SessionHost::Unknown {
                    return result;
                }
            }
        }
    }
    platform::SessionHost::Unknown
}

pub(crate) fn is_codex_cli_transcript_path(path: &str) -> bool {
    path.contains("/.codex/sessions/") || path.contains("/.codex/")
}

pub(crate) fn is_codex_desktop_transcript_path(path: &str) -> bool {
    path.contains("com.openai.codex")
        || (path.contains("/Application Support/") && path.contains("codex"))
}

pub(crate) fn is_codex_desktop_app_running() -> bool {
    platform::is_codex_desktop_app_running()
}

/// Detect Claude session host for non-permission hooks (Stop, PostToolUse, SubagentStop).
pub(crate) fn detect_host_for_claude_non_permission_hook(
    stream: Option<&TcpStream>,
    cwd: &str,
    transcript_path: Option<&str>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Claude non-permission hook detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}, transcript_path={transcript_path:?}");

    if let Some(stream) = stream {
        let peer_host = hook_peer_session_host(stream);
        eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
        if peer_host != platform::SessionHost::Unknown {
            eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
            return peer_host;
        }
    }

    if let Some(path) = transcript_path {
        if is_desktop_transcript_path(path) {
            eprintln!(
                "[Atoll:host-detect] RESULT: ClaudeDesktop (transcript path matched Desktop)"
            );
            return platform::SessionHost::ClaudeDesktop;
        }
        if is_cli_transcript_path(path) {
            if !is_claude_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: ClaudeCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::ClaudeCli;
            }
            eprintln!(
                "[Atoll:host-detect] CLI-style path but Desktop IS running, checking further..."
            );
        }
    }

    let desktop_running = is_claude_desktop_app_running();
    eprintln!("[Atoll:host-detect] claude_desktop_running={desktop_running}");
    if desktop_running && !is_any_terminal_frontmost() {
        eprintln!(
            "[Atoll:host-detect] RESULT: ClaudeDesktop (Desktop running + no terminal frontmost)"
        );
        return platform::SessionHost::ClaudeDesktop;
    }

    let fallback = platform::detect_claude_session_host(cwd);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (CWD fallback)");
    fallback
}

/// Detect Codex session host for non-permission hooks (Stop, PostToolUse, SubagentStop).
pub(crate) fn detect_host_for_codex_non_permission_hook(
    stream: Option<&TcpStream>,
    cwd: &str,
    transcript_path: Option<&str>,
) -> platform::SessionHost {
    eprintln!("[Atoll:host-detect] === Codex non-permission hook detection ===");
    eprintln!("[Atoll:host-detect] cwd={cwd:?}, transcript_path={transcript_path:?}");

    if let Some(stream) = stream {
        let peer_host = hook_peer_codex_session_host(stream);
        eprintln!("[Atoll:host-detect] peer_process_tree → {peer_host:?}");
        if peer_host != platform::SessionHost::Unknown {
            eprintln!("[Atoll:host-detect] RESULT: {peer_host:?} (from peer process tree)");
            return peer_host;
        }
    }

    if let Some(path) = transcript_path {
        if is_codex_desktop_transcript_path(path) {
            eprintln!("[Atoll:host-detect] RESULT: CodexDesktop (transcript path matched Desktop)");
            return platform::SessionHost::CodexDesktop;
        }
        if is_codex_cli_transcript_path(path) {
            if !is_codex_desktop_app_running() {
                eprintln!("[Atoll:host-detect] RESULT: CodexCli (CLI path + Desktop NOT running)");
                return platform::SessionHost::CodexCli;
            }
            eprintln!(
                "[Atoll:host-detect] CLI-style path but Desktop IS running, checking further..."
            );
        }
    }

    let desktop_running = is_codex_desktop_app_running();
    eprintln!("[Atoll:host-detect] codex_desktop_running={desktop_running}");
    if desktop_running && !is_any_terminal_frontmost() {
        eprintln!(
            "[Atoll:host-detect] RESULT: CodexDesktop (Desktop running + no terminal frontmost)"
        );
        return platform::SessionHost::CodexDesktop;
    }

    let fallback = platform::detect_codex_session_host(cwd);
    eprintln!("[Atoll:host-detect] RESULT: {fallback:?} (CWD fallback)");
    fallback
}
