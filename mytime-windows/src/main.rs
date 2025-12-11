#![windows_subsystem = "windows"]

extern crate native_windows_gui as nwg;
extern crate native_windows_derive as nwd;

use nwd::NwgUi;
use nwg::NativeUi;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::cell::RefCell;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

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

#[derive(Default, NwgUi)]
pub struct MyTimeApp {
    // Fonts
    #[nwg_resource(family: "Segoe UI", size: 18, weight: 600)]
    font_title: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 28, weight: 700)]
    font_time: nwg::Font,

    #[nwg_resource(family: "Segoe UI", size: 15)]
    font_normal: nwg::Font,

    // Main window
    #[nwg_control(size: (450, 400), position: (300, 200), title: "MyTime", flags: "WINDOW|VISIBLE|MINIMIZE_BOX")]
    #[nwg_events(OnWindowClose: [MyTimeApp::on_close], OnWindowMinimize: [MyTimeApp::on_minimize])]
    window: nwg::Window,

    // Layout
    #[nwg_layout(parent: window, spacing: 8, margin: [20, 20, 20, 20])]
    layout: nwg::GridLayout,

    // Title/Status label
    #[nwg_control(text: "⏱ Stopped", font: Some(&data.font_title))]
    #[nwg_layout_item(layout: layout, row: 0, col: 0, col_span: 2)]
    status_label: nwg::Label,

    // Time display - large and prominent
    #[nwg_control(text: "00:00:00", font: Some(&data.font_time))]
    #[nwg_layout_item(layout: layout, row: 1, col: 0, col_span: 2)]
    time_label: nwg::Label,

    // Start button
    #[nwg_control(text: "▶ Start", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 2, col: 0)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_start])]
    start_btn: nwg::Button,

    // Stop button
    #[nwg_control(text: "⏹ Stop", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 2, col: 1)]
    #[nwg_events(OnButtonClick: [MyTimeApp::on_stop])]
    stop_btn: nwg::Button,

    // Section label
    #[nwg_control(text: "Application Usage", font: Some(&data.font_normal))]
    #[nwg_layout_item(layout: layout, row: 3, col: 0, col_span: 2)]
    section_label: nwg::Label,

    // App usage list
    #[nwg_control(list_style: nwg::ListViewStyle::Detailed, ex_flags: nwg::ListViewExFlags::GRID | nwg::ListViewExFlags::FULL_ROW_SELECT)]
    #[nwg_layout_item(layout: layout, row: 4, col: 0, col_span: 2, row_span: 4)]
    app_list: nwg::ListView,

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

    #[nwg_control(parent: tray_menu, text: "Start with Windows")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_toggle_autostart])]
    tray_autostart: nwg::MenuItem,

    #[nwg_control(parent: tray_menu)]
    tray_sep2: nwg::MenuSeparator,

    #[nwg_control(parent: tray_menu, text: "Exit")]
    #[nwg_events(OnMenuItemSelected: [MyTimeApp::on_exit])]
    tray_exit: nwg::MenuItem,

    // State - stored as regular fields, not NWG controls
    is_tracking: RefCell<bool>,
    session_start: RefCell<Option<Instant>>,
    total_time: RefCell<Duration>,
    time_entries: Arc<Mutex<Vec<TimeEntry>>>,
    app_usage: Arc<Mutex<HashMap<String, Duration>>>,
    should_stop_tracking: Arc<AtomicBool>,
    tracking_thread: RefCell<Option<std::thread::JoinHandle<()>>>,
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

        // Start tracking thread
        let entries = Arc::clone(&self.time_entries);
        let app_usage = Arc::clone(&self.app_usage);
        let stop_flag = Arc::clone(&self.should_stop_tracking);

        let handle = std::thread::spawn(move || {
            tracker::track_foreground_window(entries, app_usage, stop_flag);
        });

        *self.tracking_thread.borrow_mut() = Some(handle);
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

        // Save entries
        if let Ok(mut entries) = self.time_entries.lock() {
            if !entries.is_empty() {
                storage::save_to_csv(&entries).ok();
                entries.clear();
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

        // Update app list
        self.update_app_list();
    }

    fn update_app_list(&self) {
        if let Ok(usage) = self.app_usage.lock() {
            // Filter and transform the data
            let mut filtered: Vec<(String, Duration)> = usage
                .iter()
                .filter(|(app, duration)| {
                    // Filter out noise: short entries and system processes
                    let app_lower = app.to_lowercase();
                    duration.as_secs() >= 5
                        && !app_lower.contains("explorer.exe")
                        && !app_lower.contains("mytime")
                        && !app_lower.contains("searchhost")
                        && !app_lower.contains("shellexperiencehost")
                        && !app_lower.contains("applicationframehost")
                })
                .map(|(app, duration)| {
                    // Convert to friendly name (remove .exe, capitalize)
                    let friendly_name = Self::to_friendly_name(app);
                    (friendly_name, *duration)
                })
                .collect();

            // Sort by duration (most used first)
            filtered.sort_by(|a, b| b.1.cmp(&a.1));

            // Only update if there's new data
            let current_count = self.app_list.len();
            if current_count != filtered.len() || current_count == 0 {
                self.app_list.clear();

                // Ensure columns exist
                if self.app_list.column_len() == 0 {
                    self.app_list.insert_column("Application");
                    self.app_list.insert_column("Time");
                    self.app_list.set_column_width(0, 250);
                    self.app_list.set_column_width(1, 120);
                }

                for (app, duration) in filtered.iter() {
                    let time_str = Self::format_duration(*duration);
                    self.app_list.insert_item(nwg::InsertListViewItem {
                        index: Some(self.app_list.len() as i32),
                        column_index: 0,
                        text: Some(app.clone()),
                        image: None,
                    });
                    self.app_list.insert_item(nwg::InsertListViewItem {
                        index: Some(self.app_list.len() as i32 - 1),
                        column_index: 1,
                        text: Some(time_str),
                        image: None,
                    });
                }
            }
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
}

mod tracker {
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
        app_usage: Arc<Mutex<HashMap<String, Duration>>>,
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

                        let entry = TimeEntry {
                            app_name: last_app.clone(),
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

                        if let Ok(mut usage) = app_usage.lock() {
                            *usage.entry(last_app).or_insert(Duration::ZERO) += duration;
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

            let app_name = exe_path.split('\\').last().unwrap_or("Unknown").to_string();

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

mod storage {
    use super::*;
    use std::fs::{metadata, OpenOptions};
    use std::io::Write;

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

    let app = MyTimeApp {
        is_tracking: RefCell::new(false),
        session_start: RefCell::new(None),
        total_time: RefCell::new(Duration::ZERO),
        time_entries: Arc::new(Mutex::new(Vec::new())),
        app_usage: Arc::new(Mutex::new(HashMap::new())),
        should_stop_tracking: Arc::new(AtomicBool::new(false)),
        tracking_thread: RefCell::new(None),
        ..Default::default()
    };

    let ui = MyTimeApp::build_ui(app).expect("Failed to build UI");

    // Initialize UI state
    ui.stop_btn.set_enabled(false);
    ui.tray_stop.set_enabled(false);
    ui.init_autostart_menu();

    // Start the timer
    ui.timer.start();

    nwg::dispatch_thread_events();
}
