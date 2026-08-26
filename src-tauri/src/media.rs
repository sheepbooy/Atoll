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
                return Some((script.to_string_lossy().into_owned(), framework.to_string_lossy().into_owned()));
            }
        }
    }
    // Dev mode: resolve from the manifest dir (CARGO_MANIFEST_DIR env set by cargo).
    let manifest = option_env!("CARGO_MANIFEST_DIR")?;
    let dev = std::path::Path::new(manifest).join("resources").join("media");
    let script = dev.join("mediaremote-adapter.pl");
    let framework = dev.join("MediaRemoteAdapter.framework");
    if script.exists() && framework.exists() {
        Some((script.to_string_lossy().into_owned(), framework.to_string_lossy().into_owned()))
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
    Some(NowPlayingTrack {
        title: payload.title,
        artist: payload.artist,
        album: payload.album,
        duration: payload.duration,
        position: payload.elapsed_time,
        playing: payload.playing.unwrap_or(false),
        artwork_base64: payload.artwork_data,
        app: app_name_from_bundle(payload.bundle_identifier.as_deref()),
    })
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
        assert_eq!(app_name_from_bundle(Some("com.apple.Music")), Some("Music".into()));
        assert_eq!(app_name_from_bundle(Some("com.spotify.client")), Some("Spotify".into()));
        assert_eq!(app_name_from_bundle(Some("com.tencent.QQMusicMac")), Some("QQ Music".into()));
        assert_eq!(app_name_from_bundle(Some("com.unknown.app")), Some("com.unknown.app".into()));
        assert_eq!(app_name_from_bundle(None), None);
    }
}
