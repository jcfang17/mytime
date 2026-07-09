//! Commands for historical multi-day views: per-day category history and
//! range-wide app totals powering the History tab.

use crate::commands::breakdown::{day_window, filter_noise_apps, CategoryBreakdownEntry};
use crate::models::AppSummary;
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::State;

/// Hard cap on how many days a single history request may span.
const MAX_HISTORY_DAYS: u32 = 92;

/// One day's worth of history: totals plus per-category breakdown.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DayHistory {
    pub day_offset: i32,
    pub date_label: String,
    pub weekday: String,
    pub total_ms: i64,
    pub active_ms: i64,
    pub categories: Vec<CategoryBreakdownEntry>,
}

/// Per-day history for the `days` days ending at `end_offset` (inclusive).
/// Days in the future (offset > 0) are skipped; days without data are
/// returned with zero totals so the chart shows gaps.
#[tauri::command]
pub fn get_history(
    state: State<AppState>,
    days: u32,
    end_offset: i32,
) -> Result<Vec<DayHistory>, String> {
    let days = days.clamp(1, MAX_HISTORY_DAYS);
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);

    let mut history = Vec::with_capacity(days as usize);
    for i in 0..days {
        let offset = end_offset - (days - 1 - i) as i32;
        if offset > 0 {
            continue;
        }

        let (start_ms, end_ms) = day_window(&state, offset);
        let breakdown = state
            .storage
            .get_segment_category_breakdown(start_ms, end_ms)
            .map_err(|e| e.to_string())?;

        let total_ms: i64 = breakdown.iter().map(|(_, total, _)| total).sum();
        let idle_ms: i64 = breakdown.iter().map(|(_, _, idle)| idle).sum();
        let (date_label, weekday) = utils::day_date_labels(day_start_hour, offset);

        history.push(DayHistory {
            day_offset: offset,
            date_label,
            weekday,
            total_ms,
            active_ms: total_ms - idle_ms,
            categories: breakdown
                .into_iter()
                .map(|(category, total_ms, idle_ms)| CategoryBreakdownEntry {
                    category,
                    total_ms,
                    idle_ms,
                })
                .collect(),
        });
    }

    Ok(history)
}

/// App breakdown aggregated over a multi-day window, from the start of
/// `start_offset`'s day to the end of `end_offset`'s day.
#[tauri::command]
pub fn get_range_app_breakdown(
    state: State<AppState>,
    start_offset: i32,
    end_offset: i32,
) -> Result<Vec<AppSummary>, String> {
    let (start_ms, _) = day_window(&state, start_offset.min(end_offset));
    let (_, end_ms) = day_window(&state, end_offset.max(start_offset));

    let summaries = state
        .storage
        .get_app_breakdown(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    Ok(filter_noise_apps(summaries))
}
