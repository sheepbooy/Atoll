//! Token usage accounting: per-agent transcript parsing, delta ingestion,
//! session/daily aggregation with midnight rollover, and model-rate costing.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use std::sync::atomic::Ordering;
use tauri::AppHandle;

use super::{
    extract_cursor_model, first_json_u64, local_time, pricing, token_history, transcript,
    zcode_rollout_path, AgentKind, AppState, TokenUsage,
};

pub(crate) fn effective_daily_tokens(
    session_token_usage: &HashMap<String, TokenUsage>,
    startup_floor: TokenUsage,
    absolute_sessions: &HashSet<String>,
) -> TokenUsage {
    let mut absolute_sum = TokenUsage::default();
    let mut incremental_sum = TokenUsage::default();

    for (session_id, usage) in session_token_usage {
        if absolute_sessions.contains(session_id) {
            absolute_sum.add_assign(*usage);
        } else {
            incremental_sum.add_assign(*usage);
        }
    }

    if absolute_sessions.is_empty() {
        let mut total = startup_floor;
        total.add_assign(incremental_sum);
        return total;
    }

    let mut total = startup_floor.component_wise_max(absolute_sum);
    total.add_assign(incremental_sum);
    total
}

/// Same restart-floor semantics as [`effective_daily_tokens`], keyed by model.
pub(crate) fn effective_daily_tokens_by_model(
    session_usage_by_model: &HashMap<String, HashMap<String, TokenUsage>>,
    startup_floor: &HashMap<String, TokenUsage>,
    absolute_sessions: &HashSet<String>,
) -> HashMap<String, TokenUsage> {
    let mut absolute_sum: HashMap<String, TokenUsage> = HashMap::new();
    let mut incremental_sum: HashMap<String, TokenUsage> = HashMap::new();

    for (session_id, usage_by_model) in session_usage_by_model {
        let target = if absolute_sessions.contains(session_id) {
            &mut absolute_sum
        } else {
            &mut incremental_sum
        };
        for (model_id, usage) in usage_by_model {
            target
                .entry(model_id.clone())
                .or_default()
                .add_assign(*usage);
        }
    }

    if absolute_sessions.is_empty() {
        let mut total = startup_floor.clone();
        for (model_id, usage) in incremental_sum {
            total.entry(model_id).or_default().add_assign(usage);
        }
        return total;
    }

    let mut total = startup_floor.clone();
    for (model_id, usage) in absolute_sum {
        let entry = total.entry(model_id).or_default();
        *entry = entry.component_wise_max(usage);
    }
    for (model_id, usage) in incremental_sum {
        total.entry(model_id).or_default().add_assign(usage);
    }
    total
}

pub(crate) fn merge_session_model_usage(
    target: &mut HashMap<String, TokenUsage>,
    source: &HashMap<String, TokenUsage>,
    is_full_scan: bool,
) {
    for (model_id, usage) in source {
        let entry = target.entry(model_id.clone()).or_default();
        if is_full_scan {
            *entry = entry.component_wise_max(*usage);
        } else {
            entry.add_assign(*usage);
        }
    }
}

pub(crate) fn token_usage_from_delta(delta: transcript::TokenUsageDelta) -> TokenUsage {
    TokenUsage {
        input_tokens: delta.input_tokens,
        output_tokens: delta.output_tokens,
        cache_read_tokens: delta.cache_read_tokens,
        cache_creation_tokens: delta.cache_creation_tokens,
    }
}

pub(crate) fn token_usage_map_from_delta_map(
    source: &HashMap<String, transcript::TokenUsageDelta>,
) -> HashMap<String, TokenUsage> {
    source
        .iter()
        .map(|(model_id, delta)| (model_id.clone(), token_usage_from_delta(*delta)))
        .collect()
}

pub(crate) fn aggregate_usage_by_model(
    session_usage_by_model: &HashMap<String, HashMap<String, TokenUsage>>,
    session_filter: Option<&HashSet<&str>>,
) -> HashMap<String, TokenUsage> {
    let mut totals = HashMap::new();
    for (session_id, usage_by_model) in session_usage_by_model {
        if let Some(filter) = session_filter {
            if !filter.contains(session_id.as_str()) {
                continue;
            }
        }
        for (model_id, usage) in usage_by_model {
            totals
                .entry(model_id.clone())
                .or_insert(TokenUsage::default())
                .add_assign(*usage);
        }
    }
    totals
}
pub(crate) fn current_local_day_key() -> String {
    local_time::current_local_day_key()
}

