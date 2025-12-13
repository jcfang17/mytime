#![windows_subsystem = "windows"]

extern crate native_windows_gui as nwg;
extern crate native_windows_derive as nwd;

mod categorizer;
mod models;
mod storage;
mod tracker;
mod utils;

use nwd::NwgUi;
use nwg::NativeUi;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::cell::RefCell;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use storage::{SqliteStorage, StorageAdapter};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeEntry {
    app_name: String,
    window_title: String,
    start_time: DateTime<Local>,
    end_time: DateTime<Local>,
    duration_seconds: u64,
    idle_seconds: u64,
    keystrokes: u64,
    mouse_clicks: u64,
}

#[derive(Debug, Clone, Default)]
struct AppStats {
    // Active time - sessions that passed activity threshold
    active_duration: Duration,
    active_keystrokes: u64,
    active_clicks: u64,
    // Idle time - sessions that failed activity threshold
    idle_duration: Duration,
    // Session counts for potential future use
    active_sessions: u32,
    idle_sessions: u32,
}

impl AppStats {
    /// Check if a single session is likely idle
    /// Call this BEFORE aggregating to decide which bucket the session goes into
    fn is_session_idle(duration_secs: u64, idle_secs: u64, keystrokes: u64, mouse_clicks: u64) -> bool {
        if duration_secs < 60 {
            return false; // Too short to judge, treat as active
        }

        let idle_ratio = idle_secs as f64 / duration_secs as f64;
        let has_minimal_input = keystrokes < 5 && mouse_clicks < 5;

        // Likely idle if: >80% idle time AND almost no input
        idle_ratio > 0.8 && has_minimal_input
    }

    /// Add a session to the appropriate bucket based on activity
    fn add_session(&mut self, duration: Duration, idle_secs: u64, keystrokes: u64, mouse_clicks: u64) {
        let is_idle = Self::is_session_idle(duration.as_secs(), idle_secs, keystrokes, mouse_clicks);

        if is_idle {
            self.idle_duration += duration;
            self.idle_sessions += 1;
        } else {
            self.active_duration += duration;
            self.active_keystrokes += keystrokes;
            self.active_clicks += mouse_clicks;
            self.active_sessions += 1;
        }
    }

    fn total_duration(&self) -> Duration {
        self.active_duration + self.idle_duration
    }

    #[allow(dead_code)]
    fn has_idle_time(&self) -> bool {
        self.idle_duration.as_secs() > 0
    }
}

#[derive(Default, NwgUi)]
pub struct MyTimeApp {
    // Fonts
    #[nwg_resource(family: "Segoe UI", size: 18, weight: 600)]
    font_title: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 28, weight: 700)]
    font_time: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 15)]
    font_normal: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 13)]
    font_small: nwg::Font,

    // Main window
    #[nwg_control(size: (520, 480), position: (300, 200), title: "MyTime", flags: "WINDOW|VISIBLE|MINIMIZE_BOX|RESIZABLE|MAXIMIZE_BOX")]
    #[nwg_events(OnWindowClose: [MyTimeApp::on_close], OnWindowMinimize: [MyTimeApp::on_minimize])]
    window: nwg::Window,

    // Layout - 5 columns for better date nav proportions
    #[nwg_layout(parent: window, spacing: 8, margin: [20, 20, 20, 20], max_column: Some(5))]
    layout: nwg::GridLayout,

    // Title/Status label
    #[nwg_control(text: "⏱ Stopped", font: Some(&data.font_title))]
    #[nwg_layout_item(layout: layout, row: 0, col: 0, col_span: 5)]
    status_label: nwg::Label,

    // Time display - large and prominent
    #[nwg_control(text: "00:00:00", font: Some(&data.font_time))]
    #[nwg_layout_item(layout: layout, row: 1, col: 0, col_span: 5)]
    time_label: nwg::Label,

    // Summary label - shows top app and stats
    #[nwg_control(text: "", font: Some(&data.font_small))]
    #[nwg_layout_item(layout: layout, row: 2, col: 0, col_span: 5)]
    summary_label: nwg::Label,

    // Category breakdown label
    #[nwg_control(text: "", font: Some(&data.font_small))]
    #[nwg_layout_item(layout: layout, row: 3, col: 0, col_span: 5)]
    category_label: nwg::Label,

    // Start button
    #[nwg_control(text: "▶ Start", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 4, col: 0, col_span: 2)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_start])]
    start_btn: nwg::Button,

    // Stop button
    #[nwg_control(text: "⏹ Stop", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 4, col: 2, col_span: 3)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_stop])]
    stop_btn: nwg::Button,

    // Date navigation - Previous button (narrow)
    #[nwg_control(text: "◀", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 5, col: 0)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_prev_day])]
    prev_day_btn: nwg::Button,

    // Date navigation - Date label (wide center)
    #[nwg_control(text: "Today", font: Some(&data.font_normal), h_align: nwg::HTextAlign::Center)]
    #[nwg_layout_item(layout: layout, row: 5, col: 1, col_span: 3)]
    date_label: nwg::Label,

    // Date navigation - Next button (narrow)
    #[nwg_control(text: "▶", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 5, col: 4)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_next_day])]
    next_day_btn: nwg::Button,

    // Section label with checkbox
    #[nwg_control(text: "Application Usage", font: Some(&data.font_small))]
    #[nwg_layout_item(layout: layout, row: 6, col: 0, col_span: 2)]
    section_label: nwg::Label,

    // Active time only checkbox
    #[nwg_control(text: "Active only", font: Some(&data.font_small), check_state: nwg::CheckBoxState::Checked)]
    #[nwg_layout_item(layout: layout, row: 6, col: 2, col_span: 3)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_toggle_hide_idle])]
    hide_idle_checkbox: nwg::CheckBox,

    // App usage list (columns are resizable by dragging header borders)
    #[nwg_control(list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP", ex_flags: nwg::ListViewExFlags::GRID | nwg::ListViewExFlags::FULL_ROW_SELECT)]
    #[nwg_layout_item(layout: layout, row: 7, col: 0, col_span: 5, row_span: 4)]
    #[nwg_events(OnListViewRightClick: [MyTimeApp::on_app_list_right_click])]
    app_list: nwg::ListView,

    // Context menu for changing app category
    #[nwg_control(popup: true)]
    category_menu: nwg::Menu,

    #[nwg_control(parent: category_menu, text: "Set Category")]
    category_menu_header: nwg::MenuItem,

    #[nwg_control(parent: category_menu)]
    category_menu_sep: nwg::MenuSeparator,

    #[nwg_control(parent: category_menu, text: "🎬 Entertainment")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_set_category_entertainment])]
    category_entertainment: nwg::MenuItem,

    #[nwg_control(parent: category_menu, text: "💻 Development")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_set_category_development])]
    category_development: nwg::MenuItem,

    #[nwg_control(parent: category_menu, text: "📝 Productivity")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_set_category_productivity])]
    category_productivity: nwg::MenuItem,

    #[nwg_control(parent: category_menu, text: "💬 Communication")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_set_category_communication])]
    category_communication: nwg::MenuItem,

    // Timer for UI updates
    #[nwg_control(interval: Duration::from_millis(1000))]
    #[nwg_events(OnTimerTick: [MyTimeApp::on_timer])]
    timer: nwg::AnimationTimer,

    // Tray icon - use system icon
    #[nwg_resource(source_system: Some(nwg::OemIcon::Information))]
    tray_icon: nwg::Icon,

    // Tray notification
    #[nwg_control(icon: Some(&data.tray_icon), tip: Some("MyTime - Stopped"))]
    #[nwg_events(MousePressLeftUp: [MyTimeApp::on_tray_click], OnContextMenu: [MyTimeApp::on_tray_right_click])]
    tray: nwg::TrayNotification,

    // Tray menu
    #[nwg_control(parent: window, popup: true)]
    tray_menu: nwg::Menu,

    #[nwg_control(parent: tray_menu, text: "Show")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_show])]
    tray_show: nwg::MenuItem,

    #[nwg_control(parent: tray_menu, text: "Start Tracking")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_start])]
    tray_start: nwg::MenuItem,

    #[nwg_control(parent: tray_menu, text: "Stop Tracking")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_stop])]
    tray_stop: nwg::MenuItem,

    #[nwg_control(parent: tray_menu)]
    tray_sep: nwg::MenuSeparator,

    #[nwg_control(parent: tray_menu, text: "Export Today's Data")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_export_csv])]
    tray_export: nwg::MenuItem,

    #[nwg_control(parent: tray_menu)]
    tray_sep2: nwg::MenuSeparator,

    #[nwg_control(parent: tray_menu, text: "Start with Windows")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_toggle_autostart])]
    tray_autostart: nwg::MenuItem,

    #[nwg_control(parent: tray_menu, text: "Day Starts At...")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_configure_day_start])]
    tray_day_start: nwg::MenuItem,

    #[nwg_control(parent: tray_menu)]
    tray_sep3: nwg::MenuSeparator,

    #[nwg_control(parent: tray_menu, text: "Exit")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_exit])]
    tray_exit: nwg::MenuItem,

    // State - stored as regular fields, not NWG controls
    is_tracking: RefCell<bool>,
    session_start: RefCell<Option<Instant>>,
    total_time: RefCell<Duration>,
    time_entries: Arc<Mutex<Vec<TimeEntry>>>,
    app_usage: Arc<Mutex<HashMap<String, AppStats>>>,
    should_stop_tracking: Arc<AtomicBool>,
    tracking_thread: RefCell<Option<std::thread::JoinHandle<()>>>,
    hide_idle_sessions: RefCell<bool>,
    // Selected day offset (0 = today, -1 = yesterday, etc.)
    selected_day_offset: RefCell<i32>,
    // Selected app for context menu (stores app_name, not friendly_name)
    selected_app_for_category: RefCell<Option<String>>,
    // SQLite storage for new segment-based tracking
    sqlite_storage: Option<Arc<SqliteStorage>>,
}

