//! macOS Now Playing via the MediaRemoteAdapter framework.
//!
//! macOS 26 blocks third-party (non-arm64e) apps from reading MediaRemote
//! private-framework data. The BSD-3-licensed `MediaRemoteAdapter.framework`
//! (precompiled universal binary including arm64e) bypasses this, exposing
//! plain C entry points (`adapter_get`, `adapter_send`). We invoke it through
//! the bundled `mediaremote-adapter.pl` script which prints JSON to stdout.

#![cfg(target_os = "macos")]

use serde::{Deserialize, Serialize};
use std::os::raw::c_int;
use std::process::{Command, Stdio};

/// MRCommand values, verified empirically against macOS 26.
/// Note: value 3 behaves as a second pause/stop; next/prev are 4/5, not 3/4.
pub const MR_COMMAND_PLAY: c_int = 0;
pub const MR_COMMAND_PAUSE: c_int = 1;
pub const MR_COMMAND_TOGGLE: c_int = 2;
pub const MR_COMMAND_NEXT: c_int = 4;
pub const MR_COMMAND_PREV: c_int = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingTrack {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub playing: bool,
    pub artwork_base64: Option<String>,
    pub app: Option<String>,
}

/// Raw JSON shape from the adapter (different field names).
#[derive(Deserialize)]
struct AdapterPayload {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    #[serde(rename = "elapsedTime")]
    elapsed_time: Option<f64>,
    playing: Option<bool>,
    #[serde(rename = "artworkData")]
    artwork_data: Option<String>,
    #[serde(rename = "bundleIdentifier")]
    bundle_identifier: Option<String>,
}

/// Resolve the resource dir: in dev mode it's `src-tauri/resources/media`,
/// in a bundled app it's the Tauri resource dir.
fn adapter_paths() -> Option<(String, String)> {
    // Bundled app: use the executable's dir / resources.
    if let Ok(exe) = std::env::current_exe() {
        // Tauri places resources under `Resources/media` on macOS .app bundles.
        for candidate in [
            exe.parent()?.join("resources").join("media"),
            exe.parent()?.parent()?.join("Resources").join("media"),
        ] {
            let script = candidate.join("mediaremote-adapter.pl");
            let framework = candidate.join("MediaRemoteAdapter.framework");
            if script.exists() && framework.exists() {
                return Some((
                    script.to_string_lossy().into_owned(),
                    framework.to_string_lossy().into_owned(),
                ));
            }
        }
    }
    // Dev mode: resolve from the manifest dir (CARGO_MANIFEST_DIR env set by cargo).
    let manifest = option_env!("CARGO_MANIFEST_DIR")?;
    let dev = std::path::Path::new(manifest)
        .join("resources")
        .join("media");
    let script = dev.join("mediaremote-adapter.pl");
    let framework = dev.join("MediaRemoteAdapter.framework");
    if script.exists() && framework.exists() {
        Some((
            script.to_string_lossy().into_owned(),
            framework.to_string_lossy().into_owned(),
        ))
    } else {
        None
    }
}

/// Fetch the current Now Playing track by invoking the adapter script once.
/// Returns None if the adapter is unavailable or no media is playing.
pub fn fetch_now_playing() -> Option<NowPlayingTrack> {
    let (script, framework) = adapter_paths()?;
    let output = Command::new("/usr/bin/perl")
        .arg(&script)
        .arg(&framework)
        .arg("get")
        .arg("--now")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let payload: AdapterPayload = serde_json::from_slice(&output.stdout).ok()?;
    Some(track_from_payload(payload))
}

fn track_from_payload(payload: AdapterPayload) -> NowPlayingTrack {
    NowPlayingTrack {
        title: payload.title,
        artist: payload.artist,
        album: payload.album,
        duration: payload.duration,
        position: payload.elapsed_time,
        playing: payload.playing.unwrap_or(false),
        artwork_base64: payload.artwork_data,
        app: app_name_from_bundle(payload.bundle_identifier.as_deref()),
    }
}

