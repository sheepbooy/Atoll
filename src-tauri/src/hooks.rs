//! Hook installation and health: per-agent (Claude/Codex/ZCode/Gemini/
//! Cursor) install/uninstall/read commands, competing-hook detection, node
//! and hook-script resolution, deployed-asset materialization, and the
//! launcher/config repair helpers.

use super::*;

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

const CLAUDE_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        event: "PermissionRequest",
        timeout: 1800,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "PostToolUse",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "PostToolUseFailure",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "Stop",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "StopFailure",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "SubagentStop",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "SubagentStart",
        timeout: 30,
        status_message: None,
        matcher: Some("*"),
    },
];

const CODEX_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        event: "PermissionRequest",
        timeout: 1800,
        status_message: Some("Atoll approval"),
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "PostToolUse",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "Stop",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "SubagentStop",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: Some("*"),
    },
    HookEventSpec {
        event: "SubagentStart",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: Some("*"),
    },
];

// ZCode's matcher is a case-sensitive regex on the tool name; omitting it
// matches every tool (a literal "*" is not guaranteed by the schema).
// PreToolUse is intentionally NOT registered: it fires for every tool call,
// while PermissionRequest already covers the approval flow (same split as
// the Claude/Codex integrations).
const ZCODE_HOOK_EVENTS: &[HookEventSpec] = &[
    HookEventSpec {
        event: "PermissionRequest",
        timeout: 1800,
        status_message: Some("Atoll approval"),
        matcher: None,
    },
    HookEventSpec {
        event: "PostToolUse",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: None,
    },
    HookEventSpec {
        event: "PostToolUseFailure",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: None,
    },
    HookEventSpec {
        event: "Stop",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: None,
    },
    HookEventSpec {
        event: "SessionStart",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: None,
    },
    HookEventSpec {
        event: "UserPromptSubmit",
        timeout: 30,
        status_message: Some("Atoll session sync"),
        matcher: None,
    },
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
    HookEventSpec {
        event: "SessionStart",
        timeout: 30_000,
        status_message: None,
        matcher: None,
    },
    HookEventSpec {
        event: "SessionEnd",
        timeout: 30_000,
        status_message: None,
        matcher: None,
    },
    HookEventSpec {
        event: "AfterTool",
        timeout: 30_000,
        status_message: None,
        matcher: None,
    },
    HookEventSpec {
        event: "AfterAgent",
        timeout: 30_000,
        status_message: None,
        matcher: None,
    },
    HookEventSpec {
        event: "Notification",
        timeout: 30_000,
        status_message: None,
        matcher: None,
    },
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
        let entry = match spec.matcher {
            Some(matcher) => json!({ "matcher": matcher, "hooks": [hook] }),
            None => json!({ "hooks": [hook] }),
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
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-claude-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-claude-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let settings_path =
        claude_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.claude directory: {e}"))?;
    }

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Cannot read settings: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = format_hook_command(
        hook_runner_for_command(&app).as_deref(),
        &node_path,
        &script_path,
    );
    let atoll_hooks = serde_json::json!({
        "PermissionRequest": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
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
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "PostToolUseFailure": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "StopFailure": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30
                    }
                ]
            }
        ]
    });

    let settings_obj = settings
        .as_object_mut()
        .ok_or_else(|| "Settings file is not a JSON object".to_string())?;
    let hooks_entry = settings_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_claude_hook_events(hooks_entry, &atoll_hooks);

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;

    let written = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot verify settings: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse settings after write: {e}"))?;
    if !has_atoll_claude_hooks(&verify) {
        return Err(
            "Claude hooks were not saved correctly. Check permissions on ~/.claude/settings.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Claude hook install: {error}");
    }
    hook_trust::record_hook_installed("claude", &script_path);

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(claude_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_claude_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let settings_path =
        claude_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !settings_path.exists() {
        hook_trust::clear_hook_installed("claude");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: settings_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
            competing_hooks: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    let mut settings: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(obj) = settings.as_object_mut() {
        if let Some(hooks) = obj.get_mut("hooks") {
            remove_atoll_claude_hooks(hooks);
            if hooks.as_object().map(|map| map.is_empty()).unwrap_or(false) {
                obj.remove("hooks");
            }
        }
    }

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;
    hook_trust::clear_hook_installed("claude");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(claude_hook_status(&app))
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

/// Stable hook install dir so hooks.json does not point at `target/debug/scripts`,
/// which disappears during rebuilds and makes Codex hooks exit with code 1.
pub(crate) fn atoll_local_hooks_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("Atoll").join("hooks"))
}

pub(crate) fn deployed_hook_script_path(script_name: &str) -> Option<String> {
    let path = atoll_local_hooks_dir()?.join(script_name);
    if hook_script_is_usable(&path) {
        Some(normalize_hook_script_path(&path.to_string_lossy()))
    } else {
        None
    }
}

pub(crate) fn files_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    match (std::fs::read(left), std::fs::read(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn deployed_hook_assets_current(
    source_script: &std::path::Path,
    deployed_script: &std::path::Path,
) -> bool {
    if !files_equal(source_script, deployed_script) {
        return false;
    }

    let Some(source_dir) = source_script.parent() else {
        return true;
    };
    let Some(deployed_dir) = deployed_script.parent() else {
        return true;
    };
    let source_bridge = source_dir.join("atoll-hook-bridge.mjs");
    if !source_bridge.is_file() {
        return true;
    }
    files_equal(&source_bridge, &deployed_dir.join("atoll-hook-bridge.mjs"))
}

pub(crate) fn refresh_deployed_hook_assets_if_needed(app: &AppHandle, script_name: &str) {
    let Some(deployed_script_path) = deployed_hook_script_path(script_name) else {
        return;
    };
    let Ok(source_script_path) = resolve_install_hook_script_path(app, script_name) else {
        return;
    };
    if source_script_path == deployed_script_path {
        return;
    }

    let source = std::path::Path::new(&source_script_path);
    let deployed = std::path::Path::new(&deployed_script_path);
    if deployed_hook_assets_current(source, deployed) {
        return;
    }

    if let Err(error) = materialize_hook_deployment(app, script_name, &source_script_path) {
        eprintln!("Atoll failed to refresh deployed {script_name}: {error}");
    }
}

pub(crate) fn canonical_hook_script_path(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
    marker: &str,
    fallback_path: &str,
) -> String {
    if let Some(deployed) = deployed_hook_script_path(script_name) {
        return deployed;
    }
    if let Some(configured) = config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
    {
        if std::path::Path::new(&configured).is_file() {
            return configured;
        }
    }
    if !fallback_path.is_empty() && std::path::Path::new(fallback_path).is_file() {
        return fallback_path.to_string();
    }
    resolve_hook_script_path(app, script_name).unwrap_or_default()
}

#[cfg(windows)]
pub(crate) fn maybe_repair_hook_launcher_config(
    app: &AppHandle,
    script_name: &str,
    config_filename: &str,
) {
    let Some(local_dir) = dirs::data_local_dir().map(|dir| dir.join("Atoll")) else {
        return;
    };
    let config_path = local_dir.join(config_filename);
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let current_script = config.get("script").and_then(Value::as_str).unwrap_or("");
    let needs_repair = current_script.is_empty()
        || is_dev_hook_script_path(current_script)
        || !std::path::Path::new(current_script).is_file();
    if !needs_repair {
        return;
    }
    let Some(stable_script) = deployed_hook_script_path(script_name) else {
        return;
    };
    let runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| normalize_hook_command_path(&path.to_string_lossy()))
        .or_else(|| hook_runner_for_command(app));
    let node = config
        .get("node")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| resolve_node_executable().ok());
    let (Some(runner), Some(node)) = (runner, node) else {
        return;
    };
    config["script"] = json!(normalize_hook_command_path(&stable_script));
    config["runner"] = json!(runner);
    config["node"] = json!(normalize_hook_command_path(&node));
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, formatted);
    }
}

