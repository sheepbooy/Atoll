//! Session chat/transcript reading: JSONL transcript parsing with a
//! process-local cache, the trusted-path allowlist, the ZCode sqlite chat
//! store reader, and the `get_session_transcript` / `get_session_chat`
//! commands.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::{
    discover_cursor_agent_transcript, is_safe_zcode_session_id, lock_state, transcript,
    zcode_rollout_path, AgentKind, AppState, KnownSession, PermissionRequest,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatMessage {
    pub(crate) role: String,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_input: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_output: Option<String>,
}

pub(crate) const TRANSCRIPT_MAX_MESSAGES: usize = 50;
pub(crate) const TRANSCRIPT_CACHE_MAX_ENTRIES: usize = 128;
pub(crate) const TRANSCRIPT_INITIAL_TAIL_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const TRANSCRIPT_MESSAGE_MAX_CHARS: usize = 64 * 1024;
pub(crate) const TRANSCRIPT_TOOL_INPUT_MAX_BYTES: usize = 256 * 1024;
pub(crate) const TRANSCRIPT_EXTENSIONS: &[&str] = &["jsonl", "json"];

#[derive(Clone)]
pub(crate) struct TranscriptCacheEntry {
    pub(crate) file_len: u64,
    pub(crate) modified: Option<std::time::SystemTime>,
    pub(crate) read_offset: u64,
    pub(crate) carry: Vec<u8>,
    pub(crate) format: transcript::TranscriptFormat,
    pub(crate) messages: VecDeque<ChatMessage>,
    pub(crate) last_access: u64,
}

#[derive(Default)]
pub(crate) struct TranscriptCache {
    pub(crate) entries: HashMap<PathBuf, TranscriptCacheEntry>,
    pub(crate) access_clock: u64,
}

pub(crate) fn has_parent_dir_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

pub(crate) fn has_transcript_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            TRANSCRIPT_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

pub(crate) fn canonicalize_requested_transcript_path(
    transcript_path: &str,
) -> Result<PathBuf, String> {
    let path = Path::new(transcript_path);
    if !path.is_absolute() {
        return Err("Transcript path must be absolute".into());
    }
    if has_parent_dir_component(path) {
        return Err("Transcript path cannot contain parent directory components".into());
    }
    if !has_transcript_extension(path) {
        return Err("Transcript path must point to a transcript file".into());
    }

    let canonical = dunce::canonicalize(path)
        .map_err(|error| format!("Cannot resolve transcript path: {error}"))?;
    if !canonical.is_file() {
        return Err("Transcript path must point to a file".into());
    }
    if !has_transcript_extension(&canonical) {
        return Err("Transcript path must point to a transcript file".into());
    }
    Ok(canonical)
}

pub(crate) fn collect_trusted_transcript_path_strings(state: &AppState) -> Vec<String> {
    let mut paths = Vec::new();

    if let Ok(requests) = state.requests.lock() {
        paths.extend(
            requests
                .iter()
                .filter_map(|request| request.transcript_path.clone()),
        );
    }

    if let Ok(known_sessions) = state.known_sessions.lock() {
        paths.extend(
            known_sessions
                .values()
                .filter_map(|session| session.transcript_path.clone()),
        );
    }

    if let Ok(active_subagents) = state.active_subagents.lock() {
        paths.extend(
            active_subagents
                .iter()
                .filter_map(|subagent| subagent.agent_transcript_path.clone()),
        );
    }

    paths
}

pub(crate) fn trusted_transcript_paths(state: &AppState) -> HashSet<PathBuf> {
    collect_trusted_transcript_path_strings(state)
        .into_iter()
        .filter_map(|path| canonicalize_requested_transcript_path(&path).ok())
        .collect()
}

pub(crate) fn validate_trusted_transcript_path(
    state: &AppState,
    transcript_path: &str,
) -> Result<PathBuf, String> {
    let canonical = canonicalize_requested_transcript_path(transcript_path)?;
    if trusted_transcript_paths(state).contains(&canonical) {
        return Ok(canonical);
    }
    Err("Transcript path is not associated with a known session".into())
}

pub(crate) fn push_transcript_message(messages: &mut VecDeque<ChatMessage>, message: ChatMessage) {
    messages.push_back(message);
    if messages.len() > TRANSCRIPT_MAX_MESSAGES {
        messages.pop_front();
    }
}

pub(crate) fn truncate_transcript_content(content: String) -> String {
    if content.chars().count() <= TRANSCRIPT_MESSAGE_MAX_CHARS {
        return content;
    }
    let mut truncated: String = content.chars().take(TRANSCRIPT_MESSAGE_MAX_CHARS).collect();
    truncated.push_str("\n… [message truncated by Atoll]");
    truncated
}

