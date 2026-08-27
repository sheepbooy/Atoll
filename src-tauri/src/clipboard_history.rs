//! Cross-platform clipboard history.
//!
//! Records text, images (normalized to PNG) and file lists copied to the
//! system clipboard. The in-memory list owned by the app state is the source
//! of truth; it is mirrored to `~/.atoll/clipboard.json` with atomic writes,
//! while image payloads live as blobs under `~/.atoll/clipboard/`.
//!
//! Change detection uses the platform clipboard sequence number
//! (`NSPasteboard.changeCount` / `GetClipboardSequenceNumber`) instead of
//! content comparison, so a stale read can no longer hide later changes.
//! On macOS every pasteboard call must run on the main thread (AppKit
//! threading rules); lib.rs marshals reads/writes through
//! `run_on_main_thread` before calling into this module. Privacy-gated: the
//! monitor only records while the user keeps the setting enabled.

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::ProtocolObject;
#[cfg(target_os = "macos")]
use objc2::ClassType;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF,
    NSPasteboardWriting,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

pub const DEFAULT_MAX_ENTRIES: usize = 50;
pub const MIN_HISTORY_LIMIT: usize = 10;
pub const MAX_HISTORY_LIMIT: usize = 500;
const EXPIRY_SECS: u64 = 24 * 60 * 60;
const PREVIEW_LEN: usize = 200;
/// Skip clipboard images larger than this before decoding.
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
/// Longest edge of the generated image thumbnail.
const THUMB_MAX: u32 = 320;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    #[default]
    Text,
    Image,
    Files,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntry {
    pub id: String,
    /// Payload discriminant; defaults to text so pre-image JSON still loads.
    #[serde(default)]
    pub kind: EntryKind,
    /// Text content or newline-joined file paths; empty for images.
    pub content: String,
    pub preview: String,
    pub copied_at: u64,
    /// PNG byte size for image entries.
    #[serde(default)]
    pub byte_size: u64,
    /// Stable payload fingerprint used for dedup; 0 on legacy entries.
    #[serde(default)]
    pub fingerprint: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardPayload {
    Text(String),
    Image { png: Vec<u8> },
    Files(Vec<String>),
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn clipboard_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("clipboard.json"))
}

fn blobs_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ATOLL_TEST_CLIPBOARD_DIR") {
        return Some(PathBuf::from(dir));
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("clipboard"))
}

fn image_blob_path(id: &str) -> Option<PathBuf> {
    blobs_dir().map(|dir| dir.join(format!("{id}.png")))
}

