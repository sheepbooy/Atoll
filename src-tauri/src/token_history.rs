use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::local_time::{current_local_day_key, format_local_day_key};
use crate::{AgentKind, AppState, TokenUsage};

const HISTORY_VERSION: u32 = 1;
const RETENTION_DAYS: i64 = 365;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageRecord {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl From<TokenUsage> for TokenUsageRecord {
    fn from(value: TokenUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cache_read_tokens: value.cache_read_tokens,
            cache_creation_tokens: value.cache_creation_tokens,
        }
    }
}

impl From<TokenUsageRecord> for TokenUsage {
    fn from(value: TokenUsageRecord) -> Self {
        Self {
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cache_read_tokens: value.cache_read_tokens,
            cache_creation_tokens: value.cache_creation_tokens,
        }
    }
}

impl TokenUsageRecord {
    fn add_assign(&mut self, other: TokenUsageRecord) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TokenDayRecord {
    #[serde(flatten)]
    usage: TokenUsageRecord,
    #[serde(default)]
    by_agent: HashMap<String, TokenUsageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenHistoryFile {
    version: u32,
    timezone: String,
    days: HashMap<String, TokenDayRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenHistoryDay {
    pub date: String,
    #[serde(flatten)]
    pub usage: TokenUsageRecord,
    pub by_agent: HashMap<String, TokenUsageRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenHistoryResponse {
    pub timezone: String,
    pub days: Vec<TokenHistoryDay>,
}

pub fn token_history_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_TOKEN_HISTORY_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }

    dirs::home_dir().map(|home| home.join(".atoll").join("token_history.json"))
}

fn system_timezone_name() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

fn load_history_file() -> TokenHistoryFile {
    let Some(path) = token_history_path() else {
        return TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: system_timezone_name(),
            days: HashMap::new(),
        };
    };

    let Ok(content) = std::fs::read_to_string(&path) else {
        return TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: system_timezone_name(),
            days: HashMap::new(),
        };
    };

    serde_json::from_str(&content).unwrap_or_else(|_| TokenHistoryFile {
        version: HISTORY_VERSION,
        timezone: system_timezone_name(),
        days: HashMap::new(),
    })
}

fn save_history_file(file: &TokenHistoryFile) -> Result<(), String> {
    let Some(path) = token_history_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let formatted = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    std::fs::write(path, formatted).map_err(|error| error.to_string())
}

fn prune_old_days(days: &mut HashMap<String, TokenDayRecord>) {
    let Ok(today) = NaiveDate::parse_from_str(&current_local_day_key(), "%Y-%m-%d") else {
        return;
    };
    let cutoff = today - Duration::days(RETENTION_DAYS);
    days.retain(|day_key, _| {
        NaiveDate::parse_from_str(day_key, "%Y-%m-%d")
            .map(|date| date >= cutoff)
            .unwrap_or(false)
    });
}

pub(crate) fn agent_kind_key(agent: &AgentKind) -> String {
    match agent {
        AgentKind::Claude => "claude".to_string(),
        AgentKind::Codex => "codex".to_string(),
        AgentKind::Gemini => "gemini".to_string(),
        AgentKind::Other => "other".to_string(),
    }
}