pub(crate) fn parse_transcript_line(
    format: transcript::TranscriptFormat,
    line: &str,
) -> Option<ChatMessage> {
    if format == transcript::TranscriptFormat::Codex {
        let parsed = transcript::parse_codex_message_line(line)?;
        return Some(ChatMessage {
            role: parsed.role,
            content: truncate_transcript_content(parsed.content),
            tool_name: parsed.tool_name,
            tool_input: None,
            tool_output: None,
        });
    }

    let entry: Value = serde_json::from_str(line.trim()).ok()?;
    let role = entry
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| entry.get("role").and_then(Value::as_str))?;
    let role = match role {
        "human" | "user" => "user",
        "assistant" => "assistant",
        _ => return None,
    };
    let content = truncate_transcript_content(extract_transcript_text(&entry));
    let (tool_name, mut tool_input) = if role == "assistant" {
        extract_tool_use_from_entry(&entry)
    } else {
        (None, None)
    };
    if tool_input.as_ref().is_some_and(|input| {
        serde_json::to_vec(input)
            .map(|bytes| bytes.len() > TRANSCRIPT_TOOL_INPUT_MAX_BYTES)
            .unwrap_or(true)
    }) {
        tool_input = Some(json!({ "truncated": true }));
    }
    if content.is_empty() && tool_name.is_none() {
        return None;
    }
    Some(ChatMessage {
        role: role.into(),
        content,
        tool_name,
        tool_input,
        tool_output: None,
    })
}

pub(crate) fn format_from_transcript_bytes(bytes: &[u8]) -> transcript::TranscriptFormat {
    String::from_utf8_lossy(bytes)
        .lines()
        .find_map(transcript::detect_transcript_format_from_line)
        .unwrap_or(transcript::TranscriptFormat::Claude)
}

pub(crate) fn parse_transcript_bytes(entry: &mut TranscriptCacheEntry, bytes: &[u8]) {
    let mut combined = std::mem::take(&mut entry.carry);
    combined.extend_from_slice(bytes);
    let complete_len = combined
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    for line in String::from_utf8_lossy(&combined[..complete_len]).lines() {
        if let Some(message) = parse_transcript_line(entry.format, line) {
            push_transcript_message(&mut entry.messages, message);
        }
    }
    entry.carry = combined[complete_len..].to_vec();
}

pub(crate) fn read_transcript_messages_cached(
    state: &AppState,
    transcript_path: &Path,
) -> Result<Vec<ChatMessage>, String> {
    use std::io::{Read, Seek, SeekFrom};

    let metadata = std::fs::metadata(transcript_path)
        .map_err(|error| format!("Cannot stat transcript: {error}"))?;
    let file_len = metadata.len();
    let modified = metadata.modified().ok();
    let cached = state
        .transcript_cache
        .lock()
        .ok()
        .and_then(|cache| cache.entries.get(transcript_path).cloned());

    if let Some(entry) = cached.as_ref() {
        if entry.file_len == file_len && entry.read_offset >= file_len && entry.modified == modified
        {
            return Ok(entry.messages.iter().cloned().collect());
        }
    }

    let append_only = cached
        .as_ref()
        .is_some_and(|entry| file_len > entry.file_len && entry.read_offset == entry.file_len);
    let start = if append_only {
        cached.as_ref().map(|entry| entry.read_offset).unwrap_or(0)
    } else {
        file_len.saturating_sub(TRANSCRIPT_INITIAL_TAIL_BYTES)
    };
    let mut file = std::fs::File::open(transcript_path)
        .map_err(|error| format!("Cannot open transcript: {error}"))?;
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("Cannot seek transcript: {error}"))?;
    let mut bytes =
        Vec::with_capacity((file_len - start).min(TRANSCRIPT_INITIAL_TAIL_BYTES) as usize);
    file.take(TRANSCRIPT_INITIAL_TAIL_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read transcript: {error}"))?;
    let next_offset = start.saturating_add(bytes.len() as u64);

    if !append_only && start > 0 {
        if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=newline);
        } else {
            bytes.clear();
        }
    }

    let format = cached
        .as_ref()
        .filter(|_| append_only)
        .map(|entry| entry.format)
        .unwrap_or_else(|| format_from_transcript_bytes(&bytes));
    let mut entry = if append_only {
        cached.unwrap()
    } else {
        TranscriptCacheEntry {
            file_len,
            modified,
            read_offset: start,
            carry: Vec::new(),
            format,
            messages: VecDeque::new(),
            last_access: 0,
        }
    };
    parse_transcript_bytes(&mut entry, &bytes);
    if next_offset >= file_len && !entry.carry.is_empty() {
        let final_line = String::from_utf8_lossy(&entry.carry).into_owned();
        if let Some(message) = parse_transcript_line(entry.format, &final_line) {
            push_transcript_message(&mut entry.messages, message);
            entry.carry.clear();
        }
    }
    entry.file_len = next_offset;
    entry.modified = modified;
    entry.read_offset = next_offset;

    let result: Vec<ChatMessage> = entry.messages.iter().cloned().collect();
    if let Ok(mut cache) = state.transcript_cache.lock() {
        cache.access_clock = cache.access_clock.wrapping_add(1);
        entry.last_access = cache.access_clock;
        cache.entries.insert(transcript_path.to_path_buf(), entry);
        if cache.entries.len() > TRANSCRIPT_CACHE_MAX_ENTRIES {
            if let Some(oldest) = cache
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(path, _)| path.clone())
            {
                cache.entries.remove(&oldest);
            }
        }
    }
    Ok(result)
}

