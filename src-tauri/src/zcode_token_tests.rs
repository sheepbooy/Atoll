use super::{
    refresh_session_token_usage, update_zcode_subagents, zcode_rollout_path, AgentKind,
    TOKEN_HISTORY_ENV_LOCK,
};
use crate::core_tests::test_app_state;
use serde_json::{json, Value};

const PARENT_SESSION: &str = "sess_11111111-2222-4333-8444-555555555555";
const CHILD_SESSION: &str = "sess_subagent_agent_66666666-7777-4888-9999-000000000000";

/// Fixture timestamps must be "now": the token parser drops rollout lines
/// whose local day differs from the current one, so a fixed date would go
/// stale as soon as the calendar moves past it.
fn today_iso() -> String {
    crate::iso_timestamp_now()
}

fn zcode_line(session: &str, iso: &str, model: &str, usage: Value) -> String {
    json!({
        "type": "model_io",
        "sessionId": session,
        "completedAt": iso,
        "model": { "modelId": model },
        "response": { "usage": usage }
    })
    .to_string()
}

fn today_key() -> String {
    crate::local_time::local_day_key_from_iso(&today_iso()).expect("local day key")
}

/// Redirect HOME into a temp dir so `zcode_rollout_path` lands in fixtures.
/// Serialized by TOKEN_HISTORY_ENV_LOCK like the other env-mutating tests.
pub(crate) struct HomeGuard {
    previous: Option<std::ffi::OsString>,
    pub(crate) home: std::path::PathBuf,
}

impl HomeGuard {
    pub(crate) fn new(tag: &str) -> Self {
        let home =
            std::env::temp_dir().join(format!("atoll-zcode-token-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).expect("create temp home");
        let previous = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);
        let history_path = home.join("token-history.json");
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );
        Self { previous, home }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => std::env::set_var("HOME", previous),
            None => std::env::remove_var("HOME"),
        }
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn write_parent_rollout(home: &std::path::Path) -> std::path::PathBuf {
    let rollout = home
        .join(".zcode")
        .join("cli")
        .join("rollout")
        .join(format!("model-io-{PARENT_SESSION}.jsonl"));
    std::fs::create_dir_all(rollout.parent().unwrap()).expect("create rollout dir");
    std::fs::write(
            &rollout,
            format!(
                "{}\n{}\n",
                zcode_line(
                    PARENT_SESSION,
                    &today_iso(),
                    "GLM-5.3",
                    json!({ "inputTokens": 38758, "outputTokens": 147, "cacheReadTokens": 38400, "cacheWriteTokens": 12 })
                ),
                zcode_line(
                    PARENT_SESSION,
                    &today_iso(),
                    "GLM-4.7",
                    json!({ "inputTokens": 232, "outputTokens": 77, "cacheReadTokens": 0, "cacheWriteTokens": 0 })
                ),
            ),
        )
        .expect("write parent rollout");
    rollout
}

fn write_subagent_metadata(home: &std::path::Path, status: &str, completed_at: Option<&str>) {
    let mut metadata = json!({
        "parentSessionId": PARENT_SESSION,
        "childSessionId": CHILD_SESSION,
        "profileSnapshot": { "name": "Explore" },
        "prompt": "Search the codebase for usages",
        "status": status,
        "createdAt": today_iso(),
    });
    if let Some(completed_at) = completed_at {
        metadata["completedAt"] = json!(completed_at);
    }
    let metadata_path = home
        .join(".zcode")
        .join("cli")
        .join("agents")
        .join(PARENT_SESSION)
        .join("agent_abc123")
        .join("metadata.json");
    std::fs::create_dir_all(metadata_path.parent().unwrap()).expect("create agents dir");
    std::fs::write(&metadata_path, metadata.to_string()).expect("write metadata");
}

fn write_child_rollout(home: &std::path::Path) {
    let rollout = home
        .join(".zcode")
        .join("cli")
        .join("rollout")
        .join(format!("model-io-{CHILD_SESSION}.jsonl"));
    std::fs::create_dir_all(rollout.parent().unwrap()).expect("create rollout dir");
    std::fs::write(
            &rollout,
            format!(
                "{}\n",
                zcode_line(
                    CHILD_SESSION,
                    &today_iso(),
                    "GLM-5.3",
                    json!({ "inputTokens": 34456, "outputTokens": 2992, "cacheReadTokens": 32320, "cacheWriteTokens": 0 })
                ),
            ),
        )
        .expect("write child rollout");
}

#[test]
fn zcode_rollout_path_rejects_unsafe_session_ids() {
    let valid =
        zcode_rollout_path("sess_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b").expect("valid session id");
    assert_eq!(
        valid.file_name().unwrap().to_string_lossy(),
        "model-io-sess_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b.jsonl"
    );
    assert!(
        zcode_rollout_path("sess_subagent_agent_6ea9e07c-3ff6-4ca8-9e02-8b24a06b401b").is_some()
    );
    for bad in [
        "../escape",
        "sess_../../escape",
        "/absolute/path",
        "sess_ with space",
        "claude",
        "",
    ] {
        assert!(
            zcode_rollout_path(bad).is_none(),
            "expected rejection: {bad}"
        );
    }
}

#[test]
fn zcode_refresh_parses_rollout_from_session_id() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let home = HomeGuard::new("refresh");

