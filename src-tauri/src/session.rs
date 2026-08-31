//! Session and subagent tracking: known-session registry, session hosts,
//! snapshot building, hook-health aggregation, ZCode subagent derivation,
//! Cursor subagent discovery, and approval/session archive commands.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use super::*;

/// 任一 Agent hook 已安装且 shim 存在，并且本地 bridge 可连接 → 在线监听。
pub(crate) fn compute_listening_online(app: &AppHandle) -> bool {
    if capture::listening_online() {
        return true;
    }
    let claude_ready = claude_hook_status(app);
    let codex_ready = codex_hook_status(app);
    let cursor_ready = cursor_hook_status(app);
    let any_installed = claude_ready.installed || codex_ready.installed || cursor_ready.installed;
    let any_script_found =
        claude_ready.script_found || codex_ready.script_found || cursor_ready.script_found;
    any_installed && any_script_found && hook_bridge::is_bridge_online(app)
}

pub(crate) fn touch_hook_activity(state: &AppState) {
    if let Ok(mut last) = state.last_hook_activity.lock() {
        *last = Instant::now();
    }
}

pub(crate) fn get_stored_session_host(state: &AppState, session_id: &str) -> platform::SessionHost {
    state
        .known_sessions
        .lock()
        .ok()
        .and_then(|known| known.get(session_id).map(|entry| entry.host))
        .unwrap_or(platform::SessionHost::Unknown)
}

pub(crate) fn schedule_observer_snapshot_emit(app: &AppHandle) {
    let state = app.state::<AppState>();
    state
        .snapshot_debounce_generation
        .fetch_add(1, Ordering::AcqRel);
    if state
        .snapshot_debounce_worker_running
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    thread::spawn(move || loop {
        let state = app.state::<AppState>();
        let before = state.snapshot_debounce_generation.load(Ordering::Acquire);
        thread::sleep(OBSERVER_SNAPSHOT_DEBOUNCE);
        if state.snapshot_debounce_generation.load(Ordering::Acquire) != before {
            continue;
        }
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
        state
            .snapshot_debounce_worker_running
            .store(false, Ordering::Release);
        if state.snapshot_debounce_generation.load(Ordering::Acquire) == before
            || state
                .snapshot_debounce_worker_running
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            break;
        }
    });
}

pub(crate) fn refresh_hook_health_cache(app: &AppHandle, state: &AppState) {
    let health = build_hook_health(app);
    remember_hook_health(state, &health);
}

pub(crate) fn reconcile_incomplete_subagents_now(state: &AppState) {
    reconcile_incomplete_subagents(state);
    if let Ok(mut last) = state.last_subagent_reconcile.lock() {
        *last = Instant::now();
    }
}

pub(crate) fn reconcile_incomplete_subagents_if_due(state: &AppState) {
    let should_run = {
        let mut last = state
            .last_subagent_reconcile
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        if now.duration_since(*last) < SUBAGENT_RECONCILE_MIN_INTERVAL {
            false
        } else {
            *last = now;
            true
        }
    };
    if should_run {
        reconcile_incomplete_subagents(state);
    }
}

pub(crate) fn build_snapshot(_app: &AppHandle, state: &AppState) -> IslandSnapshot {
    prune_active_subagents(state);
    // Clone each state component independently. No file, network, or process
    // operations are allowed in this path.
    let requests = lock_state(&state.requests).clone();
    let last_seen = lock_state(&state.session_last_seen).clone();
    let retention = *lock_state(&state.session_retention_secs);
    let token_usage = lock_state(&state.session_token_usage).clone();
    let token_usage_by_model = lock_state(&state.session_token_usage_by_model).clone();
    let known_sessions = lock_state(&state.known_sessions).clone();
    let pinned = lock_state(&state.pinned_sessions).clone();
    let cursor_subagent_conversation_ids: HashSet<String> =
        lock_state(&state.cursor_subagent_conversations)
            .keys()
            .cloned()
            .collect();
    let online = state
        .last_listening_online
        .lock()
        .ok()
        .and_then(|value| *value)
        .unwrap_or_else(capture::listening_online);
    let hook_health = state
        .last_hook_health
        .lock()
        .ok()
        .and_then(|value| value.clone())
        .unwrap_or_default();
    let mut snapshot = snapshot_from(
        &requests,
        &last_seen,
        retention,
        &token_usage,
        &known_sessions,
        &pinned,
        online,
        &cursor_subagent_conversation_ids,
    );
    let session_request_totals = lock_state(&state.session_request_totals).clone();
    for session in &mut snapshot.sessions {
        if let Some(total) = session_request_totals.get(&session.session_id) {
            session.total_count = session.total_count.max(*total);
        }
    }
    {
        let startup_floor = *state
            .startup_daily_floor
            .lock()
            .expect("state mutex poisoned");
        let startup_floor_by_model = state
            .startup_daily_floor_by_model
            .lock()
            .expect("state mutex poisoned")
            .clone();
        let absolute_sessions = state
            .absolute_token_sessions
            .lock()
            .expect("state mutex poisoned");
        snapshot.daily_tokens =
            effective_daily_tokens(&token_usage, startup_floor, &absolute_sessions);
        snapshot.daily_tokens_by_model = effective_daily_tokens_by_model(
            &token_usage_by_model,
            &startup_floor_by_model,
            &absolute_sessions,
        );
        let active_ids: HashSet<&str> = snapshot
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();
        snapshot.active_session_tokens_by_model =
            aggregate_usage_by_model(&token_usage_by_model, Some(&active_ids));
    }
    let subagent_retention = *lock_state(&state.subagent_retention_secs);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let active_subagents = lock_state(&state.active_subagents).clone();
    assign_active_subagents_to_sessions(
        &mut snapshot.sessions,
        &active_subagents,
        subagent_retention,
        now_secs,
    );
    persist_session_hosts(state, &snapshot.sessions);
    snapshot.hook_health = hook_health;

    snapshot
}

pub(crate) fn active_subagent_visible(
    subagent: &ActiveSubagent,
    subagent_retention: u64,
    now_secs: u64,
) -> bool {
    if subagent.archived {
        return false;
    }
    if subagent_retention > 0 {
        if let Some(ref completed) = subagent.completed_at {
            let completed_ts = parse_iso_timestamp_secs(completed);
            if now_secs.saturating_sub(completed_ts) >= subagent_retention {
                return false;
            }
        }
    }
    true
}

pub(crate) fn prune_active_subagents(state: &AppState) {
    let subagent_retention = *lock_state(&state.subagent_retention_secs);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut removed_conversations = Vec::new();

    {
        let mut subagents = lock_state(&state.active_subagents);
        subagents.retain(|subagent| {
            let keep = active_subagent_visible(subagent, subagent_retention, now_secs)
                || subagent.completed_at.is_none();
            if !keep {
                if let Some(conversation_id) = subagent.conversation_id.clone() {
                    removed_conversations.push(conversation_id);
                }
            }
            keep
        });

        let mut overflow = subagents.len().saturating_sub(MAX_ACTIVE_SUBAGENTS);
        if overflow > 0 {
            subagents.retain(|subagent| {
                let removable = subagent.archived || subagent.completed_at.is_some();
                if overflow > 0 && removable {
                    overflow -= 1;
                    if let Some(conversation_id) = subagent.conversation_id.clone() {
                        removed_conversations.push(conversation_id);
                    }
                    return false;
                }
                true
            });
        }
    }

    if !removed_conversations.is_empty() {
        if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
            for conversation_id in removed_conversations {
                map.remove(&conversation_id);
            }
        }
    }
}

pub(crate) fn subagent_summary_from_active(subagent: &ActiveSubagent) -> SubagentSummary {
    SubagentSummary {
        agent_id: subagent.agent_id.clone(),
        agent_type: subagent.agent_type.clone(),
        started_at: subagent.started_at.clone(),
        agent_transcript_path: subagent.agent_transcript_path.clone(),
        completed_at: subagent.completed_at.clone(),
        archived: subagent.archived,
        last_message: subagent.last_message.clone(),
    }
}

