use super::{
    cursor_hook_command_needs_repair, cursor_hooks_need_command_repair,
    cursor_hooks_need_lifecycle_upgrade, cursor_hooks_need_timeout_repair,
    format_cursor_hook_command, has_atoll_cursor_hooks, hook_entry_has_atoll_cursor,
    remove_atoll_cursor_hooks, repair_cursor_hook_events_with_command, upsert_cursor_hook_events,
    CURSOR_HOOK_EVENTS, CURSOR_HOOK_TIMEOUT_SECONDS,
};
use serde_json::json;

#[test]
fn format_cursor_hook_command_uses_cmd_c_on_windows() {
    let command = format_cursor_hook_command(
        Some(r"C:\Atoll\scripts\atoll-hook-runner.exe"),
        r"C:\Program Files\nodejs\node.exe",
        r"C:\Atoll\scripts\atoll-cursor-hook.mjs",
    );
    #[cfg(windows)]
    assert!(
        command.starts_with("cmd /c "),
        "expected cmd /c prefix, got: {command}"
    );
    #[cfg(not(windows))]
    assert!(!command.starts_with("cmd /c "));
}

#[test]
fn upsert_and_detect_cursor_hooks() {
    let mut hooks = json!({});
    upsert_cursor_hook_events(
        &mut hooks,
        &format_cursor_hook_command(
            Some("/tmp/atoll-hook-runner.exe"),
            "/opt/homebrew/bin/node",
            "/tmp/atoll-cursor-hook.mjs",
        ),
        "http://127.0.0.1:47777/cursor/hook",
    );

    let config = json!({ "version": 1, "hooks": hooks });
    assert!(has_atoll_cursor_hooks(&config));
    assert!(!cursor_hooks_need_lifecycle_upgrade(&config));
    assert!(hook_entry_has_atoll_cursor(
        &config["hooks"]["sessionStart"].as_array().unwrap()[0]
    ));
    assert!(hook_entry_has_atoll_cursor(
        &config["hooks"]["afterAgentResponse"].as_array().unwrap()[0]
    ));
    assert!(hook_entry_has_atoll_cursor(
        &config["hooks"]["beforeSubmitPrompt"].as_array().unwrap()[0]
    ));
    assert!(hook_entry_has_atoll_cursor(
        &config["hooks"]["afterAgentThought"].as_array().unwrap()[0]
    ));
    assert!(hook_entry_has_atoll_cursor(
        &config["hooks"]["preToolUse"].as_array().unwrap()[0]
    ));
    assert_eq!(
        config["hooks"]["preToolUse"].as_array().unwrap()[0]["env"]["ATOLL_HOOK_URL"],
        "http://127.0.0.1:47777/cursor/hook"
    );
    for (event, _) in CURSOR_HOOK_EVENTS {
        assert_eq!(
            config["hooks"][event].as_array().unwrap()[0]["timeout"],
            json!(CURSOR_HOOK_TIMEOUT_SECONDS),
            "event {event}"
        );
    }
}

#[test]
fn remove_cursor_hooks_preserves_other_entries() {
    let mut hooks = json!({
        "preToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 },
            { "command": "./custom-hook.sh", "timeout": 10 }
        ],
        "postToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "stop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "subagentStop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ]
    });

    remove_atoll_cursor_hooks(&mut hooks);

    let pre_tool_use = hooks["preToolUse"].as_array().unwrap();
    assert_eq!(pre_tool_use.len(), 1);
    assert_eq!(pre_tool_use[0]["command"], "./custom-hook.sh");
    assert!(!has_atoll_cursor_hooks(&json!({ "hooks": hooks })));
}

/// v0.1.31 installs only the five core events. After upgrading to v0.1.32,
/// those installs must still count as "installed" so the online indicator
/// and Cursor session display keep working until the user reinstalls.
#[test]
fn v0_1_31_core_only_cursor_hooks_count_as_installed() {
    let hooks = json!({
        "preToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 }
        ],
        "postToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "stop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "subagentStart": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "subagentStop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ]
    });

    let config = json!({ "version": 1, "hooks": hooks });
    assert!(has_atoll_cursor_hooks(&config));
    assert!(cursor_hooks_need_lifecycle_upgrade(&config));
}

