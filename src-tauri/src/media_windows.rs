//! Windows Now Playing via SMTC (System Media Transport Controls).
//!
//! The OS aggregates every player that registers with SMTC (Spotify, browsers,
//! QQ/NetEase Music, …) into `GlobalSystemMediaTransportControlsSessionManager`
//! sessions. We poll the sessions the same way the macOS side polls the
//! MediaRemote adapter, and produce the exact same `NowPlayingTrack` shape so
//! the frontend and the lyrics pipeline need no per-platform handling.
//!
//! Layout: everything that is testable without Windows (struct, command and
//! session selection, position math, app-name mapping) lives un-gated so the
//! unit tests run on any host; all WinRT calls are isolated in the
//! `winrt` submodule behind `#[cfg(target_os = "windows")]`.

use serde::{Deserialize, Serialize};

/// Identical field-for-field to `media::NowPlayingTrack` (macOS) and the
/// non-macOS stub in lib.rs — the frontend (`src/tauri.ts:693`) and the
/// lyrics pipeline rely on this shape. Keep them in sync.
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

/// A media command as sent by the frontend (`MediaCommand` in tauri.ts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtcAction {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
}

/// Map a frontend media command to the session-level SMTC `Try*` call.
/// Windows 0.58 metadata has no `TryChangePlaybackStatusAsync`; the session
/// family (TryPlay/TryPause/TryTogglePlayPause/…) is the same mechanism the
/// OS media keys go through.
pub fn command_to_action(command: &str) -> Option<SmtcAction> {
    match command {
        "play" => Some(SmtcAction::Play),
        "pause" => Some(SmtcAction::Pause),
        "toggle" => Some(SmtcAction::Toggle),
        "next" => Some(SmtcAction::Next),
        "prev" => Some(SmtcAction::Prev),
        _ => None,
    }
}

/// Pick the SMTC session to present: prefer one that is actively playing
/// (when several players are registered), otherwise the first session at all.
/// Returns the winning index, or None when there is no session.
pub fn pick_playing_index(playing: &[bool]) -> Option<usize> {
    if playing.is_empty() {
        return None;
    }
    Some(playing.iter().position(|p| *p).unwrap_or(0))
}

/// TimeSpan/DateTime are 100ns ticks; DateTime counts from 1601-01-01 UTC.
const TICKS_PER_SEC: f64 = 10_000_000.0;
/// Offset from the Unix epoch (1970) to the Windows epoch (1601) in seconds.
#[cfg(target_os = "windows")]
const UNIX_TO_WINDOWS_EPOCH_SECS: i64 = 11_644_473_600;

pub fn ticks_to_secs(ticks: i64) -> f64 {
    ticks as f64 / TICKS_PER_SEC
}

/// Current wall clock as Windows-epoch 100ns ticks (comparable with
/// `Foundation::DateTime.UniversalTime`).
#[cfg(target_os = "windows")]
fn now_windows_epoch_ticks() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => (elapsed.as_secs() as i64 + UNIX_TO_WINDOWS_EPOCH_SECS) * 10_000_000
            + (elapsed.subsec_nanos() as i64 / 100),
        Err(_) => 0,
    }
}

/// SMTC reports the position as of `LastUpdatedTime`; most players never
/// refresh it continuously. While playing, advance it by the wall-clock time
/// elapsed since that update (the frontend does the same between polls).
/// While paused the raw value is used; a clock jump backwards clamps to 0
/// delta and a negative position clamps to 0.
pub fn extrapolated_position_secs(
    position_secs: f64,
    playing: bool,
    last_updated_ticks: i64,
    now_ticks: i64,
) -> f64 {
    let base = position_secs.max(0.0);
    if !playing {
        return base;
    }
    let delta_secs = (now_ticks - last_updated_ticks).max(0) as f64 / TICKS_PER_SEC;
    base + delta_secs
}

/// `TimelineProperties.EndTime` is the track duration for most SMTC sources,
/// but live streams / broken apps report 0 or absurd values — reject those
/// and let the card live without a progress bar.
pub fn sane_duration_secs(end_time_secs: f64) -> Option<f64> {
    (end_time_secs > 0.0 && end_time_secs < 86_400.0).then_some(end_time_secs)
}