impl MyTimeApp {
    fn on_start(&self) {
        if *self.is_tracking.borrow() {
            return;
        }

        *self.is_tracking.borrow_mut() = true;
        *self.session_start.borrow_mut() = Some(Instant::now());
        self.should_stop_tracking.store(false, Ordering::SeqCst);

        self.status_label.set_text("⏱ Tracking");
        self.start_btn.set_enabled(false);
        self.stop_btn.set_enabled(true);
        self.tray_start.set_enabled(false);
        self.tray_stop.set_enabled(true);
        self.update_tray_icon(true);

        // Start tracking thread using new segment-based tracker
        let stop_flag = Arc::clone(&self.should_stop_tracking);

        // Use new SQLite-backed tracker if available, otherwise fall back to legacy
        if let Some(ref storage) = self.sqlite_storage {
            let storage = Arc::clone(storage);
            let handle = std::thread::spawn(move || {
                tracker::track_foreground_window(storage, stop_flag, None);
            });
            *self.tracking_thread.borrow_mut() = Some(handle);
        } else {
            // Fallback to legacy tracker (shouldn't happen in normal usage)
            let entries = Arc::clone(&self.time_entries);
            let app_usage = Arc::clone(&self.app_usage);
            let handle = std::thread::spawn(move || {
                legacy_tracker::track_foreground_window(entries, app_usage, stop_flag);
            });
            *self.tracking_thread.borrow_mut() = Some(handle);
        }
    }

    fn on_stop(&self) {
        if !*self.is_tracking.borrow() {
            return;
        }

        *self.is_tracking.borrow_mut() = false;
        self.should_stop_tracking.store(true, Ordering::SeqCst);

        if let Some(start) = self.session_start.borrow_mut().take() {
            *self.total_time.borrow_mut() += start.elapsed();
        }

        // Wait for tracking thread
        if let Some(handle) = self.tracking_thread.borrow_mut().take() {
            let _ = handle.join();
        }

        // Save entries to legacy CSV only if NOT using SQLite
        // (SQLite tracker saves segments directly, no need for CSV)
        if self.sqlite_storage.is_none() {
            if let Ok(mut entries) = self.time_entries.lock() {
                if !entries.is_empty() {
                    legacy_storage::save_to_csv(&entries).ok();
                    entries.clear();
                }
            }
        }

        self.status_label.set_text("⏱ Stopped");
        self.start_btn.set_enabled(true);
        self.stop_btn.set_enabled(false);
        self.tray_start.set_enabled(true);
        self.tray_stop.set_enabled(false);
        self.update_tray_icon(false);
    }

    fn on_timer(&self) {
        let total = if *self.is_tracking.borrow() {
            if let Some(start) = *self.session_start.borrow() {
                *self.total_time.borrow() + start.elapsed()
            } else {
                *self.total_time.borrow()
            }
        } else {
            *self.total_time.borrow()
        };

        let hours = total.as_secs() / 3600;
        let minutes = (total.as_secs() % 3600) / 60;
        let seconds = total.as_secs() % 60;

        // Display time in HH:MM:SS format
        self.time_label.set_text(&format!("{:02}:{:02}:{:02}", hours, minutes, seconds));

        // Update tray tooltip
        let status = if *self.is_tracking.borrow() { "Tracking" } else { "Stopped" };
        let tip = format!("MyTime - {} ({:02}:{:02}:{:02})", status, hours, minutes, seconds);
        self.tray.set_tip(&tip);

        // Update summary, categories, and app list
        self.update_summary();
        self.update_category_breakdown();
        self.update_app_list();
    }

