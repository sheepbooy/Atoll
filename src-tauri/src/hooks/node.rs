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

        if let Some(path) = resolve_bundled_node_executable() {
            return Ok(path);
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
        if let Some(path) = resolve_bundled_node_executable() {
            return Ok(path);
        }
        Err("Node.js not found. Install Node.js and ensure it is on PATH, then retry.".into())
    }
}

/// Probe Atoll's own bundled Node runtime (a Tauri resource written by
/// `scripts/fetch-node-runtime.mjs`) so hook installs work on fresh machines
/// without a system Node. Mirrors the exe-ancestry probing of
/// `bundled_hook_script_candidates`; takes a fallback seat after every system
/// Node so existing setups keep using their own runtime.
pub(crate) fn resolve_bundled_node_executable() -> Option<String> {
    let exe = std::env::current_exe().ok();
    first_usable_node_candidate(bundled_node_candidates(exe.as_deref()))
}

fn first_usable_node_candidate(candidates: Vec<std::path::PathBuf>) -> Option<String> {
    candidates
        .into_iter()
        .find(|candidate| hook_script_is_usable(candidate))
        .map(|path| normalize_hook_script_path(&path.to_string_lossy()))
}

#[cfg(not(windows))]
pub(crate) fn bundled_node_candidates(exe: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    // Dev builds: repo layout, where fetch-node-runtime.mjs writes the runtime.
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("generated")
            .join("node")
            .join("bin")
            .join("node"),
    );
    if let Some(exe) = exe {
        // Installed .app layout: exe at Contents/MacOS, resource at
        // Contents/Resources/node/bin/node.
        for ancestor in exe.ancestors().skip(1) {
            candidates.push(ancestor.join("Resources/node/bin/node"));
        }
    }
    candidates
}

#[cfg(windows)]
pub(crate) fn bundled_node_candidates(exe: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    // Dev builds: repo layout, where fetch-node-runtime.mjs writes the runtime.
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("generated")
            .join("node.exe"),
    );
    if let Some(exe) = exe {
        if let Some(exe_dir) = exe.parent() {
            // Installed layout (MSI): resources sit below the install dir,
            // matching the "node/node.exe" target in tauri.conf.json.
            candidates.push(exe_dir.join("node/node.exe"));
            candidates.push(exe_dir.join("resources/node/node.exe"));
        }
    }
    candidates
}

#[cfg(test)]
mod bundled_node_tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn candidates_include_app_bundle_resources_dir() {
        let exe = std::path::Path::new("/Applications/Atoll.app/Contents/MacOS/Atoll");
        let candidates = bundled_node_candidates(Some(exe));
        assert!(candidates.iter().any(|candidate| candidate
            .to_string_lossy()
            .ends_with("Atoll.app/Contents/Resources/node/bin/node")));
    }

    #[cfg(not(windows))]
    #[test]
    fn candidates_include_dev_generated_runtime() {
        let candidates = bundled_node_candidates(None);
        assert!(candidates.iter().any(|candidate| candidate
            .to_string_lossy()
            .ends_with("generated/node/bin/node")));
    }

    #[cfg(windows)]
    #[test]
    fn candidates_include_installed_resource_dir() {
        let exe = std::path::Path::new(r"C:\Program Files\Atoll\Atoll.exe");
        let candidates = bundled_node_candidates(Some(exe));
        assert!(candidates.iter().any(|candidate| candidate
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("/Atoll/node/node.exe")));
    }

    #[test]
    fn selection_skips_empty_placeholders_and_picks_non_empty_files() {
        let dir = std::env::temp_dir().join(format!("atoll-bundled-node-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty-node");
        std::fs::write(&empty, []).unwrap();
        let real = dir.join("node");
        std::fs::write(&real, b"stub").unwrap();

        let picked = first_usable_node_candidate(vec![empty, real.clone()]);
        assert_eq!(
            picked,
            Some(normalize_hook_script_path(&real.to_string_lossy()))
        );
        let _ = std::fs::remove_dir_all(dir);
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
