use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::local_time::{current_local_day_key, format_local_day_key};
use crate::{AgentKind, AppState, TokenUsage};

const HISTORY_VERSION: u32 = 2;
const RETENTION_DAYS: i64 = 365;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
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
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
    }

    fn component_wise_max(self, other: TokenUsageRecord) -> TokenUsageRecord {
        TokenUsageRecord {
            input_tokens: self.input_tokens.max(other.input_tokens),
            output_tokens: self.output_tokens.max(other.output_tokens),
            cache_read_tokens: self.cache_read_tokens.max(other.cache_read_tokens),
            cache_creation_tokens: self.cache_creation_tokens.max(other.cache_creation_tokens),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct TokenDayRecord {
    #[serde(flatten)]
    usage: TokenUsageRecord,
    #[serde(default)]
    by_agent: HashMap<String, TokenUsageRecord>,
    #[serde(default)]
    by_model: HashMap<String, TokenUsageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
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
    #[serde(default)]
    pub by_model: HashMap<String, TokenUsageRecord>,
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

    load_history_file_at(&path).unwrap_or_else(|| {
        let backup = path.with_extension("json.bak");
        load_history_file_at(&backup).unwrap_or_else(|| {
            eprintln!(
                "Atoll: failed to load token history from {} (and backup); starting fresh",
                path.display()
            );
            TokenHistoryFile {
                version: HISTORY_VERSION,
                timezone: system_timezone_name(),
                days: HashMap::new(),
            }
        })
    })
}

fn load_history_file_at(path: &std::path::Path) -> Option<TokenHistoryFile> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_history_file(file: &TokenHistoryFile) -> Result<(), String> {
    let Some(path) = token_history_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let formatted = serde_json::to_string_pretty(file).map_err(|error| error.to_string())?;
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, &formatted).map_err(|error| error.to_string())?;
    // Keep a backup so a crash mid-rename does not leave us with no recoverable file.
    let backup_path = path.with_extension("json.bak");
    if path.exists() {
        let _ = std::fs::copy(&path, &backup_path);
    }
    std::fs::rename(&temp_path, &path).map_err(|error| error.to_string())
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
        AgentKind::Cursor => "cursor".to_string(),
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
            map.entry(request.session.clone())
                .or_insert_with(|| agent_kind_key(&request.agent));
        }
    }

    map
}

fn sum_by_agent_usage(by_agent: &HashMap<String, TokenUsageRecord>) -> TokenUsageRecord {
    let mut total = TokenUsageRecord::default();
    for usage in by_agent.values() {
        total.add_assign(*usage);
    }
    total
}

/// Legacy bug: `usage` was inflated by floor+rescan double-count while `byAgent`
/// stayed accurate. Detect and repair on load/sync so restarts do not inherit bad floors.
fn repair_inflated_day_usage(record: &mut TokenDayRecord) {
    let agent_sum = sum_by_agent_usage(&record.by_agent);
    if agent_sum.input_tokens == 0 && agent_sum.output_tokens == 0 {
        return;
    }

    let usage_inflated = record.usage.input_tokens > agent_sum.input_tokens.saturating_mul(8)
        || record.usage.output_tokens > agent_sum.output_tokens.saturating_mul(8);
    if usage_inflated {
        record.usage = record.usage.component_wise_max(agent_sum);
        if record.usage.input_tokens > agent_sum.input_tokens.saturating_mul(8) {
            record.usage = agent_sum;
        }
    }
}

fn aggregate_day_record(
    session_usage: &HashMap<String, TokenUsage>,
    session_usage_by_model: &HashMap<String, HashMap<String, TokenUsage>>,
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

    for usage_by_model in session_usage_by_model.values() {
        for (model_id, usage) in usage_by_model {
            record
                .by_model
                .entry(model_id.clone())
                .or_default()
                .add_assign(TokenUsageRecord::from(*usage));
        }
    }

    record
}