pub(crate) fn roll_over_token_usage_if_needed(state: &AppState) {
    let today = current_local_day_key();
    let (needs_rollover, previous_day) = {
        let mut usage_day = state
            .token_usage_day
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *usage_day == today {
            (false, String::new())
        } else {
            let previous = usage_day.clone();
            *usage_day = today;
            (true, previous)
        }
    };

    if !needs_rollover {
        return;
    }

    let _ = token_history::flush_day_to_history(state, &previous_day);

    if let Ok(mut usage_by_session) = state.session_token_usage.lock() {
        usage_by_session.clear();
    }
    if let Ok(mut usage_by_model) = state.session_token_usage_by_model.lock() {
        usage_by_model.clear();
    }
    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky.clear();
    }
    if let Ok(mut offsets) = state.token_usage_file_offsets.lock() {
        offsets.clear();
    }
    if let Ok(mut baseline) = state.daily_tokens_baseline.lock() {
        *baseline = TokenUsage::default();
    }
    if let Ok(mut startup_floor) = state.startup_daily_floor.lock() {
        *startup_floor = TokenUsage::default();
    }
    if let Ok(mut startup_floor_by_model) = state.startup_daily_floor_by_model.lock() {
        startup_floor_by_model.clear();
    }
    if let Ok(mut absolute_sessions) = state.absolute_token_sessions.lock() {
        absolute_sessions.clear();
    }
}