pub(crate) fn assign_active_subagents_to_sessions(
    sessions: &mut [SessionSummary],
    active_subagents: &[ActiveSubagent],
    subagent_retention: u64,
    now_secs: u64,
) {
    let mut subagents_by_session: HashMap<String, Vec<SubagentSummary>> = HashMap::new();
    for subagent in active_subagents.iter() {
        if !active_subagent_visible(subagent, subagent_retention, now_secs) {
            continue;
        }
        subagents_by_session
            .entry(subagent.session_id.clone())
            .or_default()
            .push(subagent_summary_from_active(subagent));
    }
    for session in sessions.iter_mut() {
        session.active_subagents = subagents_by_session
            .remove(&session.session_id)
            .unwrap_or_default();
    }
}

pub(crate) fn persist_session_hosts(state: &AppState, sessions: &[SessionSummary]) {
    for session in sessions {
        if matches!(
            session.session_host,
            platform::SessionHost::ClaudeDesktop
                | platform::SessionHost::ClaudeCli
                | platform::SessionHost::CodexDesktop
                | platform::SessionHost::CodexCli
                | platform::SessionHost::CursorIde
        ) {
            store_session_host(state, &session.session_id, session.session_host);
        }
    }
}

pub(crate) fn sync_listening_online_snapshot(app: &AppHandle, state: &AppState) {
    let online = compute_listening_online(app);
    let should_emit = {
        let mut last = state
            .last_listening_online
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = last.map(|previous| previous != online).unwrap_or(true);
        if changed {
            *last = Some(online);
        }
        changed
    };
    if should_emit {
        let snapshot = build_snapshot(app, state);
        let _ = app.emit("snapshot-changed", &snapshot);
    }
}

pub(crate) fn remember_hook_health(state: &AppState, hook_health: &HookHealthSnapshot) {
    if let Ok(mut last) = state.last_hook_health.lock() {
        *last = Some(hook_health.clone());
    }
}

pub(crate) fn sync_hook_health_snapshot(app: &AppHandle, state: &AppState) {
    let hook_health = build_hook_health(app);
    let should_emit = {
        let mut last = state
            .last_hook_health
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = last
            .as_ref()
            .map(|previous| previous != &hook_health)
            .unwrap_or(true);
        if changed {
            *last = Some(hook_health);
        }
        changed
    };
    if should_emit {
        let snapshot = build_snapshot(app, state);
        remember_hook_health(state, &snapshot.hook_health);
        let _ = app.emit("snapshot-changed", &snapshot);
    }
}

pub(crate) fn build_hook_health(app: &AppHandle) -> HookHealthSnapshot {
    if capture::force_hook_uninstalled() {
        return HookHealthSnapshot {
            claude: forced_uninstalled_status(app, &CLAUDE_HOOK_PROFILE),
            codex: forced_uninstalled_status(app, &CODEX_HOOK_PROFILE),
            cursor: forced_uninstalled_status(app, &CURSOR_HOOK_PROFILE),
            zcode: forced_uninstalled_status(app, &ZCODE_HOOK_PROFILE),
            gemini: forced_uninstalled_status(app, &GEMINI_HOOK_PROFILE),
        };
    }

    let claude_status = claude_hook_status(app);
    let codex_status = codex_hook_status(app);
    let cursor_status = cursor_hook_status(app);
    let zcode_status = zcode_hook_status(app);
    let gemini_status = gemini_hook_status(app);

    // #region agent log (diagA)
    crate::debug_agent::log(
        "H-F",
        "lib.rs:build_hook_health",
        "hook health snapshot",
        json!({
            "online": compute_listening_online(app),
            "bridgeReachable": hook_bridge::is_bridge_reachable(app),
            "claude": {
                "installed": claude_status.installed,
                "scriptFound": claude_status.script_found,
                "scriptPath": claude_status.script_path,
                "nodeFound": claude_status.node_found,
                "nodePath": claude_status.node_path,
            },
            "codex": {
                "installed": codex_status.installed,
                "scriptFound": codex_status.script_found,
                "scriptPath": codex_status.script_path,
                "nodeFound": codex_status.node_found,
                "nodePath": codex_status.node_path,
            },
            "cursor": {
                "installed": cursor_status.installed,
                "scriptFound": cursor_status.script_found,
                "scriptPath": cursor_status.script_path,
                "nodeFound": cursor_status.node_found,
                "nodePath": cursor_status.node_path,
            },
            "zcode": {
                "installed": zcode_status.installed,
                "scriptFound": zcode_status.script_found,
                "scriptPath": zcode_status.script_path,
                "nodeFound": zcode_status.node_found,
                "nodePath": zcode_status.node_path,
            },
            "gemini": {
                "installed": gemini_status.installed,
                "scriptFound": gemini_status.script_found,
                "scriptPath": gemini_status.script_path,
                "nodeFound": gemini_status.node_found,
                "nodePath": gemini_status.node_path,
            },
        }),
    );
    // #endregion

    HookHealthSnapshot {
        claude: claude_status,
        codex: codex_status,
        cursor: cursor_status,
        zcode: zcode_status,
        gemini: gemini_status,
    }
}

pub(crate) fn claude_hook_status(app: &AppHandle) -> HookStatus {
    let settings_path = claude_settings_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let settings = read_json_file(&settings_path);
    let installed = settings
        .as_ref()
        .map(|config| has_atoll_claude_hooks(config))
        .unwrap_or(false);
    if installed {
        refresh_deployed_hook_assets_if_needed(app, "atoll-claude-hook.mjs");
    }
    let (script_path, script_found) =
        resolve_hook_script_readiness(app, "atoll-claude-hook.mjs", settings.as_ref());
    let mut status = build_hook_status(
        installed,
        script_found,
        settings_path,
        script_path,
        settings.as_ref(),
        "atoll-claude-hook",
        "claude",
    );
    status.competing_hooks = settings
        .as_ref()
        .map(|cfg| detect_competing_claude_hooks(cfg))
        .unwrap_or_default();
    status
}

