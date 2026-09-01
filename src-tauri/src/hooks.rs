//! Hook installation and health: per-agent (Claude/Codex/ZCode/Gemini/
//! Cursor) install/uninstall/read commands, competing-hook detection, node
//! and hook-script resolution, deployed-asset materialization, and the
//! launcher/config repair helpers.

use super::*;

mod assets;
mod launcher;
mod node;
mod repair;

pub(crate) use assets::*;
pub(crate) use launcher::*;
pub(crate) use node::*;
pub(crate) use repair::*;

/// A non-Atoll hook registered for the same Claude event as Atoll. When the
/// owning app is uninstalled or stopped, its binary may still exist but error
/// on invocation, poisoning Claude Code's "most restrictive wins" hook merge
/// and vetoing Atoll's approval. `binary_exists` flags hooks whose command
/// binary is missing on disk (definitely dead); a present binary may still be
/// dead if its app isn't running, so this is a lower bound.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompetingHook {
    pub(crate) event: String,
    pub(crate) command: String,
    pub(crate) binary_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HookStatus {
    pub(crate) installed: bool,
    pub(crate) script_found: bool,
    pub(crate) settings_path: String,
    pub(crate) script_path: String,
    #[serde(default)]
    pub(crate) node_path: String,
    #[serde(default = "default_node_found")]
    pub(crate) node_found: bool,
    /// True when Atoll's hook script content changed since the host CLI last
    /// trusted it (e.g. an Atoll update overwrote the script in place). The
    /// host may be silently ignoring the hook until the user re-trusts it.
    #[serde(default)]
    pub(crate) needs_retrust: bool,
    /// Non-Atoll hooks registered for Claude events. Empty for codex/cursor.
    /// Surfaced so the UI can warn about dead competitor hooks that veto
    /// Atoll's permission decisions under Claude Code's most-restrictive-wins
    /// merge. Only populated for the Claude agent.
    #[serde(default)]
    pub(crate) competing_hooks: Vec<CompetingHook>,
}

pub(crate) fn default_node_found() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HookHealthSnapshot {
    pub(crate) claude: HookStatus,
    pub(crate) codex: HookStatus,
    pub(crate) cursor: HookStatus,
    pub(crate) zcode: HookStatus,
    pub(crate) gemini: HookStatus,
}

impl Default for HookStatus {
    fn default() -> Self {
        Self {
            installed: false,
            script_found: false,
            settings_path: String::new(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: true,
            needs_retrust: false,
            competing_hooks: Vec::new(),
        }
    }
}

/// One hook event written to an agent's config on install. Timeouts are in
/// the unit the agent expects: seconds for Claude/Codex/ZCode, milliseconds
/// for Gemini.
pub(crate) struct HookEventSpec {
    pub(crate) event: &'static str,
    pub(crate) timeout: i64,
    /// Optional `statusMessage` the agent shows while the hook runs.
    pub(crate) status_message: Option<&'static str>,
    /// `Some` wraps the hook in a matcher group (Claude/Codex use `"*"`;
    /// Gemini's BeforeTool carries its gated-tools regex). `None` lists the
    /// hook bare under the event.
    pub(crate) matcher: Option<&'static str>,
}

impl HookEventSpec {
    const fn new(
        event: &'static str,
        timeout: i64,
        status_message: Option<&'static str>,
        matcher: Option<&'static str>,
    ) -> Self {
        Self {
            event,
            timeout,
            status_message,
            matcher,
        }
    }
}

const CLAUDE_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec::new("PermissionRequest", 1800, None, Some("*")),
    HookEventSpec::new("PostToolUse", 30, None, Some("*")),
    HookEventSpec::new("PostToolUseFailure", 30, None, Some("*")),
    HookEventSpec::new("Stop", 30, None, Some("*")),
    HookEventSpec::new("StopFailure", 30, None, Some("*")),
    HookEventSpec::new("SubagentStop", 30, None, Some("*")),
    HookEventSpec::new("SubagentStart", 30, None, Some("*")),
];

const CODEX_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec::new("PermissionRequest", 1800, Some("Atoll approval"), Some("*")),
    HookEventSpec::new("PostToolUse", 30, Some("Atoll session sync"), Some("*")),
    HookEventSpec::new("Stop", 30, Some("Atoll session sync"), Some("*")),
    HookEventSpec::new("SubagentStop", 30, Some("Atoll session sync"), Some("*")),
    HookEventSpec::new("SubagentStart", 30, Some("Atoll session sync"), Some("*")),
];

// ZCode's matcher is a case-sensitive regex on the tool name; omitting it
// matches every tool (a literal "*" is not guaranteed by the schema).
// PreToolUse is intentionally NOT registered: it fires for every tool call,
// while PermissionRequest already covers the approval flow (same split as
// the Claude/Codex integrations).

