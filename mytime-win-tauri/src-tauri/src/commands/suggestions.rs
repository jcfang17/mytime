//! Commands for AI suggestions: list pending, approve (creates rule),
//! reject, and create.

use super::BACKFILL_DAYS;
use crate::models::{AiSuggestion, ClassificationRule, MatchType, RuleSource, SuggestionStatus};
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_suggestions(state: State<AppState>) -> Result<Vec<AiSuggestion>, String> {
    state
        .storage
        .get_pending_suggestions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn approve_suggestion(
    state: State<AppState>,
    suggestion_id: String,
) -> Result<ClassificationRule, String> {
    let suggestion = state
        .storage
        .get_suggestion(&suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Suggestion {} not found", suggestion_id))?;

    let rule = ClassificationRule {
        rule_id: uuid::Uuid::new_v4().to_string(),
        app_pattern: suggestion.app_pattern,
        title_pattern: suggestion.title_pattern,
        match_type: suggestion.match_type,
        category: suggestion.suggested_category,
        tags: None,
        source: RuleSource::AiApproved,
        priority: 0,
        enabled: true,
        created_at: utils::now_ms(),
    };

    state
        .storage
        .upsert_rule(&rule)
        .map_err(|e| e.to_string())?;

    if let Err(e) = state.storage.backfill_labels_for_rule(&rule, BACKFILL_DAYS) {
        tracing::warn!(rule_id = %rule.rule_id, error = %e, "backfill failed");
    }

    state
        .storage
        .update_suggestion_status(&suggestion_id, SuggestionStatus::Approved)
        .map_err(|e| e.to_string())?;

    Ok(rule)
}

#[tauri::command]
pub fn reject_suggestion(state: State<AppState>, suggestion_id: String) -> Result<(), String> {
    state
        .storage
        .update_suggestion_status(&suggestion_id, SuggestionStatus::Rejected)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command contract — args mirror frontend payload
pub fn create_suggestion(
    state: State<AppState>,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: String,
    suggested_category: String,
    confidence: f64,
    reason: String,
    sample_titles: Vec<String>,
    match_count: u32,
    total_duration_ms: i64,
) -> Result<AiSuggestion, String> {
    let suggestion = AiSuggestion {
        suggestion_id: uuid::Uuid::new_v4().to_string(),
        app_pattern,
        title_pattern,
        match_type: MatchType::from_str(&match_type),
        suggested_category,
        confidence,
        reason,
        sample_titles,
        match_count,
        total_duration_ms,
        status: SuggestionStatus::Pending,
        created_at: utils::now_ms(),
        reviewed_at: None,
    };

    state
        .storage
        .insert_suggestion(&suggestion)
        .map_err(|e| e.to_string())?;

    Ok(suggestion)
}
