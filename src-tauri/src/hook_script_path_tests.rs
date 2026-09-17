use super::{
    configured_atoll_hook_node_path, configured_atoll_hook_script_path, extract_node_script_path,
    should_flag_dev_hook_drift,
};
use serde_json::json;
use std::fs;

#[test]
fn extract_node_script_path_handles_quoted_and_unquoted_commands() {
    assert_eq!(
        extract_node_script_path(
            "node \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs\""
        ),
        Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs".into())
    );
    assert_eq!(
        extract_node_script_path(
            "node /Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs"
        ),
        Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs".into())
    );
    assert_eq!(
            extract_node_script_path(
                "\"/opt/homebrew/bin/node\" \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs\""
            ),
            Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs".into())
        );
}

#[test]
fn extract_node_script_path_handles_cmd_c_runner_commands() {
    assert_eq!(
            extract_node_script_path(
                "cmd /c \"C:/Atoll/scripts/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/Atoll/scripts/atoll-cursor-hook.mjs\""
            ),
            Some("C:/Atoll/scripts/atoll-cursor-hook.mjs".into())
        );
}

#[test]
fn configured_atoll_hook_script_path_reads_hooks_json() {
    let config = json!({
        "hooks": {
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "command": "node \"/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs\""
                }]
            }]
        }
    });

    assert_eq!(
        configured_atoll_hook_script_path(&config, "atoll-codex-hook"),
        Some("/Applications/Atoll.app/Contents/Resources/scripts/atoll-codex-hook.mjs".into())
    );
}

#[test]
fn configured_atoll_hook_script_path_reads_cursor_flat_hooks_json() {
    let config = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [{
                "command": "\"C:/runner/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/tmp/atoll-cursor-hook.mjs\"",
                "timeout": 1800
            }]
        }
    });

    assert_eq!(
        configured_atoll_hook_script_path(&config, "atoll-cursor-hook"),
        Some("C:/tmp/atoll-cursor-hook.mjs".into())
    );
}

#[test]
fn configured_atoll_hook_node_path_reads_cursor_flat_hooks_json() {
    let config = json!({
        "hooks": {
            "sessionStart": [{
                "command": "\"C:/runner/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/tmp/atoll-cursor-hook.mjs\""
            }]
        }
    });

    assert_eq!(
        configured_atoll_hook_node_path(&config, "atoll-cursor-hook"),
        Some("C:/Program Files/nodejs/node.exe".into())
    );
}

#[test]
fn should_flag_dev_hook_drift_when_configured_dev_path_missing() {
    let preferred = std::env::current_exe()
        .expect("current exe")
        .to_string_lossy()
        .into_owned();
    assert!(should_flag_dev_hook_drift(
        "C:/Users/test/Atoll/target/debug/scripts/atoll-codex-hook.mjs",
        &preferred,
    ));
}

#[test]
fn should_not_flag_dev_hook_drift_when_configured_dev_path_exists() {
    let temp_root = std::env::temp_dir().join("atoll-drift-test-target-debug");
    let script_dir = temp_root.join("target").join("debug");
    fs::create_dir_all(&script_dir).expect("create temp script dir");
    let script_path = script_dir.join("atoll-codex-hook.mjs");
    fs::write(&script_path, "export {}").expect("write temp script");
    let configured = script_path.to_string_lossy().into_owned();
    let preferred = std::env::current_exe()
        .expect("current exe")
        .to_string_lossy()
        .into_owned();
    assert!(!should_flag_dev_hook_drift(&configured, &preferred));
    let _ = fs::remove_dir_all(temp_root);
}
