//! Hook asset deployment: the stable per-user hooks dir, materialization of
//! the hook script/bridge (plus the Windows runner), and the script/runner
//! path resolution and config-inspection helpers used by status reads.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::*;

/// Stable hook install dir so hooks.json does not point at `target/debug/scripts`,
/// which disappears during rebuilds and makes Codex hooks exit with code 1.
pub(crate) fn atoll_local_hooks_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("Atoll").join("hooks"))
}

pub(crate) fn deployed_hook_script_path(script_name: &str) -> Option<String> {
    let path = atoll_local_hooks_dir()?.join(script_name);
    if hook_script_is_usable(&path) {
        Some(normalize_hook_script_path(&path.to_string_lossy()))
    } else {
        None
    }
}

pub(crate) fn files_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    match (std::fs::read(left), std::fs::read(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn deployed_hook_assets_current(
    source_script: &std::path::Path,
    deployed_script: &std::path::Path,
) -> bool {
    if !files_equal(source_script, deployed_script) {
        return false;
    }

    let Some(source_dir) = source_script.parent() else {
        return true;
    };
    let Some(deployed_dir) = deployed_script.parent() else {
        return true;
    };
    let source_bridge = source_dir.join("atoll-hook-bridge.mjs");
    if !source_bridge.is_file() {
        return true;
    }
    files_equal(&source_bridge, &deployed_dir.join("atoll-hook-bridge.mjs"))
}

pub(crate) fn refresh_deployed_hook_assets_if_needed(app: &AppHandle, script_name: &str) {
    let Some(deployed_script_path) = deployed_hook_script_path(script_name) else {
        return;
    };
    let Ok(source_script_path) = resolve_install_hook_script_path(app, script_name) else {
        return;
    };
    if source_script_path == deployed_script_path {
        return;
    }

    let source = std::path::Path::new(&source_script_path);
    let deployed = std::path::Path::new(&deployed_script_path);
    if deployed_hook_assets_current(source, deployed) {
        return;
    }

    if let Err(error) = materialize_hook_deployment(app, script_name, &source_script_path) {
        eprintln!("Atoll failed to refresh deployed {script_name}: {error}");
    }
}

pub(crate) fn canonical_hook_script_path(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
    marker: &str,
    fallback_path: &str,
) -> String {
    if let Some(deployed) = deployed_hook_script_path(script_name) {
        return deployed;
    }
    if let Some(configured) = config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
    {
        if std::path::Path::new(&configured).is_file() {
            return configured;
        }
    }
    if !fallback_path.is_empty() && std::path::Path::new(fallback_path).is_file() {
        return fallback_path.to_string();
    }
    resolve_hook_script_path(app, script_name).unwrap_or_default()
}

/// Copy hook assets into the stable deploy dir. If Windows reports that the
/// destination is locked (ERROR_SHARING_VIOLATION / os error 32) because Codex,
/// Cursor, or a live hook invocation still has the runner open, keep the existing
/// file so install can finish updating hooks.json and launcher config.
///
/// Copy via a sibling temp file so `source == dest` cannot truncate the script
/// to 0 bytes (a classic `fs::copy` self-overwrite bug).
pub(crate) fn copy_deployed_hook_file(
    source: &std::path::Path,
    dest: &std::path::Path,
    label: &str,
) -> Result<(), String> {
    if !source.is_file() {
        return Err(format!(
            "Cannot copy {label} from missing {}",
            source.display()
        ));
    }
    if paths_point_to_same_file(source, dest) {
        return Ok(());
    }

    let Some(name) = dest.file_name().and_then(|name| name.to_str()) else {
        return Err(format!("Cannot copy {label} to {}", dest.display()));
    };
    let temp = dest.with_file_name(format!(".{name}.tmp"));
    match std::fs::copy(source, &temp) {
        Ok(_) => {
            if let Err(error) = std::fs::rename(&temp, dest) {
                let _ = std::fs::remove_file(&temp);
                if dest.is_file() && is_windows_file_locked_error(&error) {
                    eprintln!(
                        "Atoll kept existing {label} at {} because the file is in use ({error})",
                        dest.display()
                    );
                    return Ok(());
                }
                return Err(format!(
                    "Cannot copy {label} to {}: {error}",
                    dest.display()
                ));
            }
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            if dest.is_file() && is_windows_file_locked_error(&error) {
                eprintln!(
                    "Atoll kept existing {label} at {} because the file is in use ({error})",
                    dest.display()
                );
                Ok(())
            } else {
                Err(format!(
                    "Cannot copy {label} to {}: {error}",
                    dest.display()
                ))
            }
        }
    }
}

pub(crate) fn materialize_hook_deployment(
    #[cfg_attr(not(windows), allow(unused_variables))] app: &AppHandle,
    script_name: &str,
    source_script_path: &str,
) -> Result<String, String> {
    static DEPLOY_LOCK: Mutex<()> = Mutex::new(());
    let _guard = DEPLOY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let source = std::path::Path::new(source_script_path);
    if !hook_script_is_usable(source) {
        return Err(format!("Hook script not found at: {source_script_path}"));
    }

    let hooks_dir = atoll_local_hooks_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?;
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("Cannot create {}: {error}", hooks_dir.display()))?;

    let dest_script = hooks_dir.join(script_name);
    copy_deployed_hook_file(source, &dest_script, "hook script")?;

    if let Some(source_dir) = source.parent() {
        let bridge_name = "atoll-hook-bridge.mjs";
        let source_bridge = source_dir.join(bridge_name);
        if hook_script_is_usable(&source_bridge) {
            let dest_bridge = hooks_dir.join(bridge_name);
            copy_deployed_hook_file(&source_bridge, &dest_bridge, "hook bridge module")?;
        }
    }

    #[cfg(windows)]
    {
        let dest_runner = hooks_dir.join("atoll-hook-runner.exe");
        if let Some(runner_path) = hook_runner_for_command(app) {
            let runner_source = std::path::Path::new(&runner_path);
            if runner_source.is_file() {
                copy_deployed_hook_file(runner_source, &dest_runner, "hook runner")?;
            }
        }
        if !dest_runner.is_file() {
            return Err(
                "Cannot locate atoll-hook-runner.exe. Rebuild Atoll, then try installing hooks again."
                    .into(),
            );
        }
    }

    Ok(normalize_hook_script_path(&dest_script.to_string_lossy()))
}

pub(crate) fn normalize_hook_script_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    dunce::simplified(std::path::Path::new(path))
        .to_string_lossy()
        .into_owned()
}

