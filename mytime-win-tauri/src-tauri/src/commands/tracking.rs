//! Commands for tracking lifecycle (start/stop) and live state queries.

use crate::models::TrackingState;
use crate::storage::StorageAdapter;
use crate::tracker;
use crate::utils;
use crate::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn start_tracking(state: State<AppState>) -> Result<TrackingState, String> {
    if state.is_tracking.load(Ordering::SeqCst) {
        return Ok(get_tracking_state_inner(&state));
    }

    // Capture baseline total time before starting (to avoid double-counting)
    let baseline = state.storage.get_today_total_ms().unwrap_or(0);

    state.is_tracking.store(true, Ordering::SeqCst);
    state.should_stop.store(false, Ordering::SeqCst);
    *state.session_start_ms.lock() = Some(utils::now_ms());
    *state.baseline_ms.lock() = Some(baseline);

    // Start tracking thread
    let storage = Arc::clone(&state.storage);
    let should_stop = Arc::clone(&state.should_stop);

    let handle = std::thread::spawn(move || {
        tracker::track_foreground_window(storage, should_stop, None);
    });

    *state.tracking_thread.lock() = Some(handle);

    tracing::info!(baseline_ms = baseline, "tracking started");
    Ok(get_tracking_state_inner(&state))
}

#[tauri::command]
pub fn stop_tracking(state: State<AppState>) -> Result<TrackingState, String> {
    if !state.is_tracking.load(Ordering::SeqCst) {
        return Ok(get_tracking_state_inner(&state));
    }

    state.is_tracking.store(false, Ordering::SeqCst);
    state.should_stop.store(true, Ordering::SeqCst);

    // Wait for tracking thread to finish.
    // Extract handle first to release mutex before join().
    let handle = state.tracking_thread.lock().take();
    if let Some(h) = handle {
        let _ = h.join();
    }

    *state.session_start_ms.lock() = None;
    *state.baseline_ms.lock() = None;

    tracing::info!("tracking stopped");
    Ok(get_tracking_state_inner(&state))
}

#[tauri::command]
pub fn get_tracking_state(state: State<AppState>) -> Result<TrackingState, String> {
    Ok(get_tracking_state_inner(&state))
}

/// Build a `TrackingState` snapshot, detecting day-boundary crossings during a
/// running session and resetting the baseline so the live timer only shows
/// today's portion.
pub(crate) fn get_tracking_state_inner(state: &AppState) -> TrackingState {
    let is_tracking = state.is_tracking.load(Ordering::SeqCst);

    let (session_start_ms, baseline_ms) = {
        let mut session_start = state.session_start_ms.lock();
        let mut baseline = state.baseline_ms.lock();

        if is_tracking {
            if let Some(start) = *session_start {
                let day_start_hour = state
                    .storage
                    .get_day_start_hour()
                    .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
                let today_start = utils::today_start_ms_with_hour(day_start_hour);

                if start < today_start {
                    *session_start = Some(today_start);
                    *baseline = Some(0);
                }
            }
        }

        (*session_start, *baseline)
    };

    let total_time_ms = state.storage.get_today_total_ms().unwrap_or(0);

    TrackingState {
        is_tracking,
        session_start_ms,
        total_time_ms,
        baseline_ms,
    }
}
