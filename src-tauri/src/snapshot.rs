// Pure snapshot assembly from app state: session summaries, retention
// filtering and token totals. Called by session::build_snapshot.
use super::*;

pub(crate) fn snapshot_from(
    requests: &[PermissionRequest],
    session_last_seen: &HashMap<String, u64>,
    retention_secs: u64,
    session_token_usage: &HashMap<String, TokenUsage>,
    known_sessions: &HashMap<String, KnownSession>,
    pinned_sessions: &HashSet<String>,
    online: bool,
    excluded_session_ids: &HashSet<String>,
) -> IslandSnapshot {
    let visible: Vec<&PermissionRequest> = requests
        .iter()
        .filter(|request| !request.archived)
        .collect();
    let pending_count = visible
        .iter()
        .filter(|request| request.status == PermissionStatus::Pending)
        .count();
    let active_request = visible
        .iter()
        .find(|request| request.status == PermissionStatus::Pending)
        .cloned()
        .cloned();
    let archived_count = requests.iter().filter(|r| r.archived).count();
    let mut sessions = build_session_summaries(&visible);
    // Cursor subagent conversations are nested under their parent; never list them
    // as independent top-level sessions.
    sessions.retain(|session| !excluded_session_ids.contains(&session.session_id));

    for session in sessions.iter_mut() {
        if let Some(info) = known_sessions.get(&session.session_id) {
            if session.transcript_path.is_none() && info.transcript_path.is_some() {
                session.transcript_path = info.transcript_path.clone();
            }
            if session.cwd.is_empty() || session.cwd == "." {
                if !info.cwd.is_empty() && info.cwd != "." {
                    session.cwd = info.cwd.clone();
                }
            }
        }
        session.session_host = session_host_for_summary(
            known_sessions,
            &session.session_id,
            &session.cwd,
            &session.agent,
        );
        if matches!(session.agent, AgentKind::Codex | AgentKind::Cursor)
            && session.transcript_path.is_none()
        {
            if let Some(path) = resolve_session_transcript_path_from_snapshot(
                known_sessions,
                requests,
                &session.session_id,
                &session.agent,
            ) {
                session.transcript_path = Some(path);
            }
        }
    }

    // Mark active sessions as pinned.
    for session in sessions.iter_mut() {
        session.pinned = pinned_sessions.contains(&session.session_id);
    }

    if retention_secs > 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let active_session_ids: HashSet<&str> =
            sessions.iter().map(|s| s.session_id.as_str()).collect();

        let mut retained_map: HashMap<&str, (String, String, Option<String>, AgentKind)> =
            HashMap::new();
        for request in requests.iter().filter(|r| r.archived) {
            if active_session_ids.contains(request.session.as_str()) {
                continue;
            }
            if excluded_session_ids.contains(&request.session) {
                continue;
            }
            if matches!(request.agent, AgentKind::Codex) && is_codex_internal_cwd(&request.cwd) {
                continue;
            }
            // Pinned sessions are always retained regardless of time.
            let is_pinned = pinned_sessions.contains(&request.session);
            if !is_pinned {
                let last_seen_ts = session_last_seen
                    .get(&request.session)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&request.requested_at));
                if now.saturating_sub(last_seen_ts) >= retention_secs {
                    continue;
                }
            }
            let entry = retained_map.entry(&request.session).or_insert_with(|| {
                (
                    request.cwd.clone(),
                    request.requested_at.clone(),
                    request.transcript_path.clone(),
                    request.agent.clone(),
                )
            });
            if request.requested_at > entry.1 {
                entry.0 = request.cwd.clone();
                entry.1 = request.requested_at.clone();
                entry.3 = request.agent.clone();
            }
            if entry.2.is_none() && request.transcript_path.is_some() {
                entry.2 = request.transcript_path.clone();
            }
        }

        for (session_id, (cwd, last_activity, transcript_path, agent)) in retained_map {
            let session_host = session_host_for_summary(known_sessions, session_id, &cwd, &agent);
            sessions.push(SessionSummary {
                session_id: session_id.to_string(),
                agent,
                cwd,
                pending_count: 0,
                total_count: 0,
                last_activity,
                transcript_path,
                pinned: pinned_sessions.contains(session_id),
                session_host,
                active_subagents: Vec::new(),
            });
        }

        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.pending_count.cmp(&a.pending_count))
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    // Include known sessions (from Stop/PostToolUse events) that have no
    // permission requests – these are sessions with only text output.
    {
        let existing_ids: HashSet<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (session_id, info) in known_sessions {
            if existing_ids.contains(session_id.as_str()) {
                continue;
            }
            if excluded_session_ids.contains(session_id) {
                continue;
            }
            if matches!(info.agent, AgentKind::Codex) && is_codex_internal_cwd(&info.cwd) {
                continue;
            }
            // Cursor observer hooks often omit workspace_roots; skip ghost rows until
            // we can resolve cwd or a transcript from ~/.cursor/projects.
            if matches!(info.agent, AgentKind::Cursor)
                && is_unresolved_cursor_cwd(&info.cwd)
                && info.transcript_path.is_none()
                && !pinned_sessions.contains(session_id)
            {
                continue;
            }
            // Pinned sessions always included; non-pinned filtered by retention.
            let is_pinned = pinned_sessions.contains(session_id);
            if !is_pinned && retention_secs > 0 {
                let last_seen_ts = session_last_seen
                    .get(session_id)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&info.last_activity));
                if now.saturating_sub(last_seen_ts) >= retention_secs {
                    continue;
                }
            }
            sessions.push(SessionSummary {
                session_id: session_id.clone(),
                agent: info.agent.clone(),
                cwd: info.cwd.clone(),
                pending_count: 0,
                total_count: 0,
                last_activity: info.last_activity.clone(),
                transcript_path: info.transcript_path.clone(),
                pinned: is_pinned,
                session_host: if info.host != platform::SessionHost::Unknown {
                    info.host
                } else {
                    session_host_for_summary(known_sessions, session_id, &info.cwd, &info.agent)
                },
                active_subagents: Vec::new(),
            });
        }

        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.pending_count.cmp(&a.pending_count))
                .then(b.last_activity.cmp(&a.last_activity))
        });
    }

    // ZCode chat history lives in its sqlite store, so every ZCode session row
    // must expose the virtual transcript path the chat reader understands.
    // Applied after all session sources are merged (live requests, archived
    // retention, known sessions) because the hook payload paths they carry are
    // ephemeral temp files deleted as soon as the hook returns.
    for session in sessions.iter_mut() {
        if matches!(session.agent, AgentKind::Zcode) {
            session.transcript_path = zcode_db_session_path(&session.session_id);
        }
    }

    let mut daily_tokens = TokenUsage::default();
    for usage in session_token_usage.values() {
        daily_tokens.add_assign(*usage);
    }

    let active_ids: HashSet<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
    let mut active_session_tokens = TokenUsage::default();
    for (session_id, usage) in session_token_usage.iter() {
        if active_ids.contains(session_id.as_str()) {
            active_session_tokens.add_assign(*usage);
        }
    }

    IslandSnapshot {
        online,
        pending_count,
        archived_count,
        active_request,
        recent: visible
            .into_iter()
            .take(12)
            .map(|r| {
                let mut stripped = (*r).clone();
                if stripped.status != PermissionStatus::Pending {
                    stripped.tool_input = None;
                }
                stripped
            })
            .collect(),
        sessions,
        daily_tokens,
        active_session_tokens,
        daily_tokens_by_model: HashMap::new(),
        active_session_tokens_by_model: HashMap::new(),
        hook_health: HookHealthSnapshot::default(),
    }
}

