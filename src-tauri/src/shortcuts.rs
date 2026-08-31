//! Global shortcuts: hotkeys for summoning the island and approving/denying the
//! current pending request.
//!
//! The config lives in the shared settings KV store under the `globalShortcuts`
//! key (see `atoll_settings_path` in lib.rs). Registration happens once at
//! startup (`startup`) and again on every `set_global_shortcut_config` call;
//! both paths funnel through `apply_config`, which first drops every previous
//! registration + handler (`unregister_all`) so re-applying is idempotent.
//! Registration and validation failures are reported per action instead of
//! being swallowed, so the Settings UI can render a per-row error state.
//!
//! Accelerators are stored in a canonical `Cmd|Ctrl[+Alt][+Shift]+Key` form
//! (e.g. `Cmd+Shift+Space`). `normalize_accelerator` accepts the aliases the
//! tauri-plugin-global-shortcut parser understands (`CmdOrCtrl`, `Option`,
//! `KeyA`, digits, arrows, F1-F24, punctuation symbols) and canonicalizes them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use tauri::{AppHandle, Manager};

const SETTINGS_KEY: &str = "globalShortcuts";
/// "Bring up the island" — deliberately avoids Cmd+Space (macOS Spotlight) and
/// Alt+Space (Windows window menu / PowerToys Run).
const DEFAULT_SUMMON: &str = "CmdOrCtrl+Shift+Space";
/// Y = yes. Overrides app-local menu shortcuts while registered; users can
/// rebind or turn the feature off.
const DEFAULT_APPROVE: &str = "CmdOrCtrl+Shift+Y";
/// N = no.
const DEFAULT_DENY: &str = "CmdOrCtrl+Shift+N";

/// What a registered global shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutAction {
    Summon,
    Approve,
    Deny,
}

impl ShortcutAction {
    pub(crate) const ALL: [ShortcutAction; 3] = [
        ShortcutAction::Summon,
        ShortcutAction::Approve,
        ShortcutAction::Deny,
    ];

    fn accel<'a>(self, config: &'a GlobalShortcutConfig) -> &'a str {
        match self {
            ShortcutAction::Summon => &config.summon,
            ShortcutAction::Approve => &config.approve,
            ShortcutAction::Deny => &config.deny,
        }
    }

    fn set_accel(self, config: &mut GlobalShortcutConfig, value: String) {
        match self {
            ShortcutAction::Summon => config.summon = value,
            ShortcutAction::Approve => config.approve = value,
            ShortcutAction::Deny => config.deny = value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct GlobalShortcutConfig {
    pub enabled: bool,
    pub summon: String,
    pub approve: String,
    pub deny: String,
}

impl Default for GlobalShortcutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            summon: default_accel(DEFAULT_SUMMON),
            approve: default_accel(DEFAULT_APPROVE),
            deny: default_accel(DEFAULT_DENY),
        }
    }
}

/// Per-action error state; `None` means the action registered fine (or is off).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalShortcutErrors {
    pub summon: Option<String>,
    pub approve: Option<String>,
    pub deny: Option<String>,
}

impl GlobalShortcutErrors {
    pub(crate) fn has_errors(&self) -> bool {
        self.summon.is_some() || self.approve.is_some() || self.deny.is_some()
    }

    fn slot(&mut self, action: ShortcutAction) -> &mut Option<String> {
        match action {
            ShortcutAction::Summon => &mut self.summon,
            ShortcutAction::Approve => &mut self.approve,
            ShortcutAction::Deny => &mut self.deny,
        }
    }

    fn set(&mut self, action: ShortcutAction, message: Option<String>) {
        *self.slot(action) = message;
    }
}

/// What `AppState.global_shortcuts` caches: the live config plus the errors
/// from the last registration attempt (including at app startup).
#[derive(Debug, Clone, Default)]
pub(crate) struct GlobalShortcutsState {
    pub config: GlobalShortcutConfig,
    pub errors: GlobalShortcutErrors,
}

/// Command-facing view: config plus per-action errors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalShortcutView {
    pub config: GlobalShortcutConfig,
    pub errors: GlobalShortcutErrors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimaryModifier {
    Cmd,
    Ctrl,
}

fn default_accel(accelerator: &str) -> String {
    // The constants above are valid by construction; the fallback only keeps
    // this total if normalization ever regressed.
    normalize_accelerator(accelerator).unwrap_or_else(|_| accelerator.to_string())
}

