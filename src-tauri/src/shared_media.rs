//! One platform sampler shared by cards, artwork and lyrics.
use crate::*;

pub(crate) struct MediaSnapshot {
    pub(crate) track: Option<NowPlayingTrack>,
    sampled_at: Instant,
    track_generation: u64,
    previous_raw: Option<f64>,
    held: Option<f64>,
}
impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            track: None,
            sampled_at: Instant::now(),
            track_generation: 0,
            previous_raw: None,
            held: None,
        }
    }
}
impl MediaSnapshot {
    pub(crate) fn current(&self) -> Option<NowPlayingTrack> {
        self.track.clone().map(|mut track| {
            if track.playing {
                track.position = track.position.map(|p| {
                    (p + self.sampled_at.elapsed().as_secs_f64())
                        .min(track.duration.unwrap_or(f64::MAX))
                });
            }
            track
        })
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs((1u64 << attempt.min(5)).min(30))
}
fn enabled(state: &AppState) -> bool {
    *lock_state(&state.media_card_enabled)
        || *lock_state(&state.artwork_backdrop_enabled)
        || *lock_state(&state.lyrics_enabled)
}
fn wants_progress(state: &AppState) -> bool {
    *lock_state(&state.media_card_enabled) || *lock_state(&state.lyrics_enabled)
}
pub(crate) fn track_key(track: &NowPlayingTrack) -> String {
    serde_json::json!([track.artist, track.title, track.album, track.duration]).to_string()
}
impl MediaSnapshot {
    fn update(&mut self, mut track: Option<NowPlayingTrack>) -> bool {
        let old = self.track.as_ref();
        let new = track.as_ref();
        let changed = match (old, new) {
            (Some(a), Some(b)) => {
                track_key(a) != track_key(b)
                    || a.playing != b.playing
                    || a.app != b.app
                    || a.artwork_base64 != b.artwork_base64
            }
            (None, None) => false,
            _ => true,
        };
        let playing_changed = old.map(|t| t.playing) != new.map(|t| t.playing);
        if old.map(track_key) != new.map(track_key) {
            self.track_generation = self.track_generation.wrapping_add(1);
            self.previous_raw = None;
            self.held = None;
        }
        if playing_changed {
            self.previous_raw = None;
            self.held = None;
        }
        if let Some(ref mut current) = track {
            let raw = current.position;
            current.position =
                sanitize_paused_position(raw, current.playing, self.previous_raw, &mut self.held);
            self.previous_raw = raw;
        }
        self.track = track;
        self.sampled_at = Instant::now();
        changed
    }
    fn matches_query(&self, key: &str, generation: u64) -> bool {
        self.track_generation == generation
            && self.track.as_ref().map(track_key).as_deref() == Some(key)
    }
}
fn publish(app: &AppHandle, track: Option<NowPlayingTrack>) {
    let state = app.state::<AppState>();
    let mut media = lock_state(&state.media_state);
    let changed = media.update(track);
    let track = media.track.clone();
    drop(media);
    // Seek/pause/resume must not wait for an old five-second deadline.
    if let Some(ref track) = track.as_ref().filter(|_| wants_progress(&state)) {
        let _ = app.emit(
            "now-playing-position",
            serde_json::json!({"position":track.position,"playing":track.playing}),
        );
    }
    if changed {
        let _ = app.emit("now-playing-changed", &track);
    }
}

pub(crate) fn start(app: AppHandle) {
    thread::spawn(move || {
        let mut active = false;
        let mut next_poll = Instant::now();
        let mut next_position = Instant::now();
        #[cfg(target_os = "macos")]
        let mut stream: Option<crate::media::MediaStream> = None;
        #[cfg(target_os = "macos")]
        let mut next_restart = Instant::now();
        #[cfg(target_os = "macos")]
        let mut attempts = 0;
        loop {
            let state = app.state::<AppState>();
            if !enabled(&state) {
                #[cfg(target_os = "macos")]
                {
                    stream = None;
                    attempts = 0;
                    next_restart = Instant::now();
                }
                if active {
                    publish(&app, None);
                }
                active = false;
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            if !active {
                let initial = platform_now_playing();
                let playing = initial.as_ref().is_some_and(|track| track.playing);
                publish(&app, initial);
                active = true;
                next_poll = Instant::now() + Duration::from_secs(if playing { 1 } else { 5 });
            }
            #[cfg(target_os = "macos")]
            {
                if stream.is_none() && Instant::now() >= next_restart {
                    stream = crate::media::start_stream();
                    if stream.is_none() {
                        next_restart = Instant::now() + retry_delay(attempts);
                        attempts = attempts.saturating_add(1);
                    }
                }
                let mut failed = false;
                if let Some(ref source) = stream {
                    loop {
                        match source.updates.try_recv() {
                            Ok(Ok(track)) => {
                                attempts = 0;
                                publish(&app, track);
                            }
                            Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                failed = true;
                                break;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        }
                    }
                }
                if failed {
                    stream = None;
                    next_restart = Instant::now() + retry_delay(attempts);
                    attempts = attempts.saturating_add(1);
                }
            }
            #[cfg(target_os = "macos")]
            let polling = stream.is_none();
            #[cfg(not(target_os = "macos"))]
            let polling = true;
            if polling && Instant::now() >= next_poll {
                let current = platform_now_playing();
                let playing = current.as_ref().is_some_and(|t| t.playing);
                publish(&app, current);
                next_poll = Instant::now() + Duration::from_secs(if playing { 1 } else { 5 });
            }
            if Instant::now() >= next_position {
                if let Some(track) = lock_state(&state.media_state)
                    .current()
                    .filter(|_| wants_progress(&state))
                {
                    let _ = app.emit(
                        "now-playing-position",
                        serde_json::json!({"position": track.position, "playing": track.playing}),
                    );
                    next_position =
                        Instant::now() + Duration::from_secs(if track.playing { 1 } else { 5 });
                } else {
                    next_position = Instant::now() + Duration::from_secs(5);
                }
            }
            thread::sleep(Duration::from_millis(250));
        }
    });
}

fn clear_lyrics(app: &AppHandle, state: &AppState) {
    let had = lock_state(&state.lyrics).take().is_some();
    lock_state(&state.lyrics_track_key).clear();
    if had {
        let _ = app.emit("lyrics-changed", Option::<lyrics::LyricPayload>::None);
    }
}
pub(crate) fn start_lyrics(app: AppHandle) {
    thread::spawn(move || {
        let mut misses = HashMap::<String, Instant>::new();
        loop {
            thread::sleep(Duration::from_secs(1));
            let state = app.state::<AppState>();
            if !*lock_state(&state.lyrics_enabled) {
                clear_lyrics(&app, &state);
                continue;
            }
            let (track, generation) = {
                let media = lock_state(&state.media_state);
                (media.current(), media.track_generation)
            };
            let Some(track) = track else {
                clear_lyrics(&app, &state);
                continue;
            };
            let key = track_key(&track);
            if *lock_state(&state.lyrics_track_key) == key {
                continue;
            }
            clear_lyrics(&app, &state);
            if misses
                .get(&key)
                .is_some_and(|at| at.elapsed() < LYRICS_MISS_RETRY_AFTER)
            {
                continue;
            }
            let lines = lyrics::fetch_cached_lyrics(&key, &track);
            // Media advances independently while the network lookup blocks.
            if !*lock_state(&state.lyrics_enabled)
                || !lock_state(&state.media_state).matches_query(&key, generation)
            {
                continue;
            }
            match lines {
                Some(lines) if !lines.is_empty() => {
                    // Keep the media generation stable through adoption: a
                    // switch between validation and payload construction must
                    // never label old lines with the new track's title.
                    let media = lock_state(&state.media_state);
                    let lyrics_enabled = lock_state(&state.lyrics_enabled);
                    if !*lyrics_enabled || !media.matches_query(&key, generation) {
                        continue;
                    }
                    let current = media.current().unwrap_or(track);
                    let idx = lyrics::current_line_index(&lines, current.position.unwrap_or(0.0));
                    let payload = lyrics::LyricPayload {
                        current_index: idx,
                        next_time_ms: lines.get(idx + 1).map(|l| l.time_ms),
                        lines,
                        track_title: current.title,
                        track_artist: current.artist,
                    };
                    *lock_state(&state.lyrics_track_key) = key;
                    *lock_state(&state.lyrics) = Some(payload.clone());
                    drop(lyrics_enabled);
                    drop(media);
                    let _ = app.emit("lyrics-changed", &payload);
                }
                _ => {
                    misses.retain(|_, at| at.elapsed() < LYRICS_MISS_RETRY_AFTER);
                    misses.insert(key, Instant::now());
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(title: &str, position: f64, playing: bool) -> NowPlayingTrack {
        NowPlayingTrack {
            title: Some(title.into()),
            artist: Some("fixture".into()),
            album: None,
            duration: Some(120.0),
            position: Some(position),
            playing,
            artwork_base64: None,
            app: None,
        }
    }
    #[test]
    fn unchanged_metadata_does_not_republish_but_seek_and_pause_update_shared_clock() {
        let mut media = MediaSnapshot::default();
        assert!(media.update(Some(track("a", 10.0, true))));
        assert!(!media.update(Some(track("a", 40.0, true))));
        assert!(media.current().unwrap().position.unwrap() >= 40.0);
        assert!(media.update(Some(track("a", 41.0, false))));
        media.sampled_at = Instant::now() - Duration::from_secs(10);
        assert_eq!(media.current().unwrap().position, Some(41.0));
        assert!(!media.update(Some(track("a", 80.0, false))));
        assert_eq!(media.current().unwrap().position, Some(80.0));
        assert!(media.update(Some(track("a", 80.0, true))));
    }
    #[test]
    fn quick_track_switch_discards_old_query_even_when_switching_back() {
        let mut media = MediaSnapshot::default();
        media.update(Some(track("a", 10.0, true)));
        let key = track_key(media.track.as_ref().unwrap());
        let generation = media.track_generation;
        assert!(media.matches_query(&key, generation));
        media.update(Some(track("b", 0.0, true)));
        media.update(Some(track("a", 0.0, true)));
        assert!(!media.matches_query(&key, generation));
        media.update(None);
        assert!(media.current().is_none());
        assert!(!media.matches_query(&key, media.track_generation));
    }
    #[test]
    fn restart_backoff_is_bounded() {
        assert_eq!(
            (0..7).map(|n| retry_delay(n).as_secs()).collect::<Vec<_>>(),
            vec![1, 2, 4, 8, 16, 30, 30]
        );
    }
    #[test]
    fn all_consumers_off_stops_sampling() {
        let state = crate::core_tests::test_app_state();
        *lock_state(&state.media_card_enabled) = false;
        assert!(!enabled(&state));
        *lock_state(&state.lyrics_enabled) = true;
        assert!(enabled(&state));
    }
}
