//! Commands for tracking lifecycle (start/stop/pause) and live state queries.

use crate::models::TrackingState;
use crate::storage::StorageAdapter;
use crate::tracker;
use crate::utils;
use crate::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Manager, State};

/// Start the tracking thread. Shared by the command, auto-start at launch,
/// and quick-pause auto-resume. Clears any active quick pause.
pub(crate) fn start_tracking_inner(state: &AppState) {
    if state.is_tracking.load(Ordering::SeqCst) {
        return;
    }

    *state.paused_until_ms.lock() = None;
    let _ = state.storage.set_config("paused_until_ms", "0");
    state.is_tracking.store(true, Ordering::SeqCst);
    state.should_stop.store(false, Ordering::SeqCst);
    // Reset the capture clock so the live-timer edge doesn't span the
    // stopped period.
    tracker::mark_capture_now();

    let storage = Arc::clone(&state.storage);
    let should_stop = Arc::clone(&state.should_stop);
    let handle = std::thread::spawn(move || {
        tracker::track_foreground_window(storage, should_stop, None);
    });
    *state.tracking_thread.lock() = Some(handle);

    tracing::info!("tracking started");
}

/// Stop the tracking thread and wait for it to flush the open segment.
pub(crate) fn stop_tracking_inner(state: &AppState) {
    if !state.is_tracking.load(Ordering::SeqCst) {
        return;
    }

    state.is_tracking.store(false, Ordering::SeqCst);
    state.should_stop.store(true, Ordering::SeqCst);

    // Extract handle first to release mutex before join().
    let handle = state.tracking_thread.lock().take();
    if let Some(h) = handle {
        let _ = h.join();
    }

    tracing::info!("tracking stopped");
}

#[tauri::command]
pub fn start_tracking(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<TrackingState, String> {
    start_tracking_inner(&state);
    crate::update_tray_status(&app, "Tracking");
    Ok(get_tracking_state_inner(&state))
}

#[tauri::command]
pub fn stop_tracking(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<TrackingState, String> {
    stop_tracking_inner(&state);
    *state.paused_until_ms.lock() = None;
    let _ = state.storage.set_config("paused_until_ms", "0");
    crate::update_tray_status(&app, "Stopped");
    Ok(get_tracking_state_inner(&state))
}

/// Quick pause: stop tracking now and auto-resume later.
/// `minutes = None` pauses until the next day boundary ("until tomorrow").
#[tauri::command]
pub fn pause_tracking(
    app: tauri::AppHandle,
    state: State<AppState>,
    minutes: Option<u32>,
) -> Result<TrackingState, String> {
    stop_tracking_inner(&state);

    let resume_at = match minutes {
        Some(m) => utils::now_ms() + (m as i64) * 60_000,
        None => {
            let day_start_hour = state
                .storage
                .get_day_start_hour()
                .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
            // End of today's window = tomorrow's boundary.
            utils::day_range_ms_with_offset(day_start_hour, 0).1
        }
    };
    *state.paused_until_ms.lock() = Some(resume_at);
    // Persist so a restart during the pause honors it instead of auto-tracking.
    let _ = state
        .storage
        .set_config("paused_until_ms", &resume_at.to_string());
    crate::update_tray_status(&app, "Paused");
    tracing::info!(resume_at, "tracking paused");

    schedule_auto_resume(app, resume_at);

    Ok(get_tracking_state_inner(&state))
}

/// Auto-resume: one lightweight thread per pause. The paused_until check
/// makes stale timers (superseded pause, manual start/stop) no-ops.
/// Also used at startup to restore a persisted pause.
pub(crate) fn schedule_auto_resume(app: tauri::AppHandle, resume_at: i64) {
    std::thread::spawn(move || {
        let wait_ms = resume_at - utils::now_ms();
        if wait_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(wait_ms as u64));
        }
        if let Some(st) = app.try_state::<AppState>() {
            let still_this_pause = *st.paused_until_ms.lock() == Some(resume_at);
            if still_this_pause && !st.is_tracking.load(Ordering::SeqCst) {
                start_tracking_inner(&st);
                crate::update_tray_status(&app, "Tracking");
                tracing::info!("tracking auto-resumed after pause");
            }
        }
    });
}

#[tauri::command]
pub fn get_tracking_state(state: State<AppState>) -> Result<TrackingState, String> {
    Ok(get_tracking_state_inner(&state))
}

/// Build a `TrackingState` snapshot. The total comes straight from the DB
/// (checkpointed every ~60s by the tracker), so the timer the user sees is
/// the stored record plus a small live edge — no separate wall-clock state
/// that can drift across locks, sleeps, or day boundaries.
pub(crate) fn get_tracking_state_inner(state: &AppState) -> TrackingState {
    let is_tracking = state.is_tracking.load(Ordering::SeqCst);

    TrackingState {
        is_tracking,
        total_time_ms: state.storage.get_today_total_ms().unwrap_or(0),
        last_capture_ms: if is_tracking {
            tracker::last_capture_ms()
        } else {
            None
        },
        live_edge_ms: if is_tracking {
            tracker::live_edge_ms()
        } else {
            0
        },
        last_error: tracker::last_error(),
        paused_until_ms: *state.paused_until_ms.lock(),
    }
}