    fn update_summary(&self) {
        // Try SQLite first
        if let Some(ref storage) = self.sqlite_storage {
            let (start_ms, end_ms) = self.get_selected_day_range();
            // For today, use current time as end; for past days, use end of day
            let offset = *self.selected_day_offset.borrow();
            let end_ms = if offset == 0 { utils::now_ms() } else { end_ms };

            match storage.get_app_breakdown(start_ms, end_ms) {
                Ok(summaries) => {
                    let filtered: Vec<_> = summaries
                        .iter()
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

                    let top_app = filtered.first();
                    let app_count = filtered.len();

                    // Calculate totals
                    let total_ms: i64 = filtered.iter().map(|s| s.total_duration_ms).sum();
                    let idle_ms: i64 = filtered.iter().map(|s| s.idle_duration_ms).sum();
                    let active_ms = total_ms - idle_ms;

                    let summary = if let Some(app) = top_app {
                        let top_time = utils::format_duration_ms(app.total_duration_ms);
                        let active_str = utils::format_duration_ms(active_ms);
                        format!(
                            "Active: {} · Top: {} ({}) · {} apps",
                            active_str, app.friendly_name, top_time, app_count
                        )
                    } else {
                        "No activity tracked yet".to_string()
                    };

                    self.summary_label.set_text(&summary);
                    return;
                }
                Err(_) => {}
            }
        }

        // Fallback to legacy HashMap
        if let Ok(usage) = self.app_usage.lock() {
            let top_app = usage
                .iter()
                .filter(|(app, _)| {
                    let app_lower = app.to_lowercase();
                    !app_lower.contains("explorer.exe")
                        && !app_lower.contains("mytime")
                        && !app_lower.contains("searchhost")
                        && !app_lower.contains("shellexperiencehost")
                        && !app_lower.contains("applicationframehost")
                })
                .filter(|(_, stats)| stats.active_duration.as_secs() >= 5)
                .max_by_key(|(_, stats)| stats.active_duration);

            let app_count = usage
                .iter()
                .filter(|(app, stats)| {
                    let app_lower = app.to_lowercase();
                    stats.active_duration.as_secs() >= 5
                        && !app_lower.contains("explorer.exe")
                        && !app_lower.contains("mytime")
                })
                .count();

            let summary = if let Some((app_name, stats)) = top_app {
                let friendly = Self::to_friendly_name(app_name);
                let time_str = Self::format_duration(stats.active_duration);
                format!("Top: {} ({}) · {} apps today", friendly, time_str, app_count)
            } else {
                "No activity tracked yet".to_string()
            };

            self.summary_label.set_text(&summary);
        }
    }

    fn update_category_breakdown(&self) {
        // Try SQLite first
        if let Some(ref storage) = self.sqlite_storage {
            let (start_ms, end_ms) = self.get_selected_day_range();
            let offset = *self.selected_day_offset.borrow();
            let end_ms = if offset == 0 { utils::now_ms() } else { end_ms };

            // Get app breakdown and compute category totals directly
            if let Ok(summaries) = storage.get_app_breakdown(start_ms, end_ms) {
                // Aggregate by category
                let mut category_totals: std::collections::HashMap<String, i64> =
                    std::collections::HashMap::new();
                for summary in &summaries {
                    if let Some(cat) = &summary.primary_category {
                        *category_totals.entry(cat.clone()).or_default() += summary.total_duration_ms;
                    }
                }

                if category_totals.is_empty() {
                    self.category_label.set_text("");
                    return;
                }

                // Sort by duration (descending) and format
                let mut categories: Vec<_> = category_totals.into_iter().collect();
                categories.sort_by(|a, b| b.1.cmp(&a.1));

                // Calculate total for percentage
                let total_ms: i64 = categories.iter()
                    .filter(|(cat, _)| cat != "unknown")
                    .map(|(_, ms)| *ms)
                    .sum();

                let parts: Vec<String> = categories
                    .iter()
                    .filter(|(cat, ms)| *ms >= 5000 && cat != "unknown") // At least 5s, skip unknown
                    .take(3) // Show top 3 categories
                    .map(|(cat, ms)| {
                        let (emoji, name) = match cat.as_str() {
                            "entertainment" => ("🎬", "Entertainment"),
                            "development" => ("💻", "Development"),
                            "productivity" => ("📝", "Productivity"),
                            "communication" => ("💬", "Communication"),
                            _ => ("❓", "Other"),
                        };
                        let pct = if total_ms > 0 { (*ms * 100 / total_ms) as usize } else { 0 };
                        let time_str = utils::format_duration_ms(*ms);
                        format!("{} {} {} ({}%)", emoji, name, time_str, pct)
                    })
                    .collect();

                if parts.is_empty() {
                    self.category_label.set_text("");
                } else {
                    self.category_label.set_text(&parts.join("   "));
                }
                return;
            }
        }

        // No SQLite or no data - clear label
        self.category_label.set_text("");
    }

    fn update_app_list(&self) {
        let show_all_time = !*self.hide_idle_sessions.borrow(); // Checkbox unchecked = show all

        // Helper to get category emoji
        let category_emoji = |cat: Option<&String>| -> &'static str {
            match cat.map(|s| s.as_str()) {
                Some("entertainment") => "🎬",
                Some("development") => "💻",
                Some("productivity") => "📝",
                Some("communication") => "💬",
                _ => "📁",
            }
        };

        // Helper to populate list view - now includes category
        let populate_list = |app_list: &nwg::ListView, data: Vec<(String, i64, i64)>| {
            // Ensure columns exist
            if app_list.column_len() == 0 {
                app_list.insert_column("Application");
                app_list.insert_column("Time");
                app_list.insert_column("Idle");
                app_list.set_column_width(0, 220);
                app_list.set_column_width(1, 80);
                app_list.set_column_width(2, 100);
            }

            let current_count = app_list.len();
            if current_count != data.len() || current_count == 0 {
                app_list.clear();

                for (app, time_ms, idle_ms) in data.iter() {
                    let time_str = utils::format_duration_ms(*time_ms);
                    let idle_str = if *idle_ms > 0 {
                        format!("💤 {}", utils::format_duration_ms(*idle_ms))
                    } else {
                        "-".to_string()
                    };
                    let row_idx = app_list.len() as i32;

                    app_list.insert_item(nwg::InsertListViewItem {
                        index: Some(row_idx),
                        column_index: 0,
                        text: Some(app.clone()),
                        image: None,
                    });
                    app_list.insert_item(nwg::InsertListViewItem {
                        index: Some(row_idx),
                        column_index: 1,
                        text: Some(time_str),
                        image: None,
                    });
                    app_list.insert_item(nwg::InsertListViewItem {
                        index: Some(row_idx),
                        column_index: 2,
                        text: Some(idle_str),
                        image: None,
                    });
                }
                return;
            }

            // Row count is stable: update in-place
            for (row_idx, (app, time_ms, idle_ms)) in data.iter().enumerate() {
                let time_str = utils::format_duration_ms(*time_ms);
                let idle_str = if *idle_ms > 0 {
                    format!("💤 {}", utils::format_duration_ms(*idle_ms))
                } else {
                    "-".to_string()
                };

                app_list.update_item(
                    row_idx,
                    nwg::InsertListViewItem {
                        index: None,
                        column_index: 0,
                        text: Some(app.clone()),
                        image: None,
                    },
                );
                app_list.update_item(
                    row_idx,
                    nwg::InsertListViewItem {
                        index: None,
                        column_index: 1,
                        text: Some(time_str),
                        image: None,
                    },
                );
                app_list.update_item(
                    row_idx,
                    nwg::InsertListViewItem {
                        index: None,
                        column_index: 2,
                        text: Some(idle_str),
                        image: None,
                    },
                );
            }
        };