/// Send a media command. Returns false if the adapter is unavailable.
pub fn send_media_command_raw(command: c_int) -> bool {
    let Some((script, framework)) = adapter_paths() else {
        return false;
    };
    let _ = Command::new("/usr/bin/perl")
        .arg(&script)
        .arg(&framework)
        .arg("send")
        .arg(command.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    true
}

/// Map a bundle identifier to a friendly app name.
fn app_name_from_bundle(bundle: Option<&str>) -> Option<String> {
    let b = bundle?;
    let name = match b {
        "com.apple.Music" => "Music",
        "com.apple.Podcasts" => "Podcasts",
        "com.spotify.client" => "Spotify",
        "com.tencent.QQMusicMac" => "QQ Music",
        "com.kugou.kugou-mac" => "KuGou",
        "com.netease.163music" => "NetEase Music",
        _ => return Some(b.to_string()),
    };
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_playing_track_serializes_with_camel_case() {
        let track = NowPlayingTrack {
            title: Some("Test".into()),
            artist: Some("Artist".into()),
            album: None,
            duration: Some(180.0),
            position: Some(45.0),
            playing: true,
            artwork_base64: Some("aGVsbG8=".into()),
            app: Some("Music".into()),
        };
        let json = serde_json::to_string(&track).unwrap();
        assert!(json.contains("\"artworkBase64\""));
        assert!(json.contains("\"title\""));
        assert!(!json.contains("\"artwork_base64\""));
    }

    #[test]
    fn now_playing_track_handles_empty_fields() {
        let track = NowPlayingTrack {
            title: None,
            artist: None,
            album: None,
            duration: None,
            position: None,
            playing: false,
            artwork_base64: None,
            app: None,
        };
        let json = serde_json::to_string(&track).unwrap();
        assert!(json.contains("\"title\":null"));
        assert!(json.contains("\"playing\":false"));
    }

    #[test]
    fn parses_adapter_payload() {
        let json = r#"{"title":"Song","artist":"A","album":"Al","duration":200,"elapsedTime":50,"playing":true,"artworkData":"aGk=","bundleIdentifier":"com.apple.Music"}"#;
        let payload: AdapterPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.title.as_deref(), Some("Song"));
        assert_eq!(payload.elapsed_time, Some(50.0));
        assert!(payload.playing.unwrap_or(false));
    }

    #[test]
    fn app_name_maps_known_bundles() {
        assert_eq!(
            app_name_from_bundle(Some("com.apple.Music")),
            Some("Music".into())
        );
        assert_eq!(
            app_name_from_bundle(Some("com.spotify.client")),
            Some("Spotify".into())
        );
        assert_eq!(
            app_name_from_bundle(Some("com.tencent.QQMusicMac")),
            Some("QQ Music".into())
        );
        assert_eq!(
            app_name_from_bundle(Some("com.unknown.app")),
            Some("com.unknown.app".into())
        );
        assert_eq!(app_name_from_bundle(None), None);
    }
}

/// A single long-lived adapter. Dropping it always reaps the child, including
/// when the user disables every media consumer.
pub(crate) struct MediaStream {
    child: std::process::Child,
    pub(crate) updates: std::sync::mpsc::Receiver<Result<Option<NowPlayingTrack>, String>>,
}
impl Drop for MediaStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) fn parse_stream_track(line: &str) -> Result<Option<NowPlayingTrack>, String> {
    let envelope: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let mut value = if envelope.get("type").is_some() {
        if envelope.get("type").and_then(|v| v.as_str()) != Some("data")
            || envelope.get("diff").and_then(|v| v.as_bool()) == Some(true)
        {
            return Err("Unsupported media stream envelope".into());
        }
        envelope
            .get("payload")
            .cloned()
            .ok_or("Media stream payload missing")?
    } else {
        envelope
    };
    if value.is_null() || value.as_object().is_some_and(|v| v.is_empty()) {
        return Ok(None);
    }
    // --micros gives an unambiguous Unix anchor across adapter versions.
    for (micros, seconds) in [
        ("elapsedTimeMicros", "elapsedTime"),
        ("durationMicros", "duration"),
    ] {
        if let Some(v) = value.get(micros).and_then(|v| v.as_f64()) {
            value[seconds] = serde_json::json!(v / 1_000_000.0);
        }
    }
    let payload: AdapterPayload =
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    let mut track = track_from_payload(payload);
    if track.title.is_none() && track.artist.is_none() && track.app.is_none() {
        return Ok(None);
    }
    // Stream elapsedTime is anchored at timestamp, unlike get --now.
    let timestamp = value
        .get("timestampEpochMicros")
        .and_then(|v| v.as_i64())
        .and_then(chrono::DateTime::from_timestamp_micros)
        .or_else(|| {
            value
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|at| at.with_timezone(&chrono::Utc))
        });
    if track.playing {
        if let (Some(position), Some(at)) = (track.position, timestamp) {
            track.position = Some(
                (position
                    + (chrono::Utc::now() - at.with_timezone(&chrono::Utc))
                        .num_milliseconds()
                        .max(0) as f64
                        / 1000.0)
                    .min(track.duration.unwrap_or(f64::MAX)),
            );
        }
    }
    Ok(Some(track))
}