#[cfg(not(windows))]
pub(crate) fn maybe_repair_hook_launcher_config(
    _app: &AppHandle,
    _script_name: &str,
    _config_filename: &str,
) {
}

#[cfg(windows)]
pub(crate) fn is_windows_file_locked_error(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32))
}

#[cfg(not(windows))]
pub(crate) fn is_windows_file_locked_error(_error: &std::io::Error) -> bool {
    false
}

pub(crate) fn paths_point_to_same_file(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Copy hook assets into the stable deploy dir. If Windows reports that the
/// destination is locked (ERROR_SHARING_VIOLATION / os error 32) because Codex,
/// Cursor, or a live hook invocation still has the runner open, keep the existing
/// file so install can finish updating hooks.json and launcher config.
///
/// Copy via a sibling temp file so `source == dest` cannot truncate the script
/// to 0 bytes (a classic `fs::copy` self-overwrite bug).
pub(crate) fn copy_deployed_hook_file(
    source: &std::path::Path,
    dest: &std::path::Path,
    label: &str,
) -> Result<(), String> {
    if !source.is_file() {
        return Err(format!(
            "Cannot copy {label} from missing {}",
            source.display()
        ));
    }
    if paths_point_to_same_file(source, dest) {
        return Ok(());
    }

    let Some(name) = dest.file_name().and_then(|name| name.to_str()) else {
        return Err(format!("Cannot copy {label} to {}", dest.display()));
    };
    let temp = dest.with_file_name(format!(".{name}.tmp"));
    match std::fs::copy(source, &temp) {
        Ok(_) => {
            if let Err(error) = std::fs::rename(&temp, dest) {
                let _ = std::fs::remove_file(&temp);
                if dest.is_file() && is_windows_file_locked_error(&error) {
                    eprintln!(
                        "Atoll kept existing {label} at {} because the file is in use ({error})",
                        dest.display()
                    );
                    return Ok(());
                }
                return Err(format!(
                    "Cannot copy {label} to {}: {error}",
                    dest.display()
                ));
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            if dest.is_file() && is_windows_file_locked_error(&error) {
                eprintln!(
                    "Atoll kept existing {label} at {} because the file is in use ({error})",
                    dest.display()
                );
                Ok(())
            } else {
                Err(format!(
                    "Cannot copy {label} to {}: {error}",
                    dest.display()
                ))
            }
        }
    }
}

pub(crate) fn materialize_hook_deployment(
    #[cfg_attr(not(windows), allow(unused_variables))] app: &AppHandle,
    script_name: &str,
    source_script_path: &str,
) -> Result<String, String> {
    static DEPLOY_LOCK: Mutex<()> = Mutex::new(());
    let _guard = DEPLOY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let source = std::path::Path::new(source_script_path);
    if !hook_script_is_usable(source) {
        return Err(format!("Hook script not found at: {source_script_path}"));
    }

    let hooks_dir = atoll_local_hooks_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?;
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("Cannot create {}: {error}", hooks_dir.display()))?;

    let dest_script = hooks_dir.join(script_name);
    copy_deployed_hook_file(source, &dest_script, "hook script")?;

    if let Some(source_dir) = source.parent() {
        let bridge_name = "atoll-hook-bridge.mjs";
        let source_bridge = source_dir.join(bridge_name);
        if hook_script_is_usable(&source_bridge) {
            let dest_bridge = hooks_dir.join(bridge_name);
            copy_deployed_hook_file(&source_bridge, &dest_bridge, "hook bridge module")?;
        }
    }

    #[cfg(windows)]
    {
        let dest_runner = hooks_dir.join("atoll-hook-runner.exe");
        if let Some(runner_path) = hook_runner_for_command(app) {
            let runner_source = std::path::Path::new(&runner_path);
            if runner_source.is_file() {
                copy_deployed_hook_file(runner_source, &dest_runner, "hook runner")?;
            }
        }
        if !dest_runner.is_file() {
            return Err(
                "Cannot locate atoll-hook-runner.exe. Rebuild Atoll, then try installing hooks again."
                    .into(),
            );
        }
    }

    Ok(normalize_hook_script_path(&dest_script.to_string_lossy()))
}

#[tauri::command]
pub(crate) fn install_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-codex-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-codex-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable_for_codex()?;

    let hooks_path =
        codex_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.codex directory: {e}"))?;
    }

    let mut config: Value = if hooks_path.exists() {
        let content =
            std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    #[cfg(windows)]
    let hook_command = write_codex_hook_launcher_command(&app, &node_path, &script_path)?;
    #[cfg(not(windows))]
    let hook_command = format_hook_command(None, &node_path, &script_path);
    let atoll_hooks = serde_json::json!({
        "PermissionRequest": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 1800,
                        "statusMessage": "Atoll approval"
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
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": hook_command,
                        "timeout": 30,
                        "statusMessage": "Atoll session sync"
                    }
                ]
            }
        ]
    });

    let config_obj = config
        .as_object_mut()
        .ok_or_else(|| "hooks.json is not a JSON object".to_string())?;
    let hooks_obj = config_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_codex_hook_events(hooks_obj, &atoll_hooks);

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;

    let written =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot verify hooks: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse hooks after write: {e}"))?;
    if !has_atoll_codex_hooks(&verify) {
        return Err(
            "Codex hooks were not saved correctly. Check permissions on ~/.codex/hooks.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Codex hook install: {error}");
    }
    hook_trust::on_codex_hooks_installed(&script_path);

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(codex_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_codex_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let hooks_path =
        codex_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !hooks_path.exists() {
        hook_trust::clear_hook_installed("codex");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: hooks_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
            competing_hooks: Vec::new(),
        });
    }

    let content =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_codex_hooks(hooks);
    }

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;
    hook_trust::clear_hook_installed("codex");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(codex_hook_status(&app))
}

