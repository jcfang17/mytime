//! MyTime - Personal Time Tracking Application
//!
//! A Tauri-based time tracking application that monitors foreground windows
//! and categorizes time spent across different applications.

mod categorizer;
mod models;
mod storage;
mod tracker;
mod utils;

use models::{
    AiSuggestion, AppSummary, ClassificationRule, ContextSummary, Label, LabelSource, MatchType,
    RuleSource, SelectedBreakdownRow, SuggestionStatus, TrackingState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use storage::{SqliteStorage, StorageAdapter};
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WindowEvent,
};

/// Application state managed by Tauri
pub struct AppState {
    storage: Arc<SqliteStorage>,
    is_tracking: AtomicBool,
    session_start_ms: Mutex<Option<i64>>,
    baseline_ms: Mutex<Option<i64>>, // Total time when session started
    should_stop: Arc<AtomicBool>,
    tracking_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AppState {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let storage = SqliteStorage::new()?;
        Ok(Self {
            storage: Arc::new(storage),
            is_tracking: AtomicBool::new(false),
            session_start_ms: Mutex::new(None),
            baseline_ms: Mutex::new(None),
            should_stop: Arc::new(AtomicBool::new(false)),
            tracking_thread: Mutex::new(None),
        })
    }
}

// === Tauri Commands ===

#[tauri::command]
fn start_tracking(state: State<AppState>) -> Result<TrackingState, String> {
    if state.is_tracking.load(Ordering::SeqCst) {
        return Ok(get_tracking_state_inner(&state));
    }

    // Capture baseline total time before starting (to avoid double-counting)
    let baseline = state.storage.get_today_active_ms().unwrap_or(0);

    state.is_tracking.store(true, Ordering::SeqCst);
    state.should_stop.store(false, Ordering::SeqCst);
    *state.session_start_ms.lock().unwrap() = Some(utils::now_ms());
    *state.baseline_ms.lock().unwrap() = Some(baseline);

    // Start tracking thread
    let storage = Arc::clone(&state.storage);
    let should_stop = Arc::clone(&state.should_stop);

    let handle = std::thread::spawn(move || {
        tracker::track_foreground_window(storage, should_stop, None);
    });

    *state.tracking_thread.lock().unwrap() = Some(handle);

    Ok(get_tracking_state_inner(&state))
}

#[tauri::command]
fn stop_tracking(state: State<AppState>) -> Result<TrackingState, String> {
    if !state.is_tracking.load(Ordering::SeqCst) {
        return Ok(get_tracking_state_inner(&state));
    }

    state.is_tracking.store(false, Ordering::SeqCst);
    state.should_stop.store(true, Ordering::SeqCst);

    // Wait for tracking thread to finish
    // Extract handle first to release mutex before join()
    let handle = state.tracking_thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }

    *state.session_start_ms.lock().unwrap() = None;
    *state.baseline_ms.lock().unwrap() = None;

    Ok(get_tracking_state_inner(&state))
}

#[tauri::command]
fn get_tracking_state(state: State<AppState>) -> Result<TrackingState, String> {
    Ok(get_tracking_state_inner(&state))
}

fn get_tracking_state_inner(state: &AppState) -> TrackingState {
    let is_tracking = state.is_tracking.load(Ordering::SeqCst);
    let session_start_ms = *state.session_start_ms.lock().unwrap();
    let baseline_ms = *state.baseline_ms.lock().unwrap();
    let total_time_ms = state.storage.get_today_active_ms().unwrap_or(0);

    TrackingState {
        is_tracking,
        session_start_ms,
        total_time_ms,
        baseline_ms,
    }
}