    let rollout = write_parent_rollout(&home.home);
    let state = test_app_state();

    // The hook payload's transcript path is an ephemeral temp file; the
    // rollout path derived from the session id must be used instead.
    refresh_session_token_usage(
        &state,
        PARENT_SESSION,
        Some("/tmp/atoll-ephemeral-hook-transcript.jsonl"),
        Some(&AgentKind::Zcode),
    )
    .expect("refresh");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 358 + 232);
    assert_eq!(usage.output_tokens, 147 + 77);
    assert_eq!(usage.cache_read_tokens, 38400);
    assert_eq!(usage.cache_creation_tokens, 12);

    let by_model = state
        .session_token_usage_by_model
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .expect("by-model usage")
        .clone();
    assert_eq!(by_model.get("GLM-5.3").unwrap().input_tokens, 358);
    assert_eq!(by_model.get("GLM-4.7").unwrap().output_tokens, 77);

    let offsets = state.token_usage_file_offsets.lock().expect("lock");
    let stored = offsets
        .get(rollout.to_string_lossy().as_ref())
        .copied()
        .expect("rollout offset");
    assert_eq!(stored, std::fs::metadata(&rollout).unwrap().len());
    assert!(state
        .session_agent_map
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .map(|agent| agent == "zcode")
        .unwrap_or(false));
}

#[test]
fn zcode_refresh_tolerates_missing_rollout() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let _home = HomeGuard::new("missing-rollout");
    let state = test_app_state();

    refresh_session_token_usage(&state, PARENT_SESSION, None, Some(&AgentKind::Zcode))
        .expect("refresh without rollout file");

    // A zero entry may be registered, but nothing was counted.
    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .copied()
        .unwrap_or_default();
    assert!(usage.is_zero());
}

#[test]
fn zcode_subagent_usage_and_chips_follow_metadata() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let home = HomeGuard::new("subagent");
    let state = test_app_state();

    write_child_rollout(&home.home);
    write_subagent_metadata(&home.home, "running", None);

    let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
    assert!(changed, "running subagent should register a chip");

    {
        let usage = state
            .session_token_usage
            .lock()
            .expect("lock")
            .get(PARENT_SESSION)
            .copied()
            .expect("parent usage");
        assert_eq!(usage.input_tokens, 34456 - 32320);
        assert_eq!(usage.output_tokens, 2992);
        assert_eq!(usage.cache_read_tokens, 32320);
    }
    {
        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(subagents.len(), 1);
        let chip = &subagents[0];
        assert_eq!(chip.agent_id, CHILD_SESSION);
        assert_eq!(chip.session_id, PARENT_SESSION);
        assert_eq!(chip.agent_type, "Explore");
        assert!(matches!(chip.agent_kind, AgentKind::Zcode));
        assert!(chip.completed_at.is_none());
        assert_eq!(
            chip.last_message.as_deref(),
            Some("Search the codebase for usages")
        );
    }

    // Subagent completes: chip closes, tokens must not be counted twice.
    write_subagent_metadata(&home.home, "completed", Some(&today_iso()));
    let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
    assert!(changed, "completion should close the chip");
    {
        let subagents = state.active_subagents.lock().expect("lock");
        assert_eq!(subagents.len(), 1);
        assert!(subagents[0].completed_at.is_some());
    }
    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .copied()
        .expect("parent usage");
    assert_eq!(usage.input_tokens, 34456 - 32320);

    // Third pass: nothing new, nothing changed.
    let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
    assert!(!changed);
}

#[test]
fn zcode_historical_subagents_do_not_become_chips() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let home = HomeGuard::new("historical");
    let state = test_app_state();

    write_child_rollout(&home.home);
    write_subagent_metadata(&home.home, "completed", Some(&today_iso()));

    let changed = update_zcode_subagents(&state, PARENT_SESSION, &today_key());
    assert!(!changed, "finished-before-seen subagents stay invisible");
    assert!(state.active_subagents.lock().expect("lock").is_empty());
    // But their token usage still lands on the parent session.
    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(PARENT_SESSION)
        .copied()
        .expect("parent usage");
    assert_eq!(usage.output_tokens, 2992);
}