#[tauri::command]
pub(crate) fn get_zcode_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&ZCODE_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_zcode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-zcode-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-zcode-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let config_path =
        zcode_config_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.zcode/cli directory: {e}"))?;
    }

    let mut config: Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Cannot read config: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = write_zcode_hook_launcher_command(&app, &node_path, &script_path)?;

    // ZCode's matcher is a case-sensitive regex on the tool name; omitting it
    // matches every tool (a literal "*" is not guaranteed by the schema).
    // PreToolUse is intentionally NOT registered: it fires for every tool call,
    // while PermissionRequest already covers the approval flow (same split as
    // the Claude/Codex integrations).
    let zcode_hook = |timeout: i64, status_message: &str| {
        json!({
            "type": "command",
            "command": hook_command,
            "timeout": timeout,
            "statusMessage": status_message
        })
    };
    let atoll_hooks = serde_json::json!({
        "PermissionRequest": [
            { "hooks": [zcode_hook(1800, "Atoll approval")] }
        ],
        "PostToolUse": [
            { "hooks": [zcode_hook(30, "Atoll session sync")] }
        ],
        "PostToolUseFailure": [
            { "hooks": [zcode_hook(30, "Atoll session sync")] }
        ],
        "Stop": [
            { "hooks": [zcode_hook(30, "Atoll session sync")] }
        ],
        "SessionStart": [
            { "hooks": [zcode_hook(30, "Atoll session sync")] }
        ],
        "UserPromptSubmit": [
            { "hooks": [zcode_hook(30, "Atoll session sync")] }
        ]
    });

    let config_obj = config
        .as_object_mut()
        .ok_or_else(|| "config.json is not a JSON object".to_string())?;
    let hooks_obj = config_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
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

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize config: {e}"))?;
    std::fs::write(&config_path, formatted).map_err(|e| format!("Cannot write config: {e}"))?;

    let written =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot verify config: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse config after write: {e}"))?;
    if !has_atoll_zcode_hooks(&verify) {
        return Err(
            "ZCode hooks were not saved correctly. Check permissions on ~/.zcode/cli/config.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after ZCode hook install: {error}");
    }
    hook_trust::record_hook_installed("zcode", &script_path);

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(zcode_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_zcode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let config_path =
        zcode_config_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !config_path.exists() {
        hook_trust::clear_hook_installed("zcode");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: config_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
            competing_hooks: Vec::new(),
        });
    }

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("Cannot read config: {e}"))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    // `hooks.enabled` is left untouched: the user may have other configuration
    // hooks that depend on the flag being set.
    if let Some(events) = config
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("events"))
    {
        remove_atoll_zcode_hooks(events);
    }

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize config: {e}"))?;
    std::fs::write(&config_path, formatted).map_err(|e| format!("Cannot write config: {e}"))?;
    hook_trust::clear_hook_installed("zcode");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(zcode_hook_status(&app))
}

#[tauri::command]
pub(crate) fn get_gemini_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&GEMINI_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_gemini_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-gemini-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-gemini-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let settings_path =
        gemini_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.gemini directory: {e}"))?;
    }

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Cannot read settings: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let hook_command = format_hook_command(
        hook_runner_for_command(&app).as_deref(),
        &node_path,
        &script_path,
    );

    // Gemini CLI hook timeouts are in MILLISECONDS (CommandHookConfig.timeout,
    // default 60000). BeforeTool blocks until the Atoll user decides; observer
    // events only register sessions and must never stall a turn.
    // The BeforeTool matcher mirrors the gate list in atoll-gemini-hook.mjs so
    // read-only tools never spawn the hook process.
    let gemini_hook = |timeout: i64| {
        json!({
            "type": "command",
            "command": hook_command,
            "timeout": timeout
        })
    };
    let atoll_hooks = serde_json::json!({
        "BeforeTool": [
            {
                "matcher": "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_",
                "hooks": [gemini_hook(1_800_000)]
            }
        ],
        "SessionStart": [
            { "hooks": [gemini_hook(30_000)] }
        ],
        "SessionEnd": [
            { "hooks": [gemini_hook(30_000)] }
        ],
        "AfterTool": [
            { "hooks": [gemini_hook(30_000)] }
        ],
        "AfterAgent": [
            { "hooks": [gemini_hook(30_000)] }
        ],
        "Notification": [
            { "hooks": [gemini_hook(30_000)] }
        ]
    });

    let settings_obj = settings
        .as_object_mut()
        .ok_or_else(|| "settings.json is not a JSON object".to_string())?;
    let hooks_obj = settings_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    if !hooks_obj.is_object() {
        *hooks_obj = Value::Object(Default::default());
    }
    upsert_gemini_hook_entries(hooks_obj, &atoll_hooks);

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;

    let written = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot verify settings: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse settings after write: {e}"))?;
    if !has_atoll_gemini_hooks(&verify) {
        return Err(
            "Gemini hooks were not saved correctly. Check permissions on ~/.gemini/settings.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Gemini hook install: {error}");
    }
    hook_trust::record_hook_installed("gemini", &script_path);

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(gemini_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_gemini_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let settings_path =
        gemini_settings_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !settings_path.exists() {
        hook_trust::clear_hook_installed("gemini");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: settings_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
            competing_hooks: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {e}"))?;
    let mut settings: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(hooks) = settings.get_mut("hooks") {
        remove_atoll_gemini_hooks(hooks);
    }

    let formatted = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize settings: {e}"))?;
    std::fs::write(&settings_path, formatted).map_err(|e| format!("Cannot write settings: {e}"))?;
    hook_trust::clear_hook_installed("gemini");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(gemini_hook_status(&app))
}

#[tauri::command]
pub(crate) fn get_cursor_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    get_hook_status_for(&CURSOR_HOOK_PROFILE, app)
}