fn merge_day_records(existing: &TokenDayRecord, incoming: &TokenDayRecord) -> TokenDayRecord {
    let usage = existing.usage.component_wise_max(incoming.usage);

    let mut by_agent = existing.by_agent.clone();
    for (agent, incoming_usage) in &incoming.by_agent {
        let entry = by_agent.entry(agent.clone()).or_default();
        *entry = entry.component_wise_max(*incoming_usage);
    }

    let mut by_model = existing.by_model.clone();
    for (model_id, incoming_usage) in &incoming.by_model {
        let entry = by_model.entry(model_id.clone()).or_default();
        *entry = entry.component_wise_max(*incoming_usage);
    }

    TokenDayRecord {
        usage,
        by_agent,
        by_model,
    }
}

fn upsert_day(
    file: &mut TokenHistoryFile,
    day_key: &str,
    record: TokenDayRecord,
) -> Result<TokenDayRecord, String> {
    file.version = HISTORY_VERSION;
    file.timezone = system_timezone_name();
    file.days.insert(day_key.to_string(), record.clone());
    prune_old_days(&mut file.days);
    save_history_file(file)?;
    Ok(record)
}

fn update_daily_baseline(state: &AppState, usage: TokenUsageRecord) {
    if let Ok(mut baseline) = state.daily_tokens_baseline.lock() {
        *baseline = (*baseline).component_wise_max(TokenUsage::from(usage));
    }
}

pub(crate) fn sync_today_to_history(state: &AppState) -> Result<(), String> {
    let day_key = current_local_day_key();
    let session_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let session_usage_by_model = state
        .session_token_usage_by_model
        .lock()
        .map_err(|error| error.to_string())?;
    let agent_by_session = build_agent_by_session(state);
    let mut record = aggregate_day_record(
        &session_usage,
        &session_usage_by_model,
        &agent_by_session,
    );
    let startup_floor = *state
        .startup_daily_floor
        .lock()
        .map_err(|error| error.to_string())?;
    let absolute_sessions = state
        .absolute_token_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    record.usage = TokenUsageRecord::from(crate::effective_daily_tokens(
        &session_usage,
        startup_floor,
        &absolute_sessions,
    ));
    repair_inflated_day_usage(&mut record);
    drop(session_usage);
    drop(session_usage_by_model);

    let mut file = load_history_file();

    // Merge with existing data: never let a post-restart empty state overwrite
    // previously persisted values.  Use component-wise max so the file only grows
    // until rollover resets it at midnight.
    let saved = if let Some(existing) = file.days.get(&day_key) {
        let mut merged = merge_day_records(existing, &record);
        repair_inflated_day_usage(&mut merged);
        upsert_day(&mut file, &day_key, merged)?
    } else {
        upsert_day(&mut file, &day_key, record)?
    };
    update_daily_baseline(state, saved.usage);
    Ok(())
}

pub(crate) fn flush_day_to_history(state: &AppState, day_key: &str) -> Result<(), String> {
    if day_key.is_empty() {
        return Ok(());
    }

    let session_usage = state
        .session_token_usage
        .lock()
        .map_err(|error| error.to_string())?;
    let session_usage_by_model = state
        .session_token_usage_by_model
        .lock()
        .map_err(|error| error.to_string())?;
    if session_usage.is_empty() {
        return Ok(());
    }
    let agent_by_session = build_agent_by_session(state);
    let record = aggregate_day_record(
        &session_usage,
        &session_usage_by_model,
        &agent_by_session,
    );
    drop(session_usage);
    drop(session_usage_by_model);

    let mut file = load_history_file();
    if let Some(existing) = file.days.get(day_key) {
        let merged = merge_day_records(existing, &record);
        upsert_day(&mut file, day_key, merged)?;
    } else {
        upsert_day(&mut file, day_key, record)?;
    }
    Ok(())
}

pub(crate) fn load_today_baseline() -> TokenUsage {
    let file = load_history_file();
    let today_key = current_local_day_key();
    file.days
        .get(&today_key)
        .map(|record| {
            let mut repaired = record.clone();
            repair_inflated_day_usage(&mut repaired);
            TokenUsage {
                input_tokens: repaired.usage.input_tokens,
                output_tokens: repaired.usage.output_tokens,
                cache_read_tokens: repaired.usage.cache_read_tokens,
                cache_creation_tokens: repaired.usage.cache_creation_tokens,
            }
        })
        .unwrap_or_default()
}