pub(crate) fn codex_hook_status(app: &AppHandle) -> HookStatus {
    let hooks_path = codex_hooks_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let config = read_json_file(&hooks_path);
    let installed = config
        .as_ref()
        .map(|hooks| has_atoll_codex_hooks(hooks))
        .unwrap_or(false);
    if installed {
        refresh_deployed_hook_assets_if_needed(app, "atoll-codex-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-codex-hook.mjs", config.as_ref());
    if installed {
        #[cfg(windows)]
        maybe_repair_hook_launcher_config(app, "atoll-codex-hook.mjs", "codex-hook-launcher.json");
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-codex-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-codex-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-codex-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-codex-hook.mjs",
        config.as_ref(),
        "atoll-codex-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        hooks_path,
        script_path,
        config.as_ref(),
        "atoll-codex-hook",
        "codex",
    )
}

pub(crate) fn zcode_hook_status(app: &AppHandle) -> HookStatus {
    let config_path = zcode_config_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let config = read_json_file(&config_path);
    let installed = config
        .as_ref()
        .map(|hooks| has_atoll_zcode_hooks(hooks))
        .unwrap_or(false);
    if installed {
        refresh_deployed_hook_assets_if_needed(app, "atoll-zcode-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-zcode-hook.mjs", config.as_ref());
    if installed {
        #[cfg(windows)]
        maybe_repair_hook_launcher_config(app, "atoll-zcode-hook.mjs", "zcode-hook-launcher.json");
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-zcode-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-zcode-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-zcode-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-zcode-hook.mjs",
        config.as_ref(),
        "atoll-zcode-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        config_path,
        script_path,
        config.as_ref(),
        "atoll-zcode-hook",
        "zcode",
    )
}

pub(crate) fn gemini_hook_status(app: &AppHandle) -> HookStatus {
    let settings_path = gemini_settings_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let settings = read_json_file(&settings_path);
    let installed = settings
        .as_ref()
        .map(|config| has_atoll_gemini_hooks(config))
        .unwrap_or(false);
    if installed {
        refresh_deployed_hook_assets_if_needed(app, "atoll-gemini-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-gemini-hook.mjs", settings.as_ref());
    if installed {
        if let (Some(cfg), Ok(preferred)) = (
            settings.as_ref(),
            resolve_install_hook_script_path(app, "atoll-gemini-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-gemini-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-gemini-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-gemini-hook.mjs",
        settings.as_ref(),
        "atoll-gemini-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        settings_path,
        script_path,
        settings.as_ref(),
        "atoll-gemini-hook",
        "gemini",
    )
}

pub(crate) fn cursor_hook_status(app: &AppHandle) -> HookStatus {
    let hooks_path = cursor_hooks_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut config = read_json_file(&hooks_path);
    let installed = config
        .as_ref()
        .map(|hooks| has_atoll_cursor_hooks(hooks))
        .unwrap_or(false);
    if installed {
        if let Some(repaired) = maybe_repair_cursor_hook_events(
            app,
            &hooks_path,
            config.as_ref(),
            &hook_bridge::cursor_hook_url_for_app(app),
        ) {
            config = Some(repaired);
        }
        refresh_deployed_hook_assets_if_needed(app, "atoll-cursor-hook.mjs");
    }
    let (mut script_path, mut script_found) =
        resolve_hook_script_readiness(app, "atoll-cursor-hook.mjs", config.as_ref());
    if installed {
        #[cfg(windows)]
        maybe_repair_hook_launcher_config(
            app,
            "atoll-cursor-hook.mjs",
            "cursor-hook-launcher.json",
        );
        if let (Some(cfg), Ok(preferred)) = (
            config.as_ref(),
            resolve_install_hook_script_path(app, "atoll-cursor-hook.mjs"),
        ) {
            if let Some(configured) = configured_atoll_hook_script_path(cfg, "atoll-cursor-hook") {
                if should_flag_dev_hook_drift(&configured, &preferred)
                    && deployed_hook_script_path("atoll-cursor-hook.mjs").is_none()
                {
                    script_found = false;
                }
            }
        }
    }
    script_path = canonical_hook_script_path(
        app,
        "atoll-cursor-hook.mjs",
        config.as_ref(),
        "atoll-cursor-hook",
        &script_path,
    );
    if !script_path.is_empty() && std::path::Path::new(&script_path).is_file() {
        script_found = true;
    }
    build_hook_status(
        installed,
        script_found,
        hooks_path,
        script_path,
        config.as_ref(),
        "atoll-cursor-hook",
        "cursor",
    )
}

pub(crate) fn is_dev_hook_script_path(path: &str) -> bool {
    path.contains("/target/debug/")
        || path.contains("/target/release/")
        || path.contains("/src-tauri/target/")
}

/// Compare two hook script paths in a separator- and prefix-agnostic way.
/// `configured` arrives from hooks.json with forward slashes (written by
/// `normalize_hook_command_path`), while `preferred` comes from
/// `dunce::simplified(PathBuf)`. On Windows `dunce::simplified` keeps the
/// `\\?\` verbatim prefix for paths containing non-ASCII characters (e.g. a user
/// home directory like `C:\Users\杨帅`), so `preferred` can look like
/// `\\?\C:\Users\杨帅\...\atoll-codex-hook.mjs` while `configured` is
/// `C:/Users/杨帅/...\atoll-codex-hook.mjs`. A naive `!=` (or even `Path` equality,
/// which treats verbatim and drive prefixes as distinct components) would always
/// report them as different and falsely flag dev-path drift, flipping
/// `script_found` to false. Normalizing both sides by stripping the verbatim
/// prefix and unifying separators makes the same file compare equal.
pub(crate) fn dev_hook_paths_differ(configured: &str, preferred: &str) -> bool {
    fn normalize(p: &str) -> String {
        let stripped = p.strip_prefix(r"\\?\").unwrap_or(p);
        stripped.replace('\\', "/")
    }
    normalize(configured) != normalize(preferred)
}

/// True when hooks.json still points at a stale dev build path that no longer exists,
/// while the running app bundle exposes a valid replacement script.
pub(crate) fn should_flag_dev_hook_drift(configured: &str, preferred: &str) -> bool {
    if !is_dev_hook_script_path(configured) {
        return false;
    }
    if !dev_hook_paths_differ(configured, preferred) {
        return false;
    }
    if !std::path::Path::new(preferred).is_file() {
        return false;
    }
    // Configured path still works for the hook host — not drift.
    if std::path::Path::new(configured).is_file() {
        return false;
    }
    true
}
pub(crate) struct ZcodeSubagentMeta {
    pub(crate) child_session_id: String,
    pub(crate) is_active: bool,
    pub(crate) agent_type: String,
    pub(crate) started_at: String,
    pub(crate) completed_at: Option<String>,
    pub(crate) last_message: Option<String>,
}

/// ZCode has no subagent hook events, so subagent lifecycle and token usage
/// are derived from on-disk artifacts instead: per-subagent
/// `~/.zcode/cli/agents/<parent>/agent_*/metadata.json` files (lifecycle +
/// child session id) and each child's own model-I/O rollout (usage).
pub(crate) fn refresh_zcode_subagents(
    app: &AppHandle,
    state: &AppState,
    parent_session_id: &str,
    today_key: &str,
) {
    if update_zcode_subagents(state, parent_session_id, today_key) {
        emit_subagent_snapshot(app, state);
    }
}

pub(crate) fn update_zcode_subagents(
    state: &AppState,
    parent_session_id: &str,
    today_key: &str,
) -> bool {
    let metas = collect_zcode_subagent_metas(parent_session_id);
    if metas.is_empty() {
        return false;
    }

    for meta in &metas {
        add_zcode_subagent_usage(state, parent_session_id, &meta.child_session_id, today_key);
    }

    sync_zcode_subagent_chips(state, parent_session_id, &metas)
}

pub(crate) fn collect_zcode_subagent_metas(parent_session_id: &str) -> Vec<ZcodeSubagentMeta> {
    let Some(agents_dir) = zcode_session_agents_dir(parent_session_id) else {
        return Vec::new();
    };
    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(entries) => entries,
        // No subagents for this session (or no ZCode data dir at all).
        Err(_) => return Vec::new(),
    };

    let mut metas = Vec::new();
    for entry in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("metadata.json")) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(child_session_id) = value.get("childSessionId").and_then(Value::as_str) else {
            continue;
        };
        if !is_safe_zcode_session_id(child_session_id) {
            continue;
        }

        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        // Anything final (completed, error, aborted, ...) closes the chip;
        // only unknown or in-flight statuses count as running.
        let is_active = status.is_empty()
            || matches!(
                status.as_str(),
                "running" | "pending" | "in_progress" | "started"
            );
        let agent_type = value
            .get("profileSnapshot")
            .and_then(|profile| profile.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("subagent")
            .to_string();
        let started_at = value
            .get("createdAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut completed_at = value
            .get("completedAt")
            .and_then(Value::as_str)
            .filter(|stamp| !stamp.is_empty())
            .map(str::to_string);
        if completed_at.is_none() && !is_active {
            completed_at = Some(iso_timestamp_now());
        }
        let last_message = value
            .get("prompt")
            .and_then(Value::as_str)
            .map(|prompt| prompt.chars().take(200).collect::<String>());

        metas.push(ZcodeSubagentMeta {
            child_session_id: child_session_id.to_string(),
            is_active,
            agent_type,
            started_at,
            completed_at,
            last_message,
        });
    }
    metas
}

/// Add a subagent's rollout usage to the parent session's totals.
///
/// Subagent usage is always merged additively (never via component_wise_max):
/// byte offsets and session usage reset together in
/// `roll_over_token_usage_if_needed`, and rollouts are append-only, so a
/// rescan after an offset reset only re-counts the current day's lines onto
/// an already-cleared day.
pub(crate) fn add_zcode_subagent_usage(
    state: &AppState,
    parent_session_id: &str,
    child_session_id: &str,
    today_key: &str,
) {
    let Some(rollout) = zcode_rollout_path(child_session_id) else {
        return;
    };
    let rollout_path = rollout.to_string_lossy().into_owned();

    let last_offset = state
        .token_usage_file_offsets
        .lock()
        .ok()
        .and_then(|offsets| offsets.get(&rollout_path).copied())
        .unwrap_or(0);
    let Ok((usage, usage_by_model, next_offset, _)) =
        parse_zcode_token_usage_from_transcript(&rollout_path, last_offset, today_key)
    else {
        return;
    };
    if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
        offsets.insert(rollout_path, next_offset);
    }
    if usage.is_zero() && usage_by_model.is_empty() {
        return;
    }

    if let Ok(mut usage_by_session) = state.session_token_usage.lock() {
        usage_by_session
            .entry(parent_session_id.to_string())
            .or_default()
            .add_assign(usage);
    }
    if !usage_by_model.is_empty() {
        if let Ok(mut usage_by_model_state) = state.session_token_usage_by_model.lock() {
            let model_entry = usage_by_model_state
                .entry(parent_session_id.to_string())
                .or_default();
            merge_session_model_usage(model_entry, &usage_by_model, false);
        }
    }
    state.token_history_dirty.store(true, Ordering::Release);
}

/// Keep the parent session's subagent chips in sync with on-disk metadata.
///
/// Only subagents discovered while still running become chips; ones that
/// finished before Atoll saw them stay invisible to avoid a flood of stale
/// chips after restart.
pub(crate) fn sync_zcode_subagent_chips(
    state: &AppState,
    parent_session_id: &str,
    metas: &[ZcodeSubagentMeta],
) -> bool {
    let mut changed = false;
    let Ok(mut subagents) = state.active_subagents.lock() else {
        return false;
    };

    for meta in metas {
        if let Some(existing) = subagents
            .iter_mut()
            .find(|s| s.agent_id == meta.child_session_id)
        {
            if existing.completed_at.is_none() {
                if let Some(completed_at) = meta.completed_at.clone() {
                    existing.completed_at = Some(completed_at);
                    changed = true;
                }
            }
            continue;
        }

        if !meta.is_active || subagents.len() >= MAX_ACTIVE_SUBAGENTS {
            continue;
        }
        subagents.push(ActiveSubagent {
            agent_id: meta.child_session_id.clone(),
            session_id: parent_session_id.to_string(),
            agent_kind: AgentKind::Zcode,
            agent_type: meta.agent_type.clone(),
            started_at: if meta.started_at.is_empty() {
                iso_timestamp_now()
            } else {
                meta.started_at.clone()
            },
            agent_transcript_path: zcode_db_session_path(&meta.child_session_id),
            completed_at: None,
            archived: false,
            last_message: meta.last_message.clone(),
            conversation_id: None,
        });
        changed = true;
    }

    changed
}

/// Ingest token usage from a Cursor hook payload (`stop`, `afterAgentResponse`, etc.).
///
/// Cursor's JSONL transcript doesn't embed usage data; hook payloads may carry
/// `input_tokens`, `output_tokens`, `cache_read_tokens`, and `cache_write_tokens`
/// for the turn that just completed.
///
/// Fields may appear at the top level or nested under a `token_usage` object
/// depending on Cursor version.  Cursor reports `input_tokens` as the total
/// (cache_read + cache_write + fresh); we store the raw values and let the
/// display layer decide whether to decompose them.

pub(crate) fn cursor_payload_has_token_usage(payload: &serde_json::Value) -> bool {
    !parse_cursor_token_usage_from_payload(payload).is_zero()
}

pub(crate) fn remember_cursor_lifecycle_token_session(state: &AppState, session_id: &str) {
    if let Ok(mut sessions) = state.cursor_lifecycle_token_sessions.lock() {
        sessions.insert(session_id.to_string());
    }
}

pub(crate) fn cursor_lifecycle_token_seen(state: &AppState, session_id: &str) -> bool {
    state
        .cursor_lifecycle_token_sessions
        .lock()
        .map(|sessions| sessions.contains(session_id))
        .unwrap_or(false)
}

pub(crate) fn first_json_u64(source: &serde_json::Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| json_value_as_u64(source.get(*key)))
        .unwrap_or(0)
}

pub(crate) fn first_json_string(source: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| source.get(*key).and_then(Value::as_str).map(str::to_string))
}

pub(crate) fn extract_cursor_model(payload: &serde_json::Value) -> String {
    first_json_string(
        payload,
        &[
            "model",
            "modelName",
            "model_name",
            "model_id",
            "modelId",
            "model_slug",
            "modelSlug",
        ],
    )
    .or_else(|| {
        payload.get("response").and_then(|response| {
            first_json_string(
                response,
                &["model", "modelName", "model_name", "model_id", "modelId"],
            )
        })
    })
    .unwrap_or_else(|| pricing::UNKNOWN_MODEL.to_string())
}

/// Parse a JSON value as u64, accepting integers, floats, and numeric strings.
pub(crate) fn json_value_as_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    if let Some(f) = value.as_f64() {
        if f >= 0.0 && f <= u64::MAX as f64 {
            return Some(f as u64);
        }
    }
    if let Some(s) = value.as_str() {
        return s.parse::<u64>().ok();
    }
    None
}
pub(crate) fn archive_subagent_in_state(state: &AppState, agent_id: &str) -> Option<String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        subagents
            .iter_mut()
            .find(|s| s.agent_id == agent_id)
            .and_then(|sub| {
                let conv_id = sub.conversation_id.clone();
                sub.archived = true;
                conv_id
            })
    } else {
        None
    }
}

