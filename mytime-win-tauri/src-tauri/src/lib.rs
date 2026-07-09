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
use storage::SqliteStorage;
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
    pub(crate) session_start_ms: Mutex<Option<i64>>,
    /// Total tracked time at the moment a session started — used so the live
    /// timer adds session-elapsed without double-counting prior segments.
    pub(crate) baseline_ms: Mutex<Option<i64>>,
    pub(crate) should_stop: Arc<AtomicBool>,
    pub(crate) tracking_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
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
    let quit_item = MenuItemBuilder::new("Quit").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&start_item)
        .item(&stop_item)
        .separator()
        .item(&quit_item)
        .build()?;

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
            commands::settings::format_duration,
            commands::settings::get_autostart_enabled,
            commands::settings::set_autostart_enabled,
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