const ZCODE_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec::new("PermissionRequest", 1800, Some("Atoll approval"), None),
    HookEventSpec::new("PostToolUse", 30, Some("Atoll session sync"), None),
    HookEventSpec::new("PostToolUseFailure", 30, Some("Atoll session sync"), None),
    HookEventSpec::new("Stop", 30, Some("Atoll session sync"), None),
    HookEventSpec::new("SessionStart", 30, Some("Atoll session sync"), None),
    HookEventSpec::new("UserPromptSubmit", 30, Some("Atoll session sync"), None),
];

// Gemini CLI hook timeouts are in MILLISECONDS (CommandHookConfig.timeout,
// default 60000). BeforeTool blocks until the Atoll user decides; observer
// events only register sessions and must never stall a turn.
// The BeforeTool matcher mirrors the gate list in atoll-gemini-hook.mjs so
// read-only tools never spawn the hook process.

const GEMINI_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        event: "BeforeTool",
        timeout: 1_800_000,
        status_message: None,
        matcher: Some(
            "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_",
        ),
    },
    HookEventSpec::new("SessionStart", 30_000, None, None),
    HookEventSpec::new("SessionEnd", 30_000, None, None),
    HookEventSpec::new("AfterTool", 30_000, None, None),
    HookEventSpec::new("AfterAgent", 30_000, None, None),
    HookEventSpec::new("Notification", 30_000, None, None),
];

/// Build the Atoll hook payload from an agent's event table, keyed by event
/// name. serde_json sorts object keys on serialization, so this matches the
/// byte output of the per-agent `json!` literals it replaces.
fn atoll_events_json(hook_command: &str, events: &[HookEventSpec]) -> Value {
    let mut event_map = serde_json::Map::new();
    for spec in events {
        let mut hook = json!({
            "type": "command",
            "command": hook_command,
            "timeout": spec.timeout,
        });
        if let Some(status_message) = spec.status_message {
            hook["statusMessage"] = json!(status_message);
        }
        // Each event value must be an array of matcher groups. upsert_*
        // skips non-arrays, which is what made install look like a
        // permissions failure after the table-driven refactor.
        let entry = match spec.matcher {
            Some(matcher) => json!([{ "matcher": matcher, "hooks": [hook] }]),
            None => json!([{ "hooks": [hook] }]),
        };
        event_map.insert(spec.event.to_string(), entry);
    }
    Value::Object(event_map)
}

/// Static per-agent wiring for the hook install/read/uninstall pipeline. The
/// `#[tauri::command]` entry points stay named per agent; they look up their
/// profile and hand it to the shared implementation. Fields that vary in
/// *shape* between agents (ZCode's enabled+events nesting, Codex's desktop
/// node and trust cache, Cursor's entry-style hooks and lazy repair kit) are
/// function fields so each agent keeps its exact behavior.
pub(crate) struct AgentHookProfile {
    /// hook_trust bookkeeping key ("claude").
    pub(crate) key: &'static str,
    /// Capitalized agent name used in user-facing errors and logs ("Claude").
    pub(crate) display_name: &'static str,
    /// Deployed hook script file name ("atoll-claude-hook.mjs").
    pub(crate) script_name: &'static str,
    /// Substring identifying Atoll's command inside the agent config.
    pub(crate) marker: &'static str,
    /// Path of the agent config file that stores hooks.
    pub(crate) config_path: fn() -> Option<std::path::PathBuf>,
    /// Directory shown in the create_dir_all error ("~/.claude").
    pub(crate) config_dir_label: &'static str,
    /// Config file name used in "… is not a JSON object" errors.
    pub(crate) config_display: &'static str,
    /// Config path echoed in the not-saved error ("~/.claude/settings.json").
    pub(crate) permissions_hint: &'static str,
    /// Config file role used in read/serialize/write/verify errors
    /// ("settings", "hooks", or "config").
    pub(crate) io_label: &'static str,
    /// Event table written on install. Cursor writes entries directly (see
    /// [`upsert_cursor_hook_events`]) and carries none here.
    pub(crate) events: &'static [HookEventSpec],
    /// Does a parsed config contain Atoll's hooks for this agent?
    pub(crate) has_hooks: fn(&Value) -> bool,
    /// Build the platform-appropriate hook command for a fresh install.
    pub(crate) build_hook_command:
        fn(&AppHandle, node_path: &str, script_path: &str) -> Result<String, String>,
    /// Node resolution; Codex prefers its desktop bundle.
    pub(crate) resolve_node: fn() -> Result<String, String>,
    /// Merge Atoll's hooks into a parsed config (install path).
    pub(crate) apply_hooks:
        fn(&AppHandle, config: &mut Value, hook_command: &str) -> Result<(), String>,
    /// Strip Atoll's hooks from a parsed config (uninstall path).
    pub(crate) uninstall_from: fn(&mut Value),
    /// Record a completed install in hook-trust state.
    pub(crate) record_installed: fn(agent_key: &str, script_path: &str),
    /// Read the current status (the slow, config-inspecting path).
    pub(crate) status: fn(&AppHandle) -> HookStatus,
    /// True when installed configs get the dev-path drift check.
    pub(crate) checks_dev_drift: bool,
    /// Windows PowerShell launcher config this agent launches hooks through,
    /// if any; repaired in place on status reads.
    pub(crate) launcher_config: Option<&'static str>,
    /// Lazy repair applied to a parsed config before reading status (Cursor's
    /// repair kit); the returned config replaces the parsed one.
    pub(crate) repair_installed: Option<
        fn(&AppHandle, config_path: &str, config: Option<&Value>, hook_url: &str) -> Option<Value>,
    >,
    /// Post-status adjustment (Claude attaches competitor hooks).
    pub(crate) post_status: Option<fn(&mut HookStatus, config: Option<&Value>)>,
    /// Extra refresh before the install/uninstall snapshot is built (Cursor
    /// re-runs hook health because its status reader repairs lazily).
    pub(crate) pre_snapshot_refresh: Option<fn(&AppHandle, &AppState)>,
}