#[tauri::command]
pub(crate) fn install_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, "atoll-cursor-hook.mjs")?;
    let script_path =
        materialize_hook_deployment(&app, "atoll-cursor-hook.mjs", &source_script_path)?;

    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Hook script not found at: {script_path}"));
    }

    let node_path = resolve_node_executable()?;

    let hooks_path =
        cursor_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if let Some(parent) = hooks_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create ~/.cursor directory: {e}"))?;
    }

    let mut config: Value = if hooks_path.exists() {
        let content =
            std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    if config.get("version").is_none() {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("version".to_string(), json!(1));
        }
    }

    let hook_command = write_cursor_hook_launcher_command(&app, &node_path, &script_path)?;

    let config_obj = config
        .as_object_mut()
        .ok_or_else(|| "hooks.json is not a JSON object".to_string())?;
    let hooks_obj = config_obj
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_cursor_hook_events(
        hooks_obj,
        &hook_command,
        &hook_bridge::cursor_hook_url_for_app(&app),
    );

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;

    let written =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot verify hooks: {e}"))?;
    let verify: Value = serde_json::from_str(&written)
        .map_err(|e| format!("Cannot parse hooks after write: {e}"))?;
    if !has_atoll_cursor_hooks(&verify) {
        return Err(
            "Cursor hooks were not saved correctly. Check permissions on ~/.cursor/hooks.json."
                .into(),
        );
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Cursor hook install: {error}");
    }
    hook_trust::record_hook_installed("cursor", &script_path);

    let state = app.state::<AppState>();
    refresh_hook_health_cache(&app, &state);
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(cursor_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_cursor_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let hooks_path =
        cursor_hooks_path().ok_or_else(|| "Cannot determine home directory".to_string())?;

    if !hooks_path.exists() {
        hook_trust::clear_hook_installed("cursor");
        return Ok(HookStatus {
            installed: false,
            script_found: false,
            settings_path: hooks_path.to_string_lossy().into(),
            script_path: String::new(),
            node_path: String::new(),
            node_found: resolve_node_executable().is_ok(),
            needs_retrust: false,
            competing_hooks: Vec::new(),
        });
    }

    let content =
        std::fs::read_to_string(&hooks_path).map_err(|e| format!("Cannot read hooks: {e}"))?;
    let mut config: Value =
        serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()));

    if let Some(hooks) = config.get_mut("hooks") {
        remove_atoll_cursor_hooks(hooks);
    }

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Cannot serialize hooks: {e}"))?;
    std::fs::write(&hooks_path, formatted).map_err(|e| format!("Cannot write hooks: {e}"))?;
    hook_trust::clear_hook_installed("cursor");

    let state = app.state::<AppState>();
    let snapshot = build_snapshot(&app, &state);
    if let Ok(mut last) = state.last_listening_online.lock() {
        *last = Some(snapshot.online);
    }
    remember_hook_health(&state, &snapshot.hook_health);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    Ok(cursor_hook_status(&app))
}

pub(crate) fn normalize_hook_script_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    dunce::simplified(std::path::Path::new(path))
        .to_string_lossy()
        .into_owned()
}

#[cfg(windows)]
pub(crate) fn resolve_node_executable_from_where() -> Option<String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("where.exe")
        .arg("node")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .filter(|path| std::path::Path::new(path).exists())
        .map(normalize_hook_script_path)
}

#[cfg(not(windows))]
pub(crate) fn resolve_node_executable_from_shell() -> Option<String> {
    let output = std::process::Command::new("sh")
        .args(["-lc", "command -v node"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return None;
    }
    Some(normalize_hook_script_path(&path))
}

pub(crate) fn resolve_node_executable_from_path() -> Option<String> {
    if let Some(path_var) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            let candidate = directory.join("node.exe");
            #[cfg(not(windows))]
            let candidate = directory.join("node");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
    }
    None
}

pub(crate) fn resolve_node_executable() -> Result<String, String> {
    #[cfg(windows)]
    {
        if let Some(path) = resolve_node_executable_from_where() {
            return Ok(path);
        }
        if let Some(path) = resolve_node_executable_from_path() {
            return Ok(path);
        }

        for candidate in [
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files (x86)\nodejs\node.exe",
        ] {
            if std::path::Path::new(candidate).exists() {
                return Ok(normalize_hook_script_path(candidate));
            }
        }

        return Err(
            "Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into(),
        );
    }

    #[cfg(not(windows))]
    {
        if let Some(path) = resolve_node_executable_from_shell() {
            return Ok(path);
        }
        if let Some(path) = resolve_node_executable_from_path() {
            return Ok(path);
        }
        Err("Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into())
    }
}

/// Prefer Codex Desktop's bundled Node when available so hooks work in the app sandbox.
pub(crate) fn resolve_codex_desktop_node_executable() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        for candidate in [
            "/Applications/Codex.app/Contents/Resources/cua_node/bin/node",
            "/Applications/Codex.app/Contents/Resources/node/bin/node",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate = std::path::PathBuf::from(local_app_data)
                .join("Programs")
                .join("Codex")
                .join("resources")
                .join("cua_node")
                .join("bin")
                .join("node.exe");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
        for candidate in [
            r"C:\Program Files\Codex\resources\cua_node\bin\node.exe",
            r"C:\Program Files (x86)\Codex\resources\cua_node\bin\node.exe",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = ();
    }

    None
}

pub(crate) fn resolve_node_executable_for_codex() -> Result<String, String> {
    if let Some(path) = resolve_codex_desktop_node_executable() {
        return Ok(path);
    }
    resolve_node_executable()
}

/// Prefer the bundled/repo hook script over `~/Library/Application Support/Atoll/hooks`.
/// The local dir is the *destination* of `materialize_hook_deployment`; using it as the
/// install source lets a 0-byte leftover copy itself and wipe the real script.
pub(crate) fn resolve_install_hook_script_path(
    app: &AppHandle,
    script_name: &str,
) -> Result<String, String> {
    let resource_dir = app.path().resource_dir().ok();
    let exe = std::env::current_exe().ok();
    first_usable_hook_script(bundled_hook_script_candidates(
        resource_dir.as_deref(),
        exe.as_deref(),
        script_name,
    ))
    .ok_or_else(|| format!("Cannot locate hook script: {script_name}"))
}

pub(crate) fn hook_script_is_usable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn repo_hook_script_path(script_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join(script_name)
}

pub(crate) fn bundled_hook_script_candidates(
    resource_dir: Option<&Path>,
    exe: Option<&Path>,
    script_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = vec![repo_hook_script_path(script_name)];

    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join("scripts").join(script_name));
        candidates.push(resource_dir.join(script_name));
    }

    if let Some(exe) = exe {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources").join("scripts").join(script_name));
            candidates.push(exe_dir.join("scripts").join(script_name));
            if exe_dir.file_name().is_some_and(|name| name == "MacOS") {
                if let Some(contents) = exe_dir.parent() {
                    candidates.push(contents.join("Resources").join("scripts").join(script_name));
                }
            }
        }
        for ancestor in exe.ancestors().skip(1) {
            candidates.push(ancestor.join("Resources").join("scripts").join(script_name));
            candidates.push(ancestor.join("scripts").join(script_name));
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
                if let Some(repo_root) = ancestor.parent() {
                    candidates.push(repo_root.join("scripts").join(script_name));
                }
            }
            if ancestor.join("src-tauri").exists() {
                candidates.push(ancestor.join("scripts").join(script_name));
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("scripts")
                        .join(script_name),
                );
            }
        }
    }

    candidates
}