pub(crate) fn start_stream() -> Option<MediaStream> {
    use std::io::{BufRead, Read};
    let (script, framework) = adapter_paths()?;
    let mut child = Command::new("/usr/bin/perl")
        .arg(script)
        .arg(framework)
        .arg("stream")
        .arg("--no-diff")
        .arg("--micros")
        .arg("--debounce=100")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (sender, updates) = std::sync::mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        loop {
            // Artwork can be large, but never allow an unbounded line buffer.
            let mut line = String::new();
            let read = (&mut reader).take(8 * 1024 * 1024 + 1).read_line(&mut line);
            let message = match read {
                Ok(0) => Err("media stream ended".into()),
                Ok(n) if n > 8 * 1024 * 1024 => Err("media stream line too large".into()),
                Ok(_) => parse_stream_track(&line),
                Err(e) => Err(e.to_string()),
            };
            let failed = message.is_err();
            if sender.send(message).is_err() || failed {
                break;
            }
        }
    });
    Some(MediaStream { child, updates })
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    #[test]
    fn reads_actual_adapter_envelopes_and_rejects_partial_diffs() {
        assert!(
            parse_stream_track(r#"{"type":"data","diff":false,"payload":{}}"#)
                .unwrap()
                .is_none()
        );
        let track = parse_stream_track(r#"{"type":"data","diff":false,"payload":{"title":"Fixture","playing":true,"elapsedTimeMicros":12000000}}"#).unwrap().unwrap();
        assert_eq!(track.title.as_deref(), Some("Fixture"));
        assert_eq!(track.position, Some(12.0));
        assert!(
            parse_stream_track(r#"{"type":"data","diff":true,"payload":{"playing":false}}"#)
                .is_err()
        );
        assert!(parse_stream_track(r#"{"type":"error","payload":"disconnected"}"#).is_err());
    }
    #[test]
    fn microsecond_stream_anchor_interpolates_playback_without_creeping_when_paused() {
        let value = serde_json::json!({ "title":"fixture", "durationMicros":120_000_000,
            "elapsedTimeMicros":12_000_000, "timestampEpochMicros":chrono::Utc::now().timestamp_micros() - 2_000_000, "playing":true });
        let track = parse_stream_track(&value.to_string()).unwrap().unwrap();
        assert_eq!(track.duration, Some(120.0));
        assert!((14.0..15.0).contains(&track.position.unwrap()));
        let mut paused = value;
        paused["playing"] = serde_json::json!(false);
        assert_eq!(
            parse_stream_track(&paused.to_string())
                .unwrap()
                .unwrap()
                .position,
            Some(12.0)
        );
    }
    #[test]
    fn handles_empty_and_corrupt_stream_updates() {
        assert!(parse_stream_track("{}").unwrap().is_none());
        assert!(parse_stream_track("null").unwrap().is_none());
        assert!(parse_stream_track("garbage").is_err());
        let track = parse_stream_track(
            r#"{"title":"歌名","artist":"歌手","playing":false,"elapsedTime":12}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(track.position, Some(12.0));
        assert!(!track.playing);
    }
}