/// Claude/Gemini build the command inline; no launcher indirection.
fn runner_hook_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(
        hook_runner_for_command(app).as_deref(),
        node_path,
        script_path,
    ))
}

fn record_codex_installed(_agent_key: &str, script_path: &str) {
    hook_trust::on_codex_hooks_installed(script_path);
}

pub(crate) const CLAUDE_HOOK_PROFILE: AgentHookProfile = AgentHookProfile {
    key: "claude",
    display_name: "Claude",
    script_name: "atoll-claude-hook.mjs",
    marker: "atoll-claude-hook",
    config_path: claude_settings_path,
    config_dir_label: "~/.claude",
    config_display: "Settings file",
    permissions_hint: "~/.claude/settings.json",
    io_label: "settings",
    events: CLAUDE_HOOK_EVENTS,
    has_hooks: has_atoll_claude_hooks,
    build_hook_command: runner_hook_command,
    resolve_node: resolve_node_executable,
    apply_hooks: apply_claude_hooks,
    uninstall_from: uninstall_claude_from_config,
    record_installed: hook_trust::record_hook_installed,
    status: claude_hook_status,
    checks_dev_drift: false,
    launcher_config: None,
    repair_installed: None,
    post_status: Some(attach_competing_hooks),
    pre_snapshot_refresh: None,
};

pub(crate) const CODEX_HOOK_PROFILE: AgentHookProfile = AgentHookProfile {
    key: "codex",
    display_name: "Codex",
    script_name: "atoll-codex-hook.mjs",
    marker: "atoll-codex-hook",
    config_path: codex_hooks_path,
    config_dir_label: "~/.codex",
    config_display: "hooks.json",
    permissions_hint: "~/.codex/hooks.json",
    io_label: "hooks",
    events: CODEX_HOOK_EVENTS,
    has_hooks: has_atoll_codex_hooks,
    build_hook_command: write_codex_hook_launcher_command,
    resolve_node: resolve_node_executable_for_codex,
    apply_hooks: apply_codex_hooks,
    uninstall_from: uninstall_codex_from_config,
    record_installed: record_codex_installed,
    status: codex_hook_status,
    checks_dev_drift: true,
    launcher_config: Some("codex-hook-launcher.json"),
    repair_installed: None,
    post_status: None,
    pre_snapshot_refresh: None,
};

pub(crate) const ZCODE_HOOK_PROFILE: AgentHookProfile = AgentHookProfile {
    key: "zcode",
    display_name: "ZCode",
    script_name: "atoll-zcode-hook.mjs",
    marker: "atoll-zcode-hook",
    config_path: zcode_config_path,
    config_dir_label: "~/.zcode/cli",
    config_display: "config.json",
    permissions_hint: "~/.zcode/cli/config.json",
    io_label: "config",
    events: ZCODE_HOOK_EVENTS,
    has_hooks: has_atoll_zcode_hooks,
    build_hook_command: write_zcode_hook_launcher_command,
    resolve_node: resolve_node_executable,
    apply_hooks: apply_zcode_hooks,
    uninstall_from: uninstall_zcode_from_config,
    record_installed: hook_trust::record_hook_installed,
    status: zcode_hook_status,
    checks_dev_drift: true,
    launcher_config: Some("zcode-hook-launcher.json"),
    repair_installed: None,
    post_status: None,
    pre_snapshot_refresh: None,
};