/// Resolve a session's transcript file, checking known state, requests, and on-disk discovery.
pub(crate) fn resolve_session_transcript_path(
    state: &AppState,
    session_id: &str,
    requests: &[PermissionRequest],
) -> Option<String> {
    if let Ok(known) = state.known_sessions.lock() {
        if let Some(entry) = known.get(session_id) {
            if matches!(entry.agent, AgentKind::Zcode) {
                return zcode_db_session_path(session_id);
            }
            if let Some(path) = entry.transcript_path.clone() {
                if std::path::Path::new(&path).is_file() {
                    return Some(path);
                }
            }
            if let Some(ref conv_id) = entry.conversation_id {
                if let Some((path, _)) = discover_cursor_agent_transcript(conv_id) {
                    return Some(path);
                }
            }
        }
    }

    for request in requests {
        if request.session == session_id {
            if matches!(request.agent, AgentKind::Zcode) {
                return zcode_db_session_path(session_id);
            }
            if let Some(path) = request.transcript_path.clone() {
                if std::path::Path::new(&path).is_file() {
                    return Some(path);
                }
            }
        }
    }

    discover_cursor_agent_transcript(session_id).map(|(path, _)| path)
}

pub(crate) fn resolve_session_transcript_path_from_snapshot(
    known_sessions: &HashMap<String, KnownSession>,
    requests: &[PermissionRequest],
    session_id: &str,
    _agent: &AgentKind,
) -> Option<String> {
    if let Some(entry) = known_sessions.get(session_id) {
        if let Some(path) = entry.transcript_path.clone() {
            return Some(path);
        }
    }

    for request in requests {
        if !request.archived && request.session == session_id {
            if let Some(path) = request.transcript_path.clone() {
                return Some(path);
            }
        }
    }

    None
}

pub(crate) fn persist_session_transcript_path(state: &AppState, session_id: &str, path: &str) {
    if let Ok(mut known) = state.known_sessions.lock() {
        if let Some(entry) = known.get_mut(session_id) {
            entry.transcript_path = Some(path.to_string());
            if entry.conversation_id.is_none() {
                if let Some(stem) = std::path::Path::new(path)
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|name| name.to_str())
                {
                    entry.conversation_id = Some(stem.to_string());
                }
            }
        }
    }
}
#[tauri::command]
pub(crate) async fn get_session_transcript(
    app: AppHandle,
    transcript_path: String,
) -> Result<Vec<ChatMessage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(session_id) = parse_zcode_db_session_path(&transcript_path) {
            let result = read_zcode_chat_messages(session_id);
            if let Err(error) = &result {
                eprintln!("Atoll chat read failed for {session_id}: {error}");
            }
            return result;
        }
        let state = app.state::<AppState>();
        let canonical = validate_trusted_transcript_path(&state, &transcript_path);
        if let Err(error) = &canonical {
            eprintln!("Atoll transcript path rejected ({transcript_path}): {error}");
        }
        read_transcript_messages_cached(&state, &canonical?)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn get_session_chat(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<ChatMessage>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let requests = lock_state(&state.requests).clone();
        let path =
            resolve_session_transcript_path(&state, &session_id, &requests).ok_or_else(|| {
                eprintln!("Atoll chat: no transcript resolved for session {session_id}");
                format!("No transcript found for session {session_id}")
            })?;
        if let Some(session_id) = parse_zcode_db_session_path(&path) {
            let result = read_zcode_chat_messages(session_id);
            if let Err(error) = &result {
                eprintln!("Atoll chat read failed for {session_id}: {error}");
            }
            return result;
        }
        persist_session_transcript_path(&state, &session_id, &path);
        let canonical = std::fs::canonicalize(&path)
            .map_err(|error| format!("Cannot resolve transcript path: {error}"))?;
        read_transcript_messages_cached(&state, &canonical)
    })
    .await
    .map_err(|error| error.to_string())?
}
pub(crate) fn extract_transcript_text(entry: &Value) -> String {
    if let Some(message) = entry.get("message") {
        if let Some(content) = message.get("content") {
            if let Some(text) = content.as_str() {
                return text.to_string();
            }
            if let Some(arr) = content.as_array() {
                let parts: Vec<&str> = arr
                    .iter()
                    .filter_map(|block| {
                        if block.get("type")?.as_str()? == "text" {
                            block.get("text").and_then(Value::as_str)
                        } else {
                            None
                        }
                    })
                    .collect();
                return parts.join("\n");
            }
        }
    }
    String::new()
}

