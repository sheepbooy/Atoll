//! Opt-in local performance fixture. Never installs hooks or writes bridge.json.
use crate::*;
pub(crate) fn enabled() -> bool {
    std::env::var("ATOLL_BENCHMARK").ok().as_deref() == Some("1")
}
pub(crate) fn ensure_packaged_frontend() {
    assert!(!enabled() || !cfg!(dev), "Energy benchmarks require --release --features tauri/custom-protocol; a live dev server invalidates the comparison");
}
pub(crate) fn setup(app: &AppHandle) {
    let state = app.state::<AppState>();
    *lock_state(&state.last_listening_online) = Some(true);
    let mut health = HookHealthSnapshot::default();
    health.codex.installed = true;
    health.codex.script_found = true;
    *lock_state(&state.last_hook_health) = Some(health);
    *lock_state(&state.clipboard_history_enabled) = false;
    *lock_state(&state.media_card_enabled) = false;
    *lock_state(&state.artwork_backdrop_enabled) = false;
    *lock_state(&state.lyrics_enabled) = false;
    let scene = std::env::var("ATOLL_BENCHMARK_SCENE").unwrap_or_else(|_| "idle".into());
    if scene == "working" {
        register_known_session(
            &state,
            "atoll-benchmark",
            AgentKind::Codex,
            "/benchmark",
            None,
        );
        touch_session_activity(&state, "atoll-benchmark");
    }
    if scene == "media" {
        *lock_state(&state.media_card_enabled) = true;
        *lock_state(&state.lyrics_enabled) = true;
        if let Some(path) = std::env::var_os("ATOLL_BENCHMARK_STATUS") {
            let handle = app.clone();
            thread::spawn(move || {
                for _ in 0..60 {
                    thread::sleep(Duration::from_millis(500));
                    let state = handle.state::<AppState>();
                    let track = lock_state(&state.media_state).current();
                    let lyrics = lock_state(&state.lyrics).clone();
                    let ready = track.as_ref().is_some_and(|track| {
                        track.playing
                            && lyrics.as_ref().is_some_and(|lyrics| {
                                !lyrics.lines.is_empty()
                                    && lyrics.track_title == track.title
                                    && lyrics.track_artist == track.artist
                            })
                    });
                    if ready {
                        let _ = std::fs::write(&path, br#"{"mediaAndLyricsReady":true}"#);
                        return;
                    }
                }
                let _ = std::fs::write(&path, br#"{"mediaAndLyricsReady":false}"#);
            });
        }
    }
}
