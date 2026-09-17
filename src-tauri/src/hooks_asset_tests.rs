use super::*;

#[test]
fn deployed_hook_assets_current_checks_script_and_bridge_module() {
    let dir = std::env::temp_dir().join(format!(
        "atoll-hook-assets-current-{}",
        uuid::Uuid::new_v4()
    ));
    let source_dir = dir.join("source");
    let deployed_dir = dir.join("deployed");
    std::fs::create_dir_all(&source_dir).expect("source dir");
    std::fs::create_dir_all(&deployed_dir).expect("deployed dir");
    let source_script = source_dir.join("atoll-codex-hook.mjs");
    let deployed_script = deployed_dir.join("atoll-codex-hook.mjs");
    let source_bridge = source_dir.join("atoll-hook-bridge.mjs");
    let deployed_bridge = deployed_dir.join("atoll-hook-bridge.mjs");

    std::fs::write(&source_script, "new script").expect("source script");
    std::fs::write(&deployed_script, "old script").expect("deployed script");
    std::fs::write(&source_bridge, "new bridge").expect("source bridge");
    std::fs::write(&deployed_bridge, "old bridge").expect("deployed bridge");

    assert!(!deployed_hook_assets_current(
        &source_script,
        &deployed_script
    ));

    std::fs::write(&deployed_script, "new script").expect("deployed script update");
    assert!(!deployed_hook_assets_current(
        &source_script,
        &deployed_script
    ));

    std::fs::write(&deployed_bridge, "new bridge").expect("deployed bridge update");
    assert!(deployed_hook_assets_current(
        &source_script,
        &deployed_script
    ));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn empty_hook_scripts_are_not_usable() {
    let dir = std::env::temp_dir().join(format!("atoll-empty-hook-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let empty = dir.join("atoll-claude-hook.mjs");
    std::fs::write(&empty, []).expect("empty script");
    assert!(!hook_script_is_usable(&empty));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn repo_hook_scripts_are_usable_install_sources() {
    for name in [
        "atoll-claude-hook.mjs",
        "atoll-codex-hook.mjs",
        "atoll-cursor-hook.mjs",
        "atoll-hook-bridge.mjs",
    ] {
        let path = repo_hook_script_path(name);
        assert!(
            hook_script_is_usable(&path),
            "missing usable repo hook script {}",
            path.display()
        );
    }
}

#[test]
fn install_source_skips_empty_files_and_finds_repo_script() {
    let dir = std::env::temp_dir().join(format!("atoll-hook-source-{}", uuid::Uuid::new_v4()));
    let scripts = dir.join("scripts");
    std::fs::create_dir_all(&scripts).expect("scripts dir");
    let empty = scripts.join("atoll-claude-hook.mjs");
    std::fs::write(&empty, []).expect("empty local copy");

    let found = first_usable_hook_script(bundled_hook_script_candidates(
        Some(dir.as_path()),
        None,
        "atoll-claude-hook.mjs",
    ));
    assert!(found.is_some());
    let found = found.expect("repo script");
    assert!(
        found.ends_with("scripts/atoll-claude-hook.mjs")
            || found.ends_with("scripts\\atoll-claude-hook.mjs")
    );
    assert_ne!(
        std::fs::metadata(&found).expect("meta").len(),
        0,
        "install source must not be the empty local copy"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn copy_deployed_hook_file_does_not_truncate_when_source_is_destination() {
    let dir = std::env::temp_dir().join(format!("atoll-hook-self-copy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let script = dir.join("atoll-claude-hook.mjs");
    std::fs::write(&script, "keep me").expect("script");
    copy_deployed_hook_file(&script, &script, "hook script").expect("self copy");
    assert_eq!(std::fs::read_to_string(&script).expect("read"), "keep me");
    let _ = std::fs::remove_dir_all(dir);
}
