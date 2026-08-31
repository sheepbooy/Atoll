//! Hook command formatting and launcher repair: per-platform quoting, the
//! Windows PowerShell launcher + JSON config, and parsing of configured
//! commands back into node/script parts.

use serde_json::Value;
// Only the Windows launcher serializes its JSON config.
#[cfg(windows)]
use serde_json::json;
use tauri::AppHandle;

use super::*;

#[cfg(windows)]
pub(crate) fn maybe_repair_hook_launcher_config(
    app: &AppHandle,
    script_name: &str,
    config_filename: &str,
) {
    let Some(local_dir) = dirs::data_local_dir().map(|dir| dir.join("Atoll")) else {
        return;
    };
    let config_path = local_dir.join(config_filename);
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    let Ok(mut config) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let current_script = config.get("script").and_then(Value::as_str).unwrap_or("");
    let needs_repair = current_script.is_empty()
        || is_dev_hook_script_path(current_script)
        || !std::path::Path::new(current_script).is_file();
    if !needs_repair {
        return;
    }
    let Some(stable_script) = deployed_hook_script_path(script_name) else {
        return;
    };
    let runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| normalize_hook_command_path(&path.to_string_lossy()))
        .or_else(|| hook_runner_for_command(app));
    let node = config
        .get("node")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| resolve_node_executable().ok());
    let (Some(runner), Some(node)) = (runner, node) else {
        return;
    };
    config["script"] = json!(normalize_hook_command_path(&stable_script));
    config["runner"] = json!(runner);
    config["node"] = json!(normalize_hook_command_path(&node));
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(&config_path, formatted);
    }
}

#[cfg(not(windows))]
pub(crate) fn maybe_repair_hook_launcher_config(
    _app: &AppHandle,
    _script_name: &str,
    _config_filename: &str,
) {
}

#[cfg(windows)]
pub(crate) fn is_windows_file_locked_error(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32))
}

#[cfg(not(windows))]
pub(crate) fn is_windows_file_locked_error(_error: &std::io::Error) -> bool {
    false
}