/// Map an app user model id (e.g. "SpotifySpotify.exe", "Chrome",
/// "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic") to a friendly
/// name. Unknown AUMIDs keep their id with any "!<resource>" suffix stripped.
pub fn app_name_from_aumid(aumid: &str) -> String {
    let lower = aumid.to_lowercase();
    let known: &[(&str, &str)] = &[
        ("spotify", "Spotify"),
        ("cloudmusic", "NetEase Music"),
        ("netease", "NetEase Music"),
        ("qqmusic", "QQ Music"),
        ("kugou", "KuGou"),
        ("kuwo", "KuWo"),
        ("zunemusic", "Media Player"),
        ("windowsmediaplayer", "Windows Media Player"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("vlc", "VLC"),
        ("foobar", "foobar2000"),
    ];
    for (needle, name) in known {
        if lower.contains(needle) {
            return (*name).to_string();
        }
    }
    aumid.split('!').next().unwrap_or(aumid).to_string()
}

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&PNG_MAGIC)
}

/// Normalize raw thumbnail bytes into the base64 payload the frontend
/// expects. Already-PNG data is passed through untouched; anything else is
/// decoded and re-encoded as PNG so the bytes always match a format every
/// WebView2 build handles; if decoding fails we still pass the raw bytes —
/// Chromium sniffs the real format for `<img>` sources, so e.g. a JPEG
/// renders fine under the frontend's fixed `data:image/jpeg` prefix.
pub fn artwork_base64(raw: Vec<u8>) -> Option<String> {
    use base64::Engine as _;
    if raw.is_empty() {
        return None;
    }
    if is_png(&raw) {
        return Some(base64::engine::general_purpose::STANDARD.encode(&raw));
    }
    if let Ok(img) = image::load_from_memory(&raw) {
        let mut png: Vec<u8> = Vec::new();
        if img
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_ok()
        {
            return Some(base64::engine::general_purpose::STANDARD.encode(png));
        }
    }
    Some(base64::engine::general_purpose::STANDARD.encode(raw))
}