        // Try SQLite first
        if let Some(ref storage) = self.sqlite_storage {
            let (start_ms, end_ms) = self.get_selected_day_range();
            let offset = *self.selected_day_offset.borrow();
            let end_ms = if offset == 0 { utils::now_ms() } else { end_ms };

            if let Ok(summaries) = storage.get_app_breakdown(start_ms, end_ms) {
                let mut filtered: Vec<(String, i64, i64)> = summaries
                    .iter()
                    .filter(|s| {
                        let app_lower = s.app_name.to_lowercase();
                        !app_lower.contains("explorer.exe")
                            && !app_lower.contains("mytime")
                            && !app_lower.contains("searchhost")
                            && !app_lower.contains("shellexperiencehost")
                            && !app_lower.contains("applicationframehost")
                    })
                    .filter(|s| s.total_duration_ms >= 5000)
                    .map(|s| {
                        // Active = total - idle; show total or active based on checkbox
                        let active_ms = s.total_duration_ms - s.idle_duration_ms;
                        let display_ms = if show_all_time {
                            s.total_duration_ms
                        } else {
                            active_ms
                        };
                        // Add category emoji to app name
                        let emoji = category_emoji(s.primary_category.as_ref());
                        let app_with_cat = format!("{} {}", emoji, s.friendly_name);
                        (app_with_cat, display_ms, s.idle_duration_ms)
                    })
                    .filter(|(_, display, _)| *display >= 5000)
                    .collect();

                // Sort by displayed time (most used first)
                filtered.sort_by(|a, b| b.1.cmp(&a.1));

                populate_list(&self.app_list, filtered);
                return;
            }
        }