/// Prefer the bundled/repo hook script over `~/Library/Application Support/Atoll/hooks`.
/// The local dir is the *destination* of `materialize_hook_deployment`; using it as the
/// install source lets a 0-byte leftover copy itself and wipe the real script.
pub(crate) fn resolve_install_hook_script_path(
    app: &AppHandle,
    script_name: &str,
) -> Result<String, String> {
    let resource_dir = app.path().resource_dir().ok();
    let exe = std::env::current_exe().ok();
    first_usable_hook_script(bundled_hook_script_candidates(
        resource_dir.as_deref(),
        exe.as_deref(),
        script_name,
    ))
    .ok_or_else(|| format!("Cannot locate hook script: {script_name}"))
}

pub(crate) fn hook_script_is_usable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn repo_hook_script_path(script_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join(script_name)
}

pub(crate) fn bundled_hook_script_candidates(
    resource_dir: Option<&Path>,
    exe: Option<&Path>,
    script_name: &str,
) -> Vec<PathBuf> {
    let mut candidates = vec![repo_hook_script_path(script_name)];

    if let Some(resource_dir) = resource_dir {
        candidates.push(resource_dir.join("scripts").join(script_name));
        candidates.push(resource_dir.join(script_name));
    }

    if let Some(exe) = exe {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources").join("scripts").join(script_name));
            candidates.push(exe_dir.join("scripts").join(script_name));
            if exe_dir.file_name().is_some_and(|name| name == "MacOS") {
                if let Some(contents) = exe_dir.parent() {
                    candidates.push(contents.join("Resources").join("scripts").join(script_name));
                }
            }
        }
        for ancestor in exe.ancestors().skip(1) {
            candidates.push(ancestor.join("Resources").join("scripts").join(script_name));
            candidates.push(ancestor.join("scripts").join(script_name));
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
                if let Some(repo_root) = ancestor.parent() {
                    candidates.push(repo_root.join("scripts").join(script_name));
                }
            }
            if ancestor.join("src-tauri").exists() {
                candidates.push(ancestor.join("scripts").join(script_name));
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("scripts")
                        .join(script_name),
                );
            }
        }
    }

    candidates
}

