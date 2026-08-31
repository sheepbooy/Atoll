//! Per-agent repair kits: Claude competitor-hook detection and cleanup, and
//! the Cursor lifecycle/timeout/launcher-command repair pass.

use serde_json::{json, Value};
use tauri::AppHandle;

use super::*;

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