#[cfg(target_os = "windows")]
mod winrt {
    use super::{
        app_name_from_aumid, artwork_base64, command_to_action, extrapolated_position_secs,
        now_windows_epoch_ticks, pick_playing_index, sane_duration_secs, ticks_to_secs,
        NowPlayingTrack, SmtcAction,
    };
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as SmtcSession,
        GlobalSystemMediaTransportControlsSessionManager as SmtcManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as SmtcMediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as SmtcPlaybackStatus,
    };
    use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    /// Refuse to buffer absurd thumbnails (broken apps can report huge sizes).
    const MAX_ARTWORK_BYTES: u32 = 16 * 1024 * 1024;

    /// `RoGetActivationFactory` requires an initialized apartment. All SMTC
    /// calls happen on our own monitor threads and on tauri command threads,
    /// none of which COM-initialize by default. MTA per thread; S_FALSE and
    /// RPC_E_CHANGED_MODE both mean "an apartment exists" and are fine.
    fn ensure_com() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    /// Cached artwork so the two 1s monitors don't re-open/decode/re-encode
    /// the thumbnail stream every poll. Keyed by track identity; a mid-track
    /// artwork swap lags until the next track change (acceptable).
    struct ArtworkCache {
        key: String,
        value: Option<String>,
    }

    static ARTWORK_CACHE: std::sync::Mutex<Option<ArtworkCache>> = std::sync::Mutex::new(None);

    fn request_manager() -> Option<SmtcManager> {
        SmtcManager::RequestAsync().ok()?.get().ok()
    }

    /// All registered SMTC sessions. Iteration skips entries the OS fails to
    /// hand out (a player mid-teardown).
    fn sessions(manager: &SmtcManager) -> Vec<SmtcSession> {
        let Ok(view) = manager.GetSessions() else {
            return Vec::new();
        };
        if is_null(&view) {
            return Vec::new();
        }
        view.into_iter().collect()
    }

    /// True for a WinRT "null object": class/nullable getters may return S_OK
    /// with a null pointer (an unset `Thumbnail()` is documented to do so),
    /// and windows-rs calls the vtable directly, so touching such an object
    /// would dereference null. Treat them as "missing" instead.
    fn is_null<T: windows::core::Interface>(value: &T) -> bool {
        value.as_raw().is_null()
    }

    /// The session to present: prefer actively-playing, else first.
    fn active_session(manager: &SmtcManager) -> Option<SmtcSession> {
        let all = sessions(manager);
        let playing: Vec<bool> = all
            .iter()
            .map(|s| {
                s.GetPlaybackInfo()
                    .ok()
                    .filter(|p| !is_null(p))
                    .and_then(|p| p.PlaybackStatus().ok())
                    .map(|st| st == SmtcPlaybackStatus::Playing)
                    .unwrap_or(false)
            })
            .collect();
        pick_playing_index(&playing).and_then(|i| all.into_iter().nth(i))
    }

    fn hstring_to_string(value: windows::core::HSTRING) -> String {
        value.to_string_lossy()
    }

    /// Read the thumbnail RandomAccessStream into bytes via a DataReader.
    fn read_thumbnail(reference: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
        let stream = reference.OpenReadAsync().ok()?.get().ok()?;
        let size = stream.Size().ok()?;
        if size == 0 || size > MAX_ARTWORK_BYTES as u64 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        let loaded = reader.LoadAsync(size as u32).ok()?.get().ok()? as usize;
        let mut bytes = vec![0u8; loaded];
        reader.ReadBytes(&mut bytes).ok()?;
        Some(bytes)
    }

    fn read_artwork_cached(properties: &SmtcMediaProperties, key: &str) -> Option<String> {
        let mut guard = ARTWORK_CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cache) = guard.as_ref() {
            if cache.key == key {
                return cache.value.clone();
            }
        }
        let value = properties
            .Thumbnail()
            .ok()
            .filter(|reference| !is_null(reference))
            .and_then(|reference| read_thumbnail(&reference))
            .and_then(artwork_base64);
        *guard = Some(ArtworkCache {
            key: key.to_string(),
            value: value.clone(),
        });
        value
    }

    /// Fetch the current Now Playing track. Mirrors `media::fetch_now_playing`:
    /// None when SMTC is unavailable or no player reports meaningful media.
    pub fn fetch_now_playing() -> Option<NowPlayingTrack> {
        ensure_com();
        let manager = request_manager()?;
        let session = active_session(&manager)?;

        let playing = session
            .GetPlaybackInfo()
            .ok()
            .filter(|p| !is_null(p))
            .and_then(|p| p.PlaybackStatus().ok())
            .map(|st| st == SmtcPlaybackStatus::Playing)
            .unwrap_or(false);

        let properties = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        if is_null(&properties) {
            return None;
        }
        let title = properties.Title().map(hstring_to_string).ok();
        let artist = properties.Artist().map(hstring_to_string).ok();
        let album = properties.AlbumTitle().map(hstring_to_string).ok();
        // No session content at all and nothing playing — treat as "no media"
        // so the card and lyrics clear instead of showing an empty row.
        if !playing && title.as_deref().unwrap_or("").is_empty() {
            return None;
        }

        // Most players only update the timeline on track change / seek; the
        // position must be extrapolated from LastUpdatedTime while playing.
        let (position, duration) = session
            .GetTimelineProperties()
            .ok()
            .filter(|t| !is_null(t))
            .map(|timeline| {
                let raw_position = timeline.Position().map(|t| ticks_to_secs(t.Duration)).ok();
                let last_updated = timeline
                    .LastUpdatedTime()
                    .map(|t| t.UniversalTime)
                    .unwrap_or(0);
                let position = raw_position.map(|pos| {
                    extrapolated_position_secs(pos, playing, last_updated, now_windows_epoch_ticks())
                });
                let duration = timeline
                    .EndTime()
                    .map(|t| sane_duration_secs(ticks_to_secs(t.Duration)))
                    .ok()
                    .flatten();
                (position, duration)
            })
            .unwrap_or((None, None));

        let title_key = title.clone().unwrap_or_default();
        let artist_key = artist.clone().unwrap_or_default();
        let album_key = album.clone().unwrap_or_default();
        let artwork = read_artwork_cached(&properties, &format!("{title_key}|{artist_key}|{album_key}"));

        let app = session
            .SourceAppUserModelId()
            .map(|aumid| app_name_from_aumid(&hstring_to_string(aumid)))
            .ok();

        Some(NowPlayingTrack {
            title,
            artist,
            album,
            duration,
            position,
            playing,
            artwork_base64: artwork,
            app,
        })
    }

    /// Send a transport command to the active session. Returns false when no
    /// session exists, the command is unknown, or the app rejects it.
    pub fn send_media_command(command: &str) -> bool {
        let Some(action) = command_to_action(command) else {
            return false;
        };
        ensure_com();
        let Some(manager) = request_manager() else {
            return false;
        };
        let Some(session) = active_session(&manager) else {
            return false;
        };
        let request = match action {
            SmtcAction::Play => session.TryPlayAsync(),
            SmtcAction::Pause => session.TryPauseAsync(),
            SmtcAction::Toggle => session.TryTogglePlayPauseAsync(),
            SmtcAction::Next => session.TrySkipNextAsync(),
            SmtcAction::Prev => session.TrySkipPreviousAsync(),
        };
        // The Try* ops resolve to a bool: whether the app accepted the command.
        request.map(|op| op.get().unwrap_or(false)).unwrap_or(false)
    }
}

