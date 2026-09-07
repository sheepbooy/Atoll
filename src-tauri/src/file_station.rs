//! File staging station ("文件中转站").
//!
//! Files dropped onto the island are recorded as references: the original
//! file is never moved, copied or deleted. The in-memory list owned by the
//! app state is the source of truth; it is mirrored to
//! `~/.atoll/file_station.json` with atomic writes. Entries never expire;
//! the list is capped at `STAGED_FILES_LIMIT` and overflowing drops evict the
//! oldest references (references only — the referenced files stay put).
//!
//! Whether a reference still resolves is checked on every read-out
//! (`StagedFileView::lost`); entries whose file vanished are kept until the
//! user removes them so a temporarily unmounted volume doesn't wipe the list.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum staged references kept; oldest are evicted when exceeded.
pub const STAGED_FILES_LIMIT: usize = 30;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedFile {
    pub id: String,
    /// Canonicalized absolute path of the referenced file (dedup key).
    pub path: String,
    pub file_name: String,
    /// File size in bytes (0 for directories).
    pub byte_size: u64,
    pub is_dir: bool,
    /// ms since the Unix epoch when the file was staged.
    pub staged_at: u64,
}

/// Read-out shape sent to the frontend; `lost` is computed per request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedFileView {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub byte_size: u64,
    pub is_dir: bool,
    pub staged_at: u64,
    pub lost: bool,
}

/// Result of a drop reported back to the frontend (toast copy + animation).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageFilesResult {
    pub files: Vec<StagedFileView>,
    /// Paths staged by this drop (deduped re-stages count as added).
    pub added: usize,
    /// Paths rejected because they no longer exist on disk.
    pub skipped: usize,
    /// Oldest references evicted to stay under the cap.
    pub evicted: usize,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn station_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("file_station.json"))
}

pub fn load_history() -> Vec<StagedFile> {
    let Some(path) = station_path() else {
        return Vec::new();
    };
    load_list(&path)
}

/// Load the persisted list; an unreadable file is archived (not deleted) so
/// a corrupt write can never silently masquerade as an empty station.
pub fn load_list(path: &Path) -> Vec<StagedFile> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    if content.trim().is_empty() {
        return Vec::new();
    }
    match serde_json::from_str::<Vec<StagedFile>>(&content) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("atoll: file station unreadable ({err}); archiving");
            let _ = std::fs::rename(path, path.with_extension("json.corrupt"));
            Vec::new()
        }
    }
}

/// Write-then-rename keeps a hard kill from leaving a truncated file.
pub fn save_list(path: &Path, entries: &[StagedFile]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(formatted) = serde_json::to_string_pretty(entries) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, formatted).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

pub fn save_history(entries: &[StagedFile]) {
    if let Some(path) = station_path() {
        save_list(&path, entries);
    }
}

/// Snapshot for the frontend with live `lost` flags, newest first.
pub fn views(entries: &[StagedFile]) -> Vec<StagedFileView> {
    entries
        .iter()
        .map(|entry| StagedFileView {
            id: entry.id.clone(),
            path: entry.path.clone(),
            file_name: entry.file_name.clone(),
            byte_size: entry.byte_size,
            is_dir: entry.is_dir,
            staged_at: entry.staged_at,
            lost: !Path::new(&entry.path).exists(),
        })
        .collect()
}

/// Stage dropped paths as references: skip blank/missing paths, canonicalize
/// for dedup (re-staging refreshes `staged_at` and keeps the original id),
/// insert newest-first, then evict the oldest entries beyond `limit`.
pub fn stage_paths(entries: &mut Vec<StagedFile>, paths: &[String], limit: usize) -> StageFilesResult {
    let limit = limit.max(1);
    let mut added = 0usize;
    let mut skipped = 0usize;
    for raw in paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        // canonicalize resolves symlinks (/tmp → /private/tmp) so the same
        // file dropped via two spellings dedups; it also proves existence.
        let Ok(canonical) = std::fs::canonicalize(trimmed) else {
            skipped += 1;
            continue;
        };
        let path = canonical.to_string_lossy().into_owned();
        let meta = std::fs::metadata(&canonical);
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let byte_size = meta
            .as_ref()
            .map(|m| if m.is_dir() { 0 } else { m.len() })
            .unwrap_or(0);
        let staged_at = now_millis();
        if let Some(existing) = entries.iter_mut().find(|e| e.path == path) {
            existing.staged_at = staged_at;
        } else {
            entries.insert(
                0,
                StagedFile {
                    id: uuid::Uuid::new_v4().to_string(),
                    file_name: file_name_of(&path),
                    path,
                    byte_size,
                    is_dir,
                    staged_at,
                },
            );
        }
        added += 1;
    }
    // Re-staging refreshes staged_at in place, so order can drift from the
    // newest-first invariant; re-sort before evicting from the tail.
    entries.sort_by(|a, b| b.staged_at.cmp(&a.staged_at));
    let evicted = entries.len().saturating_sub(limit);
    if evicted > 0 {
        entries.truncate(limit);
    }
    StageFilesResult {
        files: views(entries),
        added,
        skipped,
        evicted,
    }
}