pub(crate) fn build_session_summaries(visible: &[&PermissionRequest]) -> Vec<SessionSummary> {
    let mut session_map: HashMap<&str, (String, usize, usize, String, Option<String>, AgentKind)> =
        HashMap::new();

    for request in visible {
        if matches!(request.agent, AgentKind::Codex) && is_codex_internal_cwd(&request.cwd) {
            continue;
        }
        let entry = session_map.entry(&request.session).or_insert_with(|| {
            (
                request.cwd.clone(),
                0,
                0,
                request.requested_at.clone(),
                request.transcript_path.clone(),
                request.agent.clone(),
            )
        });
        entry.2 += 1;
        if request.status == PermissionStatus::Pending {
            entry.1 += 1;
        }
        if request.requested_at > entry.3 {
            entry.0 = request.cwd.clone();
            entry.3 = request.requested_at.clone();
            entry.5 = request.agent.clone();
        }
        if entry.4.is_none() && request.transcript_path.is_some() {
            entry.4 = request.transcript_path.clone();
        }
    }

    let mut summaries: Vec<SessionSummary> = session_map
        .into_iter()
        .map(
            |(
                session_id,
                (cwd, pending_count, total_count, last_activity, transcript_path, agent),
            )| SessionSummary {
                session_id: session_id.to_string(),
                agent,
                cwd,
                pending_count,
                total_count,
                last_activity,
                transcript_path,
                pinned: false,
                session_host: platform::SessionHost::Unknown,
                active_subagents: Vec::new(),
            },
        )
        .collect();

    summaries.sort_by(|a, b| {
        b.pending_count
            .cmp(&a.pending_count)
            .then(b.last_activity.cmp(&a.last_activity))
    });
    summaries
}