/// Canonicalize + validate one accelerator string.
///
/// Accepted: modifier aliases (`Cmd`/`Command`/`Meta`/`Super`/`Windows`,
/// `Ctrl`/`Control`, `Alt`/`Option`, `Shift`, plus `CmdOrCtrl` spellings which
/// resolve per-platform like Tauri's own semantics), keys `A`-`Z`, `0`-`9`,
/// `Space`/`Enter`/`Tab`/`Escape`, arrows (`Up`/`Down`/`Left`/`Right`),
/// `F1`-`F24`, and punctuation symbols (`-` `=` `[` `]` `,` `.` `/` `;` `'` `` ` `` `\`).
/// At least one modifier is required, except bare `F1`-`F24` (a bare letter or
/// digit would swallow normal typing system-wide).
pub(crate) fn normalize_accelerator(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("accelerator is empty".to_string());
    }
    let mut primary: Option<PrimaryModifier> = None;
    let mut alt = false;
    let mut shift = false;
    let mut key: Option<String> = None;
    for token in trimmed.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return Err(format!("accelerator \"{input}\" has an empty part"));
        }
        let upper = token.to_ascii_uppercase();
        let primary_token = match upper.as_str() {
            "CMD" | "COMMAND" | "META" | "SUPER" | "WINDOWS" | "WIN" => Some(PrimaryModifier::Cmd),
            "CTRL" | "CONTROL" => Some(PrimaryModifier::Ctrl),
            "CMDORCTRL" | "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCONTROL" => {
                Some(if cfg!(target_os = "macos") {
                    PrimaryModifier::Cmd
                } else {
                    PrimaryModifier::Ctrl
                })
            }
            _ => None,
        };
        if let Some(value) = primary_token {
            if primary.is_some() && primary != Some(value) {
                return Err(format!("accelerator \"{input}\" combines Cmd and Ctrl"));
            }
            primary = Some(value);
            continue;
        }
        match upper.as_str() {
            "ALT" | "OPTION" => alt = true,
            "SHIFT" => shift = true,
            _ => {
                if key.is_some() {
                    return Err(format!("accelerator \"{input}\" has more than one key"));
                }
                key = Some(normalize_key(&upper, input)?);
            }
        }
    }
    let Some(key) = key else {
        return Err(format!("accelerator \"{input}\" is missing a key"));
    };
    let mut parts: Vec<&str> = Vec::with_capacity(4);
    match primary {
        Some(PrimaryModifier::Cmd) => parts.push("Cmd"),
        Some(PrimaryModifier::Ctrl) => parts.push("Ctrl"),
        None => {}
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if parts.is_empty() && !is_function_key(&key) {
        return Err(format!(
            "accelerator \"{input}\" needs at least one modifier (bare F1-F24 are the exception)"
        ));
    }
    parts.push(&key);
    Ok(parts.join("+"))
}

fn normalize_key(upper: &str, input: &str) -> Result<String, String> {
    let invalid = || format!("accelerator \"{input}\" uses unsupported key \"{upper}\"");
    if upper.len() == 1 {
        let byte = upper.as_bytes()[0];
        if byte.is_ascii_uppercase() || byte.is_ascii_digit() {
            return Ok(upper.to_string());
        }
        return match upper {
            "-" | "=" | "[" | "]" | "," | "." | "/" | ";" | "'" | "`" | "\\" => {
                Ok(upper.to_string())
            }
            _ => Err(invalid()),
        };
    }
    if let Some(letter) = upper.strip_prefix("KEY") {
        if letter.len() == 1 {
            let byte = letter.as_bytes()[0];
            if byte.is_ascii_uppercase() {
                return Ok(letter.to_string());
            }
            return Err(invalid());
        }
    }
    if let Some(digit) = upper.strip_prefix("DIGIT") {
        if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() {
            return Ok(digit.to_string());
        }
        return Err(invalid());
    }
    if let Some(number) = upper.strip_prefix('F') {
        if !number.is_empty()
            && number.len() <= 2
            && number.bytes().all(|byte| byte.is_ascii_digit())
        {
            if let Ok(value) = number.parse::<u8>() {
                if (1..=24).contains(&value) {
                    return Ok(format!("F{value}"));
                }
            }
        }
        return Err(invalid());
    }
    Ok(match upper {
        "SPACE" => "Space".to_string(),
        "ENTER" => "Enter".to_string(),
        "TAB" => "Tab".to_string(),
        "ESCAPE" | "ESC" => "Escape".to_string(),
        "UP" | "ARROWUP" => "Up".to_string(),
        "DOWN" | "ARROWDOWN" => "Down".to_string(),
        "LEFT" | "ARROWLEFT" => "Left".to_string(),
        "RIGHT" | "ARROWRIGHT" => "Right".to_string(),
        _ => return Err(invalid()),
    })
}

