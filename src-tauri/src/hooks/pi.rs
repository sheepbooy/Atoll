//! Pi integration: Pi has no command-hook pipeline and no built-in
//! permission system. Atoll deploys a user-global extension
//! (`atoll-pi-bridge.js`) into `~/.pi/agent/extensions`, which Pi
//! auto-discovers and loads in-process. The extension gates `tool_call`
//! events on Atoll's approval decision and forwards session/usage telemetry,
//! so Pi sessions started in a normal terminal are covered without Atoll
//! owning the process.
//!
//! The extension is plain ESM JavaScript in a `.js` file: Pi's loader accepts
//! `*.ts` and `*.js`, and `.js` keeps the shipped file directly testable with
//! `node --test`.

use tauri::AppHandle;

use super::*;

pub(crate) const PI_EXTENSION_SCRIPT: &str = "atoll-pi-bridge.js";
pub(crate) const PI_EXTENSION_MARKER: &str = "atoll-pi-bridge";

/// Pi discovers global extensions in `$PI_CODING_AGENT_DIR/extensions`
/// (default `~/.pi/agent/extensions`). Project-local `.pi/extensions` is
/// trust-gated, so Atoll must install globally.
pub(crate) fn pi_extensions_dir() -> Option<std::path::PathBuf> {
    let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".pi").join("agent")))?;
    Some(agent_dir.join("extensions"))
}

pub(crate) fn pi_extension_deployed_path() -> Option<std::path::PathBuf> {
    pi_extensions_dir().map(|dir| dir.join(PI_EXTENSION_SCRIPT))
}

/// An extension file counts as Atoll's when it exists and mentions the marker,
/// so a user's own file of the same name doesn't read as installed (and is
/// never deleted by uninstall).
fn is_atoll_pi_extension(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.contains(PI_EXTENSION_MARKER))
        .unwrap_or(false)
}

pub(crate) fn pi_hook_status(app: &AppHandle) -> HookStatus {
    if capture::force_hook_uninstalled() {
        let script_path = resolve_hook_script_path(app, PI_EXTENSION_SCRIPT).unwrap_or_default();
        return HookStatus {
            observation: Default::default(),
            installed: false,
            script_found: !script_path.is_empty() && std::path::Path::new(&script_path).exists(),
            settings_path: pi_extensions_dir()
                .map(|dir| dir.to_string_lossy().into_owned())
                .unwrap_or_default(),
            script_path,
            node_path: String::new(),
            node_found: true,
            needs_retrust: false,
            competing_hooks: Vec::new(),
        };
    }

    let deployed = pi_extension_deployed_path().unwrap_or_default();
    let installed = !deployed.as_os_str().is_empty() && is_atoll_pi_extension(&deployed);
    HookStatus {
        observation: Default::default(),
        installed,
        script_found: installed,
        settings_path: pi_extensions_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default(),
        script_path: deployed.to_string_lossy().into_owned(),
        node_path: String::new(),
        // The extension runs inside Pi's own Node runtime; node is unused.
        node_found: true,
        needs_retrust: false,
        competing_hooks: Vec::new(),
    }
    .with_observation(app, "pi")
}

#[tauri::command]
pub(crate) fn get_pi_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    Ok(pi_hook_status(&app))
}

#[tauri::command]
pub(crate) fn install_pi_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_script_path = resolve_install_hook_script_path(&app, PI_EXTENSION_SCRIPT)?;
    let extensions_dir =
        pi_extensions_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;
    std::fs::create_dir_all(&extensions_dir).map_err(|error| {
        format!(
            "Cannot create Pi extensions directory ({}): {error}",
            extensions_dir.display()
        )
    })?;

    let dest = extensions_dir.join(PI_EXTENSION_SCRIPT);
    copy_deployed_hook_file(
        std::path::Path::new(&source_script_path),
        &dest,
        "Pi bridge extension",
    )?;
    if !is_atoll_pi_extension(&dest) {
        // Roll back: an extension the status reader cannot recognize would
        // leave Pi loading a bridge Atoll reports as uninstalled.
        let _ = std::fs::remove_file(&dest);
        return Err(format!(
            "Pi extension was not saved correctly. Check permissions on {}.",
            dest.display()
        ));
    }

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after Pi extension install: {error}");
    }
    hook_trust::record_hook_installed("pi", &dest.to_string_lossy());

    crate::agent_events::reset(&app, "pi");
    emit_hook_snapshot_changed(&app)?;

    Ok(pi_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_pi_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let deployed = pi_extension_deployed_path()
        .ok_or_else(|| "Cannot determine home directory".to_string())?;
    if deployed.is_file() && is_atoll_pi_extension(&deployed) {
        std::fs::remove_file(&deployed)
            .map_err(|error| format!("Cannot remove Pi bridge extension: {error}"))?;
    }
    hook_trust::clear_hook_installed("pi");

    crate::agent_events::reset(&app, "pi");
    emit_hook_snapshot_changed(&app)?;

    Ok(pi_hook_status(&app))
}

#[cfg(test)]
mod pi_plugin_tests {
    use super::*;
    use crate::hook_bridge::{build_hook_defer_response, PermissionResponseStyle};
    use crate::Decision;
    use serde_json::Value;

    #[test]
    fn marker_check_rejects_missing_and_foreign_files() {
        let temp = std::env::temp_dir().join(format!("atoll-pi-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let foreign = temp.join("atoll-pi-bridge.js");
        std::fs::write(&foreign, "export default function () {}").expect("write");

        assert!(!is_atoll_pi_extension(&foreign));

        std::fs::write(
            &foreign,
            format!("// {PI_EXTENSION_MARKER}\nexport default 1;"),
        )
        .expect("write");
        assert!(is_atoll_pi_extension(&foreign));

        let missing = temp.join("nope.js");
        assert!(!is_atoll_pi_extension(&missing));

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn shipped_extension_script_contains_the_install_marker() {
        let script = std::fs::read_to_string(repo_hook_script_path(PI_EXTENSION_SCRIPT))
            .expect("read shipped atoll-pi-bridge.js");
        assert!(
            script.contains(PI_EXTENSION_MARKER),
            "scripts/{PI_EXTENSION_SCRIPT} must contain the marker \
             {PI_EXTENSION_MARKER:?}: the installer and status reads match on it"
        );
    }

    #[test]
    fn deployed_path_lives_under_pi_agent_extensions() {
        if std::env::var_os("PI_CODING_AGENT_DIR").is_some() {
            return;
        }
        let path = pi_extension_deployed_path().expect("home dir");
        let rendered = path.to_string_lossy().into_owned();
        assert!(rendered.contains(".pi"));
        assert!(rendered.contains("agent"));
        assert!(rendered.contains("extensions"));
        assert!(rendered.ends_with(PI_EXTENSION_SCRIPT));
    }

    #[test]
    fn pi_permission_response_allow_has_no_reason() {
        let response = crate::hook_bridge::build_permission_response(
            PermissionResponseStyle::Pi,
            "PermissionRequest",
            Decision::Approved,
            "",
            None,
        );
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("allow")
        );
        assert!(response.get("reason").is_none());
    }

    #[test]
    fn pi_permission_response_deny_carries_note() {
        let response = crate::hook_bridge::build_permission_response(
            PermissionResponseStyle::Pi,
            "PermissionRequest",
            Decision::Denied,
            "not today",
            None,
        );
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
    fn pi_defer_response_is_empty() {
        assert_eq!(
            build_hook_defer_response(
                PermissionResponseStyle::Pi,
                "PermissionRequest",
                "Atoll approval timed out",
            ),
            serde_json::json!({})
        );
    }
}