pub(crate) const GEMINI_HOOK_PROFILE: AgentHookProfile = AgentHookProfile {
    key: "gemini",
    display_name: "Gemini",
    script_name: "atoll-gemini-hook.mjs",
    marker: "atoll-gemini-hook",
    config_path: gemini_settings_path,
    config_dir_label: "~/.gemini",
    config_display: "settings.json",
    permissions_hint: "~/.gemini/settings.json",
    io_label: "settings",
    events: GEMINI_HOOK_EVENTS,
    has_hooks: has_atoll_gemini_hooks,
    build_hook_command: runner_hook_command,
    resolve_node: resolve_node_executable,
    apply_hooks: apply_gemini_hooks,
    uninstall_from: uninstall_gemini_from_config,
    record_installed: hook_trust::record_hook_installed,
    status: gemini_hook_status,
    checks_dev_drift: true,
    launcher_config: None,
    repair_installed: None,
    post_status: None,
    pre_snapshot_refresh: None,
};

pub(crate) const CURSOR_HOOK_PROFILE: AgentHookProfile = AgentHookProfile {
    key: "cursor",
    display_name: "Cursor",
    script_name: "atoll-cursor-hook.mjs",
    marker: "atoll-cursor-hook",
    config_path: cursor_hooks_path,
    config_dir_label: "~/.cursor",
    config_display: "hooks.json",
    permissions_hint: "~/.cursor/hooks.json",
    io_label: "hooks",
    events: &[],
    has_hooks: has_atoll_cursor_hooks,
    build_hook_command: write_cursor_hook_launcher_command,
    resolve_node: resolve_node_executable,
    apply_hooks: apply_cursor_hooks,
    uninstall_from: uninstall_cursor_from_config,
    record_installed: hook_trust::record_hook_installed,
    status: cursor_hook_status,
    checks_dev_drift: true,
    launcher_config: Some("cursor-hook-launcher.json"),
    repair_installed: Some(maybe_repair_cursor_hook_events),
    post_status: None,
    pre_snapshot_refresh: Some(refresh_hook_health_cache),
};

/// Status reported while the capture override forces every agent to look
/// uninstalled.
pub(crate) fn forced_uninstalled_status(app: &AppHandle, profile: &AgentHookProfile) -> HookStatus {
    let script_path = resolve_hook_script_path(app, profile.script_name).unwrap_or_default();
    HookStatus {
        installed: false,
        script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
        settings_path: (profile.config_path)()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        script_path,
        node_path: String::new(),
        node_found: resolve_node_executable().is_ok(),
        needs_retrust: false,
        competing_hooks: Vec::new(),
    }
}

fn get_hook_status_for(profile: &AgentHookProfile, app: AppHandle) -> Result<HookStatus, String> {
    if capture::force_hook_uninstalled() {
        return Ok(forced_uninstalled_status(&app, profile));
    }
    Ok((profile.status)(&app))
}

/// Status reported for an agent whose config file is already gone: nothing
/// to uninstall, and the trust record goes with it.
fn not_installed_status(config_path: &std::path::Path) -> HookStatus {
    HookStatus {
        installed: false,
        script_found: false,
        settings_path: config_path.to_string_lossy().into(),
        script_path: String::new(),
        node_path: String::new(),
        node_found: resolve_node_executable().is_ok(),
        needs_retrust: false,
        competing_hooks: Vec::new(),
    }
}

/// Rebuild the island snapshot after a hook config mutation and emit it so
/// every open window re-renders from the same state.
fn emit_hook_snapshot_changed(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let snapshot = build_snapshot(app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())
}

fn uninstall_hooks_for(profile: &AgentHookProfile, app: AppHandle) -> Result<HookStatus, String> {
    let config_path =
        (profile.config_path)().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !config_path.exists() {
        hook_trust::clear_hook_installed(profile.key);
        return Ok(not_installed_status(&config_path));
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read {}: {e}", profile.io_label))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    (profile.uninstall_from)(&mut config);

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize {}: {e}", profile.io_label))?;
    std::fs::write(&config_path, formatted)
        .map_err(|e| format!("Cannot write {}: {e}", profile.io_label))?;
    hook_trust::clear_hook_installed(profile.key);

    emit_hook_snapshot_changed(&app)?;

    Ok((profile.status)(&app))
}

fn install_hooks_for(profile: &AgentHookProfile, app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, profile.script_name)?;
    let script_path = materialize_hook_deployment(&app, profile.script_name, &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = (profile.resolve_node)()?;

    let config_path =
        (profile.config_path)().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {} directory: {e}", profile.config_dir_label))?;
    }

    let mut config: Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot read {}: {e}", profile.io_label))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = (profile.build_hook_command)(&app, &node_path, &script_path)?;
    (profile.apply_hooks)(&app, &mut config, &hook_command)?;

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize {}: {e}", profile.io_label))?;
    std::fs::write(&config_path, formatted)
        .map_err(|e| format!("Cannot write {}: {e}", profile.io_label))?;

    let written = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot verify {}: {e}", profile.io_label))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse {} after write: {e}", profile.io_label))?;
    if !(profile.has_hooks)(&verify) {
        return Err(format!(
            "{} hooks were not saved correctly. Check permissions on {}.",
            profile.display_name, profile.permissions_hint
        ));
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!(
            "Atoll failed to refresh bridge.json after {} hook install: {error}",
            profile.display_name
        );
    }
    (profile.record_installed)(profile.key, &script_path);

    let state = app.state::<AppState>();
    if let Some(refresh) = profile.pre_snapshot_refresh {
        refresh(&app, &state);
    }
    emit_hook_snapshot_changed(&app)?;

    Ok((profile.status)(&app))
}