        // Fallback to legacy HashMap
        if let Ok(usage) = self.app_usage.lock() {
            let mut filtered: Vec<(String, i64, i64)> = usage
                .iter()
                .filter(|(app, _)| {
                    let app_lower = app.to_lowercase();
                    !app_lower.contains("explorer.exe")
                        && !app_lower.contains("mytime")
                        && !app_lower.contains("searchhost")
                        && !app_lower.contains("shellexperiencehost")
                        && !app_lower.contains("applicationframehost")
                })
                .filter(|(_, stats)| {
                    stats.active_duration.as_secs() >= 5 || (show_all_time && stats.total_duration().as_secs() >= 5)
                })
                .map(|(app, stats)| {
                    let friendly_name = Self::to_friendly_name(app);
                    let display_duration = if show_all_time {
                        stats.total_duration()
                    } else {
                        stats.active_duration
                    };
                    (friendly_name, display_duration, stats.idle_duration)
                })
                .filter(|(_, display, _)| display.as_secs() >= 5)
                .map(|(name, display, idle)| (name, display.as_millis() as i64, idle.as_millis() as i64))
                .collect();

            // Sort by displayed time
            filtered.sort_by(|a, b| b.1.cmp(&a.1));

            populate_list(&self.app_list, filtered);
        }
    }

    fn to_friendly_name(app_name: &str) -> String {
        // Remove .exe extension
        let name = app_name.trim_end_matches(".exe").trim_end_matches(".EXE");

        // Map known apps to friendly names
        match name.to_lowercase().as_str() {
            "code" => "Visual Studio Code".to_string(),
            "msedge" => "Microsoft Edge".to_string(),
            "chrome" => "Google Chrome".to_string(),
            "firefox" => "Mozilla Firefox".to_string(),
            "notepad" => "Notepad".to_string(),
            "notepad++" => "Notepad++".to_string(),
            "windowsterminal" => "Windows Terminal".to_string(),
            "cmd" => "Command Prompt".to_string(),
            "powershell" => "PowerShell".to_string(),
            "slack" => "Slack".to_string(),
            "teams" => "Microsoft Teams".to_string(),
            "discord" => "Discord".to_string(),
            "spotify" => "Spotify".to_string(),
            "winword" => "Microsoft Word".to_string(),
            "excel" => "Microsoft Excel".to_string(),
            "powerpnt" => "Microsoft PowerPoint".to_string(),
            "outlook" => "Microsoft Outlook".to_string(),
            "devenv" => "Visual Studio".to_string(),
            "idea64" => "IntelliJ IDEA".to_string(),
            "webstorm64" => "WebStorm".to_string(),
            "pycharm64" => "PyCharm".to_string(),
            "cursor" => "Cursor".to_string(),
            _ => {
                // Capitalize first letter of each word
                name.split(|c: char| c == '-' || c == '_' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().chain(chars).collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_secs = duration.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let secs = total_secs % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, secs)
        } else {
            format!("{:02}:{:02}", minutes, secs)
        }
    }

    fn update_tray_icon(&self, tracking: bool) {
        // Update tray tip immediately
        let status = if tracking { "Tracking" } else { "Stopped" };
        self.tray.set_tip(&format!("MyTime - {}", status));
    }

    fn on_minimize(&self) {
        self.window.set_visible(false);
    }

    fn on_close(&self) {
        // Minimize to tray instead of closing
        self.window.set_visible(false);
    }

    fn on_show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
    }

    fn on_tray_click(&self) {
        self.on_show();
    }

    fn on_tray_right_click(&self) {
        let (x, y) = nwg::GlobalCursor::position();
        self.tray_menu.popup(x, y);
    }

    fn on_exit(&self) {
        // Stop tracking if active
        if *self.is_tracking.borrow() {
            self.on_stop();
        }
        nwg::stop_thread_dispatch();
    }

    fn on_export_csv(&self) {
        if let Some(ref storage) = self.sqlite_storage {
            // Generate filename with today's date
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let default_filename = format!("mytime_export_{}.csv", today);

            // Get desktop path for default location
            let desktop = std::env::var("USERPROFILE")
                .map(|p| std::path::PathBuf::from(p).join("Desktop"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let default_path = desktop.join(&default_filename);

            // Use file save dialog
            let mut dialog = nwg::FileDialog::default();
            if nwg::FileDialog::builder()
                .title("Export Today's Data")
                .action(nwg::FileDialogAction::Save)
                .filters("CSV Files (*.csv)|*.csv")
                .build(&mut dialog)
                .is_ok()
            {
                if dialog.run(Some(&self.window)) {
                    if let Ok(path_str) = dialog.get_selected_item() {
                        let mut output_path = std::path::PathBuf::from(&path_str);
                        if output_path.extension().is_none() {
                            output_path.set_extension("csv");
                        }

                        match legacy_storage::export_to_csv(storage, &output_path) {
                            Ok(count) => {
                                nwg::modal_info_message(
                                    &self.window,
                                    "Export Complete",
                                    &format!("Exported {} entries to:\n{}", count, output_path.display()),
                                );
                            }
                            Err(e) => {
                                nwg::modal_info_message(
                                    &self.window,
                                    "Export Failed",
                                    &format!("Failed to export: {}", e),
                                );
                            }
                        }
                    }
                }
            } else {
                // Fallback: export to default path
                match legacy_storage::export_to_csv(storage, &default_path) {
                    Ok(count) => {
                        nwg::modal_info_message(
                            &self.window,
                            "Export Complete",
                            &format!("Exported {} entries to:\n{}", count, default_path.display()),
                        );
                    }
                    Err(e) => {
                        nwg::modal_info_message(
                            &self.window,
                            "Export Failed",
                            &format!("Failed to export: {}", e),
                        );
                    }
                }
            }
        } else {
            nwg::modal_info_message(
                &self.window,
                "Export Not Available",
                "SQLite storage is not initialized. Cannot export data.",
            );
        }
    }

    fn on_toggle_hide_idle(&self) {
        let checked = self.hide_idle_checkbox.check_state() == nwg::CheckBoxState::Checked;
        *self.hide_idle_sessions.borrow_mut() = checked;
        self.app_list.clear();
        self.update_app_list();
    }

    fn on_prev_day(&self) {
        let mut offset = self.selected_day_offset.borrow_mut();
        *offset -= 1;
        drop(offset);
        self.update_date_label();
        self.refresh_data_display();
    }

    fn on_next_day(&self) {
        let mut offset = self.selected_day_offset.borrow_mut();
        if *offset < 0 {
            *offset += 1;
        }
        drop(offset);
        self.update_date_label();
        self.refresh_data_display();
    }

    fn update_date_label(&self) {
        let offset = *self.selected_day_offset.borrow();
        let label = utils::format_day_label(offset);
        self.date_label.set_text(&label);

        // Disable next button if we're at today
        self.next_day_btn.set_enabled(offset < 0);
    }

    fn refresh_data_display(&self) {
        self.app_list.clear();
        self.update_summary();
        self.update_category_breakdown();
        self.update_app_list();
    }

    /// Get the time range for the currently selected day
    fn get_selected_day_range(&self) -> (i64, i64) {
        let offset = *self.selected_day_offset.borrow();
        let day_start_hour = self.sqlite_storage
            .as_ref()
            .and_then(|s| s.get_day_start_hour().ok())
            .unwrap_or(utils::DEFAULT_DAY_START_HOUR);
        utils::day_range_ms_with_offset(day_start_hour, offset)
    }

    fn on_app_list_right_click(&self) {
        // Get selected item
        if let Some(idx) = self.app_list.selected_item() {
            // Get the app name from the list (includes emoji prefix)
            if let Some(item) = self.app_list.item(idx, 0, 256) {
                // Extract app name without emoji (format is "🎬 AppName")
                let friendly_name = item.text.trim_start_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_').trim();

                // Find the actual app_name by querying summaries
                if let Some(ref storage) = self.sqlite_storage {
                    let (start_ms, end_ms) = self.get_selected_day_range();
                    let offset = *self.selected_day_offset.borrow();
                    let end_ms = if offset == 0 { utils::now_ms() } else { end_ms };

                    if let Ok(summaries) = storage.get_app_breakdown(start_ms, end_ms) {
                        // Find matching app by friendly name
                        if let Some(summary) = summaries.iter().find(|s| s.friendly_name == friendly_name) {
                            *self.selected_app_for_category.borrow_mut() = Some(summary.app_name.clone());

                            // Show context menu at cursor position
                            let (x, y) = nwg::GlobalCursor::position();
                            self.category_menu.popup(x, y);
                            return;
                        }
                    }
                }
            }
        }
    }

    fn set_app_category(&self, category: &str) {
        use crate::storage::StorageAdapter;

        let app_name = self.selected_app_for_category.borrow().clone();
        if let (Some(app_name), Some(ref storage)) = (app_name, &self.sqlite_storage) {
            let (start_ms, end_ms) = self.get_selected_day_range();
            let offset = *self.selected_day_offset.borrow();
            let end_ms = if offset == 0 { utils::now_ms() } else { end_ms };

            // Get all segments for this app to find unique title_hashes
            if let Ok(segments) = storage.get_segments_range(start_ms, end_ms) {
                let mut updated_hashes = std::collections::HashSet::new();

                for segment in segments.iter().filter(|s| s.app_name == app_name) {
                    if updated_hashes.contains(&segment.title_hash) {
                        continue;
                    }

                    // Create/update user label for this title_hash
                    let label = models::Label {
                        title_hash: segment.title_hash.clone(),
                        category: category.to_string(),
                        source: models::LabelSource::User,
                        confidence: None,
                        updated_at: utils::now_ms(),
                    };

                    if storage.upsert_label(&label).is_ok() {
                        updated_hashes.insert(segment.title_hash.clone());
                    }
                }

                if !updated_hashes.is_empty() {
                    // Refresh the display
                    self.app_list.clear();
                    self.refresh_data_display();
                }
            }
        }
    }

    fn on_set_category_entertainment(&self) {
        self.set_app_category("entertainment");
    }

    fn on_set_category_development(&self) {
        self.set_app_category("development");
    }

    fn on_set_category_productivity(&self) {
        self.set_app_category("productivity");
    }

    fn on_set_category_communication(&self) {
        self.set_app_category("communication");
    }

    fn on_toggle_autostart(&self) {
        use windows::Win32::System::Registry::*;

        let is_enabled = self.tray_autostart.checked();

        unsafe {
            let key_path = windows::core::w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut key: HKEY = HKEY::default();

            if RegOpenKeyExW(HKEY_CURRENT_USER, key_path, 0, KEY_WRITE, &mut key).is_err() {
                nwg::modal_info_message(&self.window, "Error", "Failed to open registry key");
                // Revert checkbox state
                self.tray_autostart.set_checked(!is_enabled);
                return;
            }

            let value_name = windows::core::w!("MyTime");

            if is_enabled {
                // Was just checked, so enable auto-start
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(path_str) = exe_path.to_str() {
                        let path_wide: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
                        let byte_ptr = path_wide.as_ptr() as *const u8;
                        let byte_len = path_wide.len() * 2;

                        let _ = RegSetValueExW(
                            key,
                            value_name,
                            0,
                            REG_SZ,
                            Some(std::slice::from_raw_parts(byte_ptr, byte_len)),
                        );
                    }
                }
            } else {
                // Was just unchecked, so disable auto-start
                let _ = RegDeleteValueW(key, value_name);
            }

            let _ = RegCloseKey(key);
        }
    }

    fn is_autostart_enabled() -> bool {
        use windows::Win32::System::Registry::*;

        unsafe {
            let key_path = windows::core::w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut key: HKEY = HKEY::default();

            if RegOpenKeyExW(HKEY_CURRENT_USER, key_path, 0, KEY_READ, &mut key).is_err() {
                return false;
            }

            let value_name = windows::core::w!("MyTime");
            let result = RegQueryValueExW(key, value_name, None, None, None, None);
            let _ = RegCloseKey(key);

            result.is_ok()
        }
    }

    fn init_autostart_menu(&self) {
        self.tray_autostart.set_checked(Self::is_autostart_enabled());
    }

    fn on_configure_day_start(&self) {
        if let Some(ref storage) = self.sqlite_storage {
            let current_hour = storage.get_day_start_hour().unwrap_or(utils::DEFAULT_DAY_START_HOUR);

            // Use Rc<Cell<>> for shared state between closure and main code
            let result = std::rc::Rc::new(std::cell::Cell::new(None::<u32>));
            let result_clone = result.clone();

            // Build dialog components
            let mut input_dialog = nwg::TextInput::default();
            let mut dialog_window = nwg::Window::default();
            let mut ok_button = nwg::Button::default();
            let mut cancel_button = nwg::Button::default();
            let mut prompt_label = nwg::Label::default();

            let _ = nwg::Window::builder()
                .size((300, 180))
                .position((400, 300))
                .title("Day Start Hour")
                .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
                .build(&mut dialog_window);

            let _ = nwg::Label::builder()
                .text(&format!("Enter hour (0-23).\nCurrent: {:02}:00\n\n0=midnight, 6=6AM, 12=noon", current_hour))
                .parent(&dialog_window)
                .position((20, 15))
                .size((260, 80))
                .build(&mut prompt_label);

            let _ = nwg::TextInput::builder()
                .text(&current_hour.to_string())
                .parent(&dialog_window)
                .position((20, 100))
                .size((260, 25))
                .build(&mut input_dialog);

            let _ = nwg::Button::builder()
                .text("OK")
                .parent(&dialog_window)
                .position((60, 135))
                .size((80, 30))
                .build(&mut ok_button);

            let _ = nwg::Button::builder()
                .text("Cancel")
                .parent(&dialog_window)
                .position((160, 135))
                .size((80, 30))
                .build(&mut cancel_button);

            // Simple event loop for the dialog
            dialog_window.set_focus();
            input_dialog.set_focus();

            // Bind events
            let handler = nwg::full_bind_event_handler(&dialog_window.handle, move |evt, _evt_data, handle| {
                match evt {
                    nwg::Event::OnButtonClick => {
                        if handle == ok_button.handle {
                            if let Ok(hour) = input_dialog.text().parse::<u32>() {
                                if hour <= 23 {
                                    result_clone.set(Some(hour));
                                }
                            }
                            nwg::stop_thread_dispatch();
                        } else if handle == cancel_button.handle {
                            nwg::stop_thread_dispatch();
                        }
                    }
                    nwg::Event::OnWindowClose => {
                        nwg::stop_thread_dispatch();
                    }
                    _ => {}
                }
            });

            nwg::dispatch_thread_events();

            nwg::unbind_event_handler(&handler);

            // Save if we got a result
            if let Some(new_hour) = result.get() {
                if storage.set_day_start_hour(new_hour).is_ok() {
                    let suffix = if new_hour == 0 {
                        "midnight".to_string()
                    } else if new_hour == 12 {
                        "noon".to_string()
                    } else if new_hour < 12 {
                        format!("{} AM", new_hour)
                    } else {
                        format!("{} PM", new_hour - 12)
                    };

                    nwg::modal_info_message(
                        &self.window,
                        "Day Start Updated",
                        &format!("Day now starts at {:02}:00 ({}).\n\nThe UI will refresh to reflect this change.", new_hour, suffix),
                    );

                    // Force refresh
                    self.app_list.clear();
                    self.update_summary();
                    self.update_category_breakdown();
                    self.update_app_list();
                }
            }
        } else {
            nwg::modal_info_message(
                &self.window,
                "Not Available",
                "Day start configuration requires SQLite storage.",
            );
        }
    }
}

