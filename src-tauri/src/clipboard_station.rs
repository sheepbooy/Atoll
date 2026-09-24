//! Clipboard → file station bridge ("剪贴板 → 中转站").
//!
//! Turns clipboard history entries into staged file references:
//! - `files` entries already reference paths on disk, so they are staged as-is.
//! - `image` entries only exist as PNG blobs under `~/.atoll/clipboard/` that
//!   are deleted when the entry is pruned (24h expiry, limit eviction), so
//!   staging first copies the blob into `~/.atoll/station/` — a durable
//!   location the station owns.
//! - `text` entries stage line-wise when any line resolves to a real path;
//!   otherwise the whole text is written to a `.txt` file in the station dir
//!   so arbitrary copied text gains a file identity.
//!
//! Materialized files are never deleted by the station: removing or clearing
//! references only drops the reference (same semantics as drop-staged files).

use std::path::Path;

use crate::clipboard_history::{self, ClipboardEntry, EntryKind};
use crate::file_station;

/// Directory where clipboard images/text are materialized so their staged
/// references outlive the clipboard history expiry.
fn station_files_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".atoll").join("station"))
}

/// Bridge clipboard history entries into the file station. Images and text
/// are materialized under `~/.atoll/station/`, then every resulting path is
/// staged in one pass so dedup, the cap and eviction behave exactly like a
/// single drop.
pub fn stage_from_clipboard_entries(
    file_entries: &mut Vec<file_station::StagedFile>,
    clip_entries: &[ClipboardEntry],
    limit: usize,
) -> file_station::StageFilesResult {
    match station_files_dir() {
        Some(dir) => stage_entries_in(file_entries, clip_entries, limit, &dir, |id| {
            clipboard_history::read_image_blob(id)
        }),
        None => {
            // Nowhere to materialize; only reference-style paths could work,
            // which the files branch of `entry_paths` would have produced.
            let mut result = file_station::stage_paths_sourced(file_entries, &[], limit, true);
            result.skipped += clip_entries.len();
            result
        }
    }
}

/// The testable core: `blob_of` stands in for the clipboard image store and
/// `station_dir` for `~/.atoll/station/`.
fn stage_entries_in(
    file_entries: &mut Vec<file_station::StagedFile>,
    clip_entries: &[ClipboardEntry],
    limit: usize,
    station_dir: &Path,
    blob_of: impl Fn(&str) -> Option<Vec<u8>>,
) -> file_station::StageFilesResult {
    let mut paths = Vec::new();
    let mut skipped = 0usize;
    for entry in clip_entries {
        match entry_paths(entry, station_dir, &blob_of) {
            Ok(mut entry_paths) => paths.append(&mut entry_paths),
            Err(()) => skipped += 1,
        }
    }
    let mut result = file_station::stage_paths_sourced(file_entries, &paths, limit, true);
    result.skipped += skipped;
    result
}