fn apply_claude_hooks(
    _app: &AppHandle,
    config: &mut Value,
    hook_command: &str,
) -> Result<(), String> {
    let atoll_hooks = atoll_events_json(hook_command, CLAUDE_HOOK_PROFILE.events);
    let hooks_entry = entry_for_hooks(config, CLAUDE_HOOK_PROFILE.config_display)?;
    upsert_claude_hook_events(hooks_entry, &atoll_hooks);
    Ok(())
}

fn apply_codex_hooks(
    _app: &AppHandle,
    config: &mut Value,
    hook_command: &str,
) -> Result<(), String> {
    let atoll_hooks = atoll_events_json(hook_command, CODEX_HOOK_PROFILE.events);
    let hooks_entry = entry_for_hooks(config, CODEX_HOOK_PROFILE.config_display)?;
    upsert_codex_hook_events(hooks_entry, &atoll_hooks);
    Ok(())
}

fn apply_zcode_hooks(
    _app: &AppHandle,
    config: &mut Value,
    hook_command: &str,
) -> Result<(), String> {
    let atoll_hooks = atoll_events_json(hook_command, ZCODE_HOOK_PROFILE.events);
    let hooks_obj = entry_for_hooks(config, ZCODE_HOOK_PROFILE.config_display)?;
    if !hooks_obj.is_object() {
        *hooks_obj = Value::Object(Default::default());
    }
    let hooks_map = hooks_obj
        .as_object_mut()
        .ok_or_else(|| "config.json hooks is not a JSON object".to_string())?;
    // Configuration-file hooks are disabled by default in ZCode; the hook
    // runner only runs when this flag is set.
    hooks_map.insert("enabled".to_string(), Value::Bool(true));
    let events_obj = hooks_map
        .entry("events")
        .or_insert_with(|| Value::Object(Default::default()));
    if !events_obj.is_object() {
        *events_obj = Value::Object(Default::default());
    }
    upsert_zcode_hook_events(events_obj, &atoll_hooks);
    Ok(())
}

fn apply_gemini_hooks(
    _app: &AppHandle,
    config: &mut Value,
    hook_command: &str,
) -> Result<(), String> {
    let atoll_hooks = atoll_events_json(hook_command, GEMINI_HOOK_PROFILE.events);
    let hooks_obj = entry_for_hooks(config, GEMINI_HOOK_PROFILE.config_display)?;
    if !hooks_obj.is_object() {
        *hooks_obj = Value::Object(Default::default());
    }
    upsert_gemini_hook_entries(hooks_obj, &atoll_hooks);
    Ok(())
}

fn apply_cursor_hooks(
    app: &AppHandle,
    config: &mut Value,
    hook_command: &str,
) -> Result<(), String> {
    if config.get("version").is_none() {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("version".to_string(), json!(1));
        }
    }
    let hooks_obj = entry_for_hooks(config, CURSOR_HOOK_PROFILE.config_display)?;
    upsert_cursor_hook_events(
        hooks_obj,
        hook_command,
        &hook_bridge::cursor_hook_url_for_app(app),
    );
    Ok(())
}

/// Enter the `hooks` object of a parsed agent config, creating it when
/// missing. `config_display` names the file in the not-an-object error.
fn entry_for_hooks<'a>(
    config: &'a mut Value,
    config_display: &str,
) -> Result<&'a mut Value, String> {
    let obj = config
        .as_object_mut()
        .ok_or_else(|| format!("{config_display} is not a JSON object"))?;
    Ok(obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default())))
}

fn uninstall_claude_from_config(settings: &mut Value) {
    if let Some(obj) = settings.as_object_mut() {
        if let Some(hooks) = obj.get_mut("hooks") {
            remove_atoll_claude_hooks(hooks);
            if hooks.as_object().map(|map| map.is_empty()).unwrap_or(false) {
                obj.remove("hooks");
            }
        }
    }
}

