//! OpenCode integration: OpenCode has no command-hook pipeline to write into.
//! Instead Atoll deploys an in-process plugin (`atoll-opencode-bridge.js`)
//! into OpenCode's global plugin directory, which OpenCode auto-loads at
//! startup. The plugin forwards native `permission.updated` bus events to the
//! Atoll bridge and relays the decision back through OpenCode's permission
//! API, so Atoll being down leaves OpenCode's own TUI prompt fully in charge.
//!
//! The deployed file must end in `.js` (never `.mjs`): OpenCode's plugin
//! discovery glob only matches `{plugin,plugins}/*.{ts,js}`.

use tauri::AppHandle;

use super::*;

pub(crate) const OPENCODE_PLUGIN_SCRIPT: &str = "atoll-opencode-bridge.js";
pub(crate) const OPENCODE_PLUGIN_MARKER: &str = "atoll-opencode-bridge";

/// OpenCode loads plugins from `~/.config/opencode/plugins` on every platform
/// (its docs and default config live under `~/.config/opencode`).
pub(crate) fn opencode_plugins_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".config").join("opencode").join("plugins"))
}

pub(crate) fn opencode_plugin_deployed_path() -> Option<std::path::PathBuf> {
    opencode_plugins_dir().map(|dir| dir.join(OPENCODE_PLUGIN_SCRIPT))
}

/// A plugin file counts as Atoll's when it exists and mentions the marker, so
/// a user's own plugin of the same name doesn't read as installed.
fn is_atoll_opencode_plugin(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.contains(OPENCODE_PLUGIN_MARKER))
        .unwrap_or(false)
}

pub(crate) fn opencode_hook_status(app: &AppHandle) -> HookStatus {
    if capture::force_hook_uninstalled() {
        let script_path = resolve_hook_script_path(app, OPENCODE_PLUGIN_SCRIPT).unwrap_or_default();
        return HookStatus {
            installed: false,
            script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
            settings_path: opencode_plugins_dir()
                .map(|dir| dir.to_string_lossy().into_owned())
                .unwrap_or_default(),
            script_path,
            node_path: String::new(),
            node_found: true,
            needs_retrust: false,
            competing_hooks: Vec::new(),
        };
    }

    let deployed = opencode_plugin_deployed_path().unwrap_or_default();
    let installed = !deployed.as_os_str().is_empty() && is_atoll_opencode_plugin(&deployed);
    HookStatus {
        installed,
        script_found: installed,
        settings_path: opencode_plugins_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default(),
        script_path: deployed.to_string_lossy().into_owned(),
        node_path: String::new(),
        // The plugin runs inside OpenCode's own Bun runtime; node is unused.
        node_found: true,
        needs_retrust: false,
        competing_hooks: Vec::new(),
    }
}

#[tauri::command]
pub(crate) fn get_opencode_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    Ok(opencode_hook_status(&app))
}

#[tauri::command]
pub(crate) fn install_opencode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, OPENCODE_PLUGIN_SCRIPT)?;
    let plugins_dir =
        opencode_plugins_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    std::fs::create_dir_all(&plugins_dir).map_err(|error| {
        format!(
            "Cannot create OpenCode plugins directory ({}): {error}",
            plugins_dir.display()
        )
    })?;

    let dest = plugins_dir.join(OPENCODE_PLUGIN_SCRIPT);
    copy_deployed_hook_file(
        std::path::Path::new(&source_script_path),
        &dest,
        "OpenCode bridge plugin",
    )?;
    if !is_atoll_opencode_plugin(&dest) {
        return Err(format!(
            "OpenCode plugin was not saved correctly. Check permissions on {}.",
            dest.display()
        ));
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after OpenCode plugin install: {error}");
    }
    hook_trust::record_hook_installed("opencode", &dest.to_string_lossy());

    emit_hook_snapshot_changed(&app)?;

    Ok(opencode_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_opencode_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let deployed = opencode_plugin_deployed_path()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;
    if deployed.is_file() {
        std::fs::remove_file(&deployed)
            .map_err(|error| format!("Cannot remove OpenCode bridge plugin: {error}"))?;
    }
    hook_trust::clear_hook_installed("opencode");

    emit_hook_snapshot_changed(&app)?;

    Ok(opencode_hook_status(&app))
}

#[cfg(test)]
mod opencode_plugin_tests {
    use super::*;
    use crate::hook_bridge::{
        build_hook_defer_response, opencode_permission_response, PermissionResponseStyle,
    };
    use serde_json::Value;

    #[test]
    fn marker_check_rejects_missing_and_foreign_files() {
        let temp = std::env::temp_dir().join(format!("atoll-oc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let foreign = temp.join("atoll-opencode-bridge.mjs");
        std::fs::write(&foreign, "export default async () => ({});").expect("write");

        // Same file name, no Atoll marker inside: not ours.
        assert!(!is_atoll_opencode_plugin(&foreign));

        std::fs::write(
            &foreign,
            format!("// {OPENCODE_PLUGIN_MARKER}\nexport default 1;"),
        )
        .expect("write");
        assert!(is_atoll_opencode_plugin(&foreign));

        let missing = temp.join("nope.mjs");
        assert!(!is_atoll_opencode_plugin(&missing));

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn deployed_path_lives_under_config_opencode_plugins() {
        let path = opencode_plugin_deployed_path().expect("home dir");
        let rendered = path.to_string_lossy().into_owned();
        assert!(rendered.contains(".config"));
        assert!(rendered.contains("opencode"));
        assert!(rendered.ends_with(OPENCODE_PLUGIN_SCRIPT));
    }

    #[test]
    fn opencode_permission_response_allow_has_no_reason() {
        let response = opencode_permission_response(Decision::Approved, "");
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("allow")
        );
    }

    #[test]
    fn opencode_permission_response_deny_carries_note() {
        let response = opencode_permission_response(Decision::Denied, "not today");
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("deny")
        );
        assert_eq!(
            response.get("reason").and_then(Value::as_str),
            Some("Denied from Atoll: not today")
        );
    }

    #[test]
    fn opencode_defer_response_is_empty() {
        assert_eq!(
            build_hook_defer_response(
                PermissionResponseStyle::Opencode,
                "PermissionRequest",
                "Atoll approval timed out",
            ),
            serde_json::json!({})
        );
    }
}