pub(crate) fn first_usable_hook_script(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Option<String> {
    candidates.into_iter().find_map(|candidate| {
        hook_script_is_usable(&candidate)
            .then(|| normalize_hook_script_path(&candidate.to_string_lossy()))
    })
}

pub(crate) fn read_json_file(path: &str) -> Option<Value> {
    if path.is_empty() || !std::path::Path::new(path).exists() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

pub(crate) fn configured_atoll_hook_command(config: &Value, marker: &str) -> Option<String> {
    let hooks = config.get("hooks")?.as_object()?;
    // ZCode nests event matchers under `hooks.events` (alongside `enabled` and
    // `timeoutMs`); Claude/Codex/Cursor list the event arrays directly under
    // `hooks`.
    let events = hooks
        .get("events")
        .and_then(Value::as_object)
        .unwrap_or(hooks);
    for matchers in events.values() {
        let Some(arr) = matchers.as_array() else {
            continue;
        };
        for matcher in arr {
            if let Some(hook_arr) = matcher.get("hooks").and_then(Value::as_array) {
                for hook in hook_arr {
                    if let Some(cmd) = hook.get("command").and_then(Value::as_str) {
                        if cmd.contains(marker) {
                            return Some(cmd.to_string());
                        }
                    }
                }
            }
            if let Some(cmd) = matcher.get("command").and_then(Value::as_str) {
                if cmd.contains(marker) {
                    return Some(cmd.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn configured_atoll_hook_node_path(config: &Value, marker: &str) -> Option<String> {
    configured_atoll_hook_command(config, marker)
        .and_then(|cmd| extract_hook_command_parts(&cmd).map(|(node, _)| node))
}

pub(crate) fn node_executable_ready(node_path: &str) -> bool {
    if node_path.is_empty() {
        return resolve_node_executable().is_ok();
    }
    if node_path == "node" {
        return resolve_node_executable().is_ok();
    }
    std::path::Path::new(node_path).exists()
}

pub(crate) fn configured_atoll_hook_script_path(config: &Value, marker: &str) -> Option<String> {
    configured_atoll_hook_command(config, marker).and_then(|cmd| extract_node_script_path(&cmd))
}

pub(crate) fn resolve_hook_script_readiness(
    app: &AppHandle,
    script_name: &str,
    config: Option<&Value>,
) -> (String, bool) {
    let marker = script_name.trim_end_matches(".mjs");
    let mut script_path = resolve_hook_script_path(app, script_name).unwrap_or_default();
    let mut script_found =
        !script_path.is_empty() && hook_script_is_usable(Path::new(&script_path));

    if !script_found {
        if let Some(configured) =
            config.and_then(|cfg| configured_atoll_hook_script_path(cfg, marker))
        {
            if hook_script_is_usable(Path::new(&configured)) {
                script_found = true;
                if script_path.is_empty() {
                    script_path = configured;
                }
            }
        }
    }

    (script_path, script_found)
}

pub(crate) fn resolve_hook_script_path(app: &AppHandle, script_name: &str) -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join(script_name));
    }
    let resource_dir = app.path().resource_dir().ok();
    let exe = std::env::current_exe().ok();
    candidates.extend(bundled_hook_script_candidates(
        resource_dir.as_deref(),
        exe.as_deref(),
        script_name,
    ));

    first_usable_hook_script(candidates)
}

#[cfg(windows)]
pub(crate) fn resolve_hook_runner_path(app: &AppHandle) -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(hooks_dir) = atoll_local_hooks_dir() {
        candidates.push(hooks_dir.join("atoll-hook-runner.exe"));
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("scripts").join("atoll-hook-runner.exe"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("atoll-hook-runner.exe"));
            candidates.push(
                exe_dir
                    .join("resources")
                    .join("scripts")
                    .join("atoll-hook-runner.exe"),
            );
            candidates.push(exe_dir.join("scripts").join("atoll-hook-runner.exe"));
        }
        for ancestor in exe.ancestors().skip(1) {
            if ancestor.file_name().is_some_and(|name| name == "src-tauri") {
                candidates.push(
                    ancestor
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("target")
                        .join("release")
                        .join("atoll-hook-runner.exe"),
                );
            }
            if ancestor.join("src-tauri").exists() {
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("generated")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("debug")
                        .join("atoll-hook-runner.exe"),
                );
                candidates.push(
                    ancestor
                        .join("src-tauri")
                        .join("target")
                        .join("release")
                        .join("atoll-hook-runner.exe"),
                );
            }
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
        }
    }

    None
}

#[cfg(not(windows))]
pub(crate) fn resolve_hook_runner_path(_app: &AppHandle) -> Option<String> {
    None
}

pub(crate) fn hook_runner_for_command(app: &AppHandle) -> Option<String> {
    resolve_hook_runner_path(app)
}