pub(crate) fn first_usable_hook_script(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<String> {
    candidates.into_iter().find_map(|candidate| {
        hook_script_is_usable(&candidate)
            .then(|| normalize_hook_script_path(&candidate.to_string_lossy()))
    })
}

pub(crate) fn normalize_hook_command_path(path: &str) -> String {
    let path = normalize_hook_script_path(path);
    #[cfg(windows)]
    {
        path.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path
    }
}

pub(crate) fn format_hook_command(
    _runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    let node_path = normalize_hook_command_path(node_path);
    let script_path = normalize_hook_command_path(script_path);

    #[cfg(windows)]
    if let Some(runner_path) = _runner_path {
        let runner_path = normalize_hook_command_path(runner_path);
        return format!(
            "\"{}\" \"{}\" \"{}\"",
            runner_path.replace('"', "\\\""),
            node_path.replace('"', "\\\""),
            script_path.replace('"', "\\\"")
        );
    }

    format!(
        "\"{}\" \"{}\"",
        node_path.replace('"', "\\\""),
        script_path.replace('"', "\\\"")
    )
}

/// Windows hook hosts (Cursor, Codex) often spawn hook commands through `cmd /c`.
/// A single quoted string like `"runner.exe" "node.exe" "script.mjs"` fails on paths
/// with spaces or non-ASCII profile dirs. Write a PowerShell launcher that forwards
/// stdin to `atoll-hook-runner.exe`; paths live in a UTF-8 JSON config file.
#[cfg(windows)]
pub(crate) fn write_windows_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
    config_filename: &str,
    ps1_filename: &str,
    fallback_stdout: &str,
) -> Result<String, String> {
    let stable_runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned());
    let runner_path = stable_runner
        .or_else(|| hook_runner_for_command(app))
        .ok_or_else(|| "Cannot locate atoll-hook-runner.exe".to_string())?;
    let local_dir = dirs::data_local_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?
        .join("Atoll");
    std::fs::create_dir_all(&local_dir)
        .map_err(|error| format!("Cannot create {}: {error}", local_dir.display()))?;

    let runner = normalize_hook_command_path(&runner_path);
    let node = normalize_hook_command_path(node_path);
    let script = normalize_hook_command_path(script_path);
    let config_path = local_dir.join(config_filename);
    let config = json!({
        "runner": runner,
        "node": node,
        "script": script,
    });
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("Cannot serialize hook launcher config: {error}"))?;
    std::fs::write(&config_path, config_json.as_bytes())
        .map_err(|error| format!("Cannot write {}: {error}", config_path.display()))?;

    let ps1_path = local_dir.join(ps1_filename);
    let ps1_body = format!(
        r#"$ErrorActionPreference = 'Stop'
$configPath = Join-Path $env:LOCALAPPDATA 'Atoll\{config_filename}'
try {{
  $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $psi = New-Object System.Diagnostics.ProcessStartInfo($config.runner, ('"' + $config.node + '" "' + $config.script + '"'))
  $psi.UseShellExecute = $false
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.CreateNoWindow = $true
  $p = [System.Diagnostics.Process]::Start($psi)
  [Console]::OpenStandardInput().CopyTo($p.StandardInput.BaseStream)
  $p.StandardInput.Close()
  [Console]::Out.Write($p.StandardOutput.ReadToEnd())
  $p.WaitForExit() | Out-Null
  exit $p.ExitCode
}} catch {{
  [Console]::Out.Write('{fallback_stdout}')
  exit 0
}}
"#
    );
    // UTF-8 BOM so Windows PowerShell reads non-ASCII paths from the JSON config reliably.
    let mut ps1_bytes = vec![0xEF, 0xBB, 0xBF];
    ps1_bytes.extend_from_slice(ps1_body.as_bytes());
    std::fs::write(&ps1_path, ps1_bytes)
        .map_err(|error| format!("Cannot write {}: {error}", ps1_path.display()))?;

    Ok(format!(
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
        normalize_hook_command_path(&ps1_path.to_string_lossy()).replace('"', "\\\"")
    ))
}

#[cfg(windows)]
pub(crate) fn write_cursor_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "cursor-hook-launcher.json",
        "atoll-cursor-hook.ps1",
        r#"{"permission":"allow"}"#,
    )
}

#[cfg(windows)]
pub(crate) fn write_codex_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "codex-hook-launcher.json",
        "atoll-codex-hook.ps1",
        "{}",
    )
}

#[cfg(not(windows))]
pub(crate) fn write_codex_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

#[cfg(windows)]
pub(crate) fn write_zcode_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "zcode-hook-launcher.json",
        "atoll-zcode-hook.ps1",
        "{}",
    )
}

#[cfg(not(windows))]
pub(crate) fn write_zcode_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

#[cfg(not(windows))]
pub(crate) fn write_cursor_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

/// Legacy helper kept for tests; production Cursor installs use [`write_cursor_hook_launcher_command`].
#[cfg(windows)]
pub(crate) fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format!(
        "cmd /c {}",
        format_hook_command(runner_path, node_path, script_path)
    )
}

#[cfg(not(windows))]
pub(crate) fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format_hook_command(runner_path, node_path, script_path)
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

/// Events where a dead competitor hook can veto Atoll's permission decision
/// under Claude Code's most-restrictive-wins merge. `PermissionRequest` is the
/// the one that breaks plan approval; the others are observer-style events
/// where a crashing competitor is less harmful but still noise.
pub(crate) const COMPETING_CLAUDE_EVENTS: &[&str] = &[
    "PermissionRequest",
    "PreToolUse",
    "Notification",
    "PermissionDenied",
];

