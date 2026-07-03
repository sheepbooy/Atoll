//! Tracks a content fingerprint of each agent's hook script at the moment Atoll
//! last (re)installed it, so hook health checks can detect when an in-place app
//! update silently changed the script *after* the host CLI already cached a
//! "trusted" snapshot of the old content.
//!
//! Codex in particular pins trust to a hash computed from the hook script the
//! first time the user approves it (via `/hooks`). When Atoll auto-updates and
//! overwrites `atoll-codex-hook.mjs` at the same path, the file on disk changes
//! but nothing prompts the user to re-approve it — Codex just silently stops
//! running the hook. `needs_retrust` gives the UI a way to notice this and tell
//! the user to reconfirm trust, instead of the hook failing invisibly.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
struct AgentHookRecord {
    script_hash: String,
    #[serde(default)]
    installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct HookTrustFile {
    version: u32,
    agents: HashMap<String, AgentHookRecord>,
}

fn state_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_HOOK_TRUST_STATE_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("hook_trust_state.json"))
}

fn empty_file() -> HookTrustFile {
    HookTrustFile {
        version: STATE_VERSION,
        agents: HashMap::new(),
    }
}

fn load() -> HookTrustFile {
    let Some(path) = state_path() else {
        return empty_file();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return empty_file();
    };
    serde_json::from_str(&content).unwrap_or_else(|_| empty_file())
}

fn save(file: &HookTrustFile) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let Ok(formatted) = serde_json::to_string_pretty(file) else {
        return;
    };
    // Best-effort write: a dropped update just means the next check re-derives it.
    let temp_path = path.with_extension("json.tmp");
    if std::fs::write(&temp_path, &formatted).is_ok() {
        let _ = std::fs::rename(&temp_path, &path);
    }
}

/// Cheap content fingerprint. This only needs to detect drift, not resist
/// tampering, so a non-cryptographic hash is fine and avoids a new dependency.
fn fingerprint(script_path: &str) -> Option<String> {
    if script_path.is_empty() {
        return None;
    }
    let bytes = std::fs::read(Path::new(script_path)).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

fn codex_hook_cache_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_CODEX_HOOK_CACHE_PATH") {
        if path.is_empty() {
            return None;
        }
        return Some(PathBuf::from(path));
    }
    dirs::home_dir().map(|home| home.join(".codex").join("hooks").join("atoll-codex-hook.mjs"))
}

fn codex_config_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_CODEX_CONFIG_PATH") {
        if path.is_empty() {
            return None;
        }
        return Some(PathBuf::from(path));
    }
    dirs::home_dir().map(|home| home.join(".codex").join("config.toml"))
}

/// Codex snapshots hook script content into `~/.codex/hooks/` when the user
/// approves trust. If the live script Atoll ships/run has since changed, Codex
/// keeps executing the cached copy for trust validation and silently ignores the
/// updated file — even when hooks.json still points at the same path.
fn codex_cached_script_stale(live_script_path: &str, configured_script_path: Option<&str>) -> bool {
    let Some(cache_path) = codex_hook_cache_path() else {
        return false;
    };
    let Ok(cache_bytes) = std::fs::read(&cache_path) else {
        return false;
    };

    let mut candidates: Vec<&str> = Vec::new();
    if let Some(configured) = configured_script_path {
        if !configured.is_empty() {
            candidates.push(configured);
        }
    }
    if !live_script_path.is_empty() && !candidates.contains(&live_script_path) {
        candidates.push(live_script_path);
    }

    for path in candidates {
        let Ok(live_bytes) = std::fs::read(Path::new(path)) else {
            continue;
        };
        return live_bytes != cache_bytes;
    }
    false
}

/// True when Codex has persisted hook trust entries but we cannot read a live
/// script to compare — usually means hooks.json still references a missing path.
fn codex_trust_state_without_live_script(live_script_path: &str, configured_script_path: Option<&str>) -> bool {
    let Some(config_path) = codex_config_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return false;
    };
    if !content.contains("trusted_hash") || !content.contains("hooks.json") {
        return false;
    }

    let has_live_script = [configured_script_path.unwrap_or(""), live_script_path]
        .into_iter()
        .filter(|path| !path.is_empty())
        .any(|path| Path::new(path).is_file());
    !has_live_script
}

fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

/// Record the hook script fingerprint right after Atoll successfully (re)writes
/// an agent's hook config. This is the moment the user is being asked to go
/// trust that exact content in the host CLI, so it becomes the new baseline.
pub(crate) fn record_hook_installed(agent_key: &str, script_path: &str) {
    let Some(hash) = fingerprint(script_path) else {
        return;
    };
    let mut file = load();
    file.version = STATE_VERSION;
    file.agents.insert(
        agent_key.to_string(),
        AgentHookRecord {
            script_hash: hash,
            installed_at: now_iso(),
        },
    );
    save(&file);
}