fn is_function_key(key: &str) -> bool {
    let Some(number) = key.strip_prefix('F') else {
        return false;
    };
    !number.is_empty()
        && number.len() <= 2
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && number
            .parse::<u8>()
            .map(|value| (1..=24).contains(&value))
            .unwrap_or(false)
}

/// Canonicalize every action of a config, collecting per-action failures.
/// Invalid accelerators keep their raw value so the UI can show what the user
/// actually typed next to the error.
pub(crate) fn canonicalize_config(
    config: GlobalShortcutConfig,
) -> (GlobalShortcutConfig, GlobalShortcutErrors) {
    let mut errors = GlobalShortcutErrors::default();
    let mut canonical = config.clone();
    for action in ShortcutAction::ALL {
        let raw = action.accel(&config).trim().to_string();
        if raw.is_empty() {
            continue; // explicitly cleared action — nothing to validate
        }
        match normalize_accelerator(&raw) {
            Ok(value) => action.set_accel(&mut canonical, value),
            Err(message) => errors.set(action, Some(message)),
        }
    }
    (canonical, errors)
}

// ---------------------------------------------------------------------------
// Persistence (settings KV store)
// ---------------------------------------------------------------------------

fn sanitized_accel(value: Option<&Value>, fallback: &str) -> String {
    let Some(raw) = value.and_then(Value::as_str) else {
        return fallback.to_string();
    };
    if raw.trim().is_empty() {
        // Explicitly cleared action: keep it empty rather than resurrecting a
        // default binding the user removed.
        return String::new();
    }
    normalize_accelerator(raw).unwrap_or_else(|_| fallback.to_string())
}

/// Test-visible kernel: load from an explicit settings file so unit tests never
/// touch the real `~/.atoll/settings.json`.
fn load_config_from_path(path: &Path) -> GlobalShortcutConfig {
    let defaults = GlobalShortcutConfig::default();
    let Ok(content) = std::fs::read_to_string(path) else {
        return defaults;
    };
    let Ok(document) = serde_json::from_str::<Value>(&content) else {
        return defaults;
    };
    let Some(settings) = document.get(SETTINGS_KEY) else {
        return defaults;
    };
    GlobalShortcutConfig {
        enabled: settings
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(defaults.enabled),
        summon: sanitized_accel(settings.get("summon"), &defaults.summon),
        approve: sanitized_accel(settings.get("approve"), &defaults.approve),
        deny: sanitized_accel(settings.get("deny"), &defaults.deny),
    }
}

fn persist_config_to_path(path: &Path, config: &GlobalShortcutConfig) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut document: Value = path
        .exists()
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Ok(value) = serde_json::to_value(config) {
        if let Some(object) = document.as_object_mut() {
            object.insert(SETTINGS_KEY.to_string(), value);
        }
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&document) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn load_global_shortcut_config() -> GlobalShortcutConfig {
    match crate::atoll_settings_path() {
        Some(path) => load_config_from_path(&path),
        None => GlobalShortcutConfig::default(),
    }
}

pub(crate) fn persist_global_shortcut_config(config: &GlobalShortcutConfig) {
    if let Some(path) = crate::atoll_settings_path() {
        persist_config_to_path(&path, config);
    }
}

// ---------------------------------------------------------------------------
// Registration + dispatch
// ---------------------------------------------------------------------------

/// Register every enabled action, replacing any previous registrations. Per
/// action failures (e.g. the hotkey is taken by another app) are returned
/// instead of failing the whole batch.
#[cfg(desktop)]
pub(crate) fn apply_config(app: &AppHandle, config: &GlobalShortcutConfig) -> GlobalShortcutErrors {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
    let manager = app.global_shortcut();
    // unregister_all also clears the stored handlers, so re-applying a changed
    // config never leaves stale bindings behind.
    let _ = manager.unregister_all();
    let mut errors = GlobalShortcutErrors::default();
    if !config.enabled {
        return errors;
    }
    for action in ShortcutAction::ALL {
        let accelerator = action.accel(config).trim().to_string();
        if accelerator.is_empty() {
            continue; // cleared action — nothing to register
        }
        let shortcut = match accelerator.parse::<Shortcut>() {
            Ok(shortcut) => shortcut,
            Err(error) => {
                // Unreachable for canonicalized configs; kept as a guard.
                errors.set(
                    action,
                    Some(format!("invalid accelerator {accelerator}: {error}")),
                );
                continue;
            }
        };
        if let Err(error) = manager.on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                dispatch(app, action);
            }
        }) {
            errors.set(action, Some(format!("register {accelerator}: {error}")));
        }
    }
    errors
}