pub(crate) fn build_agent_by_session(state: &AppState) -> HashMap<String, String> {
    let mut map = HashMap::new();

    if let Ok(sticky) = state.session_agent_map.lock() {
        map.extend(sticky.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    if let Ok(known) = state.known_sessions.lock() {
        for (session_id, info) in known.iter() {
            map.insert(session_id.clone(), agent_kind_key(&info.agent));
        }
    }

    if let Ok(requests) = state.requests.lock() {
        for request in requests.iter() {
            map
                .entry(request.session.clone())
                .or_insert_with(|| agent_kind_key(&request.agent));
        }
    }

    map
}

fn aggregate_day_record(
    session_usage: &HashMap<String, TokenUsage>,
    agent_by_session: &HashMap<String, String>,
) -> TokenDayRecord {
    let mut record = TokenDayRecord::default();

    for (session_id, usage) in session_usage {
        let usage_record = TokenUsageRecord::from(*usage);
        record.usage.add_assign(usage_record);

        let agent_key = agent_by_session
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| "other".to_string());
        record
            .by_agent
            .entry(agent_key)
            .or_default()
            .add_assign(TokenUsageRecord::from(*usage));
    }

    record
}

fn upsert_day(
    file: &mut TokenHistoryFile,
    day_key: &str,
    record: TokenDayRecord,
) -> Result<(), String> {
    file.version = HISTORY_VERSION;
    file.timezone = system_timezone_name();
    file.days.insert(day_key.to_string(), record);
    prune_old_days(&mut file.days);
    save_history_file(file)
}

pub(crate) fn sync_today_to_history(state: &AppState) -> Result<(), String> {
    let day_key = current_local_day_key();
    let session_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let agent_by_session = build_agent_by_session(state);
    let record = aggregate_day_record(&session_usage, &agent_by_session);
    drop(session_usage);

    let mut file = load_history_file();
    upsert_day(&mut file, &day_key, record)
}

pub(crate) fn flush_day_to_history(state: &AppState, day_key: &str) -> Result<(), String> {
    if day_key.is_empty() {
        return Ok(());
    }

    let session_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    if session_usage.is_empty() {
        return Ok(());
    }
    let agent_by_session = build_agent_by_session(state);
    let record = aggregate_day_record(&session_usage, &agent_by_session);
    drop(session_usage);

    let mut file = load_history_file();
    upsert_day(&mut file, day_key, record)
}

pub fn get_token_history(days: u32) -> Result<TokenHistoryResponse, String> {
    let file = load_history_file();
    let today_key = current_local_day_key();
    let today = NaiveDate::parse_from_str(&today_key, "%Y-%m-%d").map_err(|error| error.to_string())?;
    let span = days.max(1).min(365) as i64;

    let mut result = Vec::new();
    for offset in (0..span).rev() {
        let date = today - Duration::days(offset);
        let date_key = format_local_day_key(date);
        if let Some(record) = file.days.get(&date_key) {
            result.push(TokenHistoryDay {
                date: date_key,
                usage: record.usage,
                by_agent: record.by_agent.clone(),
            });
        } else {
            result.push(TokenHistoryDay {
                date: date_key,
                usage: TokenUsageRecord::default(),
                by_agent: HashMap::new(),
            });
        }
    }

    Ok(TokenHistoryResponse {
        timezone: file.timezone,
        days: result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_time::format_local_day_key;
    use chrono::Local;

    fn sample_usage(input: u64, output: u64) -> TokenUsage {
        TokenUsage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        }
    }

    #[test]
    fn aggregate_groups_by_agent() {
        let session_usage = HashMap::from([
            ("s1".into(), sample_usage(100, 50)),
            ("s2".into(), sample_usage(200, 80)),
        ]);
        let agent_by_session = HashMap::from([
            ("s1".into(), "claude".into()),
            ("s2".into(), "codex".into()),
        ]);

        let record = aggregate_day_record(&session_usage, &agent_by_session);
        assert_eq!(record.usage.input_tokens, 300);
        assert_eq!(record.usage.output_tokens, 130);
        assert_eq!(record.by_agent.get("claude").unwrap().input_tokens, 100);
        assert_eq!(record.by_agent.get("codex").unwrap().input_tokens, 200);
    }

    #[test]
    fn prune_drops_days_older_than_retention() {
        let today = Local::now().date_naive();
        let old = today - Duration::days(RETENTION_DAYS + 1);
        let mut days = HashMap::from([
            (
                format_local_day_key(old),
                TokenDayRecord {
                    usage: TokenUsageRecord {
                        input_tokens: 1,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_creation_tokens: 0,
                    },
                    by_agent: HashMap::new(),
                },
            ),
            (
                format_local_day_key(today),
                TokenDayRecord::default(),
            ),
        ]);

        prune_old_days(&mut days);
        assert_eq!(days.len(), 1);
        assert!(days.contains_key(&format_local_day_key(today)));
    }
}