mod legacy_tracker {
    use super::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::Threading::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    const IDLE_THRESHOLD_MS: u32 = 30000;

    static KEYSTROKE_COUNTER: AtomicU64 = AtomicU64::new(0);
    static CLICK_COUNTER: AtomicU64 = AtomicU64::new(0);

    pub fn track_foreground_window(
        entries: Arc<Mutex<Vec<TimeEntry>>>,
        app_usage: Arc<Mutex<HashMap<String, AppStats>>>,
        should_stop: Arc<AtomicBool>,
    ) {
        let mut last_window_info: Option<(String, String)> = None;
        let mut window_start_time = Instant::now();
        let mut last_activity_check = Instant::now();
        let mut idle_time_accumulated = Duration::ZERO;

        KEYSTROKE_COUNTER.store(0, Ordering::SeqCst);
        CLICK_COUNTER.store(0, Ordering::SeqCst);

        static ACTIVITY_MONITOR_STARTED: std::sync::Once = std::sync::Once::new();
        ACTIVITY_MONITOR_STARTED.call_once(|| {
            std::thread::spawn(monitor_activity);
        });

        loop {
            if should_stop.load(Ordering::SeqCst) {
                if let Some((last_app, last_title)) = last_window_info {
                    let duration = Instant::now() - window_start_time;
                    let start_time = Local::now() - chrono::Duration::seconds(duration.as_secs() as i64);
                    let end_time = Local::now();

                    let entry = TimeEntry {
                        app_name: last_app,
                        window_title: last_title,
                        start_time,
                        end_time,
                        duration_seconds: duration.as_secs(),
                        idle_seconds: idle_time_accumulated.as_secs(),
                        keystrokes: KEYSTROKE_COUNTER.swap(0, Ordering::SeqCst),
                        mouse_clicks: CLICK_COUNTER.swap(0, Ordering::SeqCst),
                    };

                    if let Ok(mut entries_lock) = entries.lock() {
                        entries_lock.push(entry);
                    }
                }
                break;
            }

            let now = Instant::now();

            if now - last_activity_check >= Duration::from_secs(1) {
                if let Some(idle_ms) = get_idle_time() {
                    if idle_ms > IDLE_THRESHOLD_MS {
                        idle_time_accumulated += Duration::from_secs(1);
                    }
                }
                last_activity_check = now;
            }

            if let Some((app_name, window_title)) = get_foreground_window_info() {
                let current_info = (app_name.clone(), window_title.clone());

                if last_window_info.as_ref() != Some(&current_info) {
                    if let Some((last_app, last_title)) = last_window_info {
                        let duration = now - window_start_time;
                        let start_time = Local::now() - chrono::Duration::seconds(duration.as_secs() as i64);
                        let end_time = Local::now();

                        let keystrokes = KEYSTROKE_COUNTER.swap(0, Ordering::SeqCst);
                        let mouse_clicks = CLICK_COUNTER.swap(0, Ordering::SeqCst);
                        let idle_secs = idle_time_accumulated.as_secs();

                        let entry = TimeEntry {
                            app_name: last_app.clone(),
                            window_title: last_title,
                            start_time,
                            end_time,
                            duration_seconds: duration.as_secs(),
                            idle_seconds: idle_secs,
                            keystrokes,
                            mouse_clicks,
                        };

                        if let Ok(mut entries_lock) = entries.lock() {
                            entries_lock.push(entry);
                        }

                        if let Ok(mut usage) = app_usage.lock() {
                            let stats = usage.entry(last_app).or_default();
                            stats.add_session(duration, idle_secs, keystrokes, mouse_clicks);
                        }
                    }

                    last_window_info = Some(current_info);
                    window_start_time = now;
                    idle_time_accumulated = Duration::ZERO;
                }
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn get_idle_time() -> Option<u32> {
        unsafe {
            let mut last_input = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };

            if GetLastInputInfo(&mut last_input).as_bool() {
                let current_tick = GetTickCount64() as u32;
                Some(current_tick - last_input.dwTime)
            } else {
                None
            }
        }
    }

    fn monitor_activity() {
        unsafe {
            let keyboard_hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                HINSTANCE::default(),
                0,
            )
            .ok();

            let mouse_hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_proc),
                HINSTANCE::default(),
                0,
            )
            .ok();

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if let Some(hook) = keyboard_hook {
                let _ = UnhookWindowsHookEx(hook);
            }
            if let Some(hook) = mouse_hook {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
    }

    fn get_foreground_window_info() -> Option<(String, String)> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return None;
            }