/// Resolve one clipboard entry into stageable paths, materializing the
/// payload into `station_dir` when it has no file on disk yet.
fn entry_paths(
    entry: &ClipboardEntry,
    station_dir: &Path,
    blob_of: &impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<Vec<String>, ()> {
    match entry.kind {
        EntryKind::Files => Ok(entry
            .content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()),
        EntryKind::Image => {
            let png = blob_of(&entry.id).ok_or(())?;
            materialize(station_dir, &entry.id, "png", &png).map(|path| vec![path])
        }
        EntryKind::Text => {
            let path_lines: Vec<String> = entry
                .content
                .lines()
                .map(str::trim)
                .filter(|line| Path::new(line).exists())
                .map(str::to_string)
                .collect();
            if !path_lines.is_empty() {
                return Ok(path_lines);
            }
            if entry.content.trim().is_empty() {
                return Err(());
            }
            materialize(station_dir, &entry.id, "txt", entry.content.as_bytes())
                .map(|path| vec![path])
        }
    }
}

/// Write clipboard material under `station_dir` as
/// `clipboard-<local-timestamp>-<id6>.<ext>`; the id suffix keeps two
/// materializations within the same second from colliding.
fn materialize(station_dir: &Path, id: &str, ext: &str, bytes: &[u8]) -> Result<String, ()> {
    std::fs::create_dir_all(station_dir).map_err(|_| ())?;
    let short_id: String = id.chars().take(6).collect();
    let name = format!(
        "clipboard-{}-{short_id}.{ext}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = station_dir.join(name);
    std::fs::write(&path, bytes).map_err(|_| ())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_entry(id: &str, kind: EntryKind, content: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: id.into(),
            kind,
            content: content.into(),
            preview: String::new(),
            copied_at: 0,
            byte_size: 0,
            fingerprint: 0,
            favorited: false,
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atoll-clip-station-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // Staged paths are canonicalized (/tmp → /private/tmp on macOS), so
        // prefix assertions must compare against the resolved dir too.
        std::fs::canonicalize(&dir).unwrap()
    }

    fn real_file(tag: &str) -> String {
        let path = temp_dir(tag).join("sample.txt");
        std::fs::write(&path, "hi").unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn files_entry_stages_existing_paths_from_clipboard() {
        let path = real_file("files");
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry("e1", EntryKind::Files, &path)],
            30,
            &temp_dir("files-dir"),
            |_| None,
        );
        assert_eq!(result.added, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(station.len(), 1);
        assert!(station[0].from_clipboard);
    }

    #[test]
    fn image_entry_materializes_png_into_station_dir() {
        let station_dir = temp_dir("image");
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry("abc123xyz", EntryKind::Image, "")],
            30,
            &station_dir,
            |id| (id == "abc123xyz").then(|| b"png-bytes".to_vec()),
        );
        assert_eq!(result.added, 1);
        assert_eq!(station.len(), 1);
        let path = std::path::PathBuf::from(&station[0].path);
        assert!(path.starts_with(&station_dir));
        let name = station[0].file_name.clone();
        assert!(name.starts_with("clipboard-"));
        assert!(name.ends_with("-abc123.png"));
        assert_eq!(std::fs::read(&path).unwrap(), b"png-bytes");
        assert_eq!(station[0].byte_size, 9);
    }

    #[test]
    fn image_entry_without_blob_counts_as_skipped() {
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry("gone", EntryKind::Image, "")],
            30,
            &temp_dir("image-missing"),
            |_| None,
        );
        assert_eq!(result.added, 0);
        assert_eq!(result.skipped, 1);
        assert!(station.is_empty());
    }

    #[test]
    fn text_entry_with_real_path_lines_stages_them() {
        let path = real_file("text-path");
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry(
                "t1",
                EntryKind::Text,
                &format!("{path}\n/not/a/real/path"),
            )],
            30,
            &temp_dir("text-path-dir"),
            |_| None,
        );
        assert_eq!(result.added, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(
            station[0].path,
            std::fs::canonicalize(&path).unwrap().to_string_lossy()
        );
    }

    #[test]
    fn text_entry_without_paths_materializes_txt() {
        let station_dir = temp_dir("text-txt");
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry(
                "txt99",
                EntryKind::Text,
                "just some copied words",
            )],
            30,
            &station_dir,
            |_| None,
        );
        assert_eq!(result.added, 1);
        assert_eq!(station.len(), 1);
        let path = std::path::PathBuf::from(&station[0].path);
        assert!(path.starts_with(&station_dir));
        assert!(station[0].file_name.ends_with("-txt99.txt"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "just some copied words"
        );
    }

    #[test]
    fn blank_text_entry_is_skipped() {
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[clip_entry("blank", EntryKind::Text, "   \n  ")],
            30,
            &temp_dir("blank"),
            |_| None,
        );
        assert_eq!(result.added, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn mixed_batch_dedups_and_counts_skips() {
        let path = real_file("mixed");
        let station_dir = temp_dir("mixed-dir");
        let mut station = Vec::new();
        let result = stage_entries_in(
            &mut station,
            &[
                clip_entry("m1", EntryKind::Files, &path),
                // Same file again → dedup refresh, still one entry.
                clip_entry("m2", EntryKind::Files, &path),
                // Blob missing → skip.
                clip_entry("m3", EntryKind::Image, ""),
            ],
            30,
            &station_dir,
            |_| None,
        );
        assert_eq!(result.added, 2);
        assert_eq!(result.skipped, 1);
        assert_eq!(station.len(), 1);
        assert!(station[0].from_clipboard);
    }

    #[test]
    fn restage_refreshes_source_to_clipboard() {
        let path = real_file("restage");
        let mut station = Vec::new();
        file_station::stage_paths(&mut station, &[path.clone()], 30);
        assert!(!station[0].from_clipboard);
        stage_entries_in(
            &mut station,
            &[clip_entry("r1", EntryKind::Files, &path)],
            30,
            &temp_dir("restage-dir"),
            |_| None,
        );
        assert_eq!(station.len(), 1);
        assert!(station[0].from_clipboard);
    }
}
