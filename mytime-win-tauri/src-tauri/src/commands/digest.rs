//! Commands for the daily digest and the unknown-activity cleanup queue.

use crate::models::{DailyDigest, UnknownQueueItem};
use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::State;

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
pub fn get_unknown_queue(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<UnknownQueueItem>, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);
    state
        .storage
        .get_unknown_queue(start_ms, end_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_daily_digest(state: State<AppState>, day_offset: i32) -> Result<DailyDigest, String> {
    let (start_ms, end_ms) = day_window(&state, day_offset);
    state
        .storage
        .get_daily_digest(start_ms, end_ms)
        .map_err(|e| e.to_string())
}
