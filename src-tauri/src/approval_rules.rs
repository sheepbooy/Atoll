//! Persistent approval rules (`~/.atoll/rules.json`).
//!
//! Rules let Atoll resolve permission requests without human input: each rule
//! matches requests by agent, tool name, command-label glob, and project
//! directory, and either allows or denies them before the blocking approval
//! flow starts. Deny rules win over allow rules (safety first); within the
//! same decision, the first matching rule in list order wins.
//!
//! The optional risk guard (default on, `riskGuardEnabled` in settings.json)
//! never auto-allows commands flagged dangerous by `risk_patterns`.
//!
//! Storage mirrors pricing.rs overrides plus the token_history.rs atomic
//! write: tmp → bak → rename, falling back to the `.bak` copy when the main
//! file is unreadable. Tests override the location with `ATOLL_RULES_PATH`
//! (mirrors `ATOLL_APPROVAL_HISTORY_PATH` in approval_history.rs).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{token_history, AgentKind, AppState, PermissionRequest};

pub(crate) const RULES_VERSION: u32 = 1;
pub(crate) const MAX_RULES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RuleDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApprovalRule {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) decision: RuleDecision,
    /// Reason returned to the agent on deny; also shown in the settings list.
    #[serde(default)]
    pub(crate) note: String,
    /// Agent kind key ("claude", "codex", ...); None = any agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent: Option<String>,
    /// Glob (`*`/`?`, case-insensitive) matched against the raw tool name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool: Option<String>,
    /// Glob matched against the command label ("Bash: npm test").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pattern: Option<String>,
    /// Project scope: only requests whose cwd is this directory or below it.
    /// None = global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) project_path: Option<String>,
    #[serde(default)]
    pub(crate) created_at: u64,
    #[serde(default)]
    pub(crate) match_count: u64,
    #[serde(default)]
    pub(crate) last_matched_at: Option<u64>,
}

impl ApprovalRule {
    /// Human-readable label for history detail suffixes and UI fallbacks.
    pub(crate) fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        if let Some(pattern) = self.pattern.as_deref().filter(|p| !p.is_empty()) {
            return pattern.to_string();
        }
        if let Some(tool) = self.tool.as_deref().filter(|t| !t.is_empty()) {
            return tool.to_string();
        }
        format!("rule {}", &self.id[..self.id.len().min(8)])
    }
}

/// Guards the process-wide `ATOLL_RULES_PATH` override in parallel tests.
#[cfg(test)]
pub(crate) fn approval_rules_env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::APPROVAL_RULES_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn rules_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_RULES_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("rules.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RulesFile {
    version: u32,
    #[serde(default)]
    rules: Vec<ApprovalRule>,
}

fn load_rules_file_at(path: &std::path::Path) -> Option<RulesFile> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn sanitize_rules(rules: Vec<ApprovalRule>) -> Vec<ApprovalRule> {
    rules
        .into_iter()
        .filter(|rule| !rule.id.is_empty())
        .take(MAX_RULES)
        .collect()
}

/// Load rules from disk: the main file, else the `.bak` copy, else empty.
pub(crate) fn load_rules_from_disk() -> Vec<ApprovalRule> {
    let Some(path) = rules_path() else {
        return Vec::new();
    };
    let rules = load_rules_file_at(&path)
        .or_else(|| load_rules_file_at(&path.with_extension("json.bak")))
        .map(|file| file.rules)
        .unwrap_or_default();
    sanitize_rules(rules)
}

