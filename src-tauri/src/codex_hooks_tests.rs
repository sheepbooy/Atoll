use super::{
    extract_node_script_path, format_hook_command, has_atoll_codex_hooks,
    normalize_hook_script_path, remove_atoll_codex_hooks, resolve_node_executable,
    upsert_codex_hook_events,
};
use serde_json::json;

fn sample_atoll_codex_hooks() -> serde_json::Value {
    json!({
        "PermissionRequest": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                "timeout": 1800,
                "statusMessage": "Atoll approval"
            }]
        }],
        "PostToolUse": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                "timeout": 30
            }]
        }],
        "Stop": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                "timeout": 30
            }]
        }],
        "SubagentStop": [{
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": "node \"/tmp/atoll-codex-hook.mjs\"",
                "timeout": 30
            }]
        }]
    })
}

#[test]
fn has_atoll_codex_hooks_recognizes_powershell_launcher_command() {
    let config = json!({
        "hooks": {
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                }]
            }],
            "PostToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                }]
            }],
            "Stop": [{
                "matcher": "*",
                "hooks": [{
                    "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                }]
            }],
            "SubagentStop": [{
                "matcher": "*",
                "hooks": [{
                    "command": "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"C:/Users/test/AppData/Local/Atoll/atoll-codex-hook.ps1\""
                }]
            }]
        }
    });

    assert!(has_atoll_codex_hooks(&config));
}

#[test]
fn upsert_installs_into_empty_codex_hook_arrays() {
    let mut hooks = json!({
        "PermissionRequest": [],
        "PostToolUse": [],
        "Stop": [],
        "SubagentStop": []
    });
    let atoll = sample_atoll_codex_hooks();

    upsert_codex_hook_events(&mut hooks, &atoll);

    let config = json!({ "hooks": hooks });
    assert!(has_atoll_codex_hooks(&config));
}

#[test]
fn uninstall_removes_atoll_codex_hooks_and_empty_events() {
    let mut hooks = sample_atoll_codex_hooks();
    remove_atoll_codex_hooks(&mut hooks);

    assert!(hooks.as_object().unwrap().is_empty());
}

#[test]
fn format_hook_command_quotes_paths_with_spaces_codex() {
    let command = format_hook_command(
        None,
        "/opt/homebrew/bin/node",
        "/Applications/Atoll.app/scripts/atoll-codex-hook.mjs",
    );
    assert_eq!(
        command,
        "\"/opt/homebrew/bin/node\" \"/Applications/Atoll.app/scripts/atoll-codex-hook.mjs\""
    );

    let windows_command = format_hook_command(
        None,
        r"C:\Program Files\nodejs\node.exe",
        r"C:\Program Files\Atoll\resources\scripts\atoll-claude-hook.mjs",
    );
    #[cfg(windows)]
    assert_eq!(
            windows_command,
            "\"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/resources/scripts/atoll-claude-hook.mjs\""
        );
    #[cfg(not(windows))]
    assert_eq!(
            windows_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\resources\\scripts\\atoll-claude-hook.mjs\""
        );

    let runner_command = format_hook_command(
        Some(r"C:\Program Files\Atoll\resources\scripts\atoll-hook-runner.exe"),
        r"C:\Program Files\nodejs\node.exe",
        r"C:\Program Files\Atoll\resources\scripts\atoll-claude-hook.mjs",
    );
    #[cfg(windows)]
    assert_eq!(
            runner_command,
            "\"C:/Program Files/Atoll/resources/scripts/atoll-hook-runner.exe\" \"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/resources/scripts/atoll-claude-hook.mjs\""
        );
    #[cfg(not(windows))]
    assert_eq!(
            runner_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\resources\\scripts\\atoll-claude-hook.mjs\""
        );

    let unc_command = format_hook_command(
        None,
        r"C:\Program Files\nodejs\node.exe",
        r"\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs",
    );
    #[cfg(windows)]
    assert_eq!(
            unc_command,
            "\"C:/Program Files/nodejs/node.exe\" \"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs\""
        );
    #[cfg(not(windows))]
    assert_eq!(
            unc_command,
            "\"C:\\Program Files\\nodejs\\node.exe\" \"C:\\Program Files\\Atoll\\scripts\\atoll-claude-hook.mjs\""
        );
}

#[test]
fn extract_node_script_path_strips_windows_unc_prefix() {
    assert_eq!(
        extract_node_script_path(
            r#""C:\Program Files\nodejs\node.exe" "\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs""#
        ),
        Some(r"C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs".into())
    );
    assert_eq!(
        extract_node_script_path(
            r#"node "\\?\C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs""#
        ),
        Some(r"C:\Program Files\Atoll\scripts\atoll-claude-hook.mjs".into())
    );
    assert_eq!(
        extract_node_script_path(
            r#""C:/Program Files/nodejs/node.exe" "C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs""#
        ),
        Some(r"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs".into())
    );
    assert_eq!(
        extract_node_script_path(
            r#""C:/Program Files/Atoll/resources/scripts/atoll-hook-runner.exe" "C:/Program Files/nodejs/node.exe" "C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs""#
        ),
        Some(r"C:/Program Files/Atoll/scripts/atoll-claude-hook.mjs".into())
    );
}

#[test]
fn resolve_node_executable_finds_standard_windows_install() {
    #[cfg(windows)]
    {
        let standard = r"C:\Program Files\nodejs\node.exe";
        if std::path::Path::new(standard).exists() {
            let resolved = resolve_node_executable().expect("node should resolve");
            assert_eq!(resolved, normalize_hook_script_path(standard));
        }
    }
}

#[test]
fn resolve_node_executable_from_where_returns_existing_path() {
    #[cfg(windows)]
    {
        use super::resolve_node_executable_from_where;

        if std::path::Path::new(r"C:\Program Files\nodejs\node.exe").exists() {
            let resolved = resolve_node_executable_from_where().expect("where should find node");
            assert!(std::path::Path::new(&resolved).exists());
        }
    }
}