fn thumb_blob_path(id: &str) -> Option<PathBuf> {
    blobs_dir().map(|dir| dir.join(format!("{id}.thumb.png")))
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn make_preview(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= PREVIEW_LEN {
        return trimmed.replace('\n', " ");
    }
    let result: String = trimmed.chars().take(PREVIEW_LEN).collect();
    format!("{}…", result.replace('\n', " "))
}

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn files_preview(paths: &[String]) -> String {
    let names: Vec<String> = paths.iter().map(|p| file_name_of(p)).collect();
    make_preview(&names.join(", "))
}

// ─── Blob storage ──────────────────────────────────────────────────────────

fn write_image_blobs(id: &str, png: &[u8]) -> bool {
    let Some(dir) = blobs_dir() else {
        return false;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    if std::fs::write(dir.join(format!("{id}.png")), png).is_err() {
        return false;
    }
    // Thumbnail is best-effort; the UI falls back to a size label. Small
    // images are reused as-is (re-encoding would inflate them).
    if let Ok(img) = image::load_from_memory(png) {
        let thumb_path = dir.join(format!("{id}.thumb.png"));
        if img.width() <= THUMB_MAX && img.height() <= THUMB_MAX {
            let _ = std::fs::write(thumb_path, png);
        } else {
            let thumb = img.thumbnail(THUMB_MAX, THUMB_MAX);
            let _ = thumb.save_with_format(thumb_path, image::ImageFormat::Png);
        }
    }
    true
}

pub fn read_image_blob(id: &str) -> Option<Vec<u8>> {
    std::fs::read(image_blob_path(id)?).ok()
}

/// data:image/png URL of the stored thumbnail (falls back to the full image).
pub fn read_thumbnail_data_url(id: &str) -> Option<String> {
    use base64::Engine;
    let thumb_path = thumb_blob_path(id)?;
    let full_path = image_blob_path(id)?;
    let bytes = match std::fs::read(&thumb_path) {
        Ok(bytes) => bytes,
        Err(_) => std::fs::read(&full_path).ok()?,
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:image/png;base64,{encoded}"))
}

fn delete_image_blobs(id: &str) {
    if let Some(path) = image_blob_path(id) {
        let _ = std::fs::remove_file(path);
    }
    if let Some(path) = thumb_blob_path(id) {
        let _ = std::fs::remove_file(path);
    }
}

/// Remove every stored blob (used when the whole history is cleared).
pub fn clear_blobs() {
    let Some(dir) = blobs_dir() else {
        return;
    };
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

// ─── History persistence ───────────────────────────────────────────────────

pub fn load_history(limit: usize) -> Vec<ClipboardEntry> {
    let Some(path) = clipboard_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    if content.trim().is_empty() {
        return Vec::new();
    }
    match serde_json::from_str::<Vec<ClipboardEntry>>(&content) {
        Ok(mut entries) => {
            prune_expired(&mut entries, limit);
            entries
        }
        Err(err) => {
            // Keep the broken file around for inspection instead of silently
            // treating history as empty (which made new adds look like they
            // replaced everything that came before).
            eprintln!("atoll: clipboard history unreadable ({err}); archiving");
            let _ = std::fs::rename(&path, path.with_extension("json.corrupt"));
            Vec::new()
        }
    }
}

pub fn save_history(entries: &[ClipboardEntry]) {
    let Some(path) = clipboard_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(formatted) = serde_json::to_string_pretty(entries) {
        // Write-then-rename keeps a hard kill from leaving a truncated file.
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, formatted).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Remove entries older than 24h, cap to `limit` (newest first) and delete
/// the blobs of anything dropped.
pub fn prune_expired(entries: &mut Vec<ClipboardEntry>, limit: usize) {
    let now = now_secs();
    let mut kept: Vec<ClipboardEntry> = Vec::with_capacity(entries.len());
    for entry in entries.drain(..) {
        let fresh = now.saturating_sub(entry.copied_at) < EXPIRY_SECS;
        if fresh && kept.len() < limit {
            kept.push(entry);
        } else if entry.kind == EntryKind::Image {
            delete_image_blobs(&entry.id);
        }
    }
    *entries = kept;
}

fn is_duplicate(entry: &ClipboardEntry, kind: EntryKind, fingerprint: u64, content: &str) -> bool {
    if entry.fingerprint != 0 && fingerprint != 0 {
        return entry.fingerprint == fingerprint;
    }
    entry.kind == kind && entry.content == content
}

/// Insert a payload as the newest entry, dedup against existing entries,
/// prune to `limit`, and persist image blobs. Returns true when the list
/// changed. The JSON mirror is written by the caller while holding the app
/// state lock, keeping the list the single source of truth.
pub fn add_entry(
    entries: &mut Vec<ClipboardEntry>,
    payload: ClipboardPayload,
    limit: usize,
) -> bool {
    let mut image_bytes: Option<Vec<u8>> = None;
    let (kind, content, preview, byte_size, fingerprint) = match payload {
        ClipboardPayload::Text(text) => {
            if text.trim().is_empty() {
                return false;
            }
            let fingerprint = fingerprint_bytes(text.as_bytes());
            let preview = make_preview(&text);
            (EntryKind::Text, text, preview, 0, fingerprint)
        }
        ClipboardPayload::Image { png } => {
            if png.is_empty() {
                return false;
            }
            let fingerprint = fingerprint_bytes(&png);
            let byte_size = png.len() as u64;
            image_bytes = Some(png);
            (EntryKind::Image, String::new(), String::new(), byte_size, fingerprint)
        }
        ClipboardPayload::Files(paths) => {
            let paths: Vec<String> = paths
                .into_iter()
                .filter(|p| !p.trim().is_empty())
                .collect();
            if paths.is_empty() {
                return false;
            }
            let content = paths.join("\n");
            let fingerprint = fingerprint_bytes(content.as_bytes());
            let preview = files_preview(&paths);
            (EntryKind::Files, content, preview, 0, fingerprint)
        }
    };

    if let Some(first) = entries.first() {
        if is_duplicate(first, kind, fingerprint, &content) {
            return false;
        }
    }
    // Drop older duplicates so the entry moves to the front.
    entries.retain(|e| {
        let dup = is_duplicate(e, kind, fingerprint, &content);
        if dup && e.kind == EntryKind::Image {
            delete_image_blobs(&e.id);
        }
        !dup
    });

    let entry = ClipboardEntry {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        content,
        preview,
        copied_at: now_secs(),
        byte_size,
        fingerprint,
    };
    if kind == EntryKind::Image {
        let png = image_bytes.expect("image payloads keep their bytes");
        if !write_image_blobs(&entry.id, &png) {
            return false;
        }
    }
    entries.insert(0, entry);
    prune_expired(entries, limit);
    true
}

// ─── Clipboard access (call on the main thread on macOS) ──────────────────

/// Platform clipboard sequence number; changes on every clipboard write.
pub fn clipboard_sequence() -> u64 {
    #[cfg(target_os = "macos")]
    {
        NSPasteboard::generalPasteboard().changeCount().max(0) as u64
    }
    #[cfg(target_os = "windows")]
    {
        GetClipboardSequenceNumber() as u64
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        0
    }
}

/// Read the current clipboard as a payload (file lists > text > image).
pub fn read_clipboard_snapshot() -> Option<ClipboardPayload> {
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

/// Write a payload back to the clipboard. Returns false when the platform
/// path is unavailable.
pub fn write_clipboard_payload(payload: &ClipboardPayload) -> bool {
    match payload {
        ClipboardPayload::Text(text) => write_clipboard_text(text),
        ClipboardPayload::Image { png } => write_clipboard_image(png),
        ClipboardPayload::Files(paths) => write_clipboard_files(paths),
    }
}

pub fn write_clipboard_text(text: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        write_clipboard_macos(text)
    }
    #[cfg(target_os = "windows")]
    {
        write_clipboard_windows(text)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = text;
        false
    }
}

fn write_clipboard_image(png: &[u8]) -> bool {
    #[cfg(target_os = "macos")]
    {
        write_clipboard_macos_image(png)
    }
    #[cfg(target_os = "windows")]
    {
        write_clipboard_windows_image(png)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = png;
        false
    }
}

fn write_clipboard_files(paths: &[String]) -> bool {
    #[cfg(target_os = "macos")]
    {
        write_clipboard_macos_files(paths)
    }
    #[cfg(target_os = "windows")]
    {
        write_clipboard_windows_files(paths)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = paths;
        false
    }
}

/// Convert arbitrary encoded image bytes to PNG; passes PNG through.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn normalize_to_png(raw: &[u8]) -> Option<Vec<u8>> {
    if raw.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(raw.to_vec());
    }
    let img = image::load_from_memory(raw).ok()?;
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

// ─── macOS (NSPasteboard via objc2-app-kit) ────────────────────────────────

// SAFETY: readObjectsForClasses with the NSURL class only ever returns
// NSURL instances.
#[cfg(target_os = "macos")]
unsafe fn url_from_object(object: Retained<objc2::runtime::AnyObject>) -> Retained<NSURL> {
    Retained::cast_unchecked(object)
}

#[cfg(target_os = "macos")]
fn read_clipboard_macos() -> Option<ClipboardPayload> {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        // File lists win when present (Finder copies), then text, then image.
        if let Some(payload) = read_files_macos(&pb) {
            return Some(payload);
        }
        if let Some(text) = pb.stringForType(&NSPasteboardTypeString) {
            let text = text.to_string();
            if !text.trim().is_empty() {
                return Some(ClipboardPayload::Text(text));
            }
        }
        read_image_macos(&pb)
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_files_macos(pb: &NSPasteboard) -> Option<ClipboardPayload> {
    let classes = NSArray::from_slice(&[NSURL::class()]);
    let objects = pb.readObjectsForClasses_options(&classes, None)?;
    let mut paths = Vec::new();
    for index in 0..objects.count() {
        let url = url_from_object(objects.objectAtIndex(index));
        if url.isFileURL() {
            if let Some(path) = url.path() {
                paths.push(path.to_string());
            }
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(ClipboardPayload::Files(paths))
    }
}

#[cfg(target_os = "macos")]
unsafe fn read_image_macos(pb: &NSPasteboard) -> Option<ClipboardPayload> {
    let raw = pb
        .dataForType(&NSPasteboardTypePNG)
        .or_else(|| pb.dataForType(&NSPasteboardTypeTIFF))?
        .to_vec();
    if raw.is_empty() || raw.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let png = normalize_to_png(&raw)?;
    Some(ClipboardPayload::Image { png })
}

#[cfg(target_os = "macos")]
fn write_clipboard_macos(text: &str) -> bool {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let s = NSString::from_str(text);
        pb.setString_forType(&s, &NSPasteboardTypeString)
    }
}

#[cfg(target_os = "macos")]
fn write_clipboard_macos_image(png: &[u8]) -> bool {
    unsafe {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let png_data = NSData::with_bytes(png);
        let ok = pb.setData_forType(Some(&png_data), &NSPasteboardTypePNG);
        // Also offer TIFF, the format most macOS apps paste natively.
        if let Ok(img) = image::load_from_memory_with_format(png, image::ImageFormat::Png) {
            let mut tiff = Vec::new();
            if img
                .write_to(&mut Cursor::new(&mut tiff), image::ImageFormat::Tiff)
                .is_ok()
            {
                let tiff_data = NSData::with_bytes(&tiff);
                pb.setData_forType(Some(&tiff_data), &NSPasteboardTypeTIFF);
            }
        }
        ok
    }
}

#[cfg(target_os = "macos")]
fn write_clipboard_macos_files(paths: &[String]) -> bool {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = paths
        .iter()
        .map(|path| {
            let s = NSString::from_str(path);
            // SAFETY: NSURL conforms to NSPasteboardWriting in AppKit.
            unsafe { Retained::cast_unchecked(NSURL::fileURLWithPath(&s)) }
        })
        .collect();
    let array = NSArray::from_retained_slice(&objects);
    pb.writeObjects(&array)
}

// ─── Windows (Win32 clipboard API) ─────────────────────────────────────────

#[cfg(target_os = "windows")]
fn read_clipboard_windows() -> Option<ClipboardPayload> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::{CF_DIB, CF_DIBV5, CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::UI::Shell::HDROP;

    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let result = (|| {
            let has = |format: u32| IsClipboardFormatAvailable(format).as_bool();
            // File lists win when present (Explorer copies), then text, then image.
            if has(CF_HDROP.0 as u32) {
                if let Some(handle) = GetClipboardData(CF_HDROP.0 as u32).ok() {
                    let hmem = HGLOBAL(handle.0);
                    let ptr = GlobalLock(hmem) as *const u8;
                    if !ptr.is_null() {
                        let paths = hdrop_paths(HDROP(handle.0), ptr);
                        let _ = GlobalUnlock(hmem);
                        if !paths.is_empty() {
                            return Some(ClipboardPayload::Files(paths));
                        }
                    }
                }
            }
            if has(CF_UNICODETEXT.0 as u32) {
                if let Some(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32).ok() {
                    let hmem = HGLOBAL(handle.0);
                    let ptr = GlobalLock(hmem) as *const u16;
                    if !ptr.is_null() {
                        let mut len = 0usize;
                        while *ptr.add(len) != 0 {
                            len += 1;
                        }
                        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                        let _ = GlobalUnlock(hmem);
                        if !text.trim().is_empty() {
                            return Some(ClipboardPayload::Text(text));
                        }
                    }
                }
            }
            let dib_format = if has(CF_DIBV5.0 as u32) {
                Some(CF_DIBV5.0 as u32)
            } else if has(CF_DIB.0 as u32) {
                Some(CF_DIB.0 as u32)
            } else {
                None
            };
            if let Some(format) = dib_format {
                if let Some(handle) = GetClipboardData(format).ok() {
                    let hmem = HGLOBAL(handle.0);
                    let ptr = GlobalLock(hmem) as *const u8;
                    if !ptr.is_null() {
                        let len = GlobalSize(hmem);
                        let dib = std::slice::from_raw_parts(ptr, len);
                        let png = dib_to_png(dib);
                        let _ = GlobalUnlock(hmem);
                        if let Some(png) = png {
                            return Some(ClipboardPayload::Image { png });
                        }
                    }
                }
            }
            None
        })();
        let _ = CloseClipboard();
        result
    }
}

/// Parse the double-null-terminated path list behind an HDROP.
#[cfg(target_os = "windows")]
unsafe fn hdrop_paths(hdrop: windows::Win32::UI::Shell::HDROP, ptr: *const u8) -> Vec<String> {
    use windows::Win32::UI::Shell::DragQueryFileW;
    use windows::core::PWSTR;

    let mut paths = Vec::new();
    let count = DragQueryFileW(hdrop, u32::MAX, None, 0);
    for index in 0..count {
        let len = DragQueryFileW(hdrop, index, None, 0);
        if len == 0 {
            continue;
        }
        let mut buf = vec![0u16; len as usize + 1];
        DragQueryFileW(
            hdrop,
            index,
            Some(PWSTR(buf.as_mut_ptr())),
            buf.len() as u32,
        );
        paths.push(String::from_utf16_lossy(&buf[..len as usize]));
    }
    paths
}

/// Wrap a raw CF_DIB/CF_DIBV5 payload in a BMP file header and decode to PNG.
#[cfg(target_os = "windows")]
fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if header_size < 40 || dib.len() < header_size {
        return None;
    }
    let bpp = u16::from_le_bytes(dib[14..16].try_into().ok()?) as usize;
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?) as u32;
    let clr_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    let mut pixel_offset = 14 + header_size;
    if compression == 3 && header_size == 40 {
        // BI_BITFIELDS with a BITMAPINFOHEADER carries three color masks.
        pixel_offset += 12;
    } else if compression == 6 {
        // BI_ALPHABITFIELDS carries four.
        pixel_offset += 16;
    }
    if bpp <= 8 {
        let palette = if clr_used > 0 { clr_used } else { 1usize << bpp };
        pixel_offset += palette * 4;
    }
    if pixel_offset > dib.len() {
        return None;
    }
    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    let img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

#[cfg(target_os = "windows")]
fn write_clipboard_windows(text: &str) -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    let wcs: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wcs.len() * 2;

    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
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
    true
}

#[cfg(target_os = "windows")]
fn write_clipboard_windows_image(png: &[u8]) -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_DIB;

    let Some(dib) = png_to_dib(png) else {
        return false;
    };

    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let mut ok = false;
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, dib.len()) {
            let ptr = GlobalLock(hmem);
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(dib.as_ptr(), ptr as *mut u8, dib.len());
                let _ = GlobalUnlock(hmem);
                ok = SetClipboardData(CF_DIB.0 as u32, HANDLE(hmem.0)).is_ok();
            }
        }
        let _ = CloseClipboard();
        ok
    }
}