#[cfg(not(desktop))]
pub(crate) fn apply_config(
    _app: &AppHandle,
    _config: &GlobalShortcutConfig,
) -> GlobalShortcutErrors {
    GlobalShortcutErrors::default()
}

/// Load the persisted config, register it, and cache config + errors in state.
#[cfg(desktop)]
pub(crate) fn startup(app: &AppHandle) {
    let config = load_global_shortcut_config();
    let errors = apply_config(app, &config);
    record_state(app, config, errors);
}

fn record_state(app: &AppHandle, config: GlobalShortcutConfig, errors: GlobalShortcutErrors) {
    let state = app.state::<crate::AppState>();
    *crate::lock_state(&state.global_shortcuts) = GlobalShortcutsState { config, errors };
}

#[cfg(desktop)]
fn dispatch(app: &AppHandle, action: ShortcutAction) {
    match action {
        ShortcutAction::Summon => {
            crate::show_main_window_with_focus(app, true, crate::IslandOpenSource::Summon)
        }
        ShortcutAction::Approve => resolve_pending(app, crate::Decision::Approved),
        ShortcutAction::Deny => resolve_pending(app, crate::Decision::Denied),
    }
}

/// Resolve the current pending request through the same command the island UI
/// buttons invoke; a graceful no-op when nothing is pending.
#[cfg(desktop)]
fn resolve_pending(app: &AppHandle, decision: crate::Decision) {
    let state = app.state::<crate::AppState>();
    let Some(id) = pending_request_id(&state) else {
        return;
    };
    let _ = crate::resolve_permission_request(app.clone(), state, id, decision, String::new());
}

