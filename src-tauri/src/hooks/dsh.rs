//! DeepSeek Harness integration: DSH has no command-hook pipeline, so Atoll
//! deploys a Cordis plugin package into the shared profile module directory and
//! adds one managed `insert` row to the home-level `cordis.patch.yml`. The
//! plugin runs inside DSH's own Node runtime, answers `approval/request` through
//! Atoll's local bridge, and leaves DSH's native approval UI in charge whenever
//! Atoll is unreachable.
//!
//! The patch file is user-owned YAML: edits are confined to a marker-delimited
//! block, existing entries and comments are preserved, and a conflicting
//! hand-written `atoll-bridge` row aborts the install instead of being
//! overwritten.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use super::*;

pub(crate) const DSH_PLUGIN_DIR_NAME: &str = "dsh-plugin";
pub(crate) const DSH_PLUGIN_ENTRY: &str = "index.mjs";
pub(crate) const DSH_PLUGIN_PACKAGE: &str = "package.json";
pub(crate) const DSH_PLUGIN_PACKAGE_NAME: &str = "@atoll/dsh-bridge";
pub(crate) const DSH_PATCH_ID: &str = "atoll-bridge";
pub(crate) const DSH_PLUGIN_MARKER: &str = "atoll-dsh-bridge";

const DSH_PATCH_BEGIN: &str = "# >>> Atoll DSH bridge (managed) >>>";
const DSH_PATCH_END: &str = "# <<< Atoll DSH bridge (managed) <<<";
const DSH_PATCH_FILENAME: &str = "cordis.patch.yml";
const DSH_PATCH_BACKUP_SUFFIX: &str = ".atoll.bak";

fn expand_home(path: &str, home: &Path) -> PathBuf {
    if path == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// Resolve `$DSH_HOME`, else `~/.dsh`. The DSH CLI and Desktop app share this
/// root; Atoll's GUI may not inherit a shell-set `DSH_HOME`, so the default is
/// the common case.
pub(crate) fn dsh_home() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if let Some(configured) = std::env::var_os("DSH_HOME") {
        let configured = configured.to_string_lossy();
        if !configured.trim().is_empty() {
            return Some(expand_home(configured.trim(), &home));
        }
    }
    Some(home.join(".dsh"))
}

pub(crate) fn dsh_patch_path() -> Option<PathBuf> {
    dsh_home().map(|home| home.join(DSH_PATCH_FILENAME))
}

pub(crate) fn dsh_profile_modules_dir() -> Option<PathBuf> {
    dsh_home().map(|home| home.join("profiles").join("node_modules"))
}

pub(crate) fn dsh_plugin_dir() -> Option<PathBuf> {
    dsh_profile_modules_dir().map(|modules| modules.join("@atoll").join("dsh-bridge"))
}

pub(crate) fn dsh_plugin_entry_path() -> Option<PathBuf> {
    dsh_plugin_dir().map(|dir| dir.join(DSH_PLUGIN_ENTRY))
}