            let mut title_buf = vec![0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let window_title = OsString::from_wide(&title_buf[..title_len as usize])
                .to_string_lossy()
                .to_string();

            let mut process_id = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id).ok()?;

            let mut exe_buf = vec![0u16; 512];
            let result = GetModuleFileNameExW(process, HMODULE::default(), &mut exe_buf);

            let actual_len = exe_buf.iter().position(|&c| c == 0).unwrap_or(result as usize);
            let exe_path = OsString::from_wide(&exe_buf[..actual_len])
                .to_string_lossy()
                .to_string();

            let app_name = exe_path.split('\\').next_back().unwrap_or("Unknown").to_string();

            CloseHandle(process).ok();

            Some((app_name, window_title))
        }
    }

    unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && wparam.0 == WM_KEYDOWN as usize {
            KEYSTROKE_COUNTER.fetch_add(1, Ordering::SeqCst);
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && (wparam.0 == WM_LBUTTONDOWN as usize || wparam.0 == WM_RBUTTONDOWN as usize) {
            CLICK_COUNTER.fetch_add(1, Ordering::SeqCst);
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }
}

mod legacy_storage {
    use super::*;
    use std::fs::{metadata, File, OpenOptions};
    use std::io::{BufReader, Write};

    fn get_data_path() -> std::path::PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("mytime_data.csv");
            }
        }
        std::path::PathBuf::from("mytime_data.csv")
    }

    pub fn save_to_csv(entries: &[TimeEntry]) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = get_data_path();
        let file_exists = metadata(&file_path).is_ok();

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .append(true)
            .open(&file_path)?;

        if !file_exists {
            writeln!(
                &file,
                "app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks"
            )?;
        }

        let mut wtr = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);

        for entry in entries {
            wtr.serialize(entry)?;
        }

        wtr.flush()?;
        Ok(())
    }

    /// Load today's entries from CSV and aggregate into AppStats
    pub fn load_today_stats() -> HashMap<String, AppStats> {
        let mut stats: HashMap<String, AppStats> = HashMap::new();
        let file_path = get_data_path();

        let file = match File::open(&file_path) {
            Ok(f) => f,
            Err(_) => return stats, // No file yet, return empty
        };

        let reader = BufReader::new(file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        let today = Local::now().date_naive();

        for result in csv_reader.deserialize() {
            let entry: TimeEntry = match result {
                Ok(e) => e,
                Err(_) => continue, // Skip malformed rows
            };

            // Only include today's entries
            if entry.start_time.date_naive() != today {
                continue;
            }

            let app_stats = stats.entry(entry.app_name.clone()).or_default();
            app_stats.add_session(
                Duration::from_secs(entry.duration_seconds),
                entry.idle_seconds,
                entry.keystrokes,
                entry.mouse_clicks,
            );
        }

        stats
    }

    /// Check if legacy CSV file exists
    pub fn csv_exists() -> bool {
        get_data_path().exists()
    }

    /// Get the path to the legacy CSV file
    pub fn get_csv_path() -> std::path::PathBuf {
        get_data_path()
    }

    /// Import all CSV entries into SQLite storage
    /// Returns (imported_count, skipped_count)
    pub fn import_to_sqlite(storage: &crate::storage::SqliteStorage) -> Result<(usize, usize), Box<dyn std::error::Error>> {
        use crate::storage::StorageAdapter;
        use crate::categorizer::create_heuristic_label;
        use crate::utils::compute_title_hash;

        let file_path = get_data_path();
        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        let mut imported = 0;
        let mut skipped = 0;

        for result in csv_reader.deserialize() {
            let entry: TimeEntry = match result {
                Ok(e) => e,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };

            // Convert TimeEntry to Segment
            let title_hash = compute_title_hash(&entry.app_name, &entry.window_title);
            let focus_session_id = uuid::Uuid::new_v4().to_string();
            let segment_id = uuid::Uuid::new_v4().to_string();

            let segment = crate::models::Segment {
                segment_id,
                app_name: entry.app_name.clone(),
                window_title: Some(entry.window_title.clone()),
                title_hash: title_hash.clone(),
                start_time: entry.start_time.timestamp_millis(),
                end_time: entry.end_time.timestamp_millis(),
                idle_seconds: entry.idle_seconds,
                keystrokes: entry.keystrokes,
                mouse_clicks: entry.mouse_clicks,
                focus_session_id,
                device_id: storage.get_device_id().ok(),
                schema_version: 1,
                created_at: crate::utils::now_ms(),
            };

            // Insert segment
            if storage.insert_segment(&segment).is_ok() {
                imported += 1;

                // Create heuristic label if not exists
                if storage.get_label(&title_hash).ok().flatten().is_none() {
                    let label = create_heuristic_label(&title_hash, &entry.app_name, &entry.window_title);
                    let _ = storage.upsert_label(&label);
                }
            } else {
                skipped += 1;
            }
        }

        Ok((imported, skipped))
    }

    /// Backup the CSV file by renaming it
    pub fn backup_csv() -> Result<std::path::PathBuf, std::io::Error> {
        let csv_path = get_data_path();
        let backup_path = csv_path.with_extension("csv.backup");
        std::fs::rename(&csv_path, &backup_path)?;
        Ok(backup_path)
    }

    /// Export segments from SQLite to CSV file
    pub fn export_to_csv(
        storage: &crate::storage::SqliteStorage,
        output_path: &std::path::Path,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        use crate::storage::StorageAdapter;

        // Get all segments from today (using configured day start hour)
        let day_start_hour = storage.get_day_start_hour().unwrap_or(crate::utils::DEFAULT_DAY_START_HOUR);
        let today_start = crate::utils::today_start_ms_with_hour(day_start_hour);
        let now = crate::utils::now_ms();
        let segments = storage.get_segments_range(today_start, now)
            .map_err(|e| -> Box<dyn std::error::Error> { e })?;

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(output_path)?;

        // Write header
        writeln!(
            &file,
            "app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks"
        )?;

        let mut wtr = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);

        let mut count = 0;
        for seg in &segments {
            let start_time = chrono::DateTime::from_timestamp_millis(seg.start_time)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now);
            let end_time = chrono::DateTime::from_timestamp_millis(seg.end_time)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(Local::now);

            let entry = TimeEntry {
                app_name: seg.app_name.clone(),
                window_title: seg.window_title.clone().unwrap_or_default(),
                start_time,
                end_time,
                duration_seconds: seg.duration_seconds(),
                idle_seconds: seg.idle_seconds,
                keystrokes: seg.keystrokes,
                mouse_clicks: seg.mouse_clicks,
            };

            wtr.serialize(&entry)?;
            count += 1;
        }

        wtr.flush()?;
        Ok(count)
    }
}