#[test]
fn cursor_hook_repair_replaces_legacy_atoll_commands_and_preserves_custom_entries() {
    let preferred =
            "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-cursor-hook.ps1\"";
    let config = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                {
                    "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                    "timeout": 1800
                },
                {
                    "command": "./user-cursor-hook.sh",
                    "timeout": 5
                }
            ],
            "postToolUse": [
                {
                    "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                    "timeout": 30
                }
            ],
            "stop": [
                {
                    "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                    "timeout": 30
                }
            ],
            "subagentStart": [
                {
                    "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                    "timeout": 30
                }
            ],
            "subagentStop": [
                {
                    "command": "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"",
                    "timeout": 30
                }
            ]
        }
    });

    let repaired = repair_cursor_hook_events_with_command(
        &config,
        preferred,
        "http://127.0.0.1:47777/cursor/hook",
    )
    .expect("repaired hooks");

    for (event, timeout) in CURSOR_HOOK_EVENTS {
        let entries = repaired["hooks"][event].as_array().expect("event entries");
        let atoll_entries: Vec<_> = entries
            .iter()
            .filter(|entry| hook_entry_has_atoll_cursor(entry))
            .collect();
        assert_eq!(atoll_entries.len(), 1, "event {event}");
        assert_eq!(atoll_entries[0]["command"], preferred);
        assert_eq!(atoll_entries[0]["timeout"], json!(timeout));
        assert_eq!(
            atoll_entries[0]["env"]["ATOLL_HOOK_URL"],
            "http://127.0.0.1:47777/cursor/hook"
        );
    }

    let pre_tool_entries = repaired["hooks"]["preToolUse"].as_array().unwrap();
    assert!(pre_tool_entries
        .iter()
        .any(|entry| entry["command"] == "./user-cursor-hook.sh"));
}

#[test]
fn cursor_hook_command_repair_detects_windows_legacy_command() {
    let legacy =
            "cmd /c \"C:/old/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/old/atoll-cursor-hook.mjs\"";
    assert!(cursor_hook_command_needs_repair(
        legacy,
        Some("C:/Users/test/AppData/Local/Atoll/hooks/atoll-cursor-hook.mjs"),
        true,
    ));

    let config = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [{ "command": legacy, "timeout": 1800 }],
            "postToolUse": [{ "command": legacy, "timeout": 30 }],
            "stop": [{ "command": legacy, "timeout": 30 }],
            "subagentStart": [{ "command": legacy, "timeout": 30 }],
            "subagentStop": [{ "command": legacy, "timeout": 30 }]
        }
    });
    assert!(cursor_hooks_need_command_repair(
        &config,
        Some("C:/Users/test/AppData/Local/Atoll/hooks/atoll-cursor-hook.mjs"),
        true,
    ));
}

#[test]
fn cursor_hook_timeout_repair_detects_legacy_timeouts() {
    let config = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 },
                { "command": "./user-cursor-hook.sh", "timeout": 1800 }
            ],
            "postToolUse": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "stop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStart": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ],
            "subagentStop": [
                { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
            ]
        }
    });

    assert!(cursor_hooks_need_timeout_repair(&config));

    let repaired = repair_cursor_hook_events_with_command(
        &config,
        "node \"/tmp/atoll-cursor-hook.mjs\"",
        "http://127.0.0.1:47777/cursor/hook",
    )
    .expect("repaired hooks");

    for (event, _) in CURSOR_HOOK_EVENTS {
        let entries = repaired["hooks"][event].as_array().expect("event entries");
        let atoll_entries: Vec<_> = entries
            .iter()
            .filter(|entry| hook_entry_has_atoll_cursor(entry))
            .collect();
        assert_eq!(atoll_entries.len(), 1, "event {event}");
        assert_eq!(
            atoll_entries[0]["timeout"],
            json!(CURSOR_HOOK_TIMEOUT_SECONDS),
            "event {event}"
        );
    }

    let pre_tool_entries = repaired["hooks"]["preToolUse"].as_array().unwrap();
    assert!(pre_tool_entries
        .iter()
        .any(|entry| entry["command"] == "./user-cursor-hook.sh"));
}

/// Missing any one of the five core events means hooks are incomplete.
#[test]
fn missing_core_cursor_hook_event_is_not_installed() {
    let hooks = json!({
        "preToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 1800 }
        ],
        "postToolUse": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "stop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ],
        "subagentStop": [
            { "command": "node \"/tmp/atoll-cursor-hook.mjs\"", "timeout": 30 }
        ]
    });

    assert!(!has_atoll_cursor_hooks(
        &json!({ "version": 1, "hooks": hooks })
    ));
}
