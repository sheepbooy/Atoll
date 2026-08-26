//! Cross-platform clipboard history.
//!
//! Polls the system clipboard for text changes, deduplicates and stores up to
//! 50 entries with a 24h expiry. Privacy-gated: the monitor only runs when the
//! user enables it in settings. macOS uses NSPasteboard via objc2-app-kit;
//! Windows uses the Win32 clipboard API.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 50;
const EXPIRY_SECS: u64 = 24 * 60 * 60;
const PREVIEW_LEN: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub preview: String,
    pub copied_at: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn clipboard_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("clipboard.json"))
}

fn make_preview(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= PREVIEW_LEN {
        return trimmed.replace('\n', " ");
    }
    let result: String = trimmed.chars().take(PREVIEW_LEN).collect();
    format!("{}…", result.replace('\n', " "))
}

/// Read the current system clipboard text (if any).
pub fn read_clipboard() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        read_clipboard_macos()
    }
    #[cfg(target_os = "windows")]
    {
        read_clipboard_windows()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Write text to the system clipboard.
pub fn write_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        write_clipboard_macos(text);
    }
    #[cfg(target_os = "windows")]
    {
        write_clipboard_windows(text);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = text;
    }
}

pub fn load_history() -> Vec<ClipboardEntry> {
    let Some(path) = clipboard_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<ClipboardEntry>>(&content) else {
        return Vec::new();
    };
    prune_expired(entries)
}

pub fn save_history(entries: &[ClipboardEntry]) {
    let Some(path) = clipboard_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(formatted) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(path, formatted);
    }
}

/// Remove entries older than 24h and cap to 50 items (newest first).
pub fn prune_expired(mut entries: Vec<ClipboardEntry>) -> Vec<ClipboardEntry> {
    let now = now_secs();
    entries.retain(|e| now.saturating_sub(e.copied_at) < EXPIRY_SECS);
    entries.truncate(MAX_ENTRIES);
    entries
}

/// Add a new entry (dedup against the most recent entry's content), persist,
/// and return the updated list. Returns None if content is empty or whitespace.
pub fn add_entry(content: String) -> Option<Vec<ClipboardEntry>> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut entries = load_history();
    // Dedup: if the most recent entry has the same content, skip.
    if let Some(first) = entries.first() {
        if first.content == content {
            return Some(entries);
        }
    }
    // Also remove any older duplicate so the entry moves to the front.
    entries.retain(|e| e.content != content);

    let entry = ClipboardEntry {
        id: uuid::Uuid::new_v4().to_string(),
        content: content.clone(),
        preview: make_preview(&content),
        copied_at: now_secs(),
    };
    entries.insert(0, entry);
    entries = prune_expired(entries);
    save_history(&entries);
    Some(entries)
}

pub fn clear_history() {
    let empty: Vec<ClipboardEntry> = Vec::new();
    save_history(&empty);
}

// ─── macOS (NSPasteboard via objc2-app-kit) ────────────────────────────────

#[cfg(target_os = "macos")]
fn read_clipboard_macos() -> Option<String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        let text: Option<Retained<NSString>> = pb.stringForType(&NSPasteboardTypeString);
        text.map(|s| s.to_string())
    }
}

#[cfg(target_os = "macos")]
fn write_clipboard_macos(text: &str) {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let s = NSString::from_str(text);
        pb.setString_forType(&s, &NSPasteboardTypeString);
    }
}

// `Retained` is used by objc2 but not imported in the type path above. Bring it
// in so the macOS read path compiles when the NSPasteboard feature is on.
#[cfg(target_os = "macos")]
use objc2::rc::Retained;

// ─── Windows (Win32 clipboard API) ─────────────────────────────────────────

#[cfg(target_os = "windows")]
fn read_clipboard_windows() -> Option<String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::Foundation::HGLOBAL;

    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let result = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32);
            let handle = match handle {
                Ok(h) if !h.is_invalid() => h,
                _ => return None,
            };
            let hmem = HGLOBAL(handle.0);
            let ptr = GlobalLock(hmem);
            if ptr.is_null() {
                return None;
            }
            let wcs = std::slice::from_raw_parts(ptr as *const u16, {
                // Count wide chars until null terminator.
                let mut len = 0usize;
                while *(ptr as *const u16).add(len) != 0 {
                    len += 1;
                }
                len
            });
            let result = String::from_utf16_lossy(wcs);
            let _ = GlobalUnlock(hmem);
            Some(result)
        })();
        let _ = CloseClipboard();
        result.and_then(|s| if s.is_empty() { None } else { Some(s) })
    }
}

#[cfg(target_os = "windows")]
fn write_clipboard_windows(text: &str) {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::Foundation::HANDLE;

    let wcs: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wcs.len() * 2;

    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if byte_len > 0 {
            if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
                let ptr = GlobalLock(hmem);
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(wcs.as_ptr() as *const u8, ptr as *mut u8, byte_len);
                    let _ = GlobalUnlock(hmem);
                    let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hmem.0));
                }
            }
        }
        let _ = CloseClipboard();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_expired_entries() {
        let now = now_secs();
        let entries = vec![
            ClipboardEntry {
                id: "a".into(),
                content: "old".into(),
                preview: "old".into(),
                copied_at: now - EXPIRY_SECS - 1,
            },
            ClipboardEntry {
                id: "b".into(),
                content: "new".into(),
                preview: "new".into(),
                copied_at: now,
            },
        ];
        let result = prune_expired(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn prune_caps_to_max_entries() {
        let now = now_secs();
        let entries: Vec<ClipboardEntry> = (0..MAX_ENTRIES + 10)
            .map(|i| ClipboardEntry {
                id: format!("e{}", i),
                content: format!("c{}", i),
                preview: format!("c{}", i),
                copied_at: now,
            })
            .collect();
        let result = prune_expired(entries);
        assert_eq!(result.len(), MAX_ENTRIES);
    }

    #[test]
    fn make_preview_truncates_long_content() {
        let long = "x".repeat(500);
        let preview = make_preview(&long);
        assert!(preview.ends_with('…'));
        assert!(preview.chars().count() <= PREVIEW_LEN + 1);
    }

    #[test]
    fn make_preview_strips_whitespace_and_newlines() {
        let preview = make_preview("  hello\nworld  ");
        assert_eq!(preview, "hello world");
    }

    #[test]
    fn clipboard_entry_serializes_camel_case() {
        let entry = ClipboardEntry {
            id: "test".into(),
            content: "content".into(),
            preview: "preview".into(),
            copied_at: 1234567890,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"copiedAt\""));
        assert!(!json.contains("\"copied_at\""));
    }
}
