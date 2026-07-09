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

/// Export segments as CSV. `start_offset = None` exports all history;
/// otherwise the window spans `start_offset`'s day through `end_offset`'s.
#[tauri::command]
pub async fn export_csv(
    app: AppHandle,
    state: State<'_, AppState>,
    start_offset: Option<i32>,
    end_offset: i32,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);

    let range_label = match start_offset {
        None => "all-time".to_string(),
        Some(0) if end_offset == 0 => "today".to_string(),
        Some(s) => format!("last-{}-days", end_offset - s + 1),
    };
    let default_name = format!("mytime-{range_label}.csv");

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

    let start_ms = match start_offset {
        None => 0,
        Some(s) => utils::day_range_ms_with_offset(day_start_hour, s).0,
    };
    let end_ms = if end_offset >= 0 {
        utils::now_ms()
    } else {
        utils::day_range_ms_with_offset(day_start_hour, end_offset).1
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

/// Delete all segments in the window spanning `start_offset`'s day through
/// `end_offset`'s day (`start_offset = None` deletes everything, including
/// labels and AI suggestions; rules and settings are kept).
#[tauri::command]
pub fn delete_data_range(
    state: State<AppState>,
    start_offset: Option<i32>,
    end_offset: i32,
) -> Result<u64, String> {
    let deleted = match start_offset {
        None => state
            .storage
            .delete_all_activity()
            .map_err(|e| e.to_string())?,
        Some(s) => {
            let day_start_hour = state
                .storage
                .get_day_start_hour()
                .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
            let (start_ms, _) = utils::day_range_ms_with_offset(day_start_hour, s);
            let end_ms = if end_offset >= 0 {
                utils::now_ms()
            } else {
                utils::day_range_ms_with_offset(day_start_hour, end_offset).1
            };
            state
                .storage
                .delete_segments_range(start_ms, end_ms)
                .map_err(|e| e.to_string())?
        }
    };

    tracing::info!(deleted, ?start_offset, end_offset, "activity data deleted");
    Ok(deleted)
}

/// Open the data folder (database, logs) in Explorer.
#[tauri::command]
pub fn open_data_folder() -> Result<(), String> {
    let dir = crate::storage::SqliteStorage::data_dir().map_err(|e| e.to_string())?;
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