pub(crate) fn paths_point_to_same_file(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

pub(crate) fn normalize_hook_command_path(path: &str) -> String {
    let path = normalize_hook_script_path(path);
    #[cfg(windows)]
    {
        path.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        path
    }
}

pub(crate) fn format_hook_command(
    _runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    let node_path = normalize_hook_command_path(node_path);
    let script_path = normalize_hook_command_path(script_path);

    #[cfg(windows)]
    if let Some(runner_path) = _runner_path {
        let runner_path = normalize_hook_command_path(runner_path);
        return format!(
            "\"{}\" \"{}\" \"{}\"",
            runner_path.replace('"', "\\\""),
            node_path.replace('"', "\\\""),
            script_path.replace('"', "\\\"")
        );
    }

    format!(
        "\"{}\" \"{}\"",
        node_path.replace('"', "\\\""),
        script_path.replace('"', "\\\"")
    )
}

/// Windows hook hosts (Cursor, Codex) often spawn hook commands through `cmd /c`.
/// A single quoted string like `"runner.exe" "node.exe" "script.mjs"` fails on paths
/// with spaces or non-ASCII profile dirs. Write a PowerShell launcher that forwards
/// stdin to `atoll-hook-runner.exe`; paths live in a UTF-8 JSON config file.
#[cfg(windows)]
pub(crate) fn write_windows_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
    config_filename: &str,
    ps1_filename: &str,
    fallback_stdout: &str,
) -> Result<String, String> {
    let stable_runner = atoll_local_hooks_dir()
        .map(|dir| dir.join("atoll-hook-runner.exe"))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned());
    let runner_path = stable_runner
        .or_else(|| hook_runner_for_command(app))
        .ok_or_else(|| "Cannot locate atoll-hook-runner.exe".to_string())?;
    let local_dir = dirs::data_local_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?
        .join("Atoll");
    std::fs::create_dir_all(&local_dir)
        .map_err(|error| format!("Cannot create {}: {error}", local_dir.display()))?;

    let runner = normalize_hook_command_path(&runner_path);
    let node = normalize_hook_command_path(node_path);
    let script = normalize_hook_command_path(script_path);
    let config_path = local_dir.join(config_filename);
    let config = json!({
        "runner": runner,
        "node": node,
        "script": script,
    });
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("Cannot serialize hook launcher config: {error}"))?;
    std::fs::write(&config_path, config_json.as_bytes())
        .map_err(|error| format!("Cannot write {}: {error}", config_path.display()))?;

    let ps1_path = local_dir.join(ps1_filename);
    let ps1_body = format!(
        r#"$ErrorActionPreference = 'Stop'
$configPath = Join-Path $env:LOCALAPPDATA 'Atoll\{config_filename}'
try {{
  $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $psi = New-Object System.Diagnostics.ProcessStartInfo($config.runner, ('"' + $config.node + '" "' + $config.script + '"'))
  $psi.UseShellExecute = $false
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.CreateNoWindow = $true
  $p = [System.Diagnostics.Process]::Start($psi)
  [Console]::OpenStandardInput().CopyTo($p.StandardInput.BaseStream)
  $p.StandardInput.Close()
  [Console]::Out.Write($p.StandardOutput.ReadToEnd())
  $p.WaitForExit() | Out-Null
  exit $p.ExitCode
}} catch {{
  [Console]::Out.Write('{fallback_stdout}')
  exit 0
}}
"#
    );
    // UTF-8 BOM so Windows PowerShell reads non-ASCII paths from the JSON config reliably.
    let mut ps1_bytes = vec![0xEF, 0xBB, 0xBF];
    ps1_bytes.extend_from_slice(ps1_body.as_bytes());
    std::fs::write(&ps1_path, ps1_bytes)
        .map_err(|error| format!("Cannot write {}: {error}", ps1_path.display()))?;

    Ok(format!(
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
        normalize_hook_command_path(&ps1_path.to_string_lossy()).replace('"', "\\\"")
    ))
}

#[cfg(windows)]
pub(crate) fn write_cursor_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "cursor-hook-launcher.json",
        "atoll-cursor-hook.ps1",
        r#"{"permission":"allow"}"#,
    )
}

#[cfg(windows)]
pub(crate) fn write_codex_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "codex-hook-launcher.json",
        "atoll-codex-hook.ps1",
        "{}",
    )
}

#[cfg(not(windows))]
pub(crate) fn write_codex_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

#[cfg(windows)]
pub(crate) fn write_zcode_hook_launcher_command(
    app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    write_windows_hook_launcher_command(
        app,
        node_path,
        script_path,
        "zcode-hook-launcher.json",
        "atoll-zcode-hook.ps1",
        "{}",
    )
}

#[cfg(not(windows))]
pub(crate) fn write_zcode_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

#[cfg(not(windows))]
pub(crate) fn write_cursor_hook_launcher_command(
    _app: &AppHandle,
    node_path: &str,
    script_path: &str,
) -> Result<String, String> {
    Ok(format_hook_command(None, node_path, script_path))
}

/// Legacy helper kept for tests; production Cursor installs use [`write_cursor_hook_launcher_command`].
#[cfg(windows)]
pub(crate) fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format!(
        "cmd /c {}",
        format_hook_command(runner_path, node_path, script_path)
    )
}

#[cfg(not(windows))]
pub(crate) fn format_cursor_hook_command(
    runner_path: Option<&str>,
    node_path: &str,
    script_path: &str,
) -> String {
    format_hook_command(runner_path, node_path, script_path)
}

pub(crate) fn extract_first_quoted_value(input: &str) -> Option<(String, &str)> {
    let input = input.trim();
    let inner = input.strip_prefix('"')?;
    let end = inner.find('"')?;
    let value = inner[..end].replace("\\\"", "\"");
    let rest = inner[end + 1..].trim_start();
    Some((value, rest))
}

