use super::{
    detect_competing_claude_hooks, format_hook_command, has_atoll_claude_hooks,
    hook_command_binary_exists, remove_atoll_claude_hooks, remove_dead_competing_hooks_from_config,
    upsert_claude_hook_events,
};
use serde_json::json;

fn sample_atoll_claude_hooks() -> serde_json::Value {
    json!({
        "PermissionRequest": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                "timeout": 1800
            }]
        }],
        "PostToolUse": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                "timeout": 30
            }]
        }],
        "Stop": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs"),
                "timeout": 30
            }]
        }]
    })
}

#[test]
fn upsert_preserves_user_notification_hooks() {
    let mut hooks = json!({
        "Notification": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "osascript -e 'display notification \"hi\"'"
            }]
        }],
        "PermissionRequest": [],
        "PostToolUse": [],
        "Stop": []
    });
    let atoll = sample_atoll_claude_hooks();

    upsert_claude_hook_events(&mut hooks, &atoll);

    let config = json!({ "hooks": hooks });
    assert!(has_atoll_claude_hooks(&config));
    let notification = hooks
        .get("Notification")
        .and_then(|value| value.as_array())
        .and_then(|arr| arr.first())
        .and_then(|matcher| matcher.get("hooks"))
        .and_then(|value| value.as_array())
        .and_then(|arr| arr.first())
        .and_then(|hook| hook.get("command"))
        .and_then(|value| value.as_str());
    assert!(notification.unwrap_or("").contains("display notification"));
}

#[test]
fn uninstall_removes_only_atoll_claude_hooks() {
    let mut hooks = json!({
        "Notification": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "osascript -e 'display notification \"hi\"'"
            }]
        }],
        "PermissionRequest": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs")
            }]
        }],
        "PostToolUse": [],
        "Stop": []
    });

    remove_atoll_claude_hooks(&mut hooks);

    assert!(hooks.get("Notification").is_some());
    let permission = hooks
        .get("PermissionRequest")
        .and_then(|value| value.as_array());
    assert!(permission.map(|arr| arr.is_empty()).unwrap_or(true));
}

#[test]
fn detect_finds_non_atoll_hooks_and_flags_missing_binaries() {
    // A config with: Atoll (live), a dead competitor (binary missing), and a
    // real shell command (binary present on Unix test runners).
    let config = json!({
        "hooks": {
            "PermissionRequest": [
                { "matcher": "*", "hooks": [
                    { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" },
                    { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                ]}
            ],
            "Notification": [
                { "matcher": "*", "hooks": [
                    { "type": "command", "command": "/bin/echo hello" }
                ]}
            ]
        }
    });

    let competing = detect_competing_claude_hooks(&config);
    assert_eq!(competing.len(), 2);
    let ping = competing
        .iter()
        .find(|c| c.command.contains("ping-island-bridge"))
        .unwrap();
    assert!(
        !ping.binary_exists,
        "ping-island binary should be flagged missing"
    );
    assert_eq!(ping.event, "PermissionRequest");
    let echo_hook = competing
        .iter()
        .find(|c| c.command.contains("/bin/echo"))
        .unwrap();
    assert!(
        echo_hook.binary_exists,
        "/bin/echo binary should be present"
    );
    assert_eq!(echo_hook.event, "Notification");
}

#[test]
fn detect_ignores_atoll_hooks_and_empty_config() {
    let config = json!({
        "hooks": {
            "PermissionRequest": [
                { "matcher": "*", "hooks": [
                    { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                ]}
            ]
        }
    });
    assert!(detect_competing_claude_hooks(&config).is_empty());

    assert!(detect_competing_claude_hooks(&json!({})).is_empty());
    assert!(detect_competing_claude_hooks(&json!({ "hooks": {} })).is_empty());
}

#[test]
fn remove_strips_only_dead_competitors_preserves_atoll_and_live() {
    // The echo binary exists on the test runner, so it survives cleanup.
    // The nonexistent competitor binary does not, so it is removed. Atoll's hook is kept.
    let mut settings = json!({
        "hooks": {
            "PermissionRequest": [
                { "matcher": "*", "hooks": [
                    { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" },
                    { "type": "command", "command": "/bin/echo hello" },
                    { "type": "command", "command": format_hook_command(None, "/opt/homebrew/bin/node", "/tmp/atoll-claude-hook.mjs") }
                ]}
            ]
        }
    });

    let removed = remove_dead_competing_hooks_from_config(&mut settings);
    assert!(removed);

    let hooks = settings.get("hooks").unwrap();
    let pr = hooks
        .get("PermissionRequest")
        .and_then(|v| v.as_array())
        .unwrap();
    let commands: Vec<&str> = pr[0]
        .get("hooks")
        .and_then(|v| v.as_array())
        .unwrap()
        .iter()
        .map(|h| h.get("command").and_then(|c| c.as_str()).unwrap_or(""))
        .collect();
    assert!(
        commands.iter().any(|c| c.contains("atoll-claude-hook")),
        "Atoll hook must survive"
    );
    assert!(
        commands.iter().any(|c| c.contains("/bin/echo")),
        "live competitor must survive"
    );
    assert!(
        !commands.iter().any(|c| c.contains("ping-island-bridge")),
        "dead competitor must be removed"
    );
}

#[test]
fn remove_drops_empty_event_after_last_dead_hook_removed() {
    // An event with ONLY a dead competitor should be removed entirely.
    let mut settings = json!({
        "hooks": {
            "PermissionDenied": [
                { "matcher": "*", "hooks": [
                    { "type": "command", "command": "/nonexistent/ping-island-bridge --source claude" }
                ]}
            ]
        }
    });

    let removed = remove_dead_competing_hooks_from_config(&mut settings);
    assert!(removed);
    assert!(
        settings
            .get("hooks")
            .map(|h| h.as_object().map(|o| o.is_empty()).unwrap_or(true))
            .unwrap_or(true),
        "hooks object should be empty or absent after removing the only dead hook"
    );
}

#[test]
fn hook_command_binary_exists_handles_quotes_and_empty() {
    assert!(!hook_command_binary_exists(""));
    assert!(!hook_command_binary_exists("   "));
    // /bin/sh exists on all Unix test runners.
    assert!(hook_command_binary_exists("/bin/sh --flag"));
    assert!(hook_command_binary_exists("'/bin/sh' --flag"));
    assert!(hook_command_binary_exists("\"/bin/sh\" --flag"));
    assert!(!hook_command_binary_exists("/nonexistent/binary --flag"));
}
