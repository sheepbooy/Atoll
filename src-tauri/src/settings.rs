//! `~/.atoll/settings.json` read/write: every load_*/persist_* helper for
//! user-visible settings (retention, media, lyrics, clipboard, notice mode,
//! notification language, artwork backdrop).

use serde_json::Value;

use crate::clipboard_history;

pub(crate) const DEFAULT_SESSION_RETENTION_SECS: u64 = 900;
pub(crate) const DEFAULT_SUBAGENT_RETENTION_SECS: u64 = 600;

pub(crate) fn atoll_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("settings.json"))
}

pub(crate) fn load_persisted_retention_secs() -> u64 {
    let Some(path) = atoll_settings_path() else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return DEFAULT_SESSION_RETENTION_SECS;
    };
    let minutes = value
        .get("sessionRetentionMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_SESSION_RETENTION_SECS / 60);
    minutes.clamp(1, 60) * 60
}

pub(crate) fn load_persisted_subagent_retention_secs() -> u64 {
    let Some(path) = atoll_settings_path() else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return DEFAULT_SUBAGENT_RETENTION_SECS;
    };
    let minutes = value
        .get("subagentRetentionMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_SUBAGENT_RETENTION_SECS / 60);
    minutes.clamp(1, 60) * 60
}

pub(crate) fn persist_settings(session_minutes: Option<u64>, subagent_minutes: Option<u64>) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    let obj = config.as_object_mut().unwrap();
    if let Some(m) = session_minutes {
        obj.insert("sessionRetentionMinutes".into(), Value::from(m));
    }
    if let Some(m) = subagent_minutes {
        obj.insert("subagentRetentionMinutes".into(), Value::from(m));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn load_media_card_enabled() -> bool {
    let Some(path) = atoll_settings_path() else {
        return true;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return true;
    };
    value
        .get("mediaCardEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub(crate) fn persist_media_card_enabled(enabled: bool) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert("mediaCardEnabled".into(), Value::from(enabled));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) const APPROVAL_NOTICE_INTERRUPT: &str = "interrupt";
pub(crate) const APPROVAL_NOTICE_NOTIFY: &str = "notify";

/// Clamp a persisted/UI-supplied notice mode to a known value; unknown input
/// falls back to the historical interrupt behavior.
pub(crate) fn normalize_approval_notice_mode(mode: &str) -> &'static str {
    if mode == APPROVAL_NOTICE_NOTIFY {
        APPROVAL_NOTICE_NOTIFY
    } else {
        APPROVAL_NOTICE_INTERRUPT
    }
}

/// Notification copy follows the UI language; unknown values fall back to English.
pub(crate) fn normalize_notification_language(language: &str) -> &'static str {
    if language == "zh-CN" {
        "zh-CN"
    } else {
        "en"
    }
}

pub(crate) fn load_approval_notice_mode() -> String {
    let Some(path) = atoll_settings_path() else {
        return APPROVAL_NOTICE_INTERRUPT.to_string();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return APPROVAL_NOTICE_INTERRUPT.to_string();
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return APPROVAL_NOTICE_INTERRUPT.to_string();
    };
    value
        .get("approvalNoticeMode")
        .and_then(Value::as_str)
        .map(normalize_approval_notice_mode)
        .unwrap_or(APPROVAL_NOTICE_INTERRUPT)
        .to_string()
}

pub(crate) fn persist_settings_value(key: &str, value: Value) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert(key.into(), value);
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn persist_approval_notice_mode(mode: &str) {
    persist_settings_value("approvalNoticeMode", Value::from(mode));
}

pub(crate) fn load_notification_language() -> String {
    let Some(path) = atoll_settings_path() else {
        return "en".to_string();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return "en".to_string();
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return "en".to_string();
    };
    value
        .get("notificationLanguage")
        .and_then(Value::as_str)
        .map(normalize_notification_language)
        .unwrap_or("en")
        .to_string()
}

pub(crate) fn persist_notification_language(language: &str) {
    persist_settings_value("notificationLanguage", Value::from(language));
}

pub(crate) fn load_artwork_backdrop_enabled() -> bool {
    let Some(path) = atoll_settings_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    value
        .get("artworkBackdropEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn persist_artwork_backdrop_enabled(enabled: bool) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert("artworkBackdropEnabled".into(), Value::from(enabled));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn persist_retention_minutes(minutes: u64) {
    persist_settings(Some(minutes.clamp(1, 60)), None);
}

pub(crate) fn load_clipboard_history_enabled() -> bool {
    let Some(path) = atoll_settings_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    value
        .get("clipboardHistoryEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn persist_clipboard_history_enabled(enabled: bool) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert("clipboardHistoryEnabled".into(), Value::from(enabled));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn load_clipboard_history_limit() -> usize {
    let Some(path) = atoll_settings_path() else {
        return clipboard_history::DEFAULT_MAX_ENTRIES;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return clipboard_history::DEFAULT_MAX_ENTRIES;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return clipboard_history::DEFAULT_MAX_ENTRIES;
    };
    value
        .get("clipboardHistoryLimit")
        .and_then(Value::as_u64)
        .map(|limit| {
            limit.clamp(
                clipboard_history::MIN_HISTORY_LIMIT as u64,
                clipboard_history::MAX_HISTORY_LIMIT as u64,
            ) as usize
        })
        .unwrap_or(clipboard_history::DEFAULT_MAX_ENTRIES)
}

pub(crate) fn persist_clipboard_history_limit(limit: usize) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert("clipboardHistoryLimit".into(), Value::from(limit as u64));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}

pub(crate) fn load_lyrics_enabled() -> bool {
    let Some(path) = atoll_settings_path() else {
        return false;
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    value
        .get("lyricsEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn persist_lyrics_enabled(enabled: bool) {
    let Some(path) = atoll_settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut config: Value = path
        .exists()
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = config.as_object_mut() {
        obj.insert("lyricsEnabled".into(), Value::from(enabled));
    }
    if let Ok(formatted) = serde_json::to_string_pretty(&config) {
        let _ = std::fs::write(path, formatted);
    }
}