pub(crate) fn archive_completed_subagents_in_state(
    state: &AppState,
    session_id: &str,
) -> Vec<String> {
    if let Ok(mut subagents) = state.active_subagents.lock() {
        let mut conv_ids = Vec::new();
        for sub in subagents.iter_mut() {
            if sub.session_id == session_id && sub.completed_at.is_some() && !sub.archived {
                if let Some(conv_id) = sub.conversation_id.clone() {
                    conv_ids.push(conv_id);
                }
                sub.archived = true;
            }
        }
        conv_ids
    } else {
        Vec::new()
    }
}

#[tauri::command]
pub(crate) fn archive_subagent(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<IslandSnapshot, String> {
    let conv_id = archive_subagent_in_state(state.inner(), &agent_id);
    if let Some(conv_id) = conv_id {
        unbind_cursor_subagent_conversation(state.inner(), Some(&conv_id));
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn archive_completed_subagents(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<IslandSnapshot, String> {
    let conv_ids = archive_completed_subagents_in_state(state.inner(), &session_id);
    for conv_id in conv_ids {
        unbind_cursor_subagent_conversation(state.inner(), Some(&conv_id));
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

/// Codex background threads (memories, subagents, etc.) often omit `cwd` in hook payloads.
/// Atoll defaults missing cwd to `"."`, which would show up as a stray "." session.
/// Resolve the real workspace from `transcript_path` when possible, and only ignore
/// known Codex-internal directories under `~/.codex/`.
const CODEX_INTERNAL_DIR_NAMES: &[&str] = &[
    "memories",
    "process_manager",
    "computer-use",
    "computer-use-turn-ended",
];

pub(crate) fn normalize_codex_cwd(cwd: &str) -> String {
    cwd.replace('\\', "/")
}

pub(crate) fn resolve_codex_session_cwd(cwd: &str, transcript_path: Option<&str>) -> String {
    let normalized = normalize_codex_cwd(cwd);
    if !normalized.is_empty() && normalized != "." && normalized != "./" {
        return normalized;
    }

    transcript_path
        .and_then(transcript::read_codex_cwd_from_transcript)
        .map(|resolved| normalize_codex_cwd(&resolved))
        .filter(|resolved| !resolved.is_empty())
        .unwrap_or(normalized)
}

pub(crate) fn is_codex_internal_cwd(cwd: &str) -> bool {
    let normalized = normalize_codex_cwd(cwd);
    if normalized.is_empty() || normalized == "." || normalized == "./" {
        return true;
    }

    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let codex_home = normalize_codex_cwd(&home.join(".codex").to_string_lossy());

    for dir_name in CODEX_INTERNAL_DIR_NAMES {
        let internal = format!("{codex_home}/{dir_name}");
        if normalized == internal || normalized.starts_with(&(internal.clone() + "/")) {
            return true;
        }
    }

    false
}

pub(crate) fn is_codex_internal_session(
    agent: &AgentKind,
    cwd: &str,
    transcript_path: Option<&str>,
) -> bool {
    if !matches!(agent, AgentKind::Codex) {
        return false;
    }

    let resolved = resolve_codex_session_cwd(cwd, transcript_path);
    is_codex_internal_cwd(&resolved)
}

pub(crate) fn purge_tracked_session(
    state: &AppState,
    session_id: &str,
    transcript_path: Option<&str>,
) {
    if let Ok(mut known) = state.known_sessions.lock() {
        known.remove(session_id);
    }
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.remove(session_id);
    }
    if let Ok(mut totals) = state.session_request_totals.lock() {
        totals.remove(session_id);
    }
    // Keep session_token_usage so auto-archived / retention-purged sessions still
    // count toward daily totals until UTC day rollover.
    if let Some(path) = transcript_path {
        if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
            offsets.remove(path);
        }
    }
}

pub(crate) fn register_known_session(
    state: &AppState,
    session_id: &str,
    agent: AgentKind,
    cwd: &str,
    transcript_path: Option<&str>,
) {
    let resolved_cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(cwd, transcript_path),
        _ => cwd.to_string(),
    };

    if is_codex_internal_session(&agent, &resolved_cwd, None) {
        purge_tracked_session(state, session_id, transcript_path);
        return;
    }
    if let Ok(mut known) = state.known_sessions.lock() {
        let entry = known
            .entry(session_id.to_string())
            .or_insert_with(|| KnownSession {
                agent: agent.clone(),
                cwd: resolved_cwd.clone(),
                transcript_path: transcript_path.map(str::to_string),
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            });
        if !resolved_cwd.is_empty() && resolved_cwd != "." {
            entry.cwd = resolved_cwd.clone();
        }
        if let Some(path) = transcript_path {
            entry.transcript_path = Some(path.to_string());
        }
    }
    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky
            .entry(session_id.to_string())
            .or_insert_with(|| token_history::agent_kind_key(&agent));
    }
}

pub(crate) fn claude_session_host(
    state: &AppState,
    session_id: &str,
    cwd: &str,
) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
            if let Some(path) = entry.transcript_path.as_deref() {
                if let Some(host) = host_from_claude_transcript_path(path) {
                    drop(known);
                    store_session_host(state, session_id, host);
                    return host;
                }
            }
        }
    }

    let detected = platform::detect_claude_session_host(cwd);
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn codex_session_host(
    state: &AppState,
    session_id: &str,
    cwd: &str,
) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
            if let Some(path) = entry.transcript_path.as_deref() {
                if let Some(host) = host_from_codex_transcript_path(path) {
                    drop(known);
                    store_session_host(state, session_id, host);
                    return host;
                }
            }
        }
    }

    let detected = platform::detect_codex_session_host(cwd);
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn cursor_session_host(state: &AppState, session_id: &str) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
        }
    }

    let detected = platform::detect_cursor_session_host();
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn zcode_session_host(
    state: &AppState,
    session_id: &str,
    cwd: &str,
) -> platform::SessionHost {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if entry.host != platform::SessionHost::Unknown {
                return entry.host;
            }
        }
    }

    let detected = platform::detect_zcode_session_host(cwd);
    if detected != platform::SessionHost::Unknown {
        store_session_host(state, session_id, detected);
    }
    detected
}