pub(crate) fn validate_rules(rules: &[ApprovalRule]) -> Result<(), String> {
    if rules.len() > MAX_RULES {
        return Err(format!("Too many rules (max {MAX_RULES})"));
    }
    let mut seen_ids = std::collections::HashSet::new();
    for rule in rules {
        if rule.id.is_empty() {
            return Err("Rule id must not be empty".into());
        }
        if !seen_ids.insert(rule.id.clone()) {
            return Err(format!("Duplicate rule id: {}", rule.id));
        }
        let has_matcher = [
            rule.agent.as_deref(),
            rule.tool.as_deref(),
            rule.pattern.as_deref(),
            rule.project_path.as_deref(),
        ]
        .into_iter()
        .any(|matcher| matcher.map(|m| !m.trim().is_empty()).unwrap_or(false));
        if !has_matcher {
            // A rule with no matchers would govern every request from every
            // agent — too blunt to create by accident.
            return Err(format!("Rule {} needs at least one matcher", rule.label()));
        }
    }
    Ok(())
}

/// Atomic write (tmp → bak → rename) mirroring token_history.rs.
pub(crate) fn save_rules_to_disk(rules: &[ApprovalRule]) -> Result<(), String> {
    validate_rules(rules)?;
    let Some(path) = rules_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = RulesFile {
        version: RULES_VERSION,
        rules: rules.to_vec(),
    };
    let formatted = serde_json::to_string_pretty(&file).map_err(|error| error.to_string())?;
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, &formatted).map_err(|error| error.to_string())?;
    // Keep a backup so a crash mid-rename does not leave us with no recoverable file.
    if path.exists() {
        let _ = std::fs::copy(&path, path.with_extension("json.bak"));
    }
    std::fs::rename(&temp_path, &path).map_err(|error| error.to_string())
}