#[cfg(target_os = "windows")]
pub use winrt::{fetch_now_playing, send_media_command};

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
            app: Some("Spotify".into()),
        };
        let json = serde_json::to_string(&track).unwrap();
        assert!(json.contains("\"artworkBase64\""));
        assert!(json.contains("\"title\""));
        assert!(!json.contains("\"artwork_base64\""));
    }

    #[test]
    fn commands_map_to_smtc_actions() {
        assert_eq!(command_to_action("play"), Some(SmtcAction::Play));
        assert_eq!(command_to_action("pause"), Some(SmtcAction::Pause));
        assert_eq!(command_to_action("toggle"), Some(SmtcAction::Toggle));
        assert_eq!(command_to_action("next"), Some(SmtcAction::Next));
        assert_eq!(command_to_action("prev"), Some(SmtcAction::Prev));
        assert_eq!(command_to_action("stop"), None);
        assert_eq!(command_to_action(""), None);
    }

    #[test]
    fn session_pick_prefers_playing_then_first() {
        assert_eq!(pick_playing_index(&[]), None);
        assert_eq!(pick_playing_index(&[false, false]), Some(0));
        assert_eq!(pick_playing_index(&[false, true, false]), Some(1));
        // Several playing — the first one wins.
        assert_eq!(pick_playing_index(&[false, true, true]), Some(1));
    }

    #[test]
    fn position_math_extrapolates_only_while_playing() {
        // Paused: raw position, never extrapolated.
        assert_eq!(
            extrapolated_position_secs(42.5, false, 1_000_000_000, 2_000_000_000),
            42.5
        );
        // Playing: position + elapsed since the timeline update.
        // 100_000_000 ticks = 10s base position; +3.5s of wall-clock elapsed.
        let advanced = extrapolated_position_secs(10.0, true, 100_000_000, 135_000_000);
        assert!((advanced - 13.5).abs() < 1e-9);
        // Clock jumped backwards since the update — no negative creep.
        assert_eq!(
            extrapolated_position_secs(10.0, true, 5_000_000_000, 4_000_000_000),
            10.0
        );
        // Garbage negative position clamps to 0.
        assert_eq!(
            extrapolated_position_secs(-3.0, false, 0, 0),
            0.0
        );
    }

    #[test]
    fn ticks_convert_to_secs() {
        assert!((ticks_to_secs(12_345_000_000) - 1234.5).abs() < 1e-9);
        assert_eq!(ticks_to_secs(0), 0.0);
    }

    #[test]
    fn duration_only_accepts_sane_end_times() {
        assert_eq!(sane_duration_secs(0.0), None);
        assert_eq!(sane_duration_secs(-5.0), None);
        assert_eq!(sane_duration_secs(100_000.0), None); // > 24h: live stream
        assert_eq!(sane_duration_secs(233.0), Some(233.0));
    }

    #[test]
    fn app_names_map_from_aumids() {
        assert_eq!(app_name_from_aumid("SpotifySpotify.exe"), "Spotify");
        assert_eq!(app_name_from_aumid("Chrome"), "Chrome");
        assert_eq!(
            app_name_from_aumid("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Media Player"
        );
        assert_eq!(
            app_name_from_aumid("04CBCD3D.Music~AF2CABFF.163music_pc!NetEaseMusic"),
            "NetEase Music"
        );
        assert_eq!(app_name_from_aumid("Tencent.QQMusicPC"), "QQ Music");
        // Unknown: keep the id, minus any "!" resource suffix.
        assert_eq!(app_name_from_aumid("SomePlayer_8wekyb3d8bbwe!App"), "SomePlayer_8wekyb3d8bbwe");
    }

    #[test]
    fn png_detection_and_artwork_passthrough() {
        assert!(is_png(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00]));
        assert!(!is_png(b"\xFF\xD8\xFF\xe0 jpeg"));
        assert_eq!(artwork_base64(Vec::new()), None);
    }

    #[test]
    fn artwork_reencodes_non_png_to_png() {
        // 1x1 red PNG, generated once — decodable by the image crate.
        let png_1x1: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D,
            0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let encoded = artwork_base64(png_1x1.to_vec()).unwrap();
        use base64::Engine as _;
        let round_trip = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert!(is_png(&round_trip));
    }
}
