// Now Playing, media card and lyrics commands.
use tauri::{AppHandle, Emitter, State};

use crate::*;

/// Fetch the current Now Playing track from the platform media source —
/// the macOS MediaRemote adapter or the Windows SMTC session manager.
#[cfg(target_os = "macos")]
pub(crate) fn platform_now_playing() -> Option<NowPlayingTrack> {
    media::fetch_now_playing()
}

#[cfg(target_os = "windows")]
pub(crate) fn platform_now_playing() -> Option<NowPlayingTrack> {
    media_windows::fetch_now_playing()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn platform_now_playing() -> Option<NowPlayingTrack> {
    None
}

#[tauri::command]
pub(crate) fn get_now_playing() -> Option<NowPlayingTrack> {
    platform_now_playing()
}

#[tauri::command]
pub(crate) fn send_media_command(command: String) -> bool {
    #[cfg(target_os = "macos")]
    {
        let cmd = match command.as_str() {
            "play" => media::MR_COMMAND_PLAY,
            "pause" => media::MR_COMMAND_PAUSE,
            "toggle" => media::MR_COMMAND_TOGGLE,
            "next" => media::MR_COMMAND_NEXT,
            "prev" => media::MR_COMMAND_PREV,
            _ => return false,
        };
        media::send_media_command_raw(cmd)
    }
    #[cfg(target_os = "windows")]
    {
        media_windows::send_media_command(&command)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = command;
        false
    }
}

#[tauri::command]
pub(crate) fn get_media_card_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.media_card_enabled)
}

#[tauri::command]
pub(crate) fn set_media_card_enabled(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.media_card_enabled) = enabled;
    persist_media_card_enabled(enabled);
    enabled
}

#[tauri::command]
pub(crate) fn get_lyrics_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.lyrics_enabled)
}

#[tauri::command]
pub(crate) fn set_lyrics_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    *lock_state(&state.lyrics_enabled) = enabled;
    persist_lyrics_enabled(enabled);
    if !enabled {
        *lock_state(&state.lyrics) = None;
        *lock_state(&state.lyrics_track_key) = String::new();
        let _ = app.emit("lyrics-changed", Option::<lyrics::LyricPayload>::None);
    }
    enabled
}

#[tauri::command]
pub(crate) fn get_current_lyrics(state: State<'_, AppState>) -> Option<lyrics::LyricPayload> {
    lock_state(&state.lyrics).clone()
}
