// Approval rule engine commands: CRUD for ~/.atoll/rules.json, quick-pick
// rule creation from an approval card, and the risk-guard toggle.
use tauri::{AppHandle, State};

use crate::*;

#[tauri::command]
pub(crate) fn get_approval_rules(
    state: State<'_, AppState>,
) -> Vec<approval_rules::ApprovalRule> {
    lock_state(&state.approval_rules).clone()
}

/// Replace the whole rule list. Mirrors the pricing overrides flow: the
/// caller adopts the returned list as the source of truth.
#[tauri::command]
pub(crate) fn save_approval_rules(
    state: State<'_, AppState>,
    rules: Vec<approval_rules::ApprovalRule>,
) -> Result<Vec<approval_rules::ApprovalRule>, String> {
    approval_rules::save_rules_to_disk(&rules)?;
    *lock_state(&state.approval_rules) = rules;
    Ok(lock_state(&state.approval_rules).clone())
}

/// Quick-pick rule creation from an approval card ("Always ▾"). `scope` is
/// `command_project`, `command_global` or `all_project`; `decision` reuses the
/// approval decision (Approved → allow rule, Denied → deny rule). When an
/// equivalent enabled rule already exists this is a no-op. Returns the updated
/// rule list.
#[tauri::command]
pub(crate) fn create_approval_rule_from_request(
    state: State<'_, AppState>,
    id: String,
    scope: String,
    decision: Decision,
) -> Result<Vec<approval_rules::ApprovalRule>, String> {
    let request = {
        let requests = state.requests.lock().map_err(|error| error.to_string())?;
        requests.iter().find(|request| request.id == id).cloned()
    }
    .ok_or_else(|| format!("Permission request not found: {id}"))?;
    let rule_decision = match decision {
        Decision::Approved => approval_rules::RuleDecision::Allow,
        Decision::Denied => approval_rules::RuleDecision::Deny,
    };
    let candidate =
        approval_rules::build_rule_for_request(&request, &scope, rule_decision)?;

    let (already_exists, updated) = {
        let mut rules = lock_state(&state.approval_rules);
        let already_exists = approval_rules::rules_contain_equivalent(&rules, &candidate);
        if !already_exists {
            rules.insert(0, candidate);
        }
        (already_exists, rules.clone())
    };
    // Persist before committing to the cache so a failed write leaves the
    // in-memory list untouched.
    if !already_exists {
        approval_rules::save_rules_to_disk(&updated)?;
        *lock_state(&state.approval_rules) = updated;
    }
    Ok(lock_state(&state.approval_rules).clone())
}

#[tauri::command]
pub(crate) fn get_risk_guard_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.risk_guard_enabled)
}

#[tauri::command]
pub(crate) fn set_risk_guard_enabled(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.risk_guard_enabled) = enabled;
    persist_risk_guard_enabled(enabled);
    enabled
}