fn dsh_patch_display() -> String {
    dsh_patch_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn dsh_plugin_entry_display() -> String {
    dsh_plugin_entry_path()
        .map(|path| normalize_hook_script_path(&path.to_string_lossy()))
        .unwrap_or_default()
}

fn is_atoll_dsh_plugin(dir: &Path) -> bool {
    let entry = dir.join(DSH_PLUGIN_ENTRY);
    let package = dir.join(DSH_PLUGIN_PACKAGE);
    if !entry.is_file() || !package.is_file() {
        return false;
    }
    std::fs::read_to_string(entry)
        .map(|content| content.contains(DSH_PLUGIN_MARKER))
        .unwrap_or(false)
}

fn dsh_status(app: &AppHandle, installed: bool, script_found: bool) -> HookStatus {
    HookStatus {
        observation: Default::default(),
        installed,
        script_found,
        settings_path: dsh_patch_display(),
        script_path: dsh_plugin_entry_display(),
        node_path: String::new(),
        // The plugin executes inside DSH's own Node runtime.
        node_found: true,
        needs_retrust: false,
        competing_hooks: Vec::new(),
    }
    .with_observation(app, "dsh")
}

pub(crate) fn dsh_hook_status(app: &AppHandle) -> HookStatus {
    if capture::force_hook_uninstalled() {
        let script_found = dsh_plugin_dir()
            .as_deref()
            .map(is_atoll_dsh_plugin)
            .unwrap_or(false);
        return dsh_status(app, false, script_found);
    }

    let patch_installed = dsh_patch_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|content| patch_has_managed_block(&content))
        .unwrap_or(false);

    if patch_installed {
        refresh_dsh_plugin_assets_if_needed(app);
    }

    let script_found = dsh_plugin_dir()
        .as_deref()
        .map(is_atoll_dsh_plugin)
        .unwrap_or(false);

    let mut status = dsh_status(app, patch_installed, script_found);
    if patch_installed && script_found {
        let entry = status.script_path.clone();
        status.needs_retrust = crate::hook_trust::needs_retrust("dsh", &entry, None);
    }
    status
}

fn dsh_plugin_source_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let resource_dir = app.path().resource_dir().ok();
    let exe = std::env::current_exe().ok();
    let entry_name = format!("{DSH_PLUGIN_DIR_NAME}/{DSH_PLUGIN_ENTRY}");
    let mut seen = std::collections::HashSet::new();
    let mut dirs = Vec::new();
    for entry in
        bundled_hook_script_candidates(resource_dir.as_deref(), exe.as_deref(), &entry_name)
    {
        let Some(parent) = entry.parent() else {
            continue;
        };
        if seen.insert(parent.to_path_buf()) {
            dirs.push(parent.to_path_buf());
        }
    }
    // The standard repo location is also the first bundled candidate; keeping
    // this explicit fallback makes dev builds independent of resource layout.
    let repo = repo_hook_script_path(DSH_PLUGIN_DIR_NAME);
    if seen.insert(repo.clone()) {
        dirs.push(repo);
    }
    dirs
}

fn resolve_dsh_plugin_source_dir(app: &AppHandle) -> Result<PathBuf, String> {
    dsh_plugin_source_candidates(app)
        .into_iter()
        .find(|dir| is_atoll_dsh_plugin(dir))
        .ok_or_else(|| {
            format!(
                "Cannot locate the DeepSeek Harness bridge plugin. Expected {DSH_PLUGIN_DIR_NAME}/{DSH_PLUGIN_ENTRY} in the app bundle."
            )
        })
}