const CODEX_HOOK_BRIDGE_NAME: &str = "atoll-hook-bridge.mjs";

fn codex_hook_cache_dir() -> Option<PathBuf> {
    codex_hook_cache_path().and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Codex copies the hook script into `~/.codex/hooks/` when the user approves
/// trust. Reinstalling hooks in Atoll updates hooks.json but leaves that cache
/// stale, so Codex keeps running the old script and Atoll keeps flagging
/// `needs_retrust`. Refresh the cache whenever Atoll (re)installs Codex hooks.
///
/// The hook script imports `./atoll-hook-bridge.mjs` from the same directory, so
/// both files must be copied together — otherwise Codex executes the cached hook
/// and node exits with `ERR_MODULE_NOT_FOUND`.
pub(crate) fn sync_codex_hook_cache(script_path: &str) -> io::Result<()> {
    if script_path.is_empty() {
        return Ok(());
    }
    let live_path = Path::new(script_path);
    if !live_path.is_file() {
        return Ok(());
    }
    let Some(cache_path) = codex_hook_cache_path() else {
        return Ok(());
    };
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(live_path, &cache_path)?;

    let live_bridge = live_path
        .parent()
        .map(|dir| dir.join(CODEX_HOOK_BRIDGE_NAME))
        .filter(|path| path.is_file());
    if let (Some(live_bridge), Some(cache_dir)) = (live_bridge, codex_hook_cache_dir()) {
        std::fs::copy(live_bridge, cache_dir.join(CODEX_HOOK_BRIDGE_NAME))?;
    }
    Ok(())
}

fn codex_cached_bridge_stale(live_script_path: &str) -> bool {
    let live_path = Path::new(live_script_path);
    let Some(live_dir) = live_path.parent() else {
        return false;
    };
    let live_bridge = live_dir.join(CODEX_HOOK_BRIDGE_NAME);
    if !live_bridge.is_file() {
        return false;
    }
    let Some(cache_dir) = codex_hook_cache_dir() else {
        return false;
    };
    let cache_bridge = cache_dir.join(CODEX_HOOK_BRIDGE_NAME);
    let Ok(live_bytes) = std::fs::read(live_bridge) else {
        return false;
    };
    let Ok(cache_bytes) = std::fs::read(cache_bridge) else {
        return true;
    };
    live_bytes != cache_bytes
}

/// Called after a successful Codex hook install: record baseline and refresh the
/// Codex-side script cache so reinstall clears stale re-trust warnings.
pub(crate) fn on_codex_hooks_installed(script_path: &str) {
    record_hook_installed("codex", script_path);
    if let Err(error) = sync_codex_hook_cache(script_path) {
        eprintln!("Atoll failed to refresh Codex hook script cache: {error}");
    }
}

/// Drop any stored baseline for an agent, e.g. after the user uninstalls its hook.
pub(crate) fn clear_hook_installed(agent_key: &str) {
    let mut file = load();
    if file.agents.remove(agent_key).is_some() {
        save(&file);
    }
}

/// True when the live hook script content no longer matches the fingerprint
/// recorded at the last successful install — i.e. Atoll updated the script after
/// the user last confirmed/trusted it, so the host CLI may be silently ignoring it.
pub(crate) fn needs_retrust(
    agent_key: &str,
    script_path: &str,
    configured_script_path: Option<&str>,
) -> bool {
    if agent_key == "codex" {
        if codex_cached_script_stale(script_path, configured_script_path) {
            return true;
        }
        if codex_cached_bridge_stale(script_path) {
            return true;
        }
        if codex_trust_state_without_live_script(script_path, configured_script_path) {
            return true;
        }
    }

    let Some(current_hash) = fingerprint(script_path) else {
        return agent_key == "codex"
            && codex_trust_state_without_live_script(script_path, configured_script_path);
    };
    let mut file = load();
    match file.agents.get(agent_key) {
        Some(record) if !record.script_hash.is_empty() => record.script_hash != current_hash,
        _ => {
            // Codex may already have a trusted snapshot that differs from the live
            // script even on first observation — do not silently adopt a baseline.
            if agent_key == "codex"
                && (codex_cached_script_stale(script_path, configured_script_path)
                    || codex_cached_bridge_stale(script_path))
            {
                return true;
            }
            file.version = STATE_VERSION;
            file.agents.insert(
                agent_key.to_string(),
                AgentHookRecord {
                    script_hash: current_hash,
                    installed_at: now_iso(),
                },
            );
            save(&file);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_path_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Runs `f` with `ATOLL_HOOK_TRUST_STATE_PATH` pointed at a scratch file, under
    /// a process-wide lock so parallel tests don't stomp each other's env var.
    fn with_temp_state<F: FnOnce()>(test_name: &str, f: F) {
        let _guard = state_path_test_lock();
        let pid = std::process::id();
        let path =
            std::env::temp_dir().join(format!("atoll-hook-trust-state-{pid}-{test_name}.json"));
        let _ = std::fs::remove_file(&path);
        std::env::set_var("ATOLL_HOOK_TRUST_STATE_PATH", path.to_string_lossy().as_ref());
        std::env::set_var("ATOLL_CODEX_HOOK_CACHE_PATH", "");
        std::env::set_var("ATOLL_CODEX_CONFIG_PATH", "");
        f();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
        std::env::remove_var("ATOLL_HOOK_TRUST_STATE_PATH");
        std::env::remove_var("ATOLL_CODEX_HOOK_CACHE_PATH");
        std::env::remove_var("ATOLL_CODEX_CONFIG_PATH");
    }

    fn write_script(name: &str, contents: &str) -> String {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, contents).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn first_observation_adopts_baseline_without_flagging() {
        with_temp_state("baseline", || {
            let script = write_script("atoll-hook-trust-test-baseline.mjs", "console.log(1)");
            assert!(!needs_retrust("codex", &script, None));
            assert!(!needs_retrust("codex", &script, None));
            let _ = std::fs::remove_file(&script);
        });
    }

    #[test]
    fn flags_when_script_changes_after_install() {
        with_temp_state("drift", || {
            let script = write_script("atoll-hook-trust-test-drift.mjs", "console.log(1)");
            record_hook_installed("codex", &script);
            assert!(!needs_retrust("codex", &script, None));

            std::fs::write(&script, "console.log(2)").unwrap();
            assert!(needs_retrust("codex", &script, None));

            let _ = std::fs::remove_file(&script);
        });
    }

    #[test]
    fn reinstalling_clears_previous_drift() {
        with_temp_state("reinstall", || {
            let pid = std::process::id();
            let cache_path = std::env::temp_dir().join(format!(
                "atoll-codex-hook-cache-{pid}-reinstall.mjs"
            ));
            let _ = std::fs::remove_file(&cache_path);
            std::env::set_var(
                "ATOLL_CODEX_HOOK_CACHE_PATH",
                cache_path.to_string_lossy().as_ref(),
            );
            let script = write_script("atoll-hook-trust-test-reinstall.mjs", "console.log(1)");
            std::fs::write(&cache_path, "console.log('stale')").unwrap();
            record_hook_installed("codex", &script);
            std::fs::write(&script, "console.log(2)").unwrap();
            assert!(needs_retrust("codex", &script, None));

            on_codex_hooks_installed(&script);
            assert!(!needs_retrust("codex", &script, None));

            let _ = std::fs::remove_file(&script);
            let _ = std::fs::remove_file(&cache_path);
        });
    }

    #[test]
    fn clearing_removes_the_stored_baseline() {
        with_temp_state("clear", || {
            let script = write_script("atoll-hook-trust-test-clear.mjs", "console.log(1)");
            record_hook_installed("codex", &script);
            clear_hook_installed("codex");
            std::fs::write(&script, "console.log(2)").unwrap();
            // No baseline anymore, so this reads as a fresh first observation.
            assert!(!needs_retrust("codex", &script, None));

            let _ = std::fs::remove_file(&script);
        });
    }

    #[test]
    fn missing_script_never_flags_retrust() {
        with_temp_state("missing", || {
            assert!(!needs_retrust("codex", "/definitely/not/a/real/path.mjs", None));
        });
    }

    #[test]
    fn agents_are_tracked_independently() {
        with_temp_state("multi-agent", || {
            let codex_script =
                write_script("atoll-hook-trust-test-multi-codex.mjs", "console.log('codex')");
            let claude_script = write_script(
                "atoll-hook-trust-test-multi-claude.mjs",
                "console.log('claude')",
            );
            record_hook_installed("codex", &codex_script);
            record_hook_installed("claude", &claude_script);

            std::fs::write(&codex_script, "console.log('codex v2')").unwrap();
            assert!(needs_retrust("codex", &codex_script, None));
            assert!(!needs_retrust("claude", &claude_script, None));

            let _ = std::fs::remove_file(&codex_script);
            let _ = std::fs::remove_file(&claude_script);
        });
    }

    #[test]
    fn codex_sync_copies_bridge_module_alongside_hook() {
        with_temp_state("codex-bridge-sync", || {
            let pid = std::process::id();
            let cache_dir = std::env::temp_dir().join(format!("atoll-codex-hooks-cache-{pid}"));
            let _ = std::fs::remove_dir_all(&cache_dir);
            std::env::set_var(
                "ATOLL_CODEX_HOOK_CACHE_PATH",
                cache_dir
                    .join("atoll-codex-hook.mjs")
                    .to_string_lossy()
                    .as_ref(),
            );

            let script_dir = std::env::temp_dir().join(format!("atoll-codex-live-{pid}"));
            let _ = std::fs::create_dir_all(&script_dir);
            std::fs::write(
                script_dir.join("atoll-codex-hook.mjs"),
                "import './atoll-hook-bridge.mjs'",
            )
            .unwrap();
            std::fs::write(
                script_dir.join("atoll-hook-bridge.mjs"),
                "export function resolveHookUrl() {}",
            )
            .unwrap();
            let script = script_dir
                .join("atoll-codex-hook.mjs")
                .to_string_lossy()
                .into_owned();

            sync_codex_hook_cache(&script).unwrap();
            assert!(cache_dir.join("atoll-hook-bridge.mjs").is_file());

            let _ = std::fs::remove_dir_all(&cache_dir);
            let _ = std::fs::remove_dir_all(&script_dir);
        });
    }

    #[test]
    fn codex_flags_when_cached_bridge_module_is_missing() {
        with_temp_state("codex-bridge-missing", || {
            let pid = std::process::id();
            let cache_dir = std::env::temp_dir().join(format!("atoll-codex-hooks-cache-{pid}"));
            let _ = std::fs::remove_dir_all(&cache_dir);
            std::env::set_var(
                "ATOLL_CODEX_HOOK_CACHE_PATH",
                cache_dir
                    .join("atoll-codex-hook.mjs")
                    .to_string_lossy()
                    .as_ref(),
            );

            let script_dir = std::env::temp_dir().join(format!("atoll-codex-live-{pid}"));
            let _ = std::fs::create_dir_all(&script_dir);
            let live = script_dir.join("atoll-codex-hook.mjs");
            std::fs::write(&live, "import './atoll-hook-bridge.mjs'").unwrap();
            std::fs::write(
                script_dir.join("atoll-hook-bridge.mjs"),
                "export function resolveHookUrl() {}",
            )
            .unwrap();
            std::fs::create_dir_all(&cache_dir).unwrap();
            std::fs::write(cache_dir.join("atoll-codex-hook.mjs"), "import './atoll-hook-bridge.mjs'")
                .unwrap();

            let script = live.to_string_lossy().into_owned();
            assert!(needs_retrust("codex", &script, Some(&script)));

            let _ = std::fs::remove_dir_all(&cache_dir);
            let _ = std::fs::remove_dir_all(&script_dir);
        });
    }

    #[test]
    fn codex_flags_when_cached_snapshot_differs_from_live_script() {
        with_temp_state("codex-cache", || {
            let pid = std::process::id();
            let cache_path = std::env::temp_dir().join(format!(
                "atoll-codex-hook-cache-{pid}-stale.mjs"
            ));
            let _ = std::fs::remove_file(&cache_path);
            std::env::set_var(
                "ATOLL_CODEX_HOOK_CACHE_PATH",
                cache_path.to_string_lossy().as_ref(),
            );
            let live =
                write_script("atoll-hook-trust-test-live-codex.mjs", "console.log('live')");
            std::fs::write(&cache_path, "console.log('cached')").unwrap();
            assert!(needs_retrust("codex", &live, Some(&live)));
            let _ = std::fs::remove_file(&live);
            let _ = std::fs::remove_file(&cache_path);
        });
    }

    #[test]
    fn codex_does_not_flag_when_cache_matches_live_script() {
        with_temp_state("codex-cache-match", || {
            let pid = std::process::id();
            let cache_path = std::env::temp_dir().join(format!(
                "atoll-codex-hook-cache-{pid}-match.mjs"
            ));
            let _ = std::fs::remove_file(&cache_path);
            std::env::set_var(
                "ATOLL_CODEX_HOOK_CACHE_PATH",
                cache_path.to_string_lossy().as_ref(),
            );
            let live = write_script(
                "atoll-hook-trust-test-live-codex-match.mjs",
                "console.log('same')",
            );
            std::fs::write(&cache_path, "console.log('same')").unwrap();
            assert!(!needs_retrust("codex", &live, Some(&live)));
            let _ = std::fs::remove_file(&live);
            let _ = std::fs::remove_file(&cache_path);
        });
    }
}
