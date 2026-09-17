use super::{
    configured_atoll_hook_command, has_atoll_zcode_hooks, remove_atoll_zcode_hooks,
    upsert_zcode_hook_events,
};
use serde_json::{json, Value};

fn sample_atoll_zcode_hooks() -> Value {
    json!({
        "PermissionRequest": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 1800 }] }
        ],
        "PostToolUse": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 30 }] }
        ],
        "Stop": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs", "timeout": 30 }] }
        ]
    })
}

#[test]
fn has_atoll_zcode_hooks_requires_enabled_flag() {
    let hooks = sample_atoll_zcode_hooks();
    let enabled_config = json!({ "hooks": { "enabled": true, "events": hooks } });
    assert!(has_atoll_zcode_hooks(&enabled_config));

    let disabled_config = json!({ "hooks": { "enabled": false, "events": hooks } });
    assert!(!has_atoll_zcode_hooks(&disabled_config));

    let missing_flag_config = json!({ "hooks": { "events": hooks } });
    assert!(!has_atoll_zcode_hooks(&missing_flag_config));
}

#[test]
fn upsert_zcode_hook_events_is_idempotent_and_keeps_foreign_hooks() {
    let mut events = json!({
        "PermissionRequest": [
            { "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }] }
        ]
    });
    let atoll = sample_atoll_zcode_hooks();

    upsert_zcode_hook_events(&mut events, &atoll);
    upsert_zcode_hook_events(&mut events, &atoll);

    let permission_matchers = events
        .get("PermissionRequest")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(permission_matchers.len(), 2);

    let config = json!({
        "hooks": { "enabled": true, "events": events }
    });
    assert!(has_atoll_zcode_hooks(&config));
}

#[test]
fn remove_atoll_zcode_hooks_keeps_foreign_hooks() {
    let mut events = json!({
        "PermissionRequest": [
            { "hooks": [{ "type": "command", "command": "node /other/island-hook.mjs" }] },
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
        ],
        "Stop": [
            { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
        ]
    });

    remove_atoll_zcode_hooks(&mut events);

    let events_obj = events.as_object().unwrap();
    assert_eq!(events_obj.len(), 1);
    let remaining = events_obj
        .get("PermissionRequest")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .contains("island-hook.mjs"));
}

#[test]
fn configured_atoll_hook_command_reads_zcode_events_nesting() {
    let config = json!({
        "hooks": {
            "enabled": true,
            "timeoutMs": 60000,
            "events": {
                "PermissionRequest": [
                    { "hooks": [{ "type": "command", "command": "node /opt/Atoll/hooks/atoll-zcode-hook.mjs" }] }
                ]
            }
        }
    });

    let command = configured_atoll_hook_command(&config, "atoll-zcode-hook");
    assert_eq!(
        command.as_deref(),
        Some("node /opt/Atoll/hooks/atoll-zcode-hook.mjs")
    );
}