pub(crate) fn store_session_host(state: &AppState, session_id: &str, host: platform::SessionHost) {
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.host = host;
        }
    }
}

pub(crate) fn session_host_for_summary(
    known_sessions: &HashMap<String, KnownSession>,
    session_id: &str,
    _cwd: &str,
    agent: &AgentKind,
) -> platform::SessionHost {
    match agent {
        AgentKind::Claude => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
                if let Some(path) = entry.transcript_path.as_deref() {
                    if let Some(host) = host_from_claude_transcript_path(path) {
                        return host;
                    }
                }
            }
            platform::SessionHost::Unknown
        }
        AgentKind::Codex => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
                if let Some(path) = entry.transcript_path.as_deref() {
                    if let Some(host) = host_from_codex_transcript_path(path) {
                        return host;
                    }
                }
            }
            platform::SessionHost::Unknown
        }
        AgentKind::Cursor => {
            if let Some(entry) = known_sessions.get(session_id) {
                if entry.host != platform::SessionHost::Unknown {
                    return entry.host;
                }
            }
            platform::SessionHost::Unknown
        }
        _ => platform::SessionHost::Unknown,
    }
}

/// Resolve unknown session hosts away from snapshot/IPC paths. Platform host
/// detection may launch `ps`, `lsof`, or `tasklist`, so it belongs on the
/// maintenance worker rather than the webview thread.
pub(crate) fn refresh_unknown_session_hosts(state: &AppState) {
    let candidates: Vec<(String, AgentKind, String)> = state
        .known_sessions
        .lock()
        .ok()
        .map(|known| {
            known
                .iter()
                .filter(|(_, info)| info.host == platform::SessionHost::Unknown)
                .take(16)
                .map(|(id, info)| (id.clone(), info.agent.clone(), info.cwd.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (session_id, agent, cwd) in candidates {
        let host = match agent {
            AgentKind::Claude => platform::detect_claude_session_host(&cwd),
            AgentKind::Codex => platform::detect_codex_session_host(&cwd),
            AgentKind::Cursor => platform::detect_cursor_session_host(),
            _ => platform::SessionHost::Unknown,
        };
        if host != platform::SessionHost::Unknown {
            store_session_host(state, &session_id, host);
        }
    }
}

pub(crate) fn host_from_claude_transcript_path(path: &str) -> Option<platform::SessionHost> {
    if path.contains("/Application Support/") && !path.contains("/.claude/") {
        return Some(platform::SessionHost::ClaudeDesktop);
    }
    if path.contains("Claude-3p")
        || path.contains("local-agent-mode-sessions")
        || path.contains("com.anthropic.claude")
        || path.contains("agent-sessions")
    {
        return Some(platform::SessionHost::ClaudeDesktop);
    }
    // /.claude/projects/ is used by BOTH Claude CLI and Claude Desktop (newer versions).
    // Only treat it as CLI if Claude Desktop is definitely not running.
    if path.contains("/.claude/")
        || (path.contains("/claude/projects/") && !path.contains("/Application Support/"))
    {
        if !platform::is_claude_desktop_app_running() {
            return Some(platform::SessionHost::ClaudeCli);
        }
        // Ambiguous: Desktop is running and path looks like CLI — return None
        // so the caller uses other detection methods.
        return None;
    }
    None
}

pub(crate) fn host_from_codex_transcript_path(path: &str) -> Option<platform::SessionHost> {
    if path.contains("com.openai.codex")
        || (path.contains("/Application Support/") && path.contains("codex"))
    {
        return Some(platform::SessionHost::CodexDesktop);
    }
    if path.contains("/.codex/sessions/") || path.contains("/.codex/") {
        if !platform::is_codex_desktop_app_running() {
            return Some(platform::SessionHost::CodexCli);
        }
        return None;
    }
    None
}

pub(crate) fn touch_session_last_seen(state: &AppState, session_id: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.insert(session_id.to_string(), now);
    }
}

/// Bumps retention clocks for user-visible session activity (turn end, approvals).
pub(crate) fn touch_session_activity(state: &AppState, session_id: &str) {
    touch_session_last_seen(state, session_id);
    let now_iso = iso_timestamp_now();
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.last_activity = now_iso;
        }
    }
}

pub(crate) fn payload_subagent_id(payload: &Value) -> Option<&str> {
    payload
        .get("agent_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("subagent_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("tool_call_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_parent_session_id(payload: &Value) -> Option<&str> {
    payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("sessionId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("parent_conversation_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("parentConversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("conversation_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            payload
                .get("conversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_type(payload: &Value) -> &str {
    payload
        .get("agent_type")
        .and_then(Value::as_str)
        .or_else(|| payload.get("subagent_type").and_then(Value::as_str))
        .unwrap_or("unknown")
}

pub(crate) fn payload_subagent_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("agent_transcript_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("agentTranscriptPath")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_main_transcript_path(payload: &Value) -> Option<&str> {
    payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("transcriptPath")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

pub(crate) fn payload_subagent_last_message(payload: &Value) -> Option<String> {
    payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .or_else(|| payload.get("summary").and_then(Value::as_str))
        .map(|s| s.chars().take(200).collect())
}

pub(crate) fn payload_conversation_id(payload: &Value) -> Option<&str> {
    payload
        .get("conversation_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("conversationId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
}

/// Cursor stores project folders under `~/.cursor/projects/{slug}/` where macOS
/// absolute paths become slugs like `Users-me-code-Atoll`.
pub(crate) fn decode_cursor_project_slug(slug: &str) -> Option<String> {
    if slug.is_empty() || slug == "empty-window" {
        return None;
    }
    if slug.starts_with("Users-") {
        let candidate = format!("/Users/{}", slug["Users-".len()..].replace('-', "/"));
        if std::path::Path::new(&candidate).is_dir() {
            return Some(candidate);
        }
    }
    #[cfg(windows)]
    if slug.len() > 2 {
        let drive = slug.as_bytes()[0] as char;
        if drive.is_ascii_alphabetic() && slug.as_bytes()[1] == b'-' {
            let candidate = format!(
                "{}:\\{}",
                drive.to_ascii_uppercase(),
                slug[2..].replace('-', "\\")
            );
            if std::path::Path::new(&candidate).is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Keep Cursor hooks.json env URLs aligned with the running bridge port.
pub(crate) fn sync_cursor_hook_bridge_urls(app: &AppHandle, port: u16) {
    let hooks_path = match cursor_hooks_path() {
        Some(path) => path,
        None => return,
    };
    let path_str = hooks_path.to_string_lossy();
    let Some(mut config) = read_json_file(&path_str) else {
        return;
    };
    if !has_atoll_cursor_hooks(&config) {
        return;
    }
    let cursor_url = hook_bridge::cursor_hook_url(port);
    let Some(hooks_obj) = config.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let mut updated = false;
    for entries in hooks_obj.values_mut() {
        let Some(arr) = entries.as_array_mut() else {
            continue;
        };
        for entry in arr.iter_mut() {
            if !hook_entry_has_atoll_cursor(entry) {
                continue;
            }
            let Some(obj) = entry.as_object_mut() else {
                continue;
            };
            let mut env = obj
                .get("env")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let current = env
                .get("ATOLL_HOOK_URL")
                .and_then(Value::as_str)
                .unwrap_or("");
            if current != cursor_url {
                env.insert(
                    "ATOLL_HOOK_URL".to_string(),
                    Value::String(cursor_url.clone()),
                );
                obj.insert("env".to_string(), Value::Object(env));
                updated = true;
            }
            if obj.get("timeout").and_then(Value::as_u64) != Some(CURSOR_HOOK_TIMEOUT_SECONDS) {
                obj.insert("timeout".to_string(), json!(CURSOR_HOOK_TIMEOUT_SECONDS));
                updated = true;
            }
        }
    }
    if !updated {
        return;
    }
    let formatted = match serde_json::to_string_pretty(&config) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Atoll failed to serialize Cursor hooks for URL sync: {error}");
            return;
        }
    };
    if let Err(error) = std::fs::write(&hooks_path, formatted) {
        eprintln!("Atoll failed to write Cursor hooks for URL sync: {error}");
        return;
    }
    eprintln!("Atoll synced Cursor hook URLs to {cursor_url}");
    refresh_hook_health_cache(app, &app.state::<AppState>());
    // #region agent log
    crate::debug_agent::log(
        "H-C",
        "lib.rs:sync_cursor_hook_bridge_urls",
        "synced cursor hook env urls",
        json!({
            "port": port,
            "cursorUrl": cursor_url,
            "hooksPath": path_str,
        }),
    );
    // #endregion
}

pub(crate) fn payload_cursor_lookup_id(payload: &Value) -> Option<&str> {
    payload_conversation_id(payload).or_else(|| {
        payload
            .get("session_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                payload
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
            })
    })
}

/// Prefer full composer UUID for Cursor session keys (matches on-disk transcript dirs).
pub(crate) fn payload_cursor_session_id(payload: &Value) -> Option<&str> {
    payload_cursor_lookup_id(payload)
}

pub(crate) const CURSOR_TRANSCRIPT_PREFIX_MIN_LEN: usize = 6;

/// Locate a Cursor composer transcript on disk and infer its workspace cwd.
pub(crate) fn discover_cursor_agent_transcript(lookup_id: &str) -> Option<(String, String)> {
    if lookup_id.is_empty() {
        return None;
    }
    if let Some(found) = discover_cursor_agent_transcript_exact(lookup_id) {
        return Some(found);
    }
    if lookup_id.len() >= CURSOR_TRANSCRIPT_PREFIX_MIN_LEN {
        return discover_cursor_agent_transcript_by_prefix(lookup_id);
    }
    None
}

pub(crate) fn discover_cursor_agent_transcript_exact(
    conversation_id: &str,
) -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let projects = home.join(".cursor").join("projects");
    if !projects.is_dir() {
        return None;
    }
    let relative = std::path::PathBuf::from("agent-transcripts")
        .join(conversation_id)
        .join(format!("{conversation_id}.jsonl"));
    for entry in std::fs::read_dir(&projects).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let transcript = entry.path().join(&relative);
        if !transcript.is_file() {
            continue;
        }
        let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
            .unwrap_or_else(|| ".".to_string());
        return Some((transcript.to_string_lossy().into_owned(), workspace));
    }
    None
}

pub(crate) fn discover_cursor_agent_transcript_by_prefix(prefix: &str) -> Option<(String, String)> {
    let home = dirs::home_dir()?;
    let projects = home.join(".cursor").join("projects");
    if !projects.is_dir() {
        return None;
    }

    let mut best: Option<(String, String, usize)> = None;
    for entry in std::fs::read_dir(&projects).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let transcripts_dir = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts_dir) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            if !conv_id.starts_with(prefix) {
                continue;
            }
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() {
                continue;
            }
            let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
                .unwrap_or_else(|| ".".to_string());
            let path = jsonl.to_string_lossy().into_owned();
            let score = conv_id.len();
            if best
                .as_ref()
                .map(|(_, _, len)| score > *len)
                .unwrap_or(true)
            {
                best = Some((path, workspace, score));
            }
        }
    }

    best.map(|(path, workspace, _)| (path, workspace))
}

pub(crate) fn is_unresolved_cursor_cwd(cwd: &str) -> bool {
    cwd.is_empty() || cwd == "."
}

/// Fill missing Cursor cwd/transcript from on-disk agent transcripts.
pub(crate) fn backfill_cursor_session_metadata(state: &AppState) {
    let sessions_to_backfill: Vec<(String, Option<String>)> = state
        .known_sessions
        .lock()
        .ok()
        .map(|known| {
            known
                .iter()
                .filter(|(_, info)| matches!(info.agent, AgentKind::Cursor))
                .filter(|(_, info)| {
                    info.transcript_path.is_none() || is_unresolved_cursor_cwd(&info.cwd)
                })
                .map(|(id, info)| (id.clone(), info.conversation_id.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (session_id, conversation_id) in sessions_to_backfill {
        let lookup_id = conversation_id.as_deref().unwrap_or(session_id.as_str());
        let Some((path, workspace)) = discover_cursor_agent_transcript(lookup_id) else {
            continue;
        };
        if let Ok(mut known) = state.known_sessions.lock() {
            if let Some(entry) = known.get_mut(&session_id) {
                if entry.transcript_path.is_none() {
                    entry.transcript_path = Some(path);
                }
                if is_unresolved_cursor_cwd(&entry.cwd) && !is_unresolved_cursor_cwd(&workspace) {
                    entry.cwd = workspace;
                }
                if entry.conversation_id.is_none() {
                    if let Some(stem) = entry
                        .transcript_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).parent())
                        .and_then(|p| p.file_name())
                        .and_then(|name| name.to_str())
                    {
                        entry.conversation_id = Some(stem.to_string());
                    }
                }
            }
        }
    }
}

pub(crate) fn sanitize_subagent_id_for_filename(agent_id: &str) -> String {
    agent_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn cursor_subagents_dir(main_transcript: &str) -> Option<std::path::PathBuf> {
    std::path::Path::new(main_transcript)
        .parent()
        .map(|parent| parent.join("subagents"))
}

pub(crate) fn subagent_transcript_filename_candidates(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(conv) = conversation_id.filter(|value| !value.is_empty()) {
        candidates.push(format!("{conv}.jsonl"));
    }
    let sanitized = sanitize_subagent_id_for_filename(agent_id);
    if agent_id.starts_with("agent-") {
        candidates.push(format!("{agent_id}.jsonl"));
    } else {
        candidates.push(format!("agent-{agent_id}.jsonl"));
    }
    candidates.push(format!("{agent_id}.jsonl"));
    if sanitized.starts_with("agent-") {
        candidates.push(format!("{sanitized}.jsonl"));
    } else {
        candidates.push(format!("agent-{sanitized}.jsonl"));
    }
    candidates.push(format!("{sanitized}.jsonl"));
    candidates.sort();
    candidates.dedup();
    candidates
}

pub(crate) fn scan_subagents_dir_for_transcript(
    subagents_dir: &std::path::Path,
    started_at: Option<&str>,
) -> Option<String> {
    if !subagents_dir.is_dir() {
        return None;
    }
    let started_ts = started_at.map(parse_iso_timestamp_secs);
    let mut matches: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(subagents_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            if let Some(started_ts) = started_ts {
                if modified + 2 < started_ts {
                    return None;
                }
            }
            Some((modified, path))
        })
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    matches
        .first()
        .map(|(_, path)| path.to_string_lossy().into_owned())
}

pub(crate) fn derive_subagent_transcript_path(
    main_transcript: Option<&str>,
    agent_id: &str,
    conversation_id: Option<&str>,
    started_at: Option<&str>,
) -> Option<String> {
    let main = main_transcript?;
    let subagents_dir = cursor_subagents_dir(main)?;

    for filename in subagent_transcript_filename_candidates(agent_id, conversation_id) {
        let path = subagents_dir.join(&filename);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    if let Some(path) = scan_subagents_dir_for_transcript(&subagents_dir, started_at) {
        return Some(path);
    }

    if let Some(conv) = conversation_id.filter(|value| !value.is_empty()) {
        return Some(
            subagents_dir
                .join(format!("{conv}.jsonl"))
                .to_string_lossy()
                .into_owned(),
        );
    }

    None
}

pub(crate) fn known_session_transcript_path(state: &AppState, session_id: &str) -> Option<String> {
    state
        .known_sessions
        .lock()
        .ok()
        .and_then(|known| known_session_transcript_path_from_map(&known, session_id))
}

pub(crate) fn known_session_transcript_path_from_map(
    known_sessions: &HashMap<String, KnownSession>,
    session_id: &str,
) -> Option<String> {
    known_sessions
        .get(session_id)
        .and_then(|entry| entry.transcript_path.clone())
}

pub(crate) fn refreshed_subagent_transcript_path(
    main_transcript: Option<&str>,
    sub: &ActiveSubagent,
) -> Option<String> {
    let Some(resolved) = derive_subagent_transcript_path(
        main_transcript,
        &sub.agent_id,
        sub.conversation_id.as_deref(),
        Some(&sub.started_at),
    ) else {
        return None;
    };
    let current_missing = sub
        .agent_transcript_path
        .as_ref()
        .is_none_or(|path| !std::path::Path::new(path).exists());
    let resolved_exists = std::path::Path::new(&resolved).exists();
    if current_missing && (resolved_exists || sub.conversation_id.is_some()) {
        Some(resolved)
    } else {
        None
    }
}

pub(crate) fn resolve_complete_transcript_path_from_main(
    main_transcript: Option<&str>,
    sub: &ActiveSubagent,
    payload_path: Option<String>,
) -> Option<String> {
    if let Some(path) = payload_path.filter(|value| !value.is_empty()) {
        return Some(path);
    }
    derive_subagent_transcript_path(
        main_transcript,
        &sub.agent_id,
        sub.conversation_id.as_deref(),
        Some(&sub.started_at),
    )
}

pub(crate) fn bind_cursor_subagent_conversation(
    state: &AppState,
    conv_id: &str,
    parent_session_id: &str,
) {
    if conv_id.is_empty() || conv_id == parent_session_id {
        return;
    }
    if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
        map.insert(conv_id.to_string(), parent_session_id.to_string());
    }
    // Rewrite any requests that were attributed to the subagent's own conversation
    // so they do not surface as a duplicate top-level session row.
    if let Ok(mut requests) = state.requests.lock() {
        for request in requests.iter_mut() {
            if request.session == conv_id {
                request.session = parent_session_id.to_string();
            }
        }
    }
    purge_tracked_session(state, conv_id, None);
}

pub(crate) fn unbind_cursor_subagent_conversation(state: &AppState, conv_id: Option<&str>) {
    if let Some(conv_id) = conv_id {
        if let Ok(mut map) = state.cursor_subagent_conversations.lock() {
            map.remove(conv_id);
        }
    }
}

/// Resolve a Cursor hook payload to its parent session when the event belongs to a subagent.
pub(crate) fn resolve_cursor_session_for_payload(
    state: &AppState,
    payload: &Value,
) -> Option<String> {
    if let Some(agent_id) = payload_subagent_id(payload) {
        if let Ok(subagents) = state.active_subagents.lock() {
            if let Some(sub) = subagents
                .iter()
                .find(|s| s.agent_id == agent_id && !s.archived && s.completed_at.is_none())
            {
                return Some(sub.session_id.clone());
            }
        }
    }

    let conv_id = payload_conversation_id(payload)?;

    if let Ok(map) = state.cursor_subagent_conversations.lock() {
        if let Some(parent) = map.get(conv_id) {
            return Some(parent.clone());
        }
    }

    let subagents = state.active_subagents.lock().ok()?;
    let is_known_parent = subagents
        .iter()
        .any(|s| s.session_id == conv_id && s.completed_at.is_none() && !s.archived);
    if is_known_parent {
        return None;
    }

    let mut running_unbound: Vec<&ActiveSubagent> = subagents
        .iter()
        .filter(|s| {
            matches!(s.agent_kind, AgentKind::Cursor)
                && s.completed_at.is_none()
                && !s.archived
                && s.conversation_id.is_none()
        })
        .collect();
    if running_unbound.is_empty() {
        return None;
    }

    if let Some(type_filter) = payload
        .get("subagent_type")
        .or_else(|| payload.get("agent_type"))
        .and_then(Value::as_str)
    {
        running_unbound.retain(|s| s.agent_type == type_filter);
        if running_unbound.is_empty() {
            return None;
        }
    }

    let parent = running_unbound
        .iter()
        .min_by_key(|s| &s.started_at)?
        .session_id
        .clone();
    drop(subagents);

    bind_cursor_subagent_conversation(state, conv_id, &parent);
    let main_transcript = known_session_transcript_path(state, &parent);
    let refresh_target = {
        let mut subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return Some(parent),
        };
        subagents
            .iter_mut()
            .find(|s| {
                s.session_id == parent
                    && s.conversation_id.is_none()
                    && s.completed_at.is_none()
                    && !s.archived
            })
            .map(|sub| {
                sub.conversation_id = Some(conv_id.to_string());
                sub.clone()
            })
    };

    if let Some(target) = refresh_target {
        if let Some(path) = refreshed_subagent_transcript_path(main_transcript.as_deref(), &target)
        {
            if let Ok(mut subagents) = state.active_subagents.lock() {
                if let Some(sub) = subagents.iter_mut().find(|s| {
                    s.agent_id == target.agent_id
                        && s.conversation_id.as_deref() == Some(conv_id)
                        && s.completed_at.is_none()
                        && !s.archived
                }) {
                    sub.agent_transcript_path = Some(path);
                }
            }
        }
    }
    Some(parent)
}

pub(crate) fn register_subagent_start(
    state: &AppState,
    payload: &serde_json::Value,
    agent_kind: AgentKind,
) {
    let agent_id = payload_subagent_id(payload).unwrap_or("").to_string();
    let session_id = payload_subagent_parent_session_id(payload)
        .unwrap_or("")
        .to_string();
    let agent_type = payload_subagent_type(payload).to_string();
    let agent_transcript_path = payload_subagent_transcript_path(payload)
        .map(str::to_string)
        .or_else(|| {
            derive_subagent_transcript_path(
                payload_main_transcript_path(payload),
                &agent_id,
                None,
                None,
            )
        });

    if agent_id.is_empty() || session_id.is_empty() {
        return;
    }

    let subagent = ActiveSubagent {
        agent_id,
        session_id,
        agent_kind,
        agent_type,
        started_at: iso_timestamp_now(),
        agent_transcript_path,
        completed_at: None,
        archived: false,
        last_message: None,
        conversation_id: None,
    };

    if let Ok(mut subagents) = state.active_subagents.lock() {
        if !subagents.iter().any(|s| s.agent_id == subagent.agent_id) {
            if subagents.len() >= MAX_ACTIVE_SUBAGENTS {
                subagents.retain(|existing| !existing.archived && existing.completed_at.is_none());
            }
            if subagents.len() >= MAX_ACTIVE_SUBAGENTS {
                eprintln!(
                    "Atoll ignored subagent {}: active subagent limit reached",
                    subagent.agent_id
                );
                return;
            }
            subagents.push(subagent);
        }
    }
}

pub(crate) fn complete_subagent(state: &AppState, payload: &serde_json::Value) {
    let payload_transcript_path = payload_subagent_transcript_path(payload).map(str::to_string);
    let last_message = payload_subagent_last_message(payload);

    if let Some(agent_id) = payload_subagent_id(payload) {
        let target = state
            .active_subagents
            .lock()
            .ok()
            .and_then(|subagents| subagents.iter().find(|s| s.agent_id == agent_id).cloned());

        if let Some(target) = target {
            let main_transcript = known_session_transcript_path(state, &target.session_id);
            let transcript_path = resolve_complete_transcript_path_from_main(
                main_transcript.as_deref(),
                &target,
                payload_transcript_path,
            );
            let conv_id = target.conversation_id.clone();
            if let Ok(mut subagents) = state.active_subagents.lock() {
                if let Some(sub) = subagents.iter_mut().find(|s| s.agent_id == target.agent_id) {
                    mark_subagent_complete(sub, transcript_path, last_message);
                }
            }
            unbind_cursor_subagent_conversation(state, conv_id.as_deref());
        }
        return;
    }

    let Some(parent_session) = payload_subagent_parent_session_id(payload) else {
        return;
    };
    let type_filter = payload
        .get("subagent_type")
        .or_else(|| payload.get("agent_type"))
        .and_then(Value::as_str);

    let target = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|s| s.session_id == parent_session && s.completed_at.is_none() && !s.archived)
            .filter(|s| type_filter.map(|t| s.agent_type == t).unwrap_or(true))
            .min_by_key(|s| s.started_at.clone())
            .cloned()
    };

    if let Some(target) = target {
        let main_transcript = known_session_transcript_path(state, &target.session_id);
        let transcript_path = resolve_complete_transcript_path_from_main(
            main_transcript.as_deref(),
            &target,
            payload_transcript_path,
        );
        let conv_id = target.conversation_id.clone();
        if let Ok(mut subagents) = state.active_subagents.lock() {
            if let Some(sub) = subagents
                .iter_mut()
                .find(|s| s.agent_id == target.agent_id && s.completed_at.is_none() && !s.archived)
            {
                mark_subagent_complete(sub, transcript_path, last_message);
            }
        }
        unbind_cursor_subagent_conversation(state, conv_id.as_deref());
    }
}

pub(crate) fn mark_subagent_complete(
    sub: &mut ActiveSubagent,
    transcript_path: Option<String>,
    last_message: Option<String>,
) {
    if sub.completed_at.is_some() {
        return;
    }
    sub.completed_at = Some(iso_timestamp_now());
    if let Some(path) = transcript_path {
        sub.agent_transcript_path = Some(path);
    }
    if let Some(message) = last_message {
        sub.last_message = Some(message);
    }
}

pub(crate) fn reconcile_incomplete_subagents(state: &AppState) {
    let known_transcripts: HashMap<String, String> = state
        .known_sessions
        .lock()
        .ok()
        .map(|known| {
            known
                .iter()
                .filter_map(|(session_id, session)| {
                    session
                        .transcript_path
                        .as_ref()
                        .map(|path| (session_id.clone(), path.clone()))
                })
                .collect()
        })
        .unwrap_or_default();

    let refresh_candidates: Vec<ActiveSubagent> = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|sub| sub.completed_at.is_none() && !sub.archived)
            .cloned()
            .collect()
    };

    let path_updates: Vec<(String, String)> = refresh_candidates
        .iter()
        .filter_map(|sub| {
            refreshed_subagent_transcript_path(
                known_transcripts.get(&sub.session_id).map(String::as_str),
                sub,
            )
            .map(|path| (sub.agent_id.clone(), path))
        })
        .collect();

    if !path_updates.is_empty() {
        if let Ok(mut subagents) = state.active_subagents.lock() {
            for (agent_id, path) in path_updates {
                if let Some(sub) = subagents.iter_mut().find(|sub| {
                    sub.agent_id == agent_id && sub.completed_at.is_none() && !sub.archived
                }) {
                    sub.agent_transcript_path = Some(path);
                }
            }
        }
    }

    let pending_paths: Vec<(String, String)> = {
        let subagents = match state.active_subagents.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        subagents
            .iter()
            .filter(|sub| sub.completed_at.is_none() && !sub.archived)
            .filter_map(|sub| {
                sub.agent_transcript_path
                    .as_ref()
                    .map(|path| (sub.agent_id.clone(), path.clone()))
            })
            .collect()
    };

    if pending_paths.is_empty() {
        return;
    }

    let results: Vec<(String, String)> = pending_paths
        .into_iter()
        .filter_map(|(agent_id, path)| {
            transcript::extract_subagent_terminal_message(&path).map(|msg| (agent_id, msg))
        })
        .collect();

    if results.is_empty() {
        return;
    }

    let mut subagents = match state.active_subagents.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    for (agent_id, message) in results {
        if let Some(sub) = subagents.iter_mut().find(|sub| sub.agent_id == agent_id) {
            if sub.completed_at.is_none() && !sub.archived {
                mark_subagent_complete(sub, None, Some(message));
            }
        }
    }
}

const SUBAGENT_SNAPSHOT_MIN_INTERVAL: Duration = Duration::from_millis(300);
const SUBAGENT_RECONCILE_MIN_INTERVAL: Duration = Duration::from_secs(2);
const OBSERVER_SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(400);

/// Emit a snapshot for subagent lifecycle events with rate-limiting.
/// Returns true if a snapshot was emitted, false if throttled.
pub(crate) fn emit_subagent_snapshot(app: &AppHandle, state: &AppState) -> bool {
    let mut last = state
        .last_subagent_snapshot_emit
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if now.duration_since(*last) < SUBAGENT_SNAPSHOT_MIN_INTERVAL {
        return false;
    }
    *last = now;
    drop(last);
    reconcile_incomplete_subagents_now(state);
    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
    true
}
