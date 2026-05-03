//! Commands that read aggregate / time-window data: app and category
//! breakdowns, contexts, timeline, label provenance, and manual category
//! overrides.

use crate::models::{
    AppSummary, ContextSummary, Label, LabelProvenance, LabelSource, SelectedBreakdownRow,
    TimelineSegment,
};
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::State;

/// Compute the day window for a given offset, using `now_ms()` as the upper
/// bound when looking at today.
fn day_window(state: &AppState, day_offset: i32) -> (i64, i64) {
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
    let (start_ms, end_ms) = utils::day_range_ms_with_offset(day_start_hour, day_offset);
    let end_ms = if day_offset == 0 {
        utils::now_ms()
    } else {
        end_ms
    };
    (start_ms, end_ms)
}

#[tauri::command]
pub fn get_app_breakdown(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<AppSummary>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);

    let summaries = state
        .storage
        .get_app_breakdown(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    let filtered: Vec<AppSummary> = summaries
        .into_iter()
        .filter(|s| {
            let app_lower = s.app_name.to_lowercase();
            !app_lower.contains("explorer.exe")
                && !app_lower.contains("mytime")
                && !app_lower.contains("searchhost")
                && !app_lower.contains("shellexperiencehost")
                && !app_lower.contains("applicationframehost")
        })
        .filter(|s| s.total_duration_ms >= 5000)
        .collect();

    Ok(filtered)
}

/// Category breakdown entry returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoryBreakdownEntry {
    pub category: String,
    pub total_ms: i64,
    pub idle_ms: i64,
}

#[tauri::command]
pub fn get_category_breakdown(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<CategoryBreakdownEntry>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);

    // Use segment-level category breakdown so browsers (mixed-use apps) are
    // accounted per site rather than by dominant category.
    let breakdown = state
        .storage
        .get_segment_category_breakdown(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    Ok(breakdown
        .into_iter()
        .map(|(category, total_ms, idle_ms)| CategoryBreakdownEntry {
            category,
            total_ms,
            idle_ms,
        })
        .collect())
}

#[tauri::command]
pub fn get_app_contexts(
    state: State<AppState>,
    app_name: String,
    day_offset: i32,
) -> Result<Vec<ContextSummary>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);
    state
        .storage
        .get_app_contexts(&app_name, start_ms, end_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_selected_breakdown(
    state: State<AppState>,
    day_offset: i32,
    categories: Vec<String>,
) -> Result<Vec<SelectedBreakdownRow>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);
    state
        .storage
        .get_selected_breakdown(start_ms, end_ms, &categories)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_timeline_segments(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<TimelineSegment>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);
    state
        .storage
        .get_timeline_segments(start_ms, end_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_day_range(state: State<AppState>, day_offset: i32) -> Result<(i64, i64), String> {
    Ok(day_window(&state, day_offset))
}

#[tauri::command]
pub fn get_day_label(state: State<AppState>, day_offset: i32) -> String {
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
    utils::format_day_label(day_start_hour, day_offset)
}

#[tauri::command]
pub fn get_label_provenance(
    state: State<AppState>,
    title_hash: String,
) -> Result<LabelProvenance, String> {
    state
        .storage
        .get_label_provenance(&title_hash)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_category(
    state: State<AppState>,
    app_name: String,
    category: String,
    day_offset: i32,
) -> Result<(), String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);

    let segments = state
        .storage
        .get_segments_range(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    let mut updated_hashes = std::collections::HashSet::new();

    for segment in segments.iter().filter(|s| s.app_name == app_name) {
        if updated_hashes.contains(&segment.title_hash) {
            continue;
        }

        let label = Label {
            title_hash: segment.title_hash.clone(),
            category: category.clone(),
            source: LabelSource::Manual,
            confidence: None,
            updated_at: utils::now_ms(),
        };

        if state.storage.upsert_label(&label).is_ok() {
            updated_hashes.insert(segment.title_hash.clone());
        }
    }

    Ok(())
}