/// Persisted per-model totals for today — used as a restart floor for cost mode.
pub(crate) fn load_today_by_model_baseline() -> HashMap<String, TokenUsage> {
    let file = load_history_file();
    let today_key = current_local_day_key();
    file.days
        .get(&today_key)
        .map(|record| {
            let mut repaired = record.clone();
            repair_inflated_day_usage(&mut repaired);
            repaired
                .by_model
                .into_iter()
                .map(|(model_id, usage)| (model_id, TokenUsage::from(usage)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn get_token_history(days: u32) -> Result<TokenHistoryResponse, String> {
    let file = load_history_file();
    let today_key = current_local_day_key();
    let today =
        NaiveDate::parse_from_str(&today_key, "%Y-%m-%d").map_err(|error| error.to_string())?;
    let span = days.max(1).min(365) as i64;

    let mut result = Vec::new();
    for offset in (0..span).rev() {
        let date = today - Duration::days(offset);
        let date_key = format_local_day_key(date);
        if let Some(record) = file.days.get(&date_key) {
            let mut repaired = record.clone();
            repair_inflated_day_usage(&mut repaired);
            result.push(TokenHistoryDay {
                date: date_key,
                usage: repaired.usage,
                by_agent: repaired.by_agent.clone(),
                by_model: repaired.by_model.clone(),
            });
        } else {
            result.push(TokenHistoryDay {
                date: date_key,
                usage: TokenUsageRecord::default(),
                by_agent: HashMap::new(),
                by_model: HashMap::new(),
            });
        }
    }

    Ok(TokenHistoryResponse {
        timezone: file.timezone,
        days: result,
    })
}

pub fn known_model_ids() -> HashSet<String> {
    let file = load_history_file();
    let mut ids = HashSet::new();
    for day in file.days.values() {
        for model_id in day.by_model.keys() {
            if model_id != crate::pricing::UNKNOWN_MODEL {
                ids.insert(model_id.clone());
            }
        }
    }
    ids
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
    fn aggregate_groups_by_agent_and_model() {
        let session_usage = HashMap::from([
            ("s1".into(), sample_usage(100, 50)),
            ("s2".into(), sample_usage(200, 80)),
        ]);
        let session_usage_by_model = HashMap::from([
            (
                "s1".into(),
                HashMap::from([("claude-sonnet".into(), sample_usage(100, 50))]),
            ),
            (
                "s2".into(),
                HashMap::from([("gpt-4o".into(), sample_usage(200, 80))]),
            ),
        ]);
        let agent_by_session = HashMap::from([
            ("s1".into(), "claude".into()),
            ("s2".into(), "codex".into()),
        ]);

        let record = aggregate_day_record(&session_usage, &session_usage_by_model, &agent_by_session);
        assert_eq!(record.usage.input_tokens, 300);
        assert_eq!(record.by_model.get("claude-sonnet").unwrap().input_tokens, 100);
        assert_eq!(record.by_model.get("gpt-4o").unwrap().input_tokens, 200);
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

        let record = aggregate_day_record(&session_usage, &HashMap::new(), &agent_by_session);
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
                    by_model: HashMap::new(),
                },
            ),
            (format_local_day_key(today), TokenDayRecord::default()),
        ]);

        prune_old_days(&mut days);
        assert_eq!(days.len(), 1);
        assert!(days.contains_key(&format_local_day_key(today)));
    }

    #[test]
    fn deserialize_record_missing_cache_fields() {
        let json = r#"{"inputTokens": 500, "outputTokens": 200}"#;
        let record: TokenUsageRecord = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(record.input_tokens, 500);
        assert_eq!(record.output_tokens, 200);
        assert_eq!(record.cache_read_tokens, 0);
        assert_eq!(record.cache_creation_tokens, 0);
    }

    #[test]
    fn deserialize_history_file_with_legacy_day_records() {
        let json = r#"{
            "version": 1,
            "timezone": "Asia/Shanghai",
            "days": {
                "2026-06-20": {
                    "inputTokens": 1000,
                    "outputTokens": 400,
                    "byAgent": { "claude": { "inputTokens": 1000, "outputTokens": 400 } }
                }
            }
        }"#;
        let file: TokenHistoryFile = serde_json::from_str(json).expect("should deserialize");
        let day = file.days.get("2026-06-20").expect("day exists");
        assert_eq!(day.usage.input_tokens, 1000);
        assert_eq!(day.usage.output_tokens, 400);
        assert_eq!(day.usage.cache_read_tokens, 0);
        assert_eq!(day.usage.cache_creation_tokens, 0);
        let agent = day.by_agent.get("claude").expect("agent exists");
        assert_eq!(agent.cache_read_tokens, 0);
    }

    #[test]
    fn repair_inflated_usage_when_by_agent_is_sane() {
        let mut record = TokenDayRecord {
            usage: TokenUsageRecord {
                input_tokens: 6_519_812_307,
                output_tokens: 19_870_089,
                cache_read_tokens: 6_192_733_248,
                cache_creation_tokens: 0,
            },
            by_agent: HashMap::from([
                (
                    "cursor".into(),
                    TokenUsageRecord {
                        input_tokens: 2_908_577,
                        output_tokens: 12_138,
                        cache_read_tokens: 2_787_936,
                        cache_creation_tokens: 0,
                    },
                ),
                (
                    "claude".into(),
                    TokenUsageRecord {
                        input_tokens: 8_000,
                        output_tokens: 2_000,
                        cache_read_tokens: 0,
                        cache_creation_tokens: 0,
                    },
                ),
            ]),
            by_model: HashMap::new(),
        };
        let expected = sum_by_agent_usage(&record.by_agent);

        repair_inflated_day_usage(&mut record);
        assert_eq!(record.usage, expected);
    }

    #[test]
    fn merge_day_records_takes_max() {
        let existing = TokenDayRecord {
            usage: TokenUsageRecord {
                input_tokens: 5000,
                output_tokens: 2000,
                cache_read_tokens: 100,
                cache_creation_tokens: 50,
            },
            by_agent: HashMap::from([
                (
                    "claude".into(),
                    TokenUsageRecord {
                        input_tokens: 3000,
                        output_tokens: 1200,
                        cache_read_tokens: 100,
                        cache_creation_tokens: 50,
                    },
                ),
                (
                    "codex".into(),
                    TokenUsageRecord {
                        input_tokens: 2000,
                        output_tokens: 800,
                        cache_read_tokens: 0,
                        cache_creation_tokens: 0,
                    },
                ),
            ]),
            by_model: HashMap::new(),
        };

        // Simulate post-restart with only partial data recovered
        let incoming = TokenDayRecord {
            usage: TokenUsageRecord {
                input_tokens: 3000,
                output_tokens: 1200,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
            by_agent: HashMap::from([(
                "claude".into(),
                TokenUsageRecord {
                    input_tokens: 3000,
                    output_tokens: 1200,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            )]),
            by_model: HashMap::new(),
        };

        let merged = merge_day_records(&existing, &incoming);
        assert_eq!(merged.usage.input_tokens, 5000);
        assert_eq!(merged.usage.output_tokens, 2000);
        assert_eq!(merged.usage.cache_read_tokens, 100);
        assert_eq!(merged.by_agent.get("claude").unwrap().input_tokens, 3000);
        assert_eq!(merged.by_agent.get("codex").unwrap().input_tokens, 2000);
    }

    fn temp_history_paths(test_name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let pid = std::process::id();
        let history_path =
            std::env::temp_dir().join(format!("atoll-token-history-{pid}-{test_name}.json"));
        let backup_path = history_path.with_extension("json.bak");
        let temp_path = history_path.with_extension("json.tmp");
        (history_path, backup_path, temp_path)
    }

    fn cleanup_history_paths(history_path: &PathBuf) {
        let _ = std::fs::remove_file(history_path);
        let _ = std::fs::remove_file(history_path.with_extension("json.bak"));
        let _ = std::fs::remove_file(history_path.with_extension("json.tmp"));
        std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    }

    fn sample_day_record(input: u64, output: u64) -> TokenDayRecord {
        TokenDayRecord {
            usage: TokenUsageRecord {
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
            by_agent: HashMap::new(),
            by_model: HashMap::new(),
        }
    }

    fn history_path_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::TOKEN_HISTORY_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn save_history_file_creates_backup_before_overwriting_main() {
        let _guard = history_path_test_lock();
        let (history_path, backup_path, temp_path) = temp_history_paths("backup-on-save");
        cleanup_history_paths(&history_path);

        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let day_key = current_local_day_key();
        let initial = TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: "UTC".to_string(),
            days: HashMap::from([(day_key.clone(), sample_day_record(1000, 400))]),
        };
        save_history_file(&initial).expect("initial save");
        assert!(
            !backup_path.exists(),
            "first save should not create a backup when no prior main file existed"
        );

        let updated = TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: "UTC".to_string(),
            days: HashMap::from([(day_key.clone(), sample_day_record(5000, 2000))]),
        };
        save_history_file(&updated).expect("updated save");

        assert!(
            backup_path.exists(),
            "second save should snapshot the prior main file"
        );
        assert!(
            !temp_path.exists(),
            "temp file should be renamed away after a successful save"
        );

        let backup = load_history_file_at(&backup_path).expect("backup should parse");
        assert_eq!(backup.days.get(&day_key).unwrap().usage.input_tokens, 1000);
        assert_eq!(backup.days.get(&day_key).unwrap().usage.output_tokens, 400);

        let main = load_history_file_at(&history_path).expect("main should parse");
        assert_eq!(main.days.get(&day_key).unwrap().usage.input_tokens, 5000);
        assert_eq!(main.days.get(&day_key).unwrap().usage.output_tokens, 2000);

        cleanup_history_paths(&history_path);
    }

    #[test]
    fn load_history_file_recovers_from_backup_when_main_is_corrupt() {
        let _guard = history_path_test_lock();
        let (history_path, backup_path, _) = temp_history_paths("backup-recover");
        cleanup_history_paths(&history_path);

        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let day_key = current_local_day_key();
        let initial = TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: "UTC".to_string(),
            days: HashMap::from([(day_key.clone(), sample_day_record(3200, 900))]),
        };
        save_history_file(&initial).expect("initial save");

        let updated = TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: "UTC".to_string(),
            days: HashMap::from([(day_key.clone(), sample_day_record(8800, 2100))]),
        };
        save_history_file(&updated).expect("updated save");

        // Simulate crash mid-write: main file is truncated/invalid JSON but backup remains.
        std::fs::write(&history_path, "{ not valid json").expect("corrupt main");

        let recovered = load_history_file();
        assert_eq!(
            recovered.days.get(&day_key).unwrap().usage.input_tokens,
            3200,
            "should load the pre-update snapshot from .bak"
        );
        assert_eq!(
            recovered.days.get(&day_key).unwrap().usage.output_tokens,
            900
        );

        let history = get_token_history(7).expect("history query after recovery");
        let today = history
            .days
            .iter()
            .find(|day| day.date == day_key)
            .expect("today in history response");
        assert_eq!(today.usage.input_tokens, 3200);
        assert_eq!(today.usage.output_tokens, 900);

        cleanup_history_paths(&history_path);
        let _ = backup_path;
    }

    #[test]
    fn known_model_ids_skips_unknown() {
        let _lock = history_path_test_lock();
        let (history_path, _, _) = temp_history_paths("known-model-ids");
        cleanup_history_paths(&history_path);
        std::env::set_var(
            "ATOLL_TOKEN_HISTORY_PATH",
            history_path.to_string_lossy().as_ref(),
        );

        let file = TokenHistoryFile {
            version: HISTORY_VERSION,
            timezone: "UTC".to_string(),
            days: HashMap::from([(
                "2026-07-09".into(),
                TokenDayRecord {
                    usage: TokenUsageRecord::default(),
                    by_agent: HashMap::new(),
                    by_model: HashMap::from([
                        (
                            crate::pricing::UNKNOWN_MODEL.to_string(),
                            TokenUsageRecord {
                                input_tokens: 100,
                                output_tokens: 0,
                                cache_read_tokens: 0,
                                cache_creation_tokens: 0,
                            },
                        ),
                        (
                            "gpt-4o".into(),
                            TokenUsageRecord {
                                input_tokens: 200,
                                output_tokens: 0,
                                cache_read_tokens: 0,
                                cache_creation_tokens: 0,
                            },
                        ),
                    ]),
                },
            )]),
        };
        save_history_file(&file).expect("save history");

        let ids = known_model_ids();
        assert!(ids.contains("gpt-4o"));
        assert!(!ids.contains(crate::pricing::UNKNOWN_MODEL));

        cleanup_history_paths(&history_path);
    }
}
