//! Commands for AI suggestions: list pending, approve (creates rule),
//! reject, create, and generate (via the Anthropic API).

use super::BACKFILL_DAYS;
use crate::ai;
use crate::commands::breakdown::day_window;
use crate::commands::rules::compute_rule_preview;
use crate::models::{AiSuggestion, ClassificationRule, MatchType, RuleSource, SuggestionStatus};
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use serde_json::{json, Value};
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

/// Minimum time an uncategorized cluster must have before it is worth
/// proposing a rule for it.
const MIN_CLUSTER_MS: i64 = 5 * 60 * 1000;
/// Cap on how many clusters are sent to the model per run.
const MAX_CLUSTERS: usize = 25;

const SUGGESTION_SYSTEM_PROMPT: &str = "\
You are the categorization assistant inside MyTime, a personal time-tracking app. \
The user will give you clusters of uncategorized computer activity: an application \
name, an optional context (website/domain), total minutes spent, and sample window titles.

Propose classification rules that assign a category to future matching activity. \
Rules match case-insensitively: app_pattern is a substring of the process name \
(e.g. \"msedge.exe\"), title_pattern is a substring of the window title. Categories: \
\"development\" (coding, terminals, docs for programming), \"productivity\" (writing, \
research, office work, academic reading, file management), \"communication\" (chat, \
email, meetings), \"entertainment\" (video, games, social media, casual browsing).

Guidelines:
- Only propose rules you are reasonably confident about; skip ambiguous clusters entirely.
- For browsers, prefer a title_pattern for the site (e.g. \"youtube\") and leave \
app_pattern null so the rule works across browsers. For dedicated apps, set \
app_pattern and leave title_pattern null.
- Keep patterns short, distinctive, and likely to generalize; never use a full window title.
- confidence is 0.0-1.0; reason is one short sentence the user will read.
- At most one rule per cluster; fewer, well-chosen rules beat many weak ones.";

fn suggestion_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["suggestions"],
        "properties": {
            "suggestions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["app_pattern", "title_pattern", "category", "confidence", "reason"],
                    "properties": {
                        "app_pattern": {"type": ["string", "null"]},
                        "title_pattern": {"type": ["string", "null"]},
                        "category": {
                            "type": "string",
                            "enum": ["entertainment", "development", "productivity", "communication"]
                        },
                        "confidence": {"type": "number"},
                        "reason": {"type": "string"}
                    }
                }
            }
        }
    })
}

/// Normalized key used to detect duplicate patterns across rules/suggestions.
fn pattern_key(app: Option<&str>, title: Option<&str>) -> String {
    format!(
        "{}|{}",
        app.unwrap_or("").trim().to_lowercase(),
        title.unwrap_or("").trim().to_lowercase()
    )
}

#[tauri::command]
pub async fn generate_suggestions(
    state: State<'_, AppState>,
    days_back: u32,
) -> Result<Vec<AiSuggestion>, String> {
    let days_back = days_back.clamp(1, 90) as i32;

    // Gather all DB inputs before the network await (no guards across await).
    let (start_ms, _) = day_window(&state, -(days_back - 1));
    let (_, end_ms) = day_window(&state, 0);

    let mut clusters = state
        .storage
        .get_unknown_queue(start_ms, end_ms)
        .map_err(|e| e.to_string())?;
    clusters.retain(|c| c.total_duration_ms >= MIN_CLUSTER_MS);
    clusters.sort_by_key(|c| -c.total_duration_ms);
    clusters.truncate(MAX_CLUSTERS);

    if clusters.is_empty() {
        return Err("No uncategorized activity of 5+ minutes found in this period.".into());
    }

    let mut seen_patterns: std::collections::HashSet<String> = state
        .storage
        .get_all_rules()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| pattern_key(r.app_pattern.as_deref(), r.title_pattern.as_deref()))
        .chain(
            state
                .storage
                .get_pending_suggestions()
                .map_err(|e| e.to_string())?
                .iter()
                .map(|s| pattern_key(s.app_pattern.as_deref(), s.title_pattern.as_deref())),
        )
        .collect();

    let cluster_json: Vec<Value> = clusters
        .iter()
        .map(|c| {
            let samples: Vec<String> = c
                .sample_titles
                .iter()
                .take(4)
                .map(|t| t.chars().take(80).collect())
                .collect();
            json!({
                "app": c.app_name,
                "context": c.context,
                "minutes": c.total_duration_ms / 60_000,
                "sample_titles": samples,
            })
        })
        .collect();

    let user_prompt = format!(
        "Uncategorized activity clusters from the last {days_back} days:\n{}",
        serde_json::to_string_pretty(&cluster_json).map_err(|e| e.to_string())?
    );

    let output = ai::complete_json(
        SUGGESTION_SYSTEM_PROMPT,
        &user_prompt,
        suggestion_schema(),
        4000,
    )
    .await?;

    let proposals = output
        .get("suggestions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut created = Vec::new();
    for p in proposals {
        let app_pattern = p
            .get("app_pattern")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let title_pattern = p
            .get("title_pattern")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        if app_pattern.is_none() && title_pattern.is_none() {
            continue;
        }

        let key = pattern_key(app_pattern.as_deref(), title_pattern.as_deref());
        if !seen_patterns.insert(key) {
            continue; // duplicate of an existing rule or pending suggestion
        }

        let category = p
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !matches!(
            category.as_str(),
            "entertainment" | "development" | "productivity" | "communication"
        ) {
            continue;
        }

        // Ground the suggestion in real history: compute actual match stats
        // and skip proposals that would match nothing.
        let preview = compute_rule_preview(
            &state,
            app_pattern.clone(),
            title_pattern.clone(),
            MatchType::Contains,
            days_back,
        )?;
        if preview.match_count == 0 {
            continue;
        }

        let suggestion = AiSuggestion {
            suggestion_id: uuid::Uuid::new_v4().to_string(),
            app_pattern,
            title_pattern,
            match_type: MatchType::Contains,
            suggested_category: category,
            confidence: p
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or(0.5)
                .clamp(0.0, 1.0),
            reason: p
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            sample_titles: preview.sample_titles,
            match_count: preview.match_count as u32,
            total_duration_ms: preview.total_duration_ms,
            status: SuggestionStatus::Pending,
            created_at: utils::now_ms(),
            reviewed_at: None,
        };

        state
            .storage
            .insert_suggestion(&suggestion)
            .map_err(|e| e.to_string())?;
        created.push(suggestion);
    }

    tracing::info!(
        clusters = clusters.len(),
        created = created.len(),
        "AI suggestion generation finished"
    );
    Ok(created)
}