/// Drop one reference by id; returns the removed entry so callers can report.
pub fn remove_entry(entries: &mut Vec<StagedFile>, id: &str) -> Option<StagedFile> {
    let index = entries.iter().position(|e| e.id == id)?;
    Some(entries.remove(index))
}

pub fn clear_entries(entries: &mut Vec<StagedFile>) -> usize {
    let count = entries.len();
    entries.clear();
    count
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(id: &str, path: &str, staged_at: u64) -> StagedFile {
        StagedFile {
            id: id.into(),
            path: path.into(),
            file_name: file_name_of(path),
            byte_size: 1,
            is_dir: false,
            staged_at,
        }
    }

    #[test]
    fn save_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("atoll-fs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("file_station.json");
        let entries = vec![staged("a", "/tmp/one.txt", 100), staged("b", "/tmp/two", 200)];
        save_list(&path, &entries);
        assert_eq!(load_list(&path), entries);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_archives_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("atoll-fs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("file_station.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load_list(&path).is_empty());
        assert!(path.with_extension("json.corrupt").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stage_paths_skips_missing_and_blank() {
        let mut entries = Vec::new();
        let result = stage_paths(&mut entries, &["".into(), "   ".into()], 30);
        assert_eq!(result.added, 0);
        assert_eq!(result.skipped, 0);
        assert!(entries.is_empty());
        // /definitely/not/here cannot be canonicalized → skipped.
        let result = stage_paths(&mut entries, &["/definitely/not/here".into()], 30);
        assert_eq!(result.added, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn stage_paths_stages_real_file_newest_first() {
        let mut entries = Vec::new();
        let result = stage_paths(&mut entries, &[env_exe_path()], 30);
        assert_eq!(result.added, 1);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.starts_with('/'));
        assert!(!entries[0].is_dir);
        assert!(entries[0].byte_size > 0);
    }

    #[test]
    fn stage_paths_dedups_restage_by_canonical_path() {
        let mut entries = Vec::new();
        stage_paths(&mut entries, &[env_exe_path()], 30);
        entries[0].staged_at = 1;
        let before_id = entries[0].id.clone();
        let result = stage_paths(&mut entries, &[env_exe_path()], 30);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, before_id);
        assert!(entries[0].staged_at > 1);
        assert_eq!(result.added, 1);
    }

    #[test]
    fn stage_paths_evicts_oldest_beyond_limit() {
        let mut entries: Vec<StagedFile> = (0..5)
            .map(|i| staged(&format!("e{i}"), &format!("/tmp/f{i}"), 100 + i))
            .collect();
        // Oldest is e0 (staged_at 100); a new drop over the cap evicts it.
        let result = stage_paths(&mut entries, &[env_exe_path()], 5);
        assert_eq!(result.evicted, 1);
        assert_eq!(entries.len(), 5);
        assert!(!entries.iter().any(|e| e.id == "e0"));
        assert!(entries.iter().any(|e| e.id == "e1"));
    }

    #[test]
    fn remove_and_clear_entries() {
        let mut entries = vec![staged("a", "/tmp/one", 1), staged("b", "/tmp/two", 2)];
        assert!(remove_entry(&mut entries, "a").is_some());
        assert!(remove_entry(&mut entries, "missing").is_none());
        assert_eq!(entries.len(), 1);
        assert_eq!(clear_entries(&mut entries), 1);
        assert!(entries.is_empty());
    }

    #[test]
    fn views_flags_missing_files_as_lost() {
        let entries = vec![staged("gone", "/definitely/not/here", 1)];
        assert!(views(&entries)[0].lost);
        let entries = vec![staged("here", &env_exe_path(), 1)];
        assert!(!views(&entries)[0].lost);
    }

    #[test]
    fn staged_file_serializes_camel_case() {
        let json = serde_json::to_string(&staged("a", "/tmp/x", 5)).unwrap();
        assert!(json.contains("\"stagedAt\""));
        assert!(json.contains("\"isDir\""));
        assert!(json.contains("\"byteSize\""));
        assert!(!json.contains("\"staged_at\""));
    }

    /// A guaranteed-real file path: the test binary itself.
    fn env_exe_path() -> String {
        std::env::current_exe().unwrap().to_string_lossy().into_owned()
    }
}