fn uninstall_codex_from_config(config: &mut Value) {
    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_codex_hooks(hooks);
    }
}

fn uninstall_gemini_from_config(settings: &mut Value) {
    if let Some(hooks) = settings.get_mut("hooks") {
        remove_atoll_gemini_hooks(hooks);
    }
}

fn uninstall_cursor_from_config(config: &mut Value) {
    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_cursor_hooks(hooks);
    }
}

fn uninstall_zcode_from_config(config: &mut Value) {
    // `hooks.enabled` is left untouched: the user may have other configuration
    // hooks that depend on the flag being set.
    if let Some(events) = config
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("events"))
    {
        remove_atoll_zcode_hooks(events);
    }
}

#[tauri::command]
pub(crate) fn get_claude_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&CLAUDE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    install_hooks_for(&CLAUDE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn uninstall_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    uninstall_hooks_for(&CLAUDE_HOOK_PROFILE, app)
}

/// Remove non-Atoll hooks from `~/.claude/settings.json` whose command binary no
/// longer exists on disk, across the events where a dead competitor can veto
/// Atoll's permission decision. Preserves Atoll's own hooks and any competitor
/// whose binary is still present (the app may still be running). Returns the
/// post-cleanup Claude hook status.
#[tauri::command]
pub(crate) fn remove_competing_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let settings_path =
        claude_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;
    if !settings_path.exists() {
        return Ok(claude_hook_status(&app));
    }
    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    let mut settings: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    let removed_any = remove_dead_competing_hooks_from_config(&mut settings);

    if removed_any {
        let formatted = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Cannot serialize settings: {e}"))?;
        std::fs::write(&settings_path, formatted)
            .map_err(|e| format!("Cannot write settings: {e}"))?;
        let state = app.state::<AppState>();
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
        remember_hook_health(&state, &snapshot.hook_health);
    }

    Ok(claude_hook_status(&app))
}

pub(crate) fn claude_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("settings.json"))
}

pub(crate) fn codex_hooks_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".codex").join("hooks.json"))
}

pub(crate) fn cursor_hooks_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".cursor").join("hooks.json"))
}

pub(crate) fn zcode_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".zcode").join("cli").join("config.json"))
}

pub(crate) fn gemini_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".gemini").join("settings.json"))
}