pub(crate) fn extract_tool_use_from_entry(entry: &Value) -> (Option<String>, Option<Value>) {
    entry
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .and_then(|arr| {
            arr.iter().find_map(|block| {
                if block.get("type")?.as_str()? == "tool_use" {
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .map(String::from)?;
                    let input = block.get("input").cloned();
                    Some((Some(name), input))
                } else {
                    None
                }
            })
        })
        .unwrap_or((None, None))
}

pub(crate) fn collect_session_transcript_paths(
    requests: &[PermissionRequest],
    known_sessions: &HashMap<String, KnownSession>,
) -> Vec<(String, String, AgentKind)> {
    let mut session_paths: HashMap<String, (String, AgentKind)> = HashMap::new();
    for request in requests {
        if request.archived {
            continue;
        }
        let Some(transcript_path) = request.transcript_path.as_deref() else {
            continue;
        };
        session_paths
            .entry(request.session.clone())
            .or_insert_with(|| (transcript_path.to_string(), request.agent.clone()));
    }
    for (session_id, known_session) in known_sessions {
        // ZCode transcript paths from hook payloads are ephemeral temp files;
        // track the durable rollout file instead.
        let transcript_path = if matches!(known_session.agent, AgentKind::Zcode) {
            zcode_rollout_path(session_id).map(|path| path.to_string_lossy().into_owned())
        } else {
            known_session.transcript_path.clone()
        };
        let Some(transcript_path) = transcript_path else {
            continue;
        };
        session_paths
            .entry(session_id.clone())
            .or_insert_with(|| (transcript_path, known_session.agent.clone()));
    }
    session_paths
        .into_iter()
        .map(|(session_id, (path, agent))| (session_id, path, agent))
        .collect()
}
/// Virtual transcript path for ZCode sessions: their chat history lives in
/// ZCode's sqlite store (`~/.zcode/cli/db/db.sqlite`), not in a per-session
/// file, so the scheme encodes the session id for the chat reader.
pub(crate) const ZCODE_DB_SCHEME: &str = "zcode-db://";

pub(crate) fn zcode_db_session_path(session_id: &str) -> Option<String> {
    if !is_safe_zcode_session_id(session_id) {
        return None;
    }
    Some(format!("{ZCODE_DB_SCHEME}{session_id}"))
}

pub(crate) fn parse_zcode_db_session_path(path: &str) -> Option<&str> {
    let session_id = path.strip_prefix(ZCODE_DB_SCHEME)?;
    if !is_safe_zcode_session_id(session_id) {
        return None;
    }
    Some(session_id)
}

pub(crate) fn zcode_db_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".zcode").join("cli").join("db").join("db.sqlite"))
}