pub(crate) fn expand_windows_hook_env(command: &str) -> String {
    let mut result = command.to_string();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        result = result.replace("%LOCALAPPDATA%", &local);
    }
    if let Ok(user) = std::env::var("USERPROFILE") {
        result = result.replace("%USERPROFILE%", &user);
    }
    result
}

pub(crate) fn extract_hook_command_parts(command: &str) -> Option<(String, String)> {
    let mut trimmed = expand_windows_hook_env(command).trim().to_string();
    if let Some(rest) = trimmed
        .strip_prefix("cmd /c ")
        .or_else(|| trimmed.strip_prefix("cmd /C "))
    {
        trimmed = rest.trim().to_string();
    }

    let normalized = normalize_hook_script_path(&trimmed);
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.ps1")
        || normalized
            .to_ascii_lowercase()
            .ends_with("atoll-codex-hook.ps1")
    {
        return parse_hook_launcher_script(&normalized);
    }
    if normalized
        .to_ascii_lowercase()
        .ends_with("atoll-cursor-hook.cmd")
    {
        return parse_cursor_launcher_cmd(&normalized);
    }

    if trimmed.starts_with("powershell ") {
        if let Some(start) = trimmed.find("-File \"") {
            let rest = &trimmed[start + 7..];
            if let Some(end) = rest.find('"') {
                let ps1 = normalize_hook_script_path(&expand_windows_hook_env(&rest[..end]));
                return parse_hook_launcher_script(&ps1);
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("node ") {
        let script = if rest.starts_with('"') {
            extract_first_quoted_value(rest)?.0
        } else {
            rest.split_whitespace().next()?.to_string()
        };
        return Some(("node".to_string(), normalize_hook_script_path(&script)));
    }

    let (first, rest) = extract_first_quoted_value(trimmed.as_str())?;
    if is_hook_runner_path(&first) {
        let (node, script_rest) = extract_first_quoted_value(rest)?;
        let (script, _) = extract_first_quoted_value(script_rest)?;
        return Some((
            normalize_hook_script_path(&node),
            normalize_hook_script_path(&script),
        ));
    }

    let (node, rest) = (first, rest);
    let (script, _) = extract_first_quoted_value(rest)?;
    Some((
        normalize_hook_script_path(&node),
        normalize_hook_script_path(&script),
    ))
}

pub(crate) fn is_hook_runner_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("/atoll-hook-runner.exe")
        || normalized.ends_with("/atoll-hook-runner")
        || normalized.contains("/atoll-hook-runner-")
}

pub(crate) fn parse_cursor_launcher_cmd(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("@echo off") {
            continue;
        }
        return extract_hook_command_parts(line);
    }
    None
}

pub(crate) fn parse_hook_launcher_config(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: Value = serde_json::from_str(&content).ok()?;
    let node = config.get("node").and_then(Value::as_str)?;
    let script = config.get("script").and_then(Value::as_str)?;
    Some((
        normalize_hook_script_path(node),
        normalize_hook_script_path(script),
    ))
}

pub(crate) fn parse_hook_launcher_script(path: &str) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    for (marker, filename) in [
        ("cursor-hook-launcher.json", "cursor-hook-launcher.json"),
        ("codex-hook-launcher.json", "codex-hook-launcher.json"),
    ] {
        if content.contains(marker) {
            let config_path = std::path::Path::new(path)
                .parent()
                .map(|dir| dir.join(filename))
                .filter(|candidate| candidate.is_file())
                .or_else(|| dirs::data_local_dir().map(|dir| dir.join("Atoll").join(filename)))?;
            return parse_hook_launcher_config(&config_path.to_string_lossy());
        }
    }
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.contains("atoll-hook-runner") {
            continue;
        }
        let line = line
            .strip_prefix("$Input | & ")
            .or_else(|| line.strip_prefix("$input | & "))
            .unwrap_or(line);
        return extract_hook_command_parts(line);
    }
    None
}

pub(crate) fn extract_node_script_path(command: &str) -> Option<String> {
    extract_hook_command_parts(command).map(|(_, script)| script)
}