pub(crate) fn token_usage_and_model_from_transcript_entry(
    entry: &Value,
    local_today_key: &str,
) -> Option<(String, TokenUsage)> {
    if entry.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let Some(timestamp) = entry.get("timestamp").and_then(Value::as_str) else {
        return None;
    };
    if !local_time::is_local_today(timestamp, local_today_key) {
        return None;
    }

    let message = entry.get("message")?;
    let usage = message.get("usage");
    let model = message
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| pricing::UNKNOWN_MODEL.to_string());

    Some((
        model,
        TokenUsage {
            input_tokens: usage
                .and_then(|value| value.get("input_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            output_tokens: usage
                .and_then(|value| value.get("output_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cache_read_tokens: usage
                .and_then(|value| value.get("cache_read_input_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            cache_creation_tokens: usage
                .and_then(|value| value.get("cache_creation_input_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
    ))
}

pub(crate) fn token_usage_from_transcript_entry(
    entry: &Value,
    local_today_key: &str,
) -> TokenUsage {
    token_usage_and_model_from_transcript_entry(entry, local_today_key)
        .map(|(_, usage)| usage)
        .unwrap_or_default()
}

pub(crate) fn parse_claude_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, HashMap<String, TokenUsage>, u64, bool), String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Seek, SeekFrom};

    let mut file =
        File::open(transcript_path).map_err(|error| format!("Cannot open transcript: {error}"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Cannot read transcript metadata: {error}"))?
        .len();
    let start_offset = if offset > file_len { 0 } else { offset };
    let is_full_scan = start_offset == 0;

    file.seek(SeekFrom::Start(start_offset))
        .map_err(|error| format!("Cannot seek transcript: {error}"))?;

    let mut reader = BufReader::new(file);
    let mut usage = TokenUsage::default();
    let mut usage_by_model: HashMap<String, TokenUsage> = HashMap::new();
    let mut next_offset = start_offset;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| format!("Cannot read transcript: {error}"))?;
        if bytes == 0 {
            break;
        }

        next_offset = next_offset.saturating_add(bytes as u64);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some((model, entry_usage)) =
            token_usage_and_model_from_transcript_entry(&entry, today_key)
        {
            usage.add_assign(entry_usage);
            usage_by_model
                .entry(model)
                .or_default()
                .add_assign(entry_usage);
        }
    }

    Ok((usage, usage_by_model, next_offset, is_full_scan))
}

pub(crate) fn token_usage_from_codex_delta(delta: transcript::TokenUsageDelta) -> TokenUsage {
    TokenUsage {
        input_tokens: delta.input_tokens,
        output_tokens: delta.output_tokens,
        cache_read_tokens: delta.cache_read_tokens,
        cache_creation_tokens: delta.cache_creation_tokens,
    }
}

pub(crate) fn parse_codex_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, HashMap<String, TokenUsage>, u64, bool), String> {
    use std::fs::File;
    use std::io::BufReader;

    let file =
        File::open(transcript_path).map_err(|error| format!("Cannot open transcript: {error}"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Cannot read transcript metadata: {error}"))?
        .len();
    let start_offset = if offset > file_len { 0 } else { offset };
    let is_full_scan = start_offset == 0;

    let mut reader = BufReader::new(file);
    let parsed = transcript::parse_codex_tokens_from_reader(&mut reader, start_offset, today_key)?;
    Ok((
        token_usage_from_codex_delta(parsed.daily_delta),
        token_usage_map_from_delta_map(&parsed.daily_delta_by_model),
        parsed.next_offset,
        is_full_scan,
    ))
}

pub(crate) fn parse_zcode_token_usage_from_transcript(
    transcript_path: &str,
    offset: u64,
    today_key: &str,
) -> Result<(TokenUsage, HashMap<String, TokenUsage>, u64, bool), String> {
    use std::fs::File;
    use std::io::BufReader;

    // Sessions register on SessionStart before the first model I/O line is
    // written; treat a missing rollout as "nothing to count yet".
    if !std::path::Path::new(transcript_path).exists() {
        return Ok((TokenUsage::default(), HashMap::new(), offset, false));
    }

    let file =
        File::open(transcript_path).map_err(|error| format!("Cannot open transcript: {error}"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Cannot read transcript metadata: {error}"))?
        .len();
    let start_offset = if offset > file_len { 0 } else { offset };
    let is_full_scan = start_offset == 0;

    let mut reader = BufReader::new(file);
    let parsed = transcript::parse_zcode_tokens_from_reader(&mut reader, start_offset, today_key)?;
    Ok((
        token_usage_from_codex_delta(parsed.daily_delta),
        token_usage_map_from_delta_map(&parsed.daily_delta_by_model),
        parsed.next_offset,
        is_full_scan,
    ))
}

pub(crate) fn refresh_session_token_usage(
    state: &AppState,
    session_id: &str,
    transcript_path: Option<&str>,
    agent: Option<&AgentKind>,
) -> Result<(), String> {
    // ZCode hook payloads point at ephemeral temp transcripts that are deleted
    // as soon as the hook returns; the durable token data lives in the
    // session's rollout JSONL, derived from the session id instead.
    let transcript_path = match agent {
        Some(AgentKind::Zcode) => match zcode_rollout_path(session_id) {
            Some(path) => path.to_string_lossy().into_owned(),
            None => return Ok(()),
        },
        _ => match transcript_path {
            Some(path) => path.to_string(),
            None => return Ok(()),
        },
    };
    let transcript_path = transcript_path.as_str();

    roll_over_token_usage_if_needed(state);
    let today_key = current_local_day_key();
    let last_offset = state
        .token_usage_file_offsets
        .lock()
        .map_err(|error| error.to_string())?
        .get(transcript_path)
        .copied()
        .unwrap_or(0);

    let format = match agent {
        Some(AgentKind::Codex) => transcript::TranscriptFormat::Codex,
        Some(AgentKind::Claude) => transcript::TranscriptFormat::Claude,
        Some(AgentKind::Cursor) => transcript::TranscriptFormat::Cursor,
        Some(AgentKind::Zcode) => transcript::TranscriptFormat::Zcode,
        _ => transcript::detect_transcript_format(transcript_path),
    };

    let (parsed_usage, parsed_usage_by_model, next_offset, is_full_scan) = match format {
        transcript::TranscriptFormat::Codex => {
            parse_codex_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        transcript::TranscriptFormat::Zcode => {
            parse_zcode_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        transcript::TranscriptFormat::Claude => {
            parse_claude_token_usage_from_transcript(transcript_path, last_offset, &today_key)?
        }
        // Cursor transcripts carry no token-usage data; tokens arrive
        // via `ingest_cursor_token_usage_from_payload` from hook payloads.
        // Always set is_full_scan=false so we never overwrite values that
        // were already injected by the stop hook.
        transcript::TranscriptFormat::Cursor => {
            let file_len = std::fs::metadata(transcript_path)
                .map(|m| m.len())
                .unwrap_or(last_offset);
            (TokenUsage::default(), HashMap::new(), file_len, false)
        }
    };

    {
        let mut offsets = state
            .token_usage_file_offsets
            .lock()
            .map_err(|error| error.to_string())?;
        offsets.insert(transcript_path.to_string(), next_offset);
    }

    let mut usage_by_session = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let usage_entry = usage_by_session.entry(session_id.to_string()).or_default();
    if is_full_scan {
        // Transcript may be truncated or rotated; never regress a session total
        // that was already accumulated from hooks or a prior scan.
        *usage_entry = usage_entry.component_wise_max(parsed_usage);
        if let Ok(mut absolute_sessions) = state.absolute_token_sessions.lock() {
            absolute_sessions.insert(session_id.to_string());
        }
    } else {
        usage_entry.add_assign(parsed_usage);
    }
    drop(usage_by_session);

    if !parsed_usage_by_model.is_empty() {
        let mut usage_by_model = state
            .session_token_usage_by_model
            .lock()
            .map_err(|error| error.to_string())?;
        let model_entry = usage_by_model.entry(session_id.to_string()).or_default();
        merge_session_model_usage(model_entry, &parsed_usage_by_model, is_full_scan);
    }

    if let Some(agent) = agent {
        if let Ok(mut sticky) = state.session_agent_map.lock() {
            sticky
                .entry(session_id.to_string())
                .or_insert_with(|| token_history::agent_kind_key(agent));
        }
    }

    state.token_history_dirty.store(true, Ordering::Release);

    Ok(())
}
pub(crate) fn ingest_cursor_token_usage_from_payload(
    state: &AppState,
    session_id: &str,
    payload: &serde_json::Value,
    source: &str,
) -> Result<(), String> {
    let parsed_usage = parse_cursor_token_usage_from_payload(payload);

    if parsed_usage.is_zero() {
        let keys: Vec<&str> = payload
            .as_object()
            .map(|obj| obj.keys().map(String::as_str).collect())
            .unwrap_or_default();
        eprintln!(
            "Atoll Cursor {source} payload has no token fields (session={session_id}, keys={keys:?})"
        );
        return Ok(());
    }

    eprintln!(
        "Atoll Cursor {source} tokens: input={} output={} cache_read={} \
         cache_write={} session={session_id}",
        parsed_usage.input_tokens,
        parsed_usage.output_tokens,
        parsed_usage.cache_read_tokens,
        parsed_usage.cache_creation_tokens
    );

    roll_over_token_usage_if_needed(state);

    {
        let mut usage_by_session = state
            .session_token_usage
            .lock()
            .map_err(|error| error.to_string())?;
        let entry = usage_by_session.entry(session_id.to_string()).or_default();
        // sessionEnd may report cumulative session totals; per-turn hooks add.
        if source == "sessionEnd" {
            *entry = entry.component_wise_max(parsed_usage);
        } else {
            entry.add_assign(parsed_usage);
        }
    }

    let model_id = extract_cursor_model(payload);
    let model_usage = HashMap::from([(model_id, parsed_usage)]);

    {
        let mut usage_by_model = state
            .session_token_usage_by_model
            .lock()
            .map_err(|error| error.to_string())?;
        let model_entry = usage_by_model.entry(session_id.to_string()).or_default();
        if source == "sessionEnd" {
            merge_session_model_usage(model_entry, &model_usage, true);
        } else {
            merge_session_model_usage(model_entry, &model_usage, false);
        }
    }

    if let Ok(mut sticky) = state.session_agent_map.lock() {
        sticky
            .entry(session_id.to_string())
            .or_insert_with(|| token_history::agent_kind_key(&AgentKind::Cursor));
    }

    state.token_history_dirty.store(true, Ordering::Release);
    Ok(())
}

pub(crate) fn cursor_token_source(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("token_usage")
        .or_else(|| payload.get("tokenUsage"))
        .or_else(|| payload.get("usage"))
        .or_else(|| payload.get("token_usage_delta"))
        .or_else(|| payload.get("tokenUsageDelta"))
        .or_else(|| payload.get("total_token_usage"))
        .or_else(|| payload.get("totalTokenUsage"))
        .or_else(|| payload.get("response").and_then(|value| value.get("usage")))
        .or_else(|| payload.get("message").and_then(|value| value.get("usage")))
        .unwrap_or(payload)
}

pub(crate) fn parse_cursor_token_usage_from_payload(payload: &serde_json::Value) -> TokenUsage {
    let token_source = cursor_token_source(payload);
    TokenUsage {
        input_tokens: first_json_u64(
            token_source,
            &[
                "input_tokens",
                "inputTokens",
                "prompt_tokens",
                "promptTokens",
                "total_input_tokens",
                "totalInputTokens",
            ],
        ),
        output_tokens: first_json_u64(
            token_source,
            &[
                "output_tokens",
                "outputTokens",
                "completion_tokens",
                "completionTokens",
                "total_output_tokens",
                "totalOutputTokens",
            ],
        ),
        cache_read_tokens: first_json_u64(
            token_source,
            &[
                "cache_read_tokens",
                "cacheReadTokens",
                "cache_read_input_tokens",
                "cacheReadInputTokens",
                "cached_input_tokens",
                "cachedInputTokens",
            ],
        ),
        cache_creation_tokens: first_json_u64(
            token_source,
            &[
                "cache_write_tokens",
                "cacheWriteTokens",
                "cache_creation_tokens",
                "cacheCreationTokens",
                "cache_creation_input_tokens",
                "cacheCreationInputTokens",
            ],
        ),
    }
}