/// Case-insensitive glob with `*` (any run of characters) and `?` (any single
/// character). Plain substring/exact matching needs no wildcards.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();
    let (pattern, text) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti) = (None::<usize>, 0usize);
    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(star) = star_pi {
            pi = star + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

/// Project scope check: `cwd` equals `project` or lives below it.
fn cwd_within(cwd: &str, project: &str) -> bool {
    let cwd = cwd.trim_end_matches('/');
    let project = project.trim_end_matches('/');
    if project.is_empty() {
        return true;
    }
    cwd == project || cwd.starts_with(&format!("{project}/"))
}

fn rule_matches(rule: &ApprovalRule, request: &PermissionRequest) -> bool {
    if let Some(agent) = rule.agent.as_deref().filter(|a| !a.trim().is_empty()) {
        if agent != token_history::agent_kind_key(&request.agent) {
            return false;
        }
    }
    if let Some(tool) = rule.tool.as_deref().filter(|t| !t.trim().is_empty()) {
        if !glob_match(tool, &request.tool_name) {
            return false;
        }
    }
    if let Some(pattern) = rule.pattern.as_deref().filter(|p| !p.trim().is_empty()) {
        if !glob_match(pattern, &request.command) {
            return false;
        }
    }
    if let Some(project) = rule.project_path.as_deref().filter(|p| !p.trim().is_empty()) {
        if !cwd_within(&request.cwd, project) {
            return false;
        }
    }
    true
}

/// First matching rule, deny rules first. With the risk guard on, allow
/// rules never fire for commands flagged dangerous — they fall through to
/// the normal human approval flow.
pub(crate) fn evaluate_rules(
    rules: &[ApprovalRule],
    request: &PermissionRequest,
    risk_guard_enabled: bool,
) -> Option<ApprovalRule> {
    let dangerous = risk_guard_enabled && crate::risk_patterns::is_dangerous_command(&request.command);
    let mut allow_match: Option<&ApprovalRule> = None;
    for rule in rules.iter().filter(|rule| rule.enabled) {
        if !rule_matches(rule, request) {
            continue;
        }
        match rule.decision {
            RuleDecision::Deny => return Some(rule.clone()),
            RuleDecision::Allow => {
                if dangerous {
                    continue;
                }
                if allow_match.is_none() {
                    allow_match = Some(rule);
                }
            }
        }
    }
    allow_match.cloned()
}

/// Bump a rule's match statistics (cache + disk). Statistics only: failures
/// are logged and swallowed.
pub(crate) fn record_rule_match(state: &AppState, rule_id: &str) {
    let rules = {
        let mut rules = crate::lock_state(&state.approval_rules);
        let Some(rule) = rules.iter_mut().find(|rule| rule.id == rule_id) else {
            return;
        };
        rule.match_count = rule.match_count.saturating_add(1);
        rule.last_matched_at = Some(now_secs());
        rules.clone()
    };
    if let Err(error) = save_rules_to_disk(&rules) {
        eprintln!("Atoll approval rules: failed to persist match stats: {error}");
    }
}

/// Quick-pick scopes for rules created from an approval card:
/// - `command_project` — this exact command in this project
/// - `command_global` — this exact command anywhere
/// - `all_project` — every command in this project
pub(crate) fn build_rule_for_request(
    request: &PermissionRequest,
    scope: &str,
    decision: RuleDecision,
) -> Result<ApprovalRule, String> {
    let command_scoped = matches!(scope, "command_project" | "command_global");
    let project_scoped = matches!(scope, "command_project" | "all_project");
    if !command_scoped && !project_scoped {
        return Err(format!("Unknown rule scope: {scope}"));
    }
    let pattern = command_scoped.then(|| request.command.clone());
    let project_path = project_scoped.then(|| request.cwd.clone());
    let name = match (pattern.as_deref(), project_path.as_deref()) {
        (Some(pattern), _) => truncate_label(pattern),
        (None, Some(project)) => {
            let folder = project.trim_end_matches('/').rsplit('/').next().unwrap_or(project);
            format!("All tools · {folder}")
        }
        (None, None) => "Unnamed rule".into(),
    };
    Ok(ApprovalRule {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        enabled: true,
        decision,
        note: String::new(),
        // Scoped to the requesting agent so a Claude rule never silently
        // governs Codex; editable to "any agent" in Settings → Rules.
        agent: Some(token_history::agent_kind_key(&request.agent)),
        tool: None,
        pattern,
        project_path,
        created_at: now_secs(),
        match_count: 0,
        last_matched_at: None,
    })
}

/// True when an enabled rule with the same decision and matchers exists.
pub(crate) fn rules_contain_equivalent(rules: &[ApprovalRule], candidate: &ApprovalRule) -> bool {
    rules.iter().any(|rule| {
        rule.enabled
            && rule.decision == candidate.decision
            && rule.agent == candidate.agent
            && rule.tool == candidate.tool
            && rule.pattern == candidate.pattern
            && rule.project_path == candidate.project_path
    })
}

fn truncate_label(text: &str) -> String {
    if text.chars().count() <= 48 {
        return text.to_string();
    }
    format!("{}…", text.chars().take(48).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentKind;

    fn request(agent: AgentKind, tool: &str, command: &str, cwd: &str) -> PermissionRequest {
        PermissionRequest {
            id: "req-1".into(),
            tool_use_id: None,
            agent,
            session: "session-1".into(),
            tool_name: tool.into(),
            command: command.into(),
            detail: String::new(),
            cwd: cwd.into(),
            requested_at: "2026-09-21T00:00:00Z".into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        }
    }

    fn rule(decision: RuleDecision, pattern: Option<&str>) -> ApprovalRule {
        ApprovalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            enabled: true,
            decision,
            note: String::new(),
            agent: None,
            tool: None,
            pattern: pattern.map(str::to_string),
            project_path: None,
            created_at: 0,
            match_count: 0,
            last_matched_at: None,
        }
    }

    #[test]
    fn glob_matches_wildcards_case_insensitively() {
        assert!(glob_match("Bash: npm test", "Bash: npm test"));
        assert!(glob_match("bash: npm test", "Bash: npm test"));
        assert!(glob_match("Bash: npm *", "Bash: npm run build"));
        assert!(glob_match("*cargo*", "Bash: cargo build --release"));
        assert!(glob_match("Bash: rm -?f x", "Bash: rm -Rf x"));
        assert!(!glob_match("Bash: npm test", "Bash: npm run test"));
        assert!(!glob_match("Bash: npm *", "Bash: pnpm install"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn project_scope_matches_cwd_prefix() {
        let rule = ApprovalRule {
            project_path: Some("/Users/dev/Atoll".into()),
            ..rule(RuleDecision::Allow, None)
        };
        assert!(rule_matches(
            &rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/Users/dev/Atoll")
        ));
        assert!(rule_matches(
            &rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/Users/dev/Atoll/src-tauri")
        ));
        assert!(rule_matches(
            &rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/Users/dev/Atoll/")
        ));
        assert!(!rule_matches(
            &rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/Users/dev/Atollize")
        ));
        assert!(!rule_matches(
            &rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/Users/dev/other")
        ));
    }

    #[test]
    fn agent_and_tool_filters_narrow_matches() {
        let mut agent_rule = rule(RuleDecision::Allow, Some("Bash: *"));
        agent_rule.agent = Some("claude".into());
        assert!(rule_matches(
            &agent_rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/tmp")
        ));
        assert!(!rule_matches(
            &agent_rule,
            &request(AgentKind::Codex, "Bash", "Bash: ls", "/tmp")
        ));

        let mut tool_rule = rule(RuleDecision::Deny, None);
        tool_rule.tool = Some("web*".into());
        assert!(rule_matches(
            &tool_rule,
            &request(AgentKind::Claude, "WebFetch", "WebFetch", "/tmp")
        ));
        assert!(!rule_matches(
            &tool_rule,
            &request(AgentKind::Claude, "Bash", "Bash: ls", "/tmp")
        ));
    }

    #[test]
    fn deny_wins_over_allow_regardless_of_order() {
        let request = request(AgentKind::Claude, "Bash", "Bash: rm -rf /tmp/x", "/tmp");
        let rules = vec![
            rule(RuleDecision::Allow, Some("Bash: rm*")),
            rule(RuleDecision::Deny, Some("Bash: rm -rf*")),
        ];
        let matched = evaluate_rules(&rules, &request, false).expect("deny should win");
        assert_eq!(matched.decision, RuleDecision::Deny);

        let reversed = vec![
            rule(RuleDecision::Deny, Some("Bash: rm -rf*")),
            rule(RuleDecision::Allow, Some("Bash: rm*")),
        ];
        let matched = evaluate_rules(&reversed, &request, false).expect("deny should win");
        assert_eq!(matched.decision, RuleDecision::Deny);
    }

    #[test]
    fn first_matching_rule_wins_within_a_decision() {
        let request = request(AgentKind::Claude, "Bash", "Bash: npm test", "/tmp");
        let rules = vec![
            rule(RuleDecision::Allow, Some("Bash: npm *")),
            rule(RuleDecision::Allow, Some("Bash: *")),
        ];
        let matched = evaluate_rules(&rules, &request, false).expect("allow should match");
        assert_eq!(matched.pattern.as_deref(), Some("Bash: npm *"));
    }

    #[test]
    fn risk_guard_blocks_dangerous_allows_but_not_denies() {
        let request = request(AgentKind::Claude, "Bash", "Bash: rm -rf /tmp/x", "/tmp");
        let allow = vec![rule(RuleDecision::Allow, Some("Bash: rm*"))];
        assert!(evaluate_rules(&allow, &request, true).is_none());
        assert!(evaluate_rules(&allow, &request, false).is_some());

        let deny = vec![rule(RuleDecision::Deny, Some("Bash: rm*"))];
        let matched = evaluate_rules(&deny, &request, true).expect("deny ignores the guard");
        assert_eq!(matched.decision, RuleDecision::Deny);
    }

    #[test]
    fn disabled_rules_are_skipped() {
        let request = request(AgentKind::Claude, "Bash", "Bash: npm test", "/tmp");
        let mut disabled = rule(RuleDecision::Deny, Some("Bash: *"));
        disabled.enabled = false;
        assert!(evaluate_rules(&[disabled], &request, false).is_none());
    }

    #[test]
    fn no_match_falls_through_to_human_approval() {
        let request = request(AgentKind::Claude, "Bash", "Bash: npm test", "/tmp");
        let rules = vec![rule(RuleDecision::Deny, Some("Bash: git *"))];
        assert!(evaluate_rules(&rules, &request, false).is_none());
    }

    #[test]
    fn validate_rejects_duplicate_ids_and_matcherless_rules() {
        let mut duplicate = rule(RuleDecision::Allow, Some("Bash: *"));
        let same_id = rule(RuleDecision::Deny, Some("Bash: git *"));
        duplicate.id = same_id.id.clone();
        assert!(validate_rules(&[duplicate, same_id]).is_err());

        let matcherless = rule(RuleDecision::Allow, None);
        assert!(validate_rules(&[matcherless]).is_err());

        let mut overflow: Vec<ApprovalRule> = (0..=MAX_RULES)
            .map(|index| rule(RuleDecision::Allow, Some(&format!("p{index}"))))
            .collect();
        for (index, entry) in overflow.iter_mut().enumerate() {
            entry.id = format!("id-{index}");
        }
        assert!(validate_rules(&overflow).is_err());
    }

    #[test]
    fn persistence_roundtrips_and_falls_back_to_backup() {
        let guard = approval_rules_env_lock();
        let dir = std::env::temp_dir().join(format!("atoll-rules-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rules.json");
        std::env::set_var("ATOLL_RULES_PATH", &path);

        let mut stored = rule(RuleDecision::Allow, Some("Bash: npm *"));
        stored.name = "npm allow".into();
        save_rules_to_disk(&[stored.clone()]).unwrap();
        assert_eq!(load_rules_from_disk(), vec![stored.clone()]);

        // A second save backs up the now-existing main file, so a corrupt
        // main file falls back to the .bak copy.
        save_rules_to_disk(&[stored.clone()]).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load_rules_from_disk(), vec![stored]);

        // No files at all -> empty list.
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        assert!(load_rules_from_disk().is_empty());

        std::env::remove_var("ATOLL_RULES_PATH");
        let _ = std::fs::remove_dir_all(&dir);
        drop(guard);
    }

    #[test]
    fn build_rule_for_request_scopes_matchers() {
        let req = request(
            AgentKind::Claude,
            "Bash",
            "Bash: npm test",
            "/Users/dev/Atoll",
        );

        let command_project =
            build_rule_for_request(&req, "command_project", RuleDecision::Allow).unwrap();
        assert_eq!(command_project.pattern.as_deref(), Some("Bash: npm test"));
        assert_eq!(command_project.project_path.as_deref(), Some("/Users/dev/Atoll"));
        assert_eq!(command_project.agent.as_deref(), Some("claude"));

        let command_global =
            build_rule_for_request(&req, "command_global", RuleDecision::Deny).unwrap();
        assert_eq!(command_global.pattern.as_deref(), Some("Bash: npm test"));
        assert!(command_global.project_path.is_none());

        let all_project =
            build_rule_for_request(&req, "all_project", RuleDecision::Allow).unwrap();
        assert!(all_project.pattern.is_none());
        assert_eq!(all_project.project_path.as_deref(), Some("/Users/dev/Atoll"));
        assert_eq!(all_project.name, "All tools · Atoll");

        assert!(build_rule_for_request(&req, "everything", RuleDecision::Allow).is_err());
    }

    #[test]
    fn equivalent_rules_are_deduplicated() {
        let req = request(AgentKind::Claude, "Bash", "Bash: npm test", "/tmp");
        let candidate = build_rule_for_request(&req, "command_project", RuleDecision::Allow)
            .expect("candidate rule");
        let existing = vec![candidate.clone()];
        assert!(rules_contain_equivalent(&existing, &candidate));

        let mut different = candidate.clone();
        different.decision = RuleDecision::Deny;
        assert!(!rules_contain_equivalent(&existing, &different));

        let mut disabled = candidate.clone();
        disabled.enabled = false;
        assert!(!rules_contain_equivalent(&[disabled], &candidate));
    }
}