/// Inspect `~/.claude/settings.json` hooks for non-Atoll entries on events
/// where a dead competitor can veto Atoll's decision. For each, report whether
/// the command's binary exists on disk — missing means definitely dead.
pub(crate) fn detect_competing_claude_hooks(config: &Value) -> Vec<CompetingHook> {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for event in COMPETING_CLAUDE_EVENTS {
        let Some(matchers) = hooks.get(*event).and_then(Value::as_array) else {
            continue;
        };
        for matcher in matchers {
            let Some(hook_arr) = matcher.get("hooks").and_then(Value::as_array) else {
                continue;
            };
            for hook in hook_arr {
                let Some(cmd) = hook.get("command").and_then(Value::as_str) else {
                    continue;
                };
                if cmd.contains("atoll-claude-hook") {
                    continue;
                }
                found.push(CompetingHook {
                    event: event.to_string(),
                    command: cmd.to_string(),
                    binary_exists: hook_command_binary_exists(cmd),
                });
            }
        }
    }
    found
}

/// Extract the first token of a hook command (the executable path) and report
/// whether it exists on disk. Handles single-quoted commands like
/// `'/path with spaces/bin' --flag`. Node-script commands (`node "/x.mjs"`)
/// resolve to the node binary; the script path is not checked here — a missing
/// script surfaces via Atoll's own hook-health checks, not as a competitor.
pub(crate) fn hook_command_binary_exists(command: &str) -> bool {
    let trimmed = command.trim();
    let first = if let Some(rest) = trimmed.strip_prefix('\'') {
        rest.split('\'').next().unwrap_or("")
    } else if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };
    if first.is_empty() {
        return false;
    }
    Path::new(first).exists()
}

/// Remove non-Atoll hooks whose binaries are missing from `~/.claude/settings.json`,
/// across events where a dead competitor can veto Atoll's permission decision.
/// Mutates `settings` in place. Returns true if any hook was removed.
pub(crate) fn remove_dead_competing_hooks_from_config(settings: &mut Value) -> bool {
    let mut removed_any = false;
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return false;
    };
    for event in COMPETING_CLAUDE_EVENTS {
        let Some(matchers) = hooks.get_mut(*event).and_then(Value::as_array_mut) else {
            continue;
        };
        for matcher in matchers.iter_mut() {
            let Some(hook_arr) = matcher.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            let before = hook_arr.len();
            hook_arr.retain(|hook| {
                let Some(cmd) = hook.get("command").and_then(Value::as_str) else {
                    return true;
                };
                if cmd.contains("atoll-claude-hook") {
                    return true;
                }
                hook_command_binary_exists(cmd)
            });
            removed_any |= hook_arr.len() != before;
        }
        matchers.retain(|matcher| {
            matcher
                .get("hooks")
                .and_then(Value::as_array)
                .map(|arr| !arr.is_empty())
                .unwrap_or(false)
        });
        if matchers.is_empty() {
            hooks.remove(*event);
        }
    }
    if hooks.is_empty() {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("hooks");
        }
    }
    removed_any
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

pub(crate) fn read_json_file(path: &str) -> Option<Value> {
    if path.is_empty() || !std::path::Path::new(path).exists() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

pub(crate) fn extract_first_quoted_value(input: &str) -> Option<(String, &str)> {
    let input = input.trim();
    let inner = input.strip_prefix('"')?;
    let end = inner.find('"')?;
    let value = inner[..end].replace("\\\"", "\"");
    let rest = inner[end + 1..].trim_start();
    Some((value, rest))
}

pub(crate) fn expand_windows_hook_env(command: &str) -> String {
    let mut result = command.to_string();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        result = result.replace("%LOCALAPPDATA%", &local);
    }
    if let Ok(user) = std::env::var("USERPROFILE") {
        result = result.replace("%USERPROFILE%", &user);
    }
    result
}

pub(crate) fn extract_hook_command_parts(command: &str) -> Option<(String, String)> {
    let mut trimmed = expand_windows_hook_env(command).trim().to_string();
    if let Some(rest) = trimmed
        .strip_prefix("cmd /c ")
        .or_else(|| trimmed.strip_prefix("cmd /C "))
    {
        trimmed = rest.trim().to_string();
    }

    let normalized = normalize_hook_script_path(&trimmed);
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.ps1")
        || normalized
            .to_ascii_lowercase()
            .ends_with("atoll-codex-hook.ps1")
    {
        return parse_hook_launcher_script(&normalized);
    }
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.cmd")
    {
        return parse_cursor_launcher_cmd(&normalized);
    }

    if trimmed.starts_with("powershell ") {
        if let Some(start) = trimmed.find("-File \"") {
            let rest = &trimmed[start + 7..];
            if let Some(end) = rest.find('"') {
                let ps1 = normalize_hook_script_path(&expand_windows_hook_env(&rest[..end]));
                return parse_hook_launcher_script(&ps1);
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("node ") {
        let script = if rest.starts_with('"') {
            extract_first_quoted_value(rest)?.0
        } else {
            rest.split_whitespace().next()?.to_string()
        };
        return Some(("node".to_string(), normalize_hook_script_path(&script)));
    }

    let (first, rest) = extract_first_quoted_value(trimmed.as_str())?;
    if is_hook_runner_path(&first) {
        let (node, script_rest) = extract_first_quoted_value(rest)?;
        let (script, _) = extract_first_quoted_value(script_rest)?;
        return Some((
            normalize_hook_script_path(&node),
            normalize_hook_script_path(&script),
        ));
    }

    let (node, rest) = (first, rest);
    let (script, _) = extract_first_quoted_value(rest)?;
    Some((
        normalize_hook_script_path(&node),
        normalize_hook_script_path(&script),
    ))
}

pub(crate) fn is_hook_runner_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("/atoll-hook-runner.exe")
        || normalized.ends_with("/atoll-hook-runner")
        || normalized.contains("/atoll-hook-runner-")
}

pub(crate) fn parse_cursor_launcher_cmd(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("@echo off") {
            continue;
        }
        return extract_hook_command_parts(line);
    }
    None
}

pub(crate) fn parse_hook_launcher_config(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: Value = serde_json::from_str(&content).ok()?;
    let node = config.get("node").and_then(Value::as_str)?;
    let script = config.get("script").and_then(Value::as_str)?;
    Some((
        normalize_hook_script_path(node),
        normalize_hook_script_path(script),
    ))
}

pub(crate) fn parse_hook_launcher_script(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for (marker, filename) in [
        ("cursor-hook-launcher.json", "cursor-hook-launcher.json"),
        ("codex-hook-launcher.json", "codex-hook-launcher.json"),
    ] {
        if content.contains(marker) {
            let config_path = std::path::Path::new(path)
                .parent()
                .map(|dir| dir.join(filename))
                .filter(|candidate| candidate.is_file())
                .or_else(|| dirs::data_local_dir().map(|dir| dir.join("Atoll").join(filename)))?;
            return parse_hook_launcher_config(&config_path.to_string_lossy());
        }
    }
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains("atoll-hook-runner") {
            continue;
        }
        let line = line
            .strip_prefix("$Input | & ")
            .or_else(|| line.strip_prefix("$input | & "))
            .unwrap_or(line);
        return extract_hook_command_parts(line);
    }
    None
}

