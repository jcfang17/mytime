//! MyTime - Personal Time Tracking Application
//!
//! A Tauri-based time tracking application that monitors foreground windows
//! and categorizes time spent across different applications.
//!
//! This file is the entry point: it owns `AppState`, sets up tracing/logging,
//! configures the system tray, and registers the Tauri commands defined in
//! `crate::commands::*`.

mod ai;
mod categorizer;
mod commands;
mod models;
mod storage;
mod tracker;
mod utils;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use storage::{SqliteStorage, StorageAdapter};
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WindowEvent,
};

/// Application state managed by Tauri and shared across all command handlers.
pub struct AppState {
    pub(crate) storage: Arc<SqliteStorage>,
    pub(crate) is_tracking: AtomicBool,
    pub(crate) should_stop: Arc<AtomicBool>,
    pub(crate) tracking_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// If quick-paused, when tracking should auto-resume.
    pub(crate) paused_until_ms: Mutex<Option<i64>>,
}

impl AppState {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let storage = SqliteStorage::new()?;
        Ok(Self {
            storage: Arc::new(storage),
            is_tracking: AtomicBool::new(false),
            should_stop: Arc::new(AtomicBool::new(false)),
            tracking_thread: Mutex::new(None),
            paused_until_ms: Mutex::new(None),
        })
    }
}

/// Reflect tracking state in the tray tooltip ("MyTime — Tracking/Paused/…").
pub(crate) fn update_tray_status(app: &AppHandle, status: &str) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(format!("MyTime — {status}")));
    }
}

fn load_icon() -> Image<'static> {
    let icon_data = include_bytes!("../icons/32x32.png");
    let img = image::load_from_memory(icon_data).expect("Failed to load icon");
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Image::new_owned(rgba.into_raw(), width, height)
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::new("Show").id("show").build(app)?;
    let start_item = MenuItemBuilder::new("Start Tracking")
        .id("start")
        .build(app)?;
    let stop_item = MenuItemBuilder::new("Stop Tracking")
        .id("stop")
        .build(app)?;
    let pause_15_item = MenuItemBuilder::new("Pause 15 minutes")
        .id("pause15")
        .build(app)?;
    let pause_60_item = MenuItemBuilder::new("Pause 1 hour")
        .id("pause60")
        .build(app)?;
    let pause_day_item = MenuItemBuilder::new("Pause until tomorrow")
        .id("pauseday")
        .build(app)?;
    let quit_item = MenuItemBuilder::new("Quit").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&start_item)
        .item(&stop_item)
        .separator()
        .item(&pause_15_item)
        .item(&pause_60_item)
        .item(&pause_day_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let _tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("MyTime — Stopped")
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
                "pause15" => {
                    let _ = app.emit("tray-pause", 15u32);
                }
                "pause60" => {
                    let _ = app.emit("tray-pause", 60u32);
                }
                "pauseday" => {
                    // 0 = "until tomorrow" (frontend maps it to minutes: null)
                    let _ = app.emit("tray-pause", 0u32);
                }
                "quit" => {
                    if let Some(state) = app.try_state::<AppState>() {
                        if state.is_tracking.load(Ordering::SeqCst) {
                            state.is_tracking.store(false, Ordering::SeqCst);
                            state.should_stop.store(true, Ordering::SeqCst);
                            let handle = state.tracking_thread.lock().take();
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

/// Initialize tracing with a daily-rotating file appender plus a stderr layer.
/// Returns the WorkerGuard which must be kept alive for the duration of the
/// program — when dropped, it flushes any buffered log entries.
fn init_tracing(
) -> Result<tracing_appender::non_blocking::WorkerGuard, Box<dyn std::error::Error + Send + Sync>> {
    use tracing_subscriber::prelude::*;

    let data_dir = SqliteStorage::data_dir()?;
    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "mytime.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(?log_dir, "tracing initialized");
    Ok(guard)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _tracing_guard = init_tracing().expect("Failed to initialize logging");

    tauri::Builder::default()
        // Single-instance must be the first registered plugin. A second
        // launch focuses the existing window instead of starting a new
        // tracker against the same database.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let state = AppState::new().expect("Failed to initialize app state");
            app.manage(state);

            setup_tray(app.handle())?;

            // Launched with --minimized (e.g. by autostart): start in the tray.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            // A quick pause persists across restarts: restore it instead of
            // auto-tracking through an explicit "pause until X".
            let state = app.state::<AppState>();
            let persisted_pause = state
                .storage
                .get_config("paused_until_ms")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|&v| v > utils::now_ms());

            if let Some(resume_at) = persisted_pause {
                *state.paused_until_ms.lock() = Some(resume_at);
                update_tray_status(app.handle(), "Paused");
                commands::tracking::schedule_auto_resume(app.handle().clone(), resume_at);
                tracing::info!(resume_at, "restored persisted pause at startup");
            } else {
                // Auto-start tracking per saved preference (default on) so a
                // launched-but-not-tracking app can never silently lose a day.
                let auto_track = state
                    .storage
                    .get_config("auto_start_tracking")
                    .ok()
                    .flatten()
                    .map(|v| v == "true")
                    .unwrap_or(true);
                if auto_track {
                    commands::tracking::start_tracking_inner(&state);
                    update_tray_status(app.handle(), "Tracking");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close request: hide window instead of quitting.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Tracking lifecycle
            commands::tracking::start_tracking,
            commands::tracking::stop_tracking,
            commands::tracking::pause_tracking,
            commands::tracking::get_tracking_state,
            // Day-window aggregates
            commands::breakdown::get_app_breakdown,
            commands::breakdown::get_category_breakdown,
            commands::breakdown::get_app_contexts,
            commands::breakdown::get_selected_breakdown,
            commands::breakdown::get_timeline_segments,
            commands::breakdown::get_day_range,
            commands::breakdown::get_day_label,
            commands::breakdown::get_label_provenance,
            commands::breakdown::set_app_category,
            // Digest + cleanup
            commands::digest::get_unknown_queue,
            commands::digest::get_daily_digest,
            // History (multi-day views)
            commands::history::get_history,
            commands::history::get_range_app_breakdown,
            // AI insights
            commands::insights::generate_insights,
            // Settings
            commands::settings::get_day_start_hour,
            commands::settings::set_day_start_hour,
            commands::settings::export_csv,
            commands::settings::delete_data_range,
            commands::settings::open_data_folder,
            commands::settings::format_duration,
            commands::settings::get_autostart_enabled,
            commands::settings::set_autostart_enabled,
            commands::settings::get_auto_track,
            commands::settings::set_auto_track,
            // Classification rules
            commands::rules::get_rules,
            commands::rules::get_rule,
            commands::rules::create_rule,
            commands::rules::update_rule,
            commands::rules::delete_rule,
            commands::rules::preview_rule_matches,
            // AI suggestions
            commands::suggestions::get_suggestions,
            commands::suggestions::approve_suggestion,
            commands::suggestions::reject_suggestion,
            commands::suggestions::create_suggestion,
            commands::suggestions::generate_suggestions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