/// Read a ZCode session's chat history from its sqlite store. Each `message`
/// row carries the role; its ordered `part` rows hold text, reasoning, and
/// tool-call content. Only user-visible text and tool calls become chat
/// bubbles — reasoning traces and synthetic system reminders are skipped,
/// and only the newest TRANSCRIPT_MAX_MESSAGES bubbles are returned.
pub(crate) fn read_zcode_chat_messages(session_id: &str) -> Result<Vec<ChatMessage>, String> {
    if !is_safe_zcode_session_id(session_id) {
        return Err("Invalid ZCode session id".to_string());
    }
    let Some(db_path) = zcode_db_path() else {
        return Err("Cannot locate ZCode database".to_string());
    };
    if !db_path.is_file() {
        return Err("ZCode database not found".to_string());
    }

    let connection =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| format!("Cannot open ZCode database: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_millis(500))
        .map_err(|error| format!("Cannot query ZCode database: {error}"))?;

    let mut statement = connection
        .prepare(
            "SELECT m.id AS message_id, m.data AS message_data, p.data AS part_data
             FROM message m
             LEFT JOIN part p ON p.message_id = m.id
             WHERE m.session_id = ?1
             ORDER BY m.sequence ASC, p.sequence ASC",
        )
        .map_err(|error| format!("Cannot query ZCode database: {error}"))?;
    let mut rows = statement
        .query(rusqlite::params![session_id])
        .map_err(|error| format!("Cannot query ZCode database: {error}"))?;

    let mut messages: Vec<ChatMessage> = Vec::new();
    let mut current_message_id: Option<String> = None;
    let mut current_role = String::new();
    let mut current_content = String::new();

    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Cannot read ZCode database: {error}"))?
    {
        let message_id: String = row
            .get("message_id")
            .map_err(|error| format!("Cannot read ZCode database: {error}"))?;
        if current_message_id.as_deref() != Some(message_id.as_str()) {
            flush_zcode_chat_message(&mut messages, &current_role, &current_content);
            current_message_id = Some(message_id);
            current_content.clear();
            let message_data: String = row
                .get("message_data")
                .map_err(|error| format!("Cannot read ZCode database: {error}"))?;
            let message: Value = serde_json::from_str(&message_data)
                .map_err(|error| format!("Cannot parse ZCode message: {error}"))?;
            current_role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }

        let Some(part_data) = row
            .get::<_, Option<String>>("part_data")
            .map_err(|error| format!("Cannot read ZCode database: {error}"))?
        else {
            continue;
        };
        let part: Value = match serde_json::from_str(&part_data) {
            Ok(part) => part,
            Err(_) => continue,
        };
        match zcode_chat_part(&part) {
            ZcodeChatPart::Text(text) => {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(&text);
            }
            ZcodeChatPart::Tool {
                name,
                input,
                output,
            } => {
                flush_zcode_chat_message(&mut messages, &current_role, &current_content);
                current_content.clear();
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                    tool_name: Some(name),
                    tool_input: input,
                    tool_output: output,
                });
            }
            ZcodeChatPart::Skip => {}
        }
    }
    flush_zcode_chat_message(&mut messages, &current_role, &current_content);

    if messages.len() > TRANSCRIPT_MAX_MESSAGES {
        let keep = messages.len() - TRANSCRIPT_MAX_MESSAGES;
        messages.drain(..keep);
    }

    Ok(messages)
}

pub(crate) enum ZcodeChatPart {
    Text(String),
    Tool {
        name: String,
        input: Option<Value>,
        output: Option<String>,
    },
    Skip,
}

pub(crate) fn zcode_chat_part(part: &Value) -> ZcodeChatPart {
    let part_type = part.get("type").and_then(Value::as_str).unwrap_or("");
    match part_type {
        "text" => {
            if part
                .get("synthetic")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return ZcodeChatPart::Skip;
            }
            let text = part
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if text.is_empty() {
                ZcodeChatPart::Skip
            } else {
                ZcodeChatPart::Text(text)
            }
        }
        "tool" => {
            let Some(name) = part.get("tool").and_then(Value::as_str) else {
                return ZcodeChatPart::Skip;
            };
            let state = part.get("state");
            let input = state
                .and_then(|state| state.get("input"))
                .filter(|input| !input.is_null())
                .cloned();
            // The user's answers to an AskUserQuestion only exist in the tool
            // result text; other tool outputs stay out of the transcript.
            let output = if name == "AskUserQuestion" {
                state
                    .and_then(|state| state.get("output"))
                    .and_then(Value::as_str)
                    .map(|text| truncate_transcript_content(text.trim().to_string()))
                    .filter(|text| !text.is_empty())
            } else {
                None
            };
            ZcodeChatPart::Tool {
                name: name.to_string(),
                input,
                output,
            }
        }
        _ => ZcodeChatPart::Skip,
    }
}

pub(crate) fn flush_zcode_chat_message(messages: &mut Vec<ChatMessage>, role: &str, content: &str) {
    let mapped = match role {
        "user" => "user",
        "assistant" => "assistant",
        "system" => "system",
        _ => return,
    };
    if content.trim().is_empty() {
        return;
    }
    messages.push(ChatMessage {
        role: mapped.to_string(),
        content: truncate_transcript_content(content.to_string()),
        tool_name: None,
        tool_input: None,
        tool_output: None,
    });
}
