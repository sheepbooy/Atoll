//! Runtime-only evidence from authenticated, successfully handled hooks.
use crate::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HookObservation {
    pub(crate) generation: u64,
    pub(crate) last_event_at: Option<u64>,
    pub(crate) last_event_name: Option<String>,
    pub(crate) last_permission_at: Option<u64>,
    #[serde(skip)]
    lifecycle: LifecycleTracker,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LifecycleTracker(HashMap<String, (&'static str, u64)>);
impl HookObservation {
    pub(crate) fn evidence(&self) -> Self {
        Self {
            generation: self.generation,
            last_event_at: self.last_event_at,
            last_event_name: self.last_event_name.clone(),
            last_permission_at: self.last_permission_at,
            lifecycle: LifecycleTracker::default(),
        }
    }
}
impl LifecycleTracker {
    fn accept(&mut self, key: String, kind: &'static str, at: u64) -> bool {
        let phase = match kind {
            "started" | "subagentStarted" => "running",
            _ => "ended",
        };
        if self.0.get(&key).is_some_and(|(last, _)| *last == phase) {
            return false;
        }
        if self.0.len() >= 512 {
            if let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, (_, at))| at)
                .map(|(key, _)| key.clone())
            {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(key, (phase, at));
        true
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifecycleEvent {
    pub(crate) event_id: String,
    pub(crate) agent: String,
    pub(crate) session_id: String,
    pub(crate) subagent_id: Option<String>,
    pub(crate) kind: String,
    pub(crate) occurred_at: u64,
}

pub(crate) fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}
pub(crate) fn generation(app: &AppHandle, agent: &str) -> u64 {
    lock_state(&app.state::<AppState>().hook_observations)
        .get(agent)
        .map(|o| o.generation)
        .unwrap_or(0)
}
pub(crate) fn reset(app: &AppHandle, agent: &str) {
    let state = app.state::<AppState>();
    let observation = {
        let mut observations = lock_state(&state.hook_observations);
        let old = observations.get(agent).map(|o| o.generation).unwrap_or(0);
        let observation = HookObservation {
            generation: old + 1,
            ..Default::default()
        };
        observations.insert(agent.into(), observation.clone());
        observation
    };
    let _ = app.emit(
        "hook-observed",
        serde_json::json!({"agent":agent,"observation":observation}),
    );
}
pub(crate) fn lifecycle_kind(event: &str, subagent: bool) -> Option<&'static str> {
    match event {
        "SubagentStart" | "subagentStart" => Some("subagentStarted"),
        "SubagentStop" | "subagentStop" => Some("subagentEnded"),
        "Stop" | "stop" | "AfterAgent" | "afterAgentResponse" => Some(if subagent {
            "subagentEnded"
        } else {
            "turnEnded"
        }),
        "SessionStart" | "UserPromptSubmit" | "sessionStart" | "beforeSubmitPrompt" => {
            Some("started")
        }
        // Some agents expose no turn-start hook. The first actual tool event
        // after an ended turn is the earliest supported evidence of work.
        "PermissionRequest" | "BeforeTool" | "PostToolUse" | "PostToolUseFailure"
        | "preToolUse" | "postToolUse" | "postToolUseFailure" | "AfterTool"
            if !subagent =>
        {
            Some("started")
        }
        _ => None,
    }
}

pub(crate) fn record(
    app: &AppHandle,
    agent: &str,
    event: &str,
    payload: &Value,
    permission: bool,
    expected_generation: u64,
    occurred_at: u64,
) {
    let Some(session_id) = payload_subagent_parent_session_id(payload) else {
        return;
    };
    let state = app.state::<AppState>();
    // tool_call_id is a subagent alias only on subagent lifecycle hooks;
    // ordinary tool calls must not become little agents.
    let subagent_id = if matches!(
        event,
        "SubagentStart" | "SubagentStop" | "subagentStart" | "subagentStop"
    ) {
        payload_subagent_id(payload)
    } else {
        payload
            .get("agent_id")
            .or_else(|| payload.get("subagent_id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
    };
    let kind = lifecycle_kind(event, subagent_id.is_some());
    if matches!(kind, Some("subagentStarted" | "subagentEnded")) && subagent_id.is_none() {
        return;
    }
    let (observation, emit_lifecycle) = {
        let mut observations = lock_state(&state.hook_observations);
        let entry = observations.entry(agent.into()).or_default();
        if entry.generation != expected_generation {
            return;
        }
        entry.last_event_at = Some(occurred_at);
        entry.last_event_name = Some(event.into());
        if permission {
            entry.last_permission_at = Some(occurred_at);
        }
        if event == "StopFailure" {
            // Failure ends activity without presenting a successful ending.
            entry.lifecycle.accept(
                serde_json::json!([session_id, subagent_id]).to_string(),
                "turnEnded",
                occurred_at,
            );
        }
        let emit_lifecycle = kind.is_some_and(|kind| {
            entry.lifecycle.accept(
                serde_json::json!([session_id, subagent_id]).to_string(),
                kind,
                occurred_at,
            )
        });
        (entry.evidence(), emit_lifecycle)
    };
    let _ = app.emit(
        "hook-observed",
        serde_json::json!({"agent":agent,"observation":observation}),
    );
    if let Some(kind) = kind.filter(|_| emit_lifecycle) {
        let lifecycle = LifecycleEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            agent: agent.into(),
            session_id: session_id.into(),
            subagent_id: subagent_id.map(str::to_owned),
            kind: kind.into(),
            occurred_at,
        };
        let _ = app.emit("agent-lifecycle-changed", lifecycle);
    }
}

#[tauri::command]
pub(crate) fn get_hook_observations(
    state: tauri::State<'_, AppState>,
) -> HashMap<String, HookObservation> {
    lock_state(&state.hook_observations)
        .iter()
        .map(|(agent, observation)| (agent.clone(), observation.evidence()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_real_lifecycle_events_are_mapped() {
        assert_eq!(lifecycle_kind("Stop", false), Some("turnEnded"));
        assert_eq!(lifecycle_kind("Stop", true), Some("subagentEnded"));
        assert_eq!(lifecycle_kind("PostToolUse", false), Some("started"));
        assert_eq!(lifecycle_kind("StopFailure", false), None);
        assert_eq!(lifecycle_kind("sessionStart", false), Some("started"));
    }
    #[test]
    fn duplicate_lifecycle_hooks_are_not_replayed() {
        let mut tracker = LifecycleTracker::default();
        assert!(tracker.accept("session:main".into(), "started", 1));
        assert!(!tracker.accept("session:main".into(), "started", 2));
        assert!(tracker.accept("session:child".into(), "subagentStarted", 3));
        assert!(tracker.accept("session:child".into(), "subagentEnded", 4));
        assert!(!tracker.accept("session:child".into(), "subagentEnded", 5));
        assert!(tracker.accept("session:main".into(), "turnEnded", 6));
        assert!(tracker.accept("session:main".into(), "started", 7));
    }
}