/// Same "current request" rule as the snapshot's active_request
/// (first visible pending request).
#[cfg(desktop)]
fn pending_request_id(state: &crate::AppState) -> Option<String> {
    let requests = crate::lock_state(&state.requests);
    requests
        .iter()
        .find(|request| request.status == crate::PermissionStatus::Pending && !request.archived)
        .map(|request| request.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_aliases_case_and_order() {
        assert_eq!(
            normalize_accelerator(" cmd + shift + y ").unwrap(),
            "Cmd+Shift+Y"
        );
        assert_eq!(
            normalize_accelerator("SHIFT+ALT+SPACE").unwrap(),
            "Alt+Shift+Space"
        );
        assert_eq!(
            normalize_accelerator("option+command+digit7").unwrap(),
            "Cmd+Alt+7"
        );
        assert_eq!(normalize_accelerator("ctrl+arrowup").unwrap(), "Ctrl+Up");
        assert_eq!(normalize_accelerator("Control+KeyN").unwrap(), "Ctrl+N");
        // Trailing modifier tokens are reordered into the canonical form.
        assert_eq!(normalize_accelerator("y+shift+cmd").unwrap(), "Cmd+Shift+Y");
        // Duplicate modifiers collapse instead of failing.
        assert_eq!(normalize_accelerator("Cmd+Cmd+Y").unwrap(), "Cmd+Y");
    }

    #[test]
    fn maps_cmdorctrl_per_platform() {
        let value = normalize_accelerator("CmdOrCtrl+Shift+Y").unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(value, "Cmd+Shift+Y");
        } else {
            assert_eq!(value, "Ctrl+Shift+Y");
        }
    }

    #[test]
    fn allows_bare_function_keys() {
        assert_eq!(normalize_accelerator("F5").unwrap(), "F5");
        assert_eq!(normalize_accelerator("f24").unwrap(), "F24");
    }

    #[test]
    fn rejects_invalid_accelerators() {
        for input in [
            "",
            "   ",
            "++",
            "Cmd+",
            "Cmd",
            "Shift",
            "Y",
            "Cmd+Y+W",
            "Cmd+Bogus",
            "Cmd+Ctrl+Y", // conflicting primaries
            "Cmd+F25",
            "Cmd+F0",
        ] {
            assert!(
                normalize_accelerator(input).is_err(),
                "expected reject: {input}"
            );
        }
    }

    #[test]
    fn defaults_are_valid() {
        let config = GlobalShortcutConfig::default();
        assert!(config.enabled);
        for action in ShortcutAction::ALL {
            let accel = action.accel(&config);
            assert!(normalize_accelerator(accel).is_ok(), "{accel}");
        }
    }

    #[test]
    fn canonicalize_reports_invalid_action_without_touching_others() {
        let (config, errors) = canonicalize_config(GlobalShortcutConfig {
            approve: "Bogus Key".to_string(),
            ..GlobalShortcutConfig::default()
        });
        assert!(errors.approve.is_some());
        assert!(errors.summon.is_none());
        assert!(errors.deny.is_none());
        assert_eq!(config.summon, GlobalShortcutConfig::default().summon);
        assert_eq!(config.approve, "Bogus Key");
        assert!(errors.has_errors());
    }

    #[test]
    fn cleared_actions_stay_empty_through_canonicalize() {
        let (config, errors) = canonicalize_config(GlobalShortcutConfig {
            deny: String::new(),
            ..GlobalShortcutConfig::default()
        });
        assert!(errors.deny.is_none());
        assert_eq!(config.deny, "");
    }

    #[test]
    fn config_round_trips_through_settings_file() {
        let path = std::env::temp_dir().join(format!(
            "atoll-shortcuts-roundtrip-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let config = GlobalShortcutConfig {
            enabled: false,
            summon: "Ctrl+Alt+K".to_string(),
            approve: String::new(),
            deny: "Cmd+Shift+N".to_string(),
        };
        persist_config_to_path(&path, &config);
        let loaded = load_config_from_path(&path);
        assert_eq!(loaded, config);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_falls_back_to_defaults_on_missing_or_corrupt_file() {
        let missing = std::env::temp_dir().join(format!(
            "atoll-shortcuts-missing-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&missing);
        assert_eq!(
            load_config_from_path(&missing),
            GlobalShortcutConfig::default()
        );

        let corrupt = std::env::temp_dir().join(format!(
            "atoll-shortcuts-corrupt-{}.json",
            std::process::id()
        ));
        std::fs::write(&corrupt, "not json").unwrap();
        assert_eq!(
            load_config_from_path(&corrupt),
            GlobalShortcutConfig::default()
        );
        let _ = std::fs::remove_file(&corrupt);
    }

    #[test]
    fn load_keeps_explicitly_cleared_action_and_replaces_invalid_one() {
        let path = std::env::temp_dir().join(format!(
            "atoll-shortcuts-sanitize-{}.json",
            std::process::id()
        ));
        let defaults = GlobalShortcutConfig::default();
        let document = serde_json::json!({
            "globalShortcuts": {
                "enabled": true,
                "summon": "",
                "approve": "Bogus Key",
                "deny": "shift+cmd+n"
            }
        });
        std::fs::write(&path, serde_json::to_string(&document).unwrap()).unwrap();
        let loaded = load_config_from_path(&path);
        assert!(loaded.enabled);
        assert_eq!(loaded.summon, "");
        assert_eq!(loaded.approve, defaults.approve);
        assert_eq!(loaded.deny, "Cmd+Shift+N");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_uses_defaults_when_key_missing() {
        let path =
            std::env::temp_dir().join(format!("atoll-shortcuts-nokey-{}.json", std::process::id()));
        std::fs::write(&path, serde_json::json!({ "other": 1 }).to_string()).unwrap();
        assert_eq!(
            load_config_from_path(&path),
            GlobalShortcutConfig::default()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[cfg(desktop)]
    fn canonical_forms_parse_with_plugin_parser() {
        use tauri_plugin_global_shortcut::Shortcut;
        for accelerator in [
            "Cmd+Shift+Space",
            "Ctrl+Shift+Y",
            "Cmd+Alt+7",
            "F5",
            "Cmd+-",
            "Cmd+=",
            "Cmd+[",
            "Cmd+]",
            "Cmd+,",
            "Cmd+.",
            "Cmd+/",
            "Cmd+;",
            "Cmd+'",
            "Cmd+`",
            "Cmd+\\",
            "Cmd+Up",
            "Cmd+Down",
            "Cmd+Left",
            "Cmd+Right",
            "Cmd+Escape",
            "Ctrl+Enter",
            "Ctrl+Tab",
        ] {
            let normalized = normalize_accelerator(accelerator)
                .unwrap_or_else(|error| panic!("{accelerator}: {error}"));
            normalized
                .parse::<Shortcut>()
                .unwrap_or_else(|error| panic!("{normalized}: {error}"));
        }
    }
}