fn copy_dsh_plugin(source_dir: &Path, destination_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination_dir).map_err(|error| {
        format!(
            "Cannot create DeepSeek Harness plugin directory ({}): {error}",
            destination_dir.display()
        )
    })?;

    let entries = std::fs::read_dir(source_dir).map_err(|error| {
        format!(
            "Cannot read DeepSeek Harness plugin source ({}): {error}",
            source_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Cannot read plugin source entry: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        copy_deployed_hook_file(
            &path,
            &destination_dir.join(name),
            "DeepSeek Harness bridge plugin",
        )?;
    }

    if !is_atoll_dsh_plugin(destination_dir) {
        return Err(format!(
            "DeepSeek Harness plugin was not saved correctly. Check permissions on {}.",
            destination_dir.display()
        ));
    }
    Ok(())
}

fn plugin_dir_for_home(home: &Path) -> PathBuf {
    home.join("profiles")
        .join("node_modules")
        .join("@atoll")
        .join("dsh-bridge")
}

fn refresh_dsh_plugin_assets_if_needed(app: &AppHandle) {
    let (Some(destination), Ok(source)) = (dsh_plugin_dir(), resolve_dsh_plugin_source_dir(app))
    else {
        return;
    };
    if !destination.is_dir() {
        return;
    }
    let source_entry = source.join(DSH_PLUGIN_ENTRY);
    let destination_entry = destination.join(DSH_PLUGIN_ENTRY);
    if files_equal(&source_entry, &destination_entry) {
        return;
    }
    if let Err(error) = copy_dsh_plugin(&source, &destination) {
        eprintln!("Atoll failed to refresh the DeepSeek Harness bridge plugin: {error}");
    }
}

fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn managed_block(line_ending: &str) -> String {
    format!(
        "{DSH_PATCH_BEGIN}{line_ending}- insert:{line_ending}    - id: {DSH_PATCH_ID}{line_ending}      name: '{DSH_PLUGIN_PACKAGE_NAME}'{line_ending}{DSH_PATCH_END}{line_ending}"
    )
}

fn managed_block_range(content: &str) -> Option<(usize, usize)> {
    let begin = content.find(DSH_PATCH_BEGIN)?;
    let end_marker_start = content[begin..].find(DSH_PATCH_END)? + begin;
    let mut end = end_marker_start + DSH_PATCH_END.len();
    if content[end..].starts_with("\r\n") {
        end += 2;
    } else if content[end..].starts_with('\n') {
        end += 1;
    }
    Some((begin, end))
}

fn patch_has_managed_block(content: &str) -> bool {
    let Some((begin, end)) = managed_block_range(content) else {
        return false;
    };
    let block = &content[begin..end];
    block.contains(DSH_PATCH_ID) && block.contains(DSH_PLUGIN_PACKAGE_NAME)
}

fn content_without_managed_block(content: &str) -> String {
    match managed_block_range(content) {
        Some((begin, end)) => format!("{}{}", &content[..begin], &content[end..]),
        None => content.to_string(),
    }
}

fn line_has_yaml_code(line: &str) -> bool {
    let code = line.split('#').next().unwrap_or("").trim();
    !code.is_empty() && code != "---" && code != "[]"
}

fn content_has_yaml_entries(content: &str) -> bool {
    content.lines().any(line_has_yaml_code)
}

fn has_inline_empty_array(content: &str) -> bool {
    content
        .lines()
        .any(|line| line.split('#').next().unwrap_or("").trim() == "[]")
}

fn patch_has_conflicting_entry(content: &str) -> bool {
    let outside = content_without_managed_block(content);
    let normalized = outside.replace('"', "'");
    normalized.contains(&format!("id: {DSH_PATCH_ID}"))
        || normalized.contains(DSH_PLUGIN_PACKAGE_NAME)
}

fn upsert_dsh_patch(content: &str) -> Result<String, String> {
    let line_ending = detect_line_ending(content);
    let block = managed_block(line_ending);

    if managed_block_range(content).is_some() {
        let (begin, end) = managed_block_range(content).expect("range checked above");
        return Ok(format!("{}{}{}", &content[..begin], block, &content[end..]));
    }

    if patch_has_conflicting_entry(content) {
        return Err(format!(
            "{} already contains an `{}` entry outside Atoll's managed block. Remove or rename it before installing.",
            dsh_patch_display(),
            DSH_PATCH_ID
        ));
    }

    if has_inline_empty_array(content) {
        let mut out = Vec::new();
        let mut replaced = false;
        for line in content.lines() {
            if !replaced && line.split('#').next().unwrap_or("").trim() == "[]" {
                out.push(block.trim_end_matches(line_ending).to_string());
                replaced = true;
            } else {
                out.push(line.to_string());
            }
        }
        return Ok(format!("{}{line_ending}", out.join(line_ending)));
    }

    if !content_has_yaml_entries(content) {
        // Empty or comments-only file. Appending the block keeps any header
        // comments the user wrote and still produces a valid top-level array.
        let mut out = content.to_string();
        if !out.is_empty() && !out.ends_with(line_ending) {
            out.push_str(line_ending);
        }
        if !out.is_empty() {
            out.push_str(line_ending);
        }
        out.push_str(&block);
        return Ok(out);
    }

    let mut out = content.to_string();
    if !out.ends_with(line_ending) {
        out.push_str(line_ending);
    }
    out.push_str(line_ending);
    out.push_str(&block);
    Ok(out)
}

fn remove_dsh_patch(content: &str) -> Result<String, String> {
    let line_ending = detect_line_ending(content);
    if managed_block_range(content).is_none() {
        if patch_has_conflicting_entry(content) {
            return Err(format!(
                "{} has a hand-written `{}` entry. Remove it manually before uninstalling the Atoll bridge.",
                dsh_patch_display(),
                DSH_PATCH_ID
            ));
        }
        return Ok(content.to_string());
    }

    let mut remaining = content_without_managed_block(content);
    // Collapse the blank lines left behind by removing the block.
    while remaining.contains(&format!("{line_ending}{line_ending}{line_ending}")) {
        remaining = remaining.replace(
            &format!("{line_ending}{line_ending}{line_ending}"),
            &format!("{line_ending}{line_ending}"),
        );
    }
    remaining = remaining
        .trim_matches(|c| c == '\r' || c == '\n')
        .to_string();
    if remaining.trim().is_empty() {
        return Ok(format!("[]{}", line_ending));
    }
    if !content_has_yaml_entries(&remaining) {
        // Keep the user's header comments; DSH only needs a top-level array.
        return Ok(format!(
            "{remaining}{line_ending}{line_ending}[]{line_ending}"
        ));
    }
    Ok(format!("{remaining}{line_ending}"))
}

fn write_patch_with_backup(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    }

    let backup = path.with_file_name(format!("{DSH_PATCH_FILENAME}{DSH_PATCH_BACKUP_SUFFIX}"));
    if path.is_file() && !backup.is_file() {
        let _ = std::fs::copy(path, &backup);
    }

    let previous = std::fs::read_to_string(path).ok();
    std::fs::write(path, content).map_err(|error| {
        if let Some(previous) = previous {
            let _ = std::fs::write(path, previous);
        }
        format!("Cannot write {}: {error}", path.display())
    })
}