#[cfg(target_os = "windows")]
fn write_clipboard_windows_files(paths: &[String]) -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::CF_HDROP;
    use windows::Win32::UI::Shell::DROPFILES;

    let mut wide: Vec<u16> = Vec::new();
    for path in paths {
        wide.extend(path.encode_utf16());
        wide.push(0);
    }
    wide.push(0); // double-null terminated list
    let header = std::mem::size_of::<DROPFILES>();
    let byte_len = header + wide.len() * 2;

    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let mut ok = false;
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem) as *mut u8;
            if !ptr.is_null() {
                std::ptr::write_bytes(ptr, 0, byte_len);
                let dropfiles = ptr as *mut DROPFILES;
                (*dropfiles).pFiles = header as u32;
                (*dropfiles).fWide = 1;
                std::ptr::copy_nonoverlapping(
                    wide.as_ptr() as *const u8,
                    ptr.add(header),
                    wide.len() * 2,
                );
                let _ = GlobalUnlock(hmem);
                ok = SetClipboardData(CF_HDROP.0 as u32, HANDLE(hmem.0)).is_ok();
            }
        }
        let _ = CloseClipboard();
        ok
    }
}

/// Encode PNG as a BMP and strip the 14-byte file header, yielding CF_DIB
/// bytes the Windows clipboard expects.
#[cfg(target_os = "windows")]
fn png_to_dib(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut bmp = Vec::new();
    let encoder = image::codecs::bmp::BmpEncoder::new(&mut bmp);
    encoder
        .encode(
            img.as_bytes(),
            img.width(),
            img.height(),
            img.color().into(),
        )
        .ok()?;
    if bmp.len() <= 14 {
        return None;
    }
    Some(bmp[14..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_entry(id: &str, content: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: id.into(),
            kind: EntryKind::Text,
            content: content.into(),
            preview: content.into(),
            copied_at: now_secs(),
            byte_size: 0,
            fingerprint: fingerprint_bytes(content.as_bytes()),
        }
    }

    #[test]
    fn prune_removes_expired_entries() {
        let now = now_secs();
        let old = ClipboardEntry {
            copied_at: now - EXPIRY_SECS - 1,
            ..text_entry("a", "old")
        };
        let mut entries = vec![old, text_entry("b", "new")];
        prune_expired(&mut entries, 50);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "b");
    }

    #[test]
    fn prune_caps_to_limit() {
        let mut entries: Vec<ClipboardEntry> = (0..60)
            .map(|i| text_entry(&format!("e{i}"), &format!("c{i}")))
            .collect();
        prune_expired(&mut entries, 50);
        assert_eq!(entries.len(), 50);
        prune_expired(&mut entries, 10);
        assert_eq!(entries.len(), 10);
    }

    #[test]
    fn add_entry_accumulates_different_content() {
        let mut entries = Vec::new();
        assert!(add_entry(&mut entries, ClipboardPayload::Text("a".into()), 50));
        assert!(add_entry(&mut entries, ClipboardPayload::Text("b".into()), 50));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "b");
        assert_eq!(entries[1].content, "a");
    }

    #[test]
    fn add_entry_dedups_same_content() {
        let mut entries = Vec::new();
        assert!(add_entry(&mut entries, ClipboardPayload::Text("a".into()), 50));
        assert!(!add_entry(&mut entries, ClipboardPayload::Text("a".into()), 50));
        assert_eq!(entries.len(), 1);
        // A duplicate further back moves to the front without a second row.
        assert!(add_entry(&mut entries, ClipboardPayload::Text("b".into()), 50));
        assert!(add_entry(&mut entries, ClipboardPayload::Text("a".into()), 50));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "a");
    }

    #[test]
    fn add_entry_rejects_blank_text() {
        let mut entries = Vec::new();
        assert!(!add_entry(&mut entries, ClipboardPayload::Text("   ".into()), 50));
        assert!(entries.is_empty());
    }

    #[test]
    fn files_preview_lists_names() {
        let preview = files_preview(&["/tmp/one.txt".into(), "/tmp/two.md".into()]);
        assert_eq!(preview, "one.txt, two.md");
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
            kind: EntryKind::Image,
            content: String::new(),
            preview: "preview".into(),
            copied_at: 1234567890,
            byte_size: 42,
            fingerprint: 7,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"copiedAt\""));
        assert!(json.contains("\"byteSize\""));
        assert!(!json.contains("\"copied_at\""));
    }

    #[test]
    fn legacy_json_without_kind_parses_as_text() {
        let json = r#"[{"id":"x","content":"hello","preview":"hello","copiedAt":1700000000}]"#;
        let mut entries: Vec<ClipboardEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, EntryKind::Text);
        assert_eq!(entries[0].fingerprint, 0);
        // A legacy row still dedups against a matching new payload.
        assert!(!add_entry(&mut entries, ClipboardPayload::Text("hello".into()), 50));
    }
}
