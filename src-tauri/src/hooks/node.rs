//! Node.js executable discovery for hook commands, including the Codex
//! desktop bundle preference.

use super::*;

#[cfg(windows)]
pub(crate) fn resolve_node_executable_from_where() -> Option<String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("where.exe")
        .arg("node")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .filter(|path| std::path::Path::new(path).exists())
        .map(normalize_hook_script_path)
}

#[cfg(not(windows))]
pub(crate) fn resolve_node_executable_from_shell() -> Option<String> {
    let output = std::process::Command::new("sh")
        .args(["-lc", "command -v node"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return None;
    }
    Some(normalize_hook_script_path(&path))
}

pub(crate) fn resolve_node_executable_from_path() -> Option<String> {
    if let Some(path_var) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_var) {
            #[cfg(windows)]
            let candidate = directory.join("node.exe");
            #[cfg(not(windows))]
            let candidate = directory.join("node");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
    }
    None
}

pub(crate) fn resolve_node_executable() -> Result<String, String> {
    #[cfg(windows)]
    {
        if let Some(path) = resolve_node_executable_from_where() {
            return Ok(path);
        }
        if let Some(path) = resolve_node_executable_from_path() {
            return Ok(path);
        }

        for candidate in [
            r"C:\Program Files\nodejs\node.exe",
            r"C:\Program Files (x86)\nodejs\node.exe",
        ] {
            if std::path::Path::new(candidate).exists() {
                return Ok(normalize_hook_script_path(candidate));
            }
        }

        return Err(
            "Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into(),
        );
    }

    #[cfg(not(windows))]
    {
        if let Some(path) = resolve_node_executable_from_shell() {
            return Ok(path);
        }
        if let Some(path) = resolve_node_executable_from_path() {
            return Ok(path);
        }
        Err("Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into())
    }
}

/// Prefer Codex Desktop's bundled Node when available so hooks work in the app sandbox.
pub(crate) fn resolve_codex_desktop_node_executable() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        for candidate in [
            "/Applications/Codex.app/Contents/Resources/cua_node/bin/node",
            "/Applications/Codex.app/Contents/Resources/node/bin/node",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate = std::path::PathBuf::from(local_app_data)
                .join("Programs")
                .join("Codex")
                .join("resources")
                .join("cua_node")
                .join("bin")
                .join("node.exe");
            if candidate.is_file() {
                return Some(normalize_hook_script_path(&candidate.to_string_lossy()));
            }
        }
        for candidate in [
            r"C:\Program Files\Codex\resources\cua_node\bin\node.exe",
            r"C:\Program Files (x86)\Codex\resources\cua_node\bin\node.exe",
        ] {
            if std::path::Path::new(candidate).is_file() {
                return Some(normalize_hook_script_path(candidate));
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = ();
    }

    None
}

pub(crate) fn resolve_node_executable_for_codex() -> Result<String, String> {
    if let Some(path) = resolve_codex_desktop_node_executable() {
        return Ok(path);
    }
    resolve_node_executable()
}