fn remove_plugin_dir(plugin_dir: &Path) {
    let _ = std::fs::remove_dir_all(plugin_dir);
    if let Some(parent) = plugin_dir.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

fn install_dsh_plugin(home: &Path, source_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(home)
        .map_err(|error| format!("Cannot create {}: {error}", home.display()))?;
    let modules = home.join("profiles").join("node_modules");
    let plugin_dir = modules.join("@atoll").join("dsh-bridge");
    let patch_path = home.join(DSH_PATCH_FILENAME);
    let patch_content = std::fs::read_to_string(&patch_path).unwrap_or_default();
    let patch_installed = patch_has_managed_block(&patch_content);
    let plugin_dir_existed = plugin_dir.exists();

    if plugin_dir_existed && !patch_installed && !is_atoll_dsh_plugin(&plugin_dir) {
        return Err(format!(
            "{} already exists and is not Atoll's DeepSeek Harness bridge. Move it away before installing.",
            plugin_dir.display()
        ));
    }

    copy_dsh_plugin(source_dir, &plugin_dir)?;

    let next_patch = match upsert_dsh_patch(&patch_content) {
        Ok(next) => next,
        Err(error) => {
            if !plugin_dir_existed {
                remove_plugin_dir(&plugin_dir);
            }
            return Err(error);
        }
    };
    if let Err(error) = write_patch_with_backup(&patch_path, &next_patch) {
        if !plugin_dir_existed {
            remove_plugin_dir(&plugin_dir);
        }
        return Err(error);
    }
    if !patch_has_managed_block(&next_patch) {
        if !plugin_dir_existed {
            remove_plugin_dir(&plugin_dir);
        }
        return Err(format!(
            "DeepSeek Harness home patch was not saved correctly. Check permissions on {}.",
            patch_path.display()
        ));
    }
    Ok(())
}

fn uninstall_dsh_plugin(home: &Path) -> Result<(), String> {
    let patch_path = home.join(DSH_PATCH_FILENAME);
    if patch_path.is_file() {
        let current = std::fs::read_to_string(&patch_path)
            .map_err(|error| format!("Cannot read {}: {error}", patch_path.display()))?;
        let next = remove_dsh_patch(&current)?;
        if next != current {
            write_patch_with_backup(&patch_path, &next)?;
        }
    }
    let plugin_dir = plugin_dir_for_home(home);
    if is_atoll_dsh_plugin(&plugin_dir) {
        remove_plugin_dir(&plugin_dir);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn get_dsh_hook_status(app: AppHandle) -> Result<HookStatus, String> {
    Ok(dsh_hook_status(&app))
}

#[tauri::command]
pub(crate) fn install_dsh_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let source_dir = resolve_dsh_plugin_source_dir(&app)?;
    let home = dsh_home().ok_or_else(|| "Cannot determine home directory".to_string())?;
    install_dsh_plugin(&home, &source_dir)?;

    if let Err(error) = hook_bridge::refresh_bridge_config_file(&app) {
        eprintln!("Atoll failed to refresh bridge.json after DeepSeek Harness install: {error}");
    }
    let entry = dsh_plugin_entry_display();
    crate::hook_trust::record_hook_installed("dsh", &entry);
    crate::agent_events::reset(&app, "dsh");
    emit_hook_snapshot_changed(&app)?;
    Ok(dsh_hook_status(&app))
}

#[tauri::command]
pub(crate) fn uninstall_dsh_hooks(app: AppHandle) -> Result<HookStatus, String> {
    let home = dsh_home().ok_or_else(|| "Cannot determine home directory".to_string())?;
    uninstall_dsh_plugin(&home)?;
    crate::hook_trust::clear_hook_installed("dsh");
    crate::agent_events::reset(&app, "dsh");
    emit_hook_snapshot_changed(&app)?;
    Ok(dsh_hook_status(&app))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atoll-dsh-{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write_plugin(dir: &Path) {
        std::fs::create_dir_all(dir).expect("plugin dir");
        std::fs::write(
            dir.join(DSH_PLUGIN_ENTRY),
            format!("// {DSH_PLUGIN_MARKER}\nexport function apply() {{}}\n"),
        )
        .expect("write entry");
        std::fs::write(
            dir.join(DSH_PLUGIN_PACKAGE),
            format!(r#"{{"name":"{DSH_PLUGIN_PACKAGE_NAME}","type":"module"}}"#),
        )
        .expect("write package");
    }

    #[test]
    fn shipped_plugin_assets_are_present_and_marked() {
        let dir = repo_hook_script_path(DSH_PLUGIN_DIR_NAME);
        assert!(
            is_atoll_dsh_plugin(&dir),
            "scripts/{DSH_PLUGIN_DIR_NAME} must contain {DSH_PLUGIN_ENTRY} with the {DSH_PLUGIN_MARKER} marker"
        );
    }

    #[test]
    fn upsert_preserves_existing_entries_and_is_idempotent() {
        let existing = "# my patches\n- id: mcp-comfy\n  name: foo\n";
        let once = upsert_dsh_patch(existing).expect("first upsert");
        assert!(once.contains("- id: mcp-comfy"));
        assert!(once.contains(DSH_PATCH_BEGIN));
        assert!(once.contains(DSH_PLUGIN_PACKAGE_NAME));
        let twice = upsert_dsh_patch(&once).expect("second upsert");
        assert_eq!(once, twice);
        assert_eq!(twice.matches(DSH_PATCH_BEGIN).count(), 1);
    }

    #[test]
    fn upsert_replaces_inline_empty_array_and_comments_only() {
        let empty = upsert_dsh_patch("[]\n").expect("empty array");
        assert!(!empty.contains("[]\n"));
        assert!(patch_has_managed_block(&empty));

        let comments = upsert_dsh_patch("# keep me\n").expect("comments only");
        assert!(comments.starts_with("# keep me"));
        assert!(patch_has_managed_block(&comments));

        let empty_file = upsert_dsh_patch("").expect("empty file");
        assert!(patch_has_managed_block(&empty_file));
    }

    #[test]
    fn upsert_refuses_a_hand_written_conflict() {
        let conflict = "- id: atoll-bridge\n  name: something-else\n";
        let error = upsert_dsh_patch(conflict).expect_err("conflict must fail");
        assert!(error.contains("outside Atoll's managed block"));
    }

    #[test]
    fn remove_restores_empty_array_and_keeps_other_entries() {
        let existing = "- id: mcp-comfy\n  name: foo\n";
        let installed = upsert_dsh_patch(existing).expect("install");
        let removed = remove_dsh_patch(&installed).expect("remove");
        assert!(removed.contains("- id: mcp-comfy"));
        assert!(!removed.contains(DSH_PATCH_BEGIN));

        let only_block = upsert_dsh_patch("").expect("install");
        let emptied = remove_dsh_patch(&only_block).expect("remove");
        assert_eq!(emptied, "[]\n");
    }

    #[test]
    fn upsert_and_remove_preserve_crlf_line_endings() {
        let installed = upsert_dsh_patch("[]\r\n").expect("install");
        assert!(installed.contains("\r\n"));
        let removed = remove_dsh_patch(&installed).expect("remove");
        assert_eq!(removed, "[]\r\n");
    }

    #[test]
    fn remove_refuses_hand_written_conflict() {
        let error = remove_dsh_patch("- id: atoll-bridge\n  name: user-owned\n")
            .expect_err("hand-written entry must not be deleted");
        assert!(error.contains("hand-written"));
    }

    #[test]
    fn installs_the_shipped_plugin_package_into_temp_home() {
        let home = temp_dir("shipped-home");
        let source = repo_hook_script_path(DSH_PLUGIN_DIR_NAME);
        install_dsh_plugin(&home, &source).expect("install shipped plugin");
        let deployed = plugin_dir_for_home(&home);
        assert!(is_atoll_dsh_plugin(&deployed));
        assert!(deployed.join(DSH_PLUGIN_PACKAGE).is_file());
        assert!(patch_has_managed_block(
            &std::fs::read_to_string(home.join(DSH_PATCH_FILENAME)).expect("patch")
        ));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn install_and_uninstall_round_trip_in_temp_home() {
        let home = temp_dir("home");
        let source = temp_dir("source");
        write_plugin(&source);

        install_dsh_plugin(&home, &source).expect("install");
        let patch = std::fs::read_to_string(home.join(DSH_PATCH_FILENAME)).expect("patch");
        assert!(patch_has_managed_block(&patch));
        let deployed = home
            .join("profiles")
            .join("node_modules")
            .join("@atoll")
            .join("dsh-bridge");
        assert!(is_atoll_dsh_plugin(&deployed));

        // Reinstall must not duplicate the managed block.
        install_dsh_plugin(&home, &source).expect("reinstall");
        let patch = std::fs::read_to_string(home.join(DSH_PATCH_FILENAME)).expect("patch");
        assert_eq!(patch.matches(DSH_PATCH_BEGIN).count(), 1);

        uninstall_dsh_plugin(&home).expect("uninstall");
        let patch = std::fs::read_to_string(home.join(DSH_PATCH_FILENAME)).expect("patch");
        assert_eq!(patch, "[]\n");
        assert!(!deployed.exists());

        let _ = std::fs::remove_dir_all(home);
        let _ = std::fs::remove_dir_all(source);
    }
}
