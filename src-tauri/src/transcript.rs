use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptFormat {
    Claude,
    Codex,
}

pub struct CodexTokenParseResult {
    #[allow(dead_code)] // parsed for correctness; session UI uses daily_delta increments
    pub session_total: TokenUsageDelta,
    pub daily_delta: TokenUsageDelta,
    pub next_offset: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsageDelta {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl TokenUsageDelta {
    pub fn add_assign(&mut self, other: TokenUsageDelta) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }
}

pub fn detect_transcript_format_from_line(line: &str) -> Option<TranscriptFormat> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let entry: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let msg_type = entry.get("type").and_then(Value::as_str)?;
    match msg_type {
        "session_meta" | "response_item" | "event_msg" | "turn_context" => {
            Some(TranscriptFormat::Codex)
        }
        "human" | "user" | "assistant" => Some(TranscriptFormat::Claude),
        _ => None,
    }
}

pub fn detect_transcript_format(path: &str) -> TranscriptFormat {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path).ok();
    if let Some(file) = file {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Some(format) = detect_transcript_format_from_line(&line) {
                return format;
            }
        }
    }

    TranscriptFormat::Claude
}

pub struct ParsedChatMessage {
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
}

pub fn read_codex_cwd_from_transcript(path: &str) -> Option<String> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let entry: Value = serde_json::from_str(trimmed).ok()?;
    if entry.get("type").and_then(Value::as_str) != Some("session_meta") {
        return None;
    }

    entry
        .get("payload")
        .and_then(|payload| payload.get("cwd"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
        .map(str::to_string)
}

pub fn parse_codex_messages(lines: &[String]) -> Vec<ParsedChatMessage> {
    let mut messages: Vec<ParsedChatMessage> = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if entry.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }

        let payload = entry.get("payload").unwrap_or(&Value::Null);
        let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or("");

        match payload_type {
            "message" => {
                let role = payload
                    .get("role")
                    .and_then(Value::as_str)
                    .unwrap_or("assistant");
                let content = extract_codex_message_content(payload);
                if !content.is_empty() {
                    messages.push(ParsedChatMessage {
                        role: role.to_string(),
                        content,
                        tool_name: None,
                    });
                }
            }
            "function_call" | "custom_tool_call" => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(payload_type);
                messages.push(ParsedChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_name: Some(name.to_string()),
                });
            }
            _ => {}
        }
    }

    messages
}

fn extract_codex_message_content(payload: &Value) -> String {
    let content = payload.get("content");
    if let Some(text) = content.and_then(Value::as_str) {
        return text.to_string();
    }

    if let Some(arr) = content.and_then(Value::as_array) {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|block| {
                let block_type = block.get("type").and_then(Value::as_str)?;
                if matches!(block_type, "input_text" | "output_text" | "text") {
                    block
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
            .collect();
        return parts.join("\n");
    }

    String::new()
}

pub fn parse_codex_tokens_from_reader<R: std::io::BufRead + std::io::Seek>(
    reader: &mut R,
    offset: u64,
    today_key: &str,
) -> Result<CodexTokenParseResult, String> {
    use std::io::SeekFrom;

    let mut session_total = TokenUsageDelta::default();
    let mut daily_delta = TokenUsageDelta::default();
    let mut next_offset = offset;

    if offset > 0 {
        reader
            .seek(SeekFrom::Start(offset))
            .map_err(|error| format!("Cannot seek transcript: {error}"))?;
    }

    let mut line_buf = String::new();
    loop {
        line_buf.clear();
        let bytes = reader
            .read_line(&mut line_buf)
            .map_err(|error| format!("Cannot read transcript: {error}"))?;
        if bytes == 0 {
            break;
        }
        next_offset += bytes as u64;

        let trimmed = line_buf.trim();
        if trimmed.is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if entry.get("type").and_then(Value::as_str) != Some("event_msg") {
            continue;
        }

        let payload = entry.get("payload").unwrap_or(&Value::Null);
        if payload.get("type").and_then(Value::as_str) != Some("token_count") {
            continue;
        }

        let info = payload.get("info");
        if info.is_none() {
            continue;
        }

        if let Some(total) = token_usage_from_codex_usage(info.and_then(|v| v.get("total_token_usage"))) {
            session_total = total;
        }

        let timestamp = entry.get("timestamp").and_then(Value::as_str).unwrap_or("");
        if timestamp.starts_with(today_key) {
            if let Some(last) =
                token_usage_from_codex_usage(info.and_then(|v| v.get("last_token_usage")))
            {
                daily_delta.add_assign(last);
            }
        }
    }

    Ok(CodexTokenParseResult {
        session_total,
        daily_delta,
        next_offset,
    })
}

fn token_usage_from_codex_usage(usage: Option<&Value>) -> Option<TokenUsageDelta> {
    let usage = usage?;
    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_output_tokens = usage
        .get("reasoning_output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cached_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Some(TokenUsageDelta {
        input_tokens,
        output_tokens: output_tokens.saturating_add(reasoning_output_tokens),
        cache_read_tokens,
        cache_creation_tokens: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_codex_format() {
        let line = r#"{"type":"session_meta","payload":{"id":"abc"}}"#;
        assert_eq!(
            detect_transcript_format_from_line(line),
            Some(TranscriptFormat::Codex)
        );
    }

    #[test]
    fn reads_codex_cwd_from_session_meta() {
        let dir = std::env::temp_dir().join(format!(
            "atoll-codex-transcript-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("rollout-test.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"session_meta","payload":{"id":"abc","cwd":"C:/Users/test/project"}}"#,
        )
        .expect("write transcript");

        let cwd = read_codex_cwd_from_transcript(&path.to_string_lossy()).expect("read cwd");
        assert_eq!(cwd, "C:/Users/test/project");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_codex_messages() {
        let lines = vec![
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Hello"}]}}"#
                .to_string(),
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hi there"}]}}"#
                .to_string(),
            r#"{"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"c1"}}"#
                .to_string(),
        ];
        let messages = parse_codex_messages(&lines);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi there");
        assert_eq!(messages[2].tool_name.as_deref(), Some("exec_command"));
    }

    #[test]
    fn parses_codex_token_count() {
        let jsonl = r#"{"type":"event_msg","timestamp":"2026-06-19T10:00:00.000Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":50,"output_tokens":20,"reasoning_output_tokens":5},"last_token_usage":{"input_tokens":10,"cached_input_tokens":5,"output_tokens":2,"reasoning_output_tokens":1}}}}
"#;
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(jsonl.as_bytes()));
        let result = parse_codex_tokens_from_reader(&mut reader, 0, "2026-06-19").expect("parse");
        assert_eq!(result.session_total.input_tokens, 100);
        assert_eq!(result.session_total.output_tokens, 25);
        assert_eq!(result.session_total.cache_read_tokens, 50);
        assert_eq!(result.daily_delta.input_tokens, 10);
        assert_eq!(result.daily_delta.output_tokens, 3);
    }
}
