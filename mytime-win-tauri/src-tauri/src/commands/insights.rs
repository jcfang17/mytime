//! AI-generated insights over a multi-day period, powering the History
//! tab's Insights card. Sends only aggregates (categories, app names,
//! durations) to the Anthropic API — never window titles.

use crate::ai;
use crate::commands::breakdown::{day_window, filter_noise_apps};
use crate::commands::history::collect_history;
use crate::storage::StorageAdapter;
use crate::AppState;
use serde_json::{json, Value};
use tauri::State;

const INSIGHTS_SYSTEM_PROMPT: &str = "\
You are the insights assistant inside MyTime, a personal time-tracking app. \
You receive aggregated statistics about how the user spent time on their computer: \
per-day active hours split by category, top applications, and totals for the \
previous period of the same length.

Write a short headline (one sentence, the single most notable thing about this \
period) and 3-5 insights. Address the user as \"you\". Good insights:
- cite concrete numbers (hours, percentages, day names) from the data;
- surface patterns (weekday vs weekend, streaks, unusually heavy or light days);
- compare against the previous period when the change is meaningful;
- end with at most one gentle, actionable observation.

Avoid moralizing, generic advice, and restating the raw table. If the data is \
too sparse for a claim, don't make it. Each insight is one or two sentences.";

/// Report returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InsightReport {
    pub headline: String,
    pub insights: Vec<String>,
}

fn insights_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["headline", "insights"],
        "properties": {
            "headline": {"type": "string"},
            "insights": {"type": "array", "items": {"type": "string"}}
        }
    })
}

fn hours(ms: i64) -> f64 {
    (ms as f64 / 3_600_000.0 * 10.0).round() / 10.0
}

#[tauri::command]
pub async fn generate_insights(
    state: State<'_, AppState>,
    period_days: u32,
    end_offset: i32,
) -> Result<InsightReport, String> {
    // Gather all DB inputs before the network await.
    let current = collect_history(&state, period_days, end_offset)?;
    let previous = collect_history(&state, period_days, end_offset - period_days as i32)?;

    let tracked_days = current.iter().filter(|d| d.total_ms > 0).count();
    if tracked_days == 0 {
        return Err("No tracked time in this period to analyze.".into());
    }

    let (range_start, _) = day_window(&state, end_offset - period_days as i32 + 1);
    let (_, range_end) = day_window(&state, end_offset);
    let top_apps = filter_noise_apps(
        state
            .storage
            .get_app_breakdown(range_start, range_end)
            .map_err(|e| e.to_string())?,
    );

    let days_json: Vec<Value> = current
        .iter()
        .filter(|d| d.total_ms > 0)
        .map(|d| {
            let cats: serde_json::Map<String, Value> = d
                .categories
                .iter()
                .filter(|c| c.total_ms - c.idle_ms > 0)
                .map(|c| (c.category.clone(), json!(hours(c.total_ms - c.idle_ms))))
                .collect();
            json!({
                "date": d.date_label,
                "weekday": d.weekday,
                "active_hours": hours(d.active_ms),
                "by_category_hours": cats,
            })
        })
        .collect();

    let apps_json: Vec<Value> = top_apps
        .iter()
        .take(10)
        .map(|a| {
            json!({
                "app": a.friendly_name,
                "active_hours": hours(a.total_duration_ms - a.idle_duration_ms),
            })
        })
        .collect();

    let prev_active_ms: i64 = previous.iter().map(|d| d.active_ms).sum();
    let cur_active_ms: i64 = current.iter().map(|d| d.active_ms).sum();

    let stats = json!({
        "period_days": period_days,
        "days": days_json,
        "top_apps": apps_json,
        "total_active_hours": hours(cur_active_ms),
        "previous_period_active_hours": hours(prev_active_ms),
    });

    let user_prompt = format!(
        "Time-tracking statistics for the selected {period_days}-day period:\n{}",
        serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?
    );

    let output = ai::complete_json(
        INSIGHTS_SYSTEM_PROMPT,
        &user_prompt,
        insights_schema(),
        1500,
    )
    .await?;

    let headline = output
        .get("headline")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let insights: Vec<String> = output
        .get("insights")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    if headline.is_empty() && insights.is_empty() {
        return Err("The model returned no insights. Try again.".into());
    }

    Ok(InsightReport { headline, insights })
}
