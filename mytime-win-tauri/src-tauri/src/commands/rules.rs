//! Commands for classification-rule CRUD + preview.

use super::BACKFILL_DAYS;
use crate::models::{ClassificationRule, MatchType, RuleSource};
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_rules(state: State<AppState>) -> Result<Vec<ClassificationRule>, String> {
    // get_all_rules so the UI can show disabled rules too
    state.storage.get_all_rules().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_rule(
    state: State<AppState>,
    rule_id: String,
) -> Result<Option<ClassificationRule>, String> {
    state.storage.get_rule(&rule_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_rule(
    state: State<AppState>,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: String,
    category: String,
    tags: Option<Vec<String>>,
) -> Result<ClassificationRule, String> {
    let rule = ClassificationRule {
        rule_id: uuid::Uuid::new_v4().to_string(),
        app_pattern,
        title_pattern,
        match_type: MatchType::from_str(&match_type),
        category,
        tags,
        source: RuleSource::User,
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

    Ok(rule)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command contract — args mirror frontend payload
pub fn update_rule(
    state: State<AppState>,
    rule_id: String,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: String,
    category: String,
    tags: Option<Vec<String>>,
    enabled: bool,
    priority: i32,
) -> Result<(), String> {
    let existing = state
        .storage
        .get_rule(&rule_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Rule {} not found", rule_id))?;

    let rule = ClassificationRule {
        rule_id,
        app_pattern,
        title_pattern,
        match_type: MatchType::from_str(&match_type),
        category,
        tags,
        source: existing.source,
        priority,
        enabled,
        created_at: existing.created_at,
    };

    state
        .storage
        .upsert_rule(&rule)
        .map_err(|e| e.to_string())?;

    if rule.enabled {
        if let Err(e) = state.storage.backfill_labels_for_rule(&rule, BACKFILL_DAYS) {
            tracing::warn!(rule_id = %rule.rule_id, error = %e, "backfill failed");
        }
    }

    Ok(())
}

#[tauri::command]
pub fn delete_rule(state: State<AppState>, rule_id: String) -> Result<(), String> {
    state
        .storage
        .delete_rule(&rule_id)
        .map_err(|e| e.to_string())
}

/// Preview result for rule testing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RulePreview {
    pub match_count: usize,
    pub total_duration_ms: i64,
    pub sample_titles: Vec<String>,
}

/// Compute match statistics for a candidate rule against recent history.
/// Shared by the rule-form preview and the AI suggestion generator.
pub(crate) fn compute_rule_preview(
    state: &AppState,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: MatchType,
    days_back: i32,
) -> Result<RulePreview, String> {
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);

    let (start_ms, _) = utils::day_range_ms_with_offset(day_start_hour, -days_back);
    let end_ms = utils::now_ms();

    let segments = state
        .storage
        .get_segments_range(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    // Temporary rule for matching only — never persisted.
    let temp_rule = ClassificationRule {
        rule_id: String::new(),
        app_pattern,
        title_pattern,
        match_type,
        category: String::new(),
        tags: None,
        source: RuleSource::User,
        priority: 0,
        enabled: true,
        created_at: 0,
    };

    let mut match_count = 0;
    let mut total_duration_ms: i64 = 0;
    let mut sample_titles: Vec<String> = Vec::new();

    for seg in &segments {
        let title = seg.window_title.as_deref().unwrap_or("");
        if temp_rule.matches(&seg.app_name, title) {
            match_count += 1;
            total_duration_ms += seg.end_time - seg.start_time;

            if sample_titles.len() < 5 {
                let sample = format!("{}: {}", seg.app_name, title);
                if !sample_titles.contains(&sample) {
                    sample_titles.push(sample);
                }
            }
        }
    }

    Ok(RulePreview {
        match_count,
        total_duration_ms,
        sample_titles,
    })
}

#[tauri::command]
pub fn preview_rule_matches(
    state: State<AppState>,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: String,
    days_back: i32,
) -> Result<RulePreview, String> {
    compute_rule_preview(
        &state,
        app_pattern,
        title_pattern,
        MatchType::from_str(&match_type),
        days_back,
    )
}