#[tauri::command]
fn get_app_breakdown(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<AppSummary>, String> {
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
    let (start_ms, end_ms) = utils::day_range_ms_with_offset(day_start_hour, day_offset);

    // For today, use current time as end
    let end_ms = if day_offset == 0 {
        utils::now_ms()
    } else {
        end_ms
    };

    let summaries = state
        .storage
        .get_app_breakdown(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    // Filter out system apps and short sessions
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

/// Category breakdown entry: (category, total_ms, idle_ms)
#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoryBreakdownEntry {
    pub category: String,
    pub total_ms: i64,
    pub idle_ms: i64,
}

#[tauri::command]
fn get_category_breakdown(
    state: State<AppState>,
    day_offset: i32,
) -> Result<Vec<CategoryBreakdownEntry>, String> {
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

    // Use segment-level category breakdown (not app-level)
    // This properly handles browsers where different sites have different categories
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
fn get_app_contexts(
    state: State<AppState>,
    app_name: String,
    day_offset: i32,
) -> Result<Vec<ContextSummary>, String> {
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

    state
        .storage
        .get_app_contexts(&app_name, start_ms, end_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_selected_breakdown(
    state: State<AppState>,
    day_offset: i32,
    categories: Vec<String>,
) -> Result<Vec<SelectedBreakdownRow>, String> {
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

    state
        .storage
        .get_selected_breakdown(start_ms, end_ms, &categories)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_app_category(
    state: State<AppState>,
    app_name: String,
    category: String,
    day_offset: i32,
) -> Result<(), String> {
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

    // Get all segments for this app
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
            source: LabelSource::Manual, // Direct user assignment (highest priority)
            confidence: None,
            updated_at: utils::now_ms(),
        };

        if state.storage.upsert_label(&label).is_ok() {
            updated_hashes.insert(segment.title_hash.clone());
        }
    }

    Ok(())
}

#[tauri::command]
fn get_day_label(day_offset: i32) -> String {
    utils::format_day_label(day_offset)
}

#[tauri::command]
fn get_day_start_hour(state: State<AppState>) -> Result<u32, String> {
    state
        .storage
        .get_day_start_hour()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_day_start_hour(state: State<AppState>, hour: u32) -> Result<(), String> {
    state
        .storage
        .set_day_start_hour(hour)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_csv(
    app: AppHandle,
    state: State<'_, AppState>,
    day_offset: i32,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    // Show save dialog
    let day_label = utils::format_day_label(day_offset);
    let default_name = format!("mytime-{}.csv", day_label.replace(" ", "-").to_lowercase());

    let file_path = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("CSV Files", &["csv"])
        .blocking_save_file();

    let path = match file_path {
        Some(p) => p,
        None => return Ok(0), // User cancelled
    };

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

    // Write header
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

#[tauri::command]
fn format_duration(ms: i64) -> String {
    utils::format_duration_ms(ms)
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(|e| e.to_string())
    } else {
        autostart.disable().map_err(|e| e.to_string())
    }
}

// === Classification Rules Commands ===

#[tauri::command]
fn get_rules(state: State<AppState>) -> Result<Vec<ClassificationRule>, String> {
    // Use get_all_rules for UI to show all rules including disabled
    state.storage.get_all_rules().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_rule(state: State<AppState>, rule_id: String) -> Result<Option<ClassificationRule>, String> {
    state.storage.get_rule(&rule_id).map_err(|e| e.to_string())
}

/// Default number of days to backfill when a rule is created/edited
const BACKFILL_DAYS: u32 = 7;

#[tauri::command]
fn create_rule(
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

    state.storage.upsert_rule(&rule).map_err(|e| e.to_string())?;

    // Backfill labels for matching segments (last N days)
    if let Err(e) = state.storage.backfill_labels_for_rule(&rule, BACKFILL_DAYS) {
        eprintln!("Warning: backfill failed for rule {}: {}", rule.rule_id, e);
    }

    Ok(rule)
}

#[tauri::command]
fn update_rule(
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
    // Get existing rule to preserve source and created_at
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
        source: existing.source, // Preserve original source
        priority,
        enabled,
        created_at: existing.created_at, // Preserve original creation time
    };

    state.storage.upsert_rule(&rule).map_err(|e| e.to_string())?;

    // Backfill labels for matching segments (last N days) if rule is enabled
    if rule.enabled {
        if let Err(e) = state.storage.backfill_labels_for_rule(&rule, BACKFILL_DAYS) {
            eprintln!("Warning: backfill failed for rule {}: {}", rule.rule_id, e);
        }
    }

    Ok(())
}

#[tauri::command]
fn delete_rule(state: State<AppState>, rule_id: String) -> Result<(), String> {
    state.storage.delete_rule(&rule_id).map_err(|e| e.to_string())
}

/// Preview what segments a rule would match (for rule testing UI)
#[tauri::command]
fn preview_rule_matches(
    state: State<AppState>,
    app_pattern: Option<String>,
    title_pattern: Option<String>,
    match_type: String,
    days_back: i32,
) -> Result<RulePreview, String> {
    let day_start_hour = state
        .storage
        .get_day_start_hour()
        .unwrap_or(utils::DEFAULT_DAY_START_HOUR);

    // Get segments from the last N days
    let (start_ms, _) = utils::day_range_ms_with_offset(day_start_hour, -days_back);
    let end_ms = utils::now_ms();

    let segments = state
        .storage
        .get_segments_range(start_ms, end_ms)
        .map_err(|e| e.to_string())?;

    // Create a temporary rule for matching
    let temp_rule = ClassificationRule {
        rule_id: String::new(),
        app_pattern: app_pattern.clone(),
        title_pattern: title_pattern.clone(),
        match_type: MatchType::from_str(&match_type),
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

            // Collect up to 5 sample titles
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

/// Preview result for rule testing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RulePreview {
    pub match_count: usize,
    pub total_duration_ms: i64,
    pub sample_titles: Vec<String>,
}

// === AI Suggestions Commands ===

#[tauri::command]
fn get_suggestions(state: State<AppState>) -> Result<Vec<AiSuggestion>, String> {
    state
        .storage
        .get_pending_suggestions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn approve_suggestion(state: State<AppState>, suggestion_id: String) -> Result<ClassificationRule, String> {
    // Get the suggestion
    let suggestion = state
        .storage
        .get_suggestion(&suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Suggestion {} not found", suggestion_id))?;

    // Create a rule from the suggestion
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

    // Save the rule
    state.storage.upsert_rule(&rule).map_err(|e| e.to_string())?;

    // Backfill labels for matching segments (last N days)
    if let Err(e) = state.storage.backfill_labels_for_rule(&rule, BACKFILL_DAYS) {
        eprintln!("Warning: backfill failed for rule {}: {}", rule.rule_id, e);
    }

    // Mark suggestion as approved
    state
        .storage
        .update_suggestion_status(&suggestion_id, SuggestionStatus::Approved)
        .map_err(|e| e.to_string())?;

    Ok(rule)
}

#[tauri::command]
fn reject_suggestion(state: State<AppState>, suggestion_id: String) -> Result<(), String> {
    state
        .storage
        .update_suggestion_status(&suggestion_id, SuggestionStatus::Rejected)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_suggestion(
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

fn load_icon() -> Image<'static> {
    // Decode the PNG at compile time
    let icon_data = include_bytes!("../icons/32x32.png");
    let img = image::load_from_memory(icon_data).expect("Failed to load icon");
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Image::new_owned(rgba.into_raw(), width, height)
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::new("Show").id("show").build(app)?;
    let start_item = MenuItemBuilder::new("Start Tracking").id("start").build(app)?;
    let stop_item = MenuItemBuilder::new("Stop Tracking").id("stop").build(app)?;
    let quit_item = MenuItemBuilder::new("Quit").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&start_item)
        .item(&stop_item)
        .separator()
        .item(&quit_item)
        .build()?;

    // Note: In Tauri 2.x, the TrayIcon is registered with the app's tray manager
    // and persists even when this handle is dropped. No need to store it.
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("MyTime - Time Tracker")
        .icon(load_icon())
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "start" => {
                    let _ = app.emit("tray-start", ());
                }
                "stop" => {
                    let _ = app.emit("tray-stop", ());
                }
                "quit" => {
                    // Stop tracking if active
                    if let Some(state) = app.try_state::<AppState>() {
                        if state.is_tracking.load(Ordering::SeqCst) {
                            state.is_tracking.store(false, Ordering::SeqCst);
                            state.should_stop.store(true, Ordering::SeqCst);
                            // Extract handle first to release mutex before join()
                            let handle = state.tracking_thread.lock().unwrap().take();
                            if let Some(h) = handle {
                                let _ = h.join();
                            }
                        }
                    }
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            // Initialize app state
            let state = AppState::new().expect("Failed to initialize app state");
            app.manage(state);

            // Setup system tray
            setup_tray(app.handle())?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close request: hide window instead of quitting
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Prevent the window from being destroyed
                api.prevent_close();
                // Hide the window
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_tracking,
            stop_tracking,
            get_tracking_state,
            get_app_breakdown,
            get_category_breakdown,
            get_app_contexts,
            get_selected_breakdown,
            set_app_category,
            get_day_label,
            get_day_start_hour,
            set_day_start_hour,
            export_csv,
            format_duration,
            get_autostart_enabled,
            set_autostart_enabled,
            // Classification rules
            get_rules,
            get_rule,
            create_rule,
            update_rule,
            delete_rule,
            preview_rule_matches,
            // AI suggestions
            get_suggestions,
            approve_suggestion,
            reject_suggestion,
            create_suggestion,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