fn main() {
    // Single instance check
    unsafe {
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};

        let mutex_name = windows::core::w!("MyTimeAppMutex");
        let mutex = CreateMutexW(None, true, mutex_name);

        if mutex.is_err() || GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
    }

    // Enable modern visual styles
    nwg::enable_visual_styles();

    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");

    // Initialize SQLite storage
    let sqlite_storage = match SqliteStorage::new() {
        Ok(storage) => Some(Arc::new(storage)),
        Err(_) => None,
    };

    // Check for legacy CSV and offer import
    if let Some(ref storage) = sqlite_storage {
        if legacy_storage::csv_exists() {
            // Check if we've already imported (by checking if DB has any segments)
            let has_segments = storage.get_today_active_ms().unwrap_or(0) > 0
                || storage.get_segments_range(0, i64::MAX).map(|s| !s.is_empty()).unwrap_or(false);

            if !has_segments {
                // Show import dialog - need a temporary window for the dialog
                let csv_path = legacy_storage::get_csv_path();
                let msg = format!(
                    "Found existing data at:\n{}\n\nWould you like to import it into the new database?",
                    csv_path.display()
                );

                // Use simple message box via Windows API
                let result = unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::*;
                    use windows::core::w;
                    let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                    MessageBoxW(
                        None,
                        windows::core::PCWSTR(msg_wide.as_ptr()),
                        w!("Import Existing Data"),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                };

                if result == windows::Win32::UI::WindowsAndMessaging::IDYES {
                    match legacy_storage::import_to_sqlite(storage) {
                        Ok((imported, skipped)) => {
                            let backup_msg = match legacy_storage::backup_csv() {
                                Ok(path) => format!("\n\nOriginal CSV backed up to:\n{}", path.display()),
                                Err(_) => String::new(),
                            };
                            let success_msg = format!(
                                "Successfully imported {} entries ({} skipped).{}",
                                imported, skipped, backup_msg
                            );
                            unsafe {
                                use windows::Win32::UI::WindowsAndMessaging::*;
                                use windows::core::w;
                                let msg_wide: Vec<u16> = success_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(
                                    None,
                                    windows::core::PCWSTR(msg_wide.as_ptr()),
                                    w!("Import Complete"),
                                    MB_OK | MB_ICONINFORMATION,
                                );
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to import data: {}", e);
                            unsafe {
                                use windows::Win32::UI::WindowsAndMessaging::*;
                                use windows::core::w;
                                let msg_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(
                                    None,
                                    windows::core::PCWSTR(msg_wide.as_ptr()),
                                    w!("Import Failed"),
                                    MB_OK | MB_ICONERROR,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Load today's data - prefer SQLite if available, otherwise legacy CSV
    let (today_stats, today_total) = if let Some(ref storage) = sqlite_storage {
        match storage.get_today_active_ms() {
            Ok(ms) => {
                let legacy_stats = legacy_storage::load_today_stats();
                (legacy_stats, Duration::from_millis(ms as u64))
            }
            Err(_) => {
                let legacy_stats = legacy_storage::load_today_stats();
                let total: Duration = legacy_stats.values().map(|s| s.active_duration).sum();
                (legacy_stats, total)
            }
        }
    } else {
        let legacy_stats = legacy_storage::load_today_stats();
        let total: Duration = legacy_stats.values().map(|s| s.active_duration).sum();
        (legacy_stats, total)
    };

    let app = MyTimeApp {
        is_tracking: RefCell::new(false),
        session_start: RefCell::new(None),
        total_time: RefCell::new(today_total),
        time_entries: Arc::new(Mutex::new(Vec::new())),
        app_usage: Arc::new(Mutex::new(today_stats)),
        should_stop_tracking: Arc::new(AtomicBool::new(false)),
        tracking_thread: RefCell::new(None),
        hide_idle_sessions: RefCell::new(true), // Default: hide idle sessions
        selected_day_offset: RefCell::new(0),   // Start at today
        selected_app_for_category: RefCell::new(None),
        sqlite_storage,
        ..Default::default()
    };

    let ui = MyTimeApp::build_ui(app).expect("Failed to build UI");

    // Initialize UI state
    ui.stop_btn.set_enabled(false);
    ui.tray_stop.set_enabled(false);
    ui.app_list.set_headers_enabled(true); // Show column headers
    ui.init_autostart_menu();
    ui.update_date_label(); // Initialize date navigation
    ui.next_day_btn.set_enabled(false); // Can't go to future

    // Update display with loaded data
    ui.update_app_list();
    ui.on_timer(); // Update time display

    // Start the timer
    ui.timer.start();

    nwg::dispatch_thread_events();
}