pub(crate) fn extract_node_script_path(command: &str) -> Option<String> {
    extract_hook_command_parts(command).map(|(_, script)| script)
}

pub(crate) fn configured_atoll_hook_command(config: &Value, marker: &str) -> Option<String> {
    let hooks = config.get("hooks")?.as_object()?;
    // ZCode nests event matchers under `hooks.events` (alongside `enabled` and
    // `timeoutMs`); Claude/Codex/Cursor list the event arrays directly under
    // `hooks`.
    let events = hooks
        .get("events")
        .and_then(Value::as_object)
        .unwrap_or(hooks);
    for matchers in events.values() {
        let Some(arr) = matchers.as_array() else {
            continue;
        };
        for matcher in arr {
            if let Some(hook_arr) = matcher.get("hooks").and_then(Value::as_array) {
                for hook in hook_arr {
                    if let Some(cmd) = hook.get("command").and_then(Value::as_str) {
                        if cmd.contains(marker) {
                            return Some(cmd.to_string());
                        }
                    }
                }
            }
            if let Some(cmd) = matcher.get("command").and_then(Value::as_str) {
                if cmd.contains(marker) {
                    return Some(cmd.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn configured_atoll_hook_node_path(config: &Value, marker: &str) -> Option<String> {
    configured_atoll_hook_command(config, marker)
        .and_then(|cmd| extract_hook_command_parts(&cmd).map(|(node, _)| node))
}

pub(crate) fn node_executable_ready(node_path: &str) -> bool {
    if node_path.is_empty() {
        return resolve_node_executable().is_ok();
    }
    if node_path == "node" {
        return resolve_node_executable().is_ok();
    }
    std::path::Path::new(node_path).exists()
}

pub(crate) fn configured_atoll_hook_script_path(config: &Value, marker: &str) -> Option<String> {
    configured_atoll_hook_command(config, marker).and_then(|cmd| extract_node_script_path(&cmd))
}

pub(crate) fn resolve_hook_script_readiness(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
) -> (String, bool) {
    let marker = script_name.trim_end_matches(".mjs");
    let mut script_path = resolve_hook_script_path(app, script_name).unwrap_or_default();
    let mut script_found =
        !script_path.is_empty() && hook_script_is_usable(Path::new(&script_path));

    if !script_found {
        if let Some(configured) =
            config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
        {
            if hook_script_is_usable(Path::new(&configured)) {
                script_found = true;
                if script_path.is_empty() {
                    script_path = configured;
                }
            }
        }
    }

    (script_path, script_found)
}

pub(crate) fn resolve_hook_script_path(app: &AppHandle, script_name: &str) -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join(script_name));
    }
    let resource_dir = app.path().resource_dir().ok();
    let exe = std::env::current_exe().ok();
    candidates.extend(bundled_hook_script_candidates(
        resource_dir.as_deref(),
        exe.as_deref(),
        script_name,
    ));

    first_usable_hook_script(candidates)
}

#[cfg(windows)]
pub(crate) fn resolve_hook_runner_path(app: &AppHandle) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join("atoll-hook-runner.exe"));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("atoll-hook-runner.exe"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("atoll-hook-runner.exe"));
            candidates.push(
                exe_dir
                    .join("resources")
                    .join("scripts")
                    .join("atoll-hook-runner.exe"),
            );
            candidates.push(exe_dir.join("scripts").join("atoll-hook-runner.exe"));
        }
        for ancestor in exe.ancestors().skip(1) {
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
                candidates.push(
                    ancestor
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("target")
                        .join("release")
                        .join("atoll-hook-runner.exe"),
                );
            }
            if ancestor.join("src-tauri").exists() {
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("generated")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("release")
                        .join("atoll-hook-runner.exe"),
                );
            }
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
        }
    }

    None
}

#[cfg(not(windows))]
pub(crate) fn resolve_hook_runner_path(_app: &AppHandle) -> Option<String> {
    None
}