#[tauri::command]
pub(crate) fn get_codex_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&CODEX_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    install_hooks_for(&CODEX_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn uninstall_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    uninstall_hooks_for(&CODEX_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn get_zcode_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&ZCODE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_zcode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    install_hooks_for(&ZCODE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn uninstall_zcode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    uninstall_hooks_for(&ZCODE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn get_gemini_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&GEMINI_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_gemini_hooks(app: AppHandle) -> Result<HookStatus, String> {
    install_hooks_for(&GEMINI_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn uninstall_gemini_hooks(app: AppHandle) -> Result<HookStatus, String> {
    uninstall_hooks_for(&GEMINI_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn get_cursor_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&CURSOR_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    install_hooks_for(&CURSOR_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn uninstall_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    uninstall_hooks_for(&CURSOR_HOOK_PROFILE, app)
}

/// Merge Atoll's per-event matcher groups into `existing_hooks`, replacing
/// any previous Atoll entries event-by-event and keeping every foreign
/// matcher intact.
fn upsert_hook_events_matching(
    existing_hooks: &mut Value,
    atoll_hooks: &Value,
    panic_context: &str,
    matcher_has_atoll: fn(&Value) -> bool,
) {
    let Some(atoll_map) = atoll_hooks.as_object() else {
        return;
    };
    let hooks_obj = existing_hooks.as_object_mut().expect(panic_context);

    for (event, atoll_matchers) in atoll_map {
        let Some(atoll_array) = atoll_matchers.as_array() else {
            continue;
        };

        let mut merged: Vec<Value> = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter(|matcher| !matcher_has_atoll(matcher))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        for matcher in atoll_array {
            merged.push(matcher.clone());
        }

        hooks_obj.insert(event.clone(), Value::Array(merged));
    }
}

pub(crate) fn upsert_claude_hook_events(existing_hooks: &mut Value, atoll_hooks: &Value) {
    upsert_hook_events_matching(
        existing_hooks,
        atoll_hooks,
        "hooks value should be object",
        matcher_group_has_atoll_claude,
    );
}

/// Strip every hook whose command carries `marker` from all event arrays,
/// then drop the events left empty. `preserve_non_array_keys` keeps
/// non-array values sharing the object with the events; Gemini keeps
/// `enabled`/`disabled`/`notifications` config keys alongside its event
/// entries, the other agents prune anything non-array.
fn remove_hooks_with_marker(hooks: &mut Value, marker: &str, preserve_non_array_keys: bool) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    for matchers in hooks_obj.values_mut() {
        if let Some(arr) = matchers.as_array_mut() {
            for matcher in arr.iter_mut() {
                if let Some(hook_arr) = matcher.get_mut("hooks").and_then(Value::as_array_mut) {
                    hook_arr.retain(|hook| {
                        !hook
                            .get("command")
                            .and_then(Value::as_str)
                            .map(|cmd| cmd.contains(marker))
                            .unwrap_or(false)
                    });
                }
            }
            arr.retain(|matcher| {
                matcher
                    .get("hooks")
                    .and_then(Value::as_array)
                    .map(|hooks| !hooks.is_empty())
                    .unwrap_or(false)
            });
        }
    }

    hooks_obj.retain(|_, matchers| {
        matchers
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(preserve_non_array_keys)
    });
}

pub(crate) fn remove_atoll_claude_hooks(hooks: &mut Value) {
    remove_hooks_with_marker(hooks, "atoll-claude-hook", false);
}

/// True when the matcher group's hooks array contains a command carrying
/// `marker`.
fn matcher_group_has_marker(matcher: &Value, marker: &str) -> bool {
    matcher
        .get("hooks")
        .and_then(Value::as_array)
        .map(|hook_arr| {
            hook_arr.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .map(|cmd| cmd.contains(marker))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn matcher_group_has_atoll_claude(matcher: &Value) -> bool {
    matcher_group_has_marker(matcher, "atoll-claude-hook")
}

pub(crate) fn upsert_codex_hook_events(existing_hooks: &mut Value, atoll_hooks: &Value) {
    upsert_hook_events_matching(
        existing_hooks,
        atoll_hooks,
        "hooks value should be object",
        matcher_group_has_atoll_codex,
    );
}

pub(crate) fn remove_atoll_codex_hooks(hooks: &mut Value) {
    remove_hooks_with_marker(hooks, "atoll-codex-hook", false);
}

pub(crate) fn upsert_zcode_hook_events(existing_events: &mut Value, atoll_hooks: &Value) {
    upsert_hook_events_matching(
        existing_events,
        atoll_hooks,
        "zcode events value should be object",
        matcher_group_has_atoll_zcode,
    );
}

pub(crate) fn remove_atoll_zcode_hooks(events: &mut Value) {
    remove_hooks_with_marker(events, "atoll-zcode-hook", false);
}

pub(crate) fn upsert_gemini_hook_entries(existing_hooks: &mut Value, atoll_hooks: &Value) {
    upsert_hook_events_matching(
        existing_hooks,
        atoll_hooks,
        "gemini hooks value should be object",
        matcher_group_has_atoll_gemini,
    );
}

pub(crate) fn remove_atoll_gemini_hooks(hooks: &mut Value) {
    remove_hooks_with_marker(hooks, "atoll-gemini-hook", true);
}

/// Gemini stores hook event entries directly under `hooks` in settings.json
/// (alongside optional `enabled`/`disabled`/`notifications` config keys).
pub(crate) fn has_atoll_gemini_hooks(settings: &Value) -> bool {
    has_atoll_hooks_in(
        settings,
        HookEventsLayout::Direct,
        GEMINI_CORE_HOOK_EVENTS,
        matcher_group_has_atoll_gemini,
    )
}

pub(crate) fn matcher_group_has_atoll_gemini(matcher: &Value) -> bool {
    matcher_group_has_marker(matcher, "atoll-gemini-hook")
}

/// Where an agent stores its event arrays beneath the `hooks` key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HookEventsLayout {
    /// Event arrays sit directly under `hooks` (Claude, Codex, Gemini).
    Direct,
    /// ZCode nests event arrays under `hooks.events` and only runs
    /// configuration-file hooks when `hooks.enabled` is true.
    EnabledNestedEvents,
}

/// Shared body of the `has_atoll_*_hooks` predicates: every core event must
/// list at least one matcher group belonging to Atoll.
fn has_atoll_hooks_in(
    config: &Value,
    layout: HookEventsLayout,
    core_events: &[&str],
    matcher_has_atoll: fn(&Value) -> bool,
) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let events = match layout {
        HookEventsLayout::Direct => hooks,
        HookEventsLayout::EnabledNestedEvents => {
            // ZCode runs configuration-file hooks only when `hooks.enabled` is true.
            if !hooks
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return false;
            }
            match hooks.get("events").and_then(Value::as_object) {
                Some(events) => events,
                None => return false,
            }
        }
    };

    core_events.iter().all(|event| {
        events
            .get(*event)
            .map(|matchers| {
                matchers
                    .as_array()
                    .map(|arr| arr.iter().any(matcher_has_atoll))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    })
}

/// Events that must carry an Atoll hook for the agent to count as installed.
const CLAUDE_CORE_HOOK_EVENTS: &[&str] = &["PermissionRequest", "PostToolUse", "Stop"];

const CODEX_CORE_HOOK_EVENTS: &[&str] =
    &["PermissionRequest", "PostToolUse", "Stop", "SubagentStop"];

const ZCODE_CORE_HOOK_EVENTS: &[&str] = &["PermissionRequest", "PostToolUse", "Stop"];

const GEMINI_CORE_HOOK_EVENTS: &[&str] = &["BeforeTool", "SessionStart", "AfterTool"];

/// Claude's post-status hook: surface non-Atoll hooks registered for the
/// events where a dead competitor can veto Atoll's permission decisions.
fn attach_competing_hooks(status: &mut HookStatus, config: Option<&Value>) {
    status.competing_hooks = config
        .map(|cfg| detect_competing_claude_hooks(cfg))
        .unwrap_or_default();
}

pub(crate) fn has_atoll_claude_hooks(settings: &Value) -> bool {
    // "Stop" is required for token refresh on normal (no-tool) turns.
    has_atoll_hooks_in(
        settings,
        HookEventsLayout::Direct,
        CLAUDE_CORE_HOOK_EVENTS,
        matcher_group_has_atoll_claude,
    )
}

pub(crate) fn has_atoll_codex_hooks(config: &Value) -> bool {
    has_atoll_hooks_in(
        config,
        HookEventsLayout::Direct,
        CODEX_CORE_HOOK_EVENTS,
        matcher_group_has_atoll_codex,
    )
}

pub(crate) fn matcher_group_has_atoll_codex(matcher: &Value) -> bool {
    matcher_group_has_marker(matcher, "atoll-codex-hook")
}

pub(crate) fn has_atoll_zcode_hooks(config: &Value) -> bool {
    has_atoll_hooks_in(
        config,
        HookEventsLayout::EnabledNestedEvents,
        ZCODE_CORE_HOOK_EVENTS,
        matcher_group_has_atoll_zcode,
    )
}

pub(crate) fn matcher_group_has_atoll_zcode(matcher: &Value) -> bool {
    matcher_group_has_marker(matcher, "atoll-zcode-hook")
}

#[cfg(test)]
mod atoll_events_json_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_values_are_matcher_group_arrays() {
        let events = atoll_events_json(
            "/opt/homebrew/bin/node /tmp/atoll-claude-hook.mjs",
            CLAUDE_HOOK_EVENTS,
        );
        for spec in CLAUDE_HOOK_EVENTS {
            let value = events
                .get(spec.event)
                .unwrap_or_else(|| panic!("missing {}", spec.event));
            assert!(
                value.is_array(),
                "{} must be an array of matcher groups, got {value}",
                spec.event
            );
        }
        assert!(has_atoll_claude_hooks(&json!({ "hooks": events })));
    }

    #[test]
    fn upsert_adds_atoll_alongside_existing_claude_hooks() {
        let mut hooks = json!({
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{ "type": "command", "command": "/other/bridge --source claude" }]
            }],
            "PostToolUse": [{
                "matcher": "*",
                "hooks": [{ "type": "command", "command": "/other/bridge --source claude" }]
            }],
            "Stop": [{
                "hooks": [{ "type": "command", "command": "/other/bridge --source claude" }]
            }]
        });
        let atoll = atoll_events_json(
            "/opt/homebrew/bin/node /tmp/atoll-claude-hook.mjs",
            CLAUDE_HOOK_EVENTS,
        );
        upsert_claude_hook_events(&mut hooks, &atoll);
        assert!(has_atoll_claude_hooks(&json!({ "hooks": hooks })));
        let permission = hooks["PermissionRequest"].as_array().unwrap();
        assert_eq!(permission.len(), 2);
        assert!(
            permission.iter().any(|group| group
                .get("hooks")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().any(|hook| hook
                    .get("command")
                    .and_then(Value::as_str)
                    .map(|cmd| cmd.contains("bridge --source claude"))
                    .unwrap_or(false)))
                .unwrap_or(false)),
            "existing competitor hooks must be preserved"
        );
    }

    #[test]
    fn zcode_and_gemini_events_are_also_arrays() {
        let zcode = atoll_events_json(
            "/opt/homebrew/bin/node /tmp/atoll-zcode-hook.mjs",
            ZCODE_HOOK_EVENTS,
        );
        assert!(zcode["PermissionRequest"].is_array());
        let gemini = atoll_events_json(
            "/opt/homebrew/bin/node /tmp/atoll-gemini-hook.mjs",
            GEMINI_HOOK_EVENTS,
        );
        assert!(gemini["BeforeTool"].is_array());
        assert!(gemini["SessionStart"].is_array());
    }
}
