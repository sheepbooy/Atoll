use super::{has_atoll_gemini_hooks, remove_atoll_gemini_hooks, upsert_gemini_hook_entries};
use serde_json::{json, Value};

fn sample_atoll_gemini_hooks() -> Value {
    json!({
        "BeforeTool": [
            {
                "matcher": "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_",
                "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 1800000 }]
            }
        ],
        "SessionStart": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 30000 }] }
        ],
        "AfterTool": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs", "timeout": 30000 }] }
        ]
    })
}

#[test]
fn has_atoll_gemini_hooks_reads_flat_hooks_object() {
    let config = json!({ "hooks": sample_atoll_gemini_hooks() });
    assert!(has_atoll_gemini_hooks(&config));

    // Gemini keeps optional config keys (enabled/disabled/notifications)
    // alongside the event entries; they must not break detection.
    let with_extra_keys = json!({
        "hooks": {
            "notifications": true,
            "SessionStart": [{ "hooks": [{ "type": "command", "command": "other" }] }]
        }
    });
    assert!(!has_atoll_gemini_hooks(&with_extra_keys));
}

#[test]
fn has_atoll_gemini_hooks_requires_all_core_events() {
    let mut hooks = sample_atoll_gemini_hooks();
    hooks
        .as_object_mut()
        .unwrap()
        .remove("AfterTool")
        .expect("AfterTool entry");
    let config = json!({ "hooks": hooks });
    assert!(!has_atoll_gemini_hooks(&config));
}

#[test]
fn upsert_gemini_hook_entries_is_idempotent_and_keeps_foreign_hooks() {
    let mut hooks = json!({
        "BeforeTool": [
            {
                "matcher": "run_shell_command",
                "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }]
            }
        ],
        "notifications": true
    });
    let atoll = sample_atoll_gemini_hooks();

    upsert_gemini_hook_entries(&mut hooks, &atoll);
    upsert_gemini_hook_entries(&mut hooks, &atoll);

    let hooks_obj = hooks.as_object().unwrap();
    // notifications is not an event array and must be preserved untouched.
    assert_eq!(hooks_obj.get("notifications"), Some(&json!(true)));

    let before_tool = hooks_obj
        .get("BeforeTool")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(before_tool.len(), 2);
    assert!(before_tool[0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .contains("island-hook.mjs"));
    assert!(before_tool[1]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .contains("atoll-gemini-hook"));

    assert!(has_atoll_gemini_hooks(&json!({ "hooks": hooks })));
}

#[test]
fn remove_atoll_gemini_hooks_keeps_foreign_hooks_and_config_keys() {
    let mut hooks = json!({
        "notifications": true,
        "BeforeTool": [
            {
                "matcher": "run_shell_command",
                "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }]
            },
            {
                "matcher": "run_shell_command",
                "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs" }]
            }
        ],
        "SessionStart": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-gemini-hook.mjs" }] }
        ]
    });

    remove_atoll_gemini_hooks(&mut hooks);

    let hooks_obj = hooks.as_object().unwrap();
    assert_eq!(hooks_obj.get("notifications"), Some(&json!(true)));
    assert_eq!(hooks_obj.len(), 2);
    let before_tool = hooks_obj
        .get("BeforeTool")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(before_tool.len(), 1);
    assert!(before_tool[0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .contains("island-hook.mjs"));
}