pub(crate) fn hook_runner_for_command(app: &AppHandle) -> Option<String> {
    resolve_hook_runner_path(app)
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

pub(crate) fn hook_entry_has_atoll_cursor(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .map(|cmd| {
            cmd.contains("atoll-cursor-hook")
                || cmd.contains("atoll-cursor-hook.ps1")
                || cmd.contains("atoll-cursor-hook.cmd")
        })
        .unwrap_or(false)
}

pub(crate) const CURSOR_HOOK_TIMEOUT_SECONDS: u64 = 5;
pub(crate) const CURSOR_HOOK_EVENTS: [(&str, u64); 10] = [
    ("sessionStart", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("beforeSubmitPrompt", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("afterAgentResponse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("afterAgentThought", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("sessionEnd", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("preToolUse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("postToolUse", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("stop", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("subagentStart", CURSOR_HOOK_TIMEOUT_SECONDS),
    ("subagentStop", CURSOR_HOOK_TIMEOUT_SECONDS),
];

pub(crate) const CURSOR_CORE_HOOK_EVENTS: [&str; 5] = [
    "preToolUse",
    "postToolUse",
    "stop",
    "subagentStart",
    "subagentStop",
];

pub(crate) const CURSOR_LIFECYCLE_HOOK_EVENTS: [&str; 5] = [
    "sessionStart",
    "beforeSubmitPrompt",
    "afterAgentResponse",
    "afterAgentThought",
    "sessionEnd",
];

pub(crate) fn upsert_cursor_hook_events(hooks: &mut Value, hook_command: &str, hook_url: &str) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    // Composer / Agent Chat hooks only. Tab inline-completion hooks (`beforeTabFileRead`,
    // `afterTabFileEdit`) are intentionally excluded: Tab does not create a Composer
    // session or emit sessionStart, so Atoll cannot attribute usage to a session.
    for (event, timeout) in CURSOR_HOOK_EVENTS {
        let mut merged: Vec<Value> = hooks_obj
            .get(event)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter(|entry| !hook_entry_has_atoll_cursor(entry))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        merged.push(json!({
            "command": hook_command,
            "timeout": timeout,
            "env": {
                "ATOLL_HOOK_URL": hook_url
            }
        }));
        hooks_obj.insert(event.to_string(), Value::Array(merged));
    }
}

pub(crate) fn remove_atoll_cursor_hooks(hooks: &mut Value) {
    let Some(hooks_obj) = hooks.as_object_mut() else {
        return;
    };

    for entries in hooks_obj.values_mut() {
        if let Some(arr) = entries.as_array_mut() {
            arr.retain(|entry| !hook_entry_has_atoll_cursor(entry));
        }
    }

    hooks_obj.retain(|_, entries| {
        entries
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });
}

pub(crate) fn cursor_hooks_have_events(config: &Value, events: &[&str]) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    events.iter().all(|event| {
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .map(|arr| arr.iter().any(hook_entry_has_atoll_cursor))
            .unwrap_or(false)
    })
}

pub(crate) fn cursor_hooks_need_lifecycle_upgrade(config: &Value) -> bool {
    has_atoll_cursor_hooks(config)
        && !cursor_hooks_have_events(config, &CURSOR_LIFECYCLE_HOOK_EVENTS)
}

pub(crate) fn cursor_hooks_need_timeout_repair(config: &Value) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    hooks.values().any(|entries| {
        entries
            .as_array()
            .map(|arr| {
                arr.iter().any(|entry| {
                    hook_entry_has_atoll_cursor(entry)
                        && entry.get("timeout").and_then(Value::as_u64)
                            != Some(CURSOR_HOOK_TIMEOUT_SECONDS)
                })
            })
            .unwrap_or(false)
    })
}

pub(crate) fn cursor_hook_command_needs_repair(
    command: &str,
    preferred_script_path: Option<&str>,
    require_powershell_launcher: bool,
) -> bool {
    let lower = command.to_ascii_lowercase();
    if require_powershell_launcher
        && !(lower.starts_with("powershell ")
            && lower.contains("atoll-cursor-hook.ps1")
            && lower.contains("-file "))
    {
        return true;
    }

    let Some((_node, script)) = extract_hook_command_parts(command) else {
        return true;
    };

    if let Some(preferred) = preferred_script_path {
        if should_flag_dev_hook_drift(&script, preferred) {
            return true;
        }
    }

    !std::path::Path::new(&script).is_file()
}

pub(crate) fn cursor_hooks_need_command_repair(
    config: &Value,
    preferred_script_path: Option<&str>,
    require_powershell_launcher: bool,
) -> bool {
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return false;
    };

    hooks.values().any(|entries| {
        entries
            .as_array()
            .map(|arr| {
                arr.iter().any(|entry| {
                    entry
                        .get("command")
                        .and_then(Value::as_str)
                        .filter(|command| {
                            hook_entry_has_atoll_cursor(entry)
                                && cursor_hook_command_needs_repair(
                                    command,
                                    preferred_script_path,
                                    require_powershell_launcher,
                                )
                        })
                        .is_some()
                })
            })
            .unwrap_or(false)
    })
}

pub(crate) fn repair_cursor_hook_events_with_command(
    config: &Value,
    hook_command: &str,
    hook_url: &str,
) -> Option<Value> {
    let mut repaired = config.clone();
    if repaired.get("version").is_none() {
        if let Some(obj) = repaired.as_object_mut() {
            obj.insert("version".to_string(), json!(1));
        }
    }
    let hooks_obj = repaired
        .as_object_mut()?
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));
    upsert_cursor_hook_events(hooks_obj, hook_command, hook_url);
    Some(repaired)
}

pub(crate) fn preferred_cursor_hook_command(
    app: &AppHandle,
    source_script_path: &str,
) -> Result<(String, String), String> {
    let script_path =
        materialize_hook_deployment(app, "atoll-cursor-hook.mjs", source_script_path)?;
    let node_path = resolve_node_executable()?;
    let hook_command = write_cursor_hook_launcher_command(app, &node_path, &script_path)?;
    Ok((hook_command, script_path))
}

pub(crate) fn maybe_repair_cursor_hook_events(
    app: &AppHandle,
    hooks_path: &str,
    config: Option<&Value>,
    hook_url: &str,
) -> Option<Value> {
    let config = config?;
    if !has_atoll_cursor_hooks(config) {
        return None;
    }

    let source_script_path = resolve_install_hook_script_path(app, "atoll-cursor-hook.mjs").ok()?;
    let preferred_script_path = deployed_hook_script_path("atoll-cursor-hook.mjs")
        .unwrap_or_else(|| source_script_path.clone());
    let needs_repair = cursor_hooks_need_lifecycle_upgrade(config)
        || cursor_hooks_need_timeout_repair(config)
        || cursor_hooks_need_command_repair(config, Some(&preferred_script_path), cfg!(windows));
    if !needs_repair {
        return None;
    }

    let (hook_command, _script_path) =
        preferred_cursor_hook_command(app, &source_script_path).ok()?;
    let repaired = repair_cursor_hook_events_with_command(config, &hook_command, hook_url)?;
    let formatted = serde_json::to_string_pretty(&repaired).ok()?;
    if std::fs::write(hooks_path, formatted).is_err() {
        return None;
    }
    eprintln!("Atoll repaired Cursor hooks with current launcher command");
    Some(repaired)
}

/// Returns true when Atoll's Cursor hooks are installed.
///
/// Only the core Composer/Agent events that shipped with v0.1.31
/// (`preToolUse`, `postToolUse`, `stop`, `subagentStart`, `subagentStop`) are
/// required. The v0.1.32 lifecycle hooks (`sessionStart`, `beforeSubmitPrompt`,
/// `afterAgentResponse`, `afterAgentThought`, `sessionEnd`) are an optional
/// enhancement for Ask/Composer-mode session tracking: users who installed
/// hooks with v0.1.31 only have the core five, and treating them as
/// "not installed" regresses session display and the online indicator. Those
/// users keep working; hook status repair can add the new events in place.
pub(crate) fn has_atoll_cursor_hooks(config: &Value) -> bool {
    cursor_hooks_have_events(config, &CURSOR_CORE_HOOK_EVENTS)
}
