//! Commands for user settings: day boundary, autostart, CSV export, and
//! frontend duration formatting.

use crate::storage::StorageAdapter;
use crate::utils;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_day_start_hour(state: State<AppState>) -> Result<u32, String> {
    state
        .storage
        .get_day_start_hour()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_day_start_hour(state: State<AppState>, hour: u32) -> Result<(), String> {
    state
        .storage
        .set_day_start_hour(hour)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn format_duration(ms: i64) -> String {
    utils::format_duration_ms(ms)
}

/// Whether tracking starts automatically when the app launches (default on).
#[tauri::command]
pub fn get_auto_track(state: State<AppState>) -> Result<bool, String> {
    Ok(state
        .storage
        .get_config("auto_start_tracking")
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(true))
}

#[tauri::command]
pub fn set_auto_track(state: State<AppState>, enabled: bool) -> Result<(), String> {
    state
        .storage
        .set_config(
            "auto_start_tracking",
            if enabled { "true" } else { "false" },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| e.to_string())
    } else {
        autostart.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn export_csv(
    app: AppHandle,
    state: State<'_, AppState>,
    day_offset: i32,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);

    let day_label = utils::format_day_label(day_start_hour, day_offset);
    let default_name = format!("mytime-{}.csv", day_label.replace(' ', "-").to_lowercase());

    let file_path = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("CSV Files", &["csv"])
        .blocking_save_file();

    let path = match file_path {
        Some(p) => p,
        None => return Ok(0), // user cancelled
    };

    let (start_ms, end_ms) = utils::day_range_ms_with_offset(day_start_hour, day_offset);
    let end_ms = if day_offset == 0 {
        utils::now_ms()
    } else {
        end_ms
    };

    let segments = state
        .storage
        .get_segments_range(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    let path_buf = path.into_path().map_err(|_| "Invalid file path")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path_buf)
        .map_err(|e| e.to_string())?;

    use std::io::Write;
    writeln!(
        &file,
        "app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks"
    )
    .map_err(|e| e.to_string())?;

    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(file);

    let mut count = 0;
    for seg in &segments {
        let start_time = chrono::DateTime::from_timestamp_millis(seg.start_time)
            .map(|dt| dt.with_timezone(&chrono::Local))
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();
        let end_time = chrono::DateTime::from_timestamp_millis(seg.end_time)
            .map(|dt| dt.with_timezone(&chrono::Local))
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        wtr.serialize((
            &seg.app_name,
            seg.window_title.as_deref().unwrap_or(""),
            start_time,
            end_time,
            seg.duration_seconds(),
            seg.idle_seconds,
            seg.keystrokes,
            seg.mouse_clicks,
        ))
        .map_err(|e| e.to_string())?;
        count += 1;
    }

    wtr.flush().map_err(|e| e.to_string())?;
    Ok(count)
}
