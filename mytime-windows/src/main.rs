#![windows_subsystem = "windows"] // Hide console window on Windows

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

mod tray;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimeEntry {
    app_name: String,
    window_title: String,
    start_time: DateTime<Local>,
    duration_seconds: u64,
}

struct MyTimeApp {
    is_tracking: bool,
    current_session_start: Option<Instant>,
    total_tracked_time: Duration,
    time_entries: Arc<Mutex<Vec<TimeEntry>>>,
    app_usage: Arc<Mutex<HashMap<String, Duration>>>,
    tracking_thread: Option<std::thread::JoinHandle<()>>,
    should_quit: Arc<AtomicBool>,
    window_visible: bool,
    tray_command_rx: Option<mpsc::Receiver<tray::TrayCommand>>,
    tray_manager: Option<tray::TrayManager>,
}

impl Default for MyTimeApp {
    fn default() -> Self {
        Self {
            is_tracking: false,
            current_session_start: None,
            total_tracked_time: Duration::default(),
            time_entries: Arc::new(Mutex::new(Vec::new())),
            app_usage: Arc::new(Mutex::new(HashMap::new())),
            tracking_thread: None,
            should_quit: Arc::new(AtomicBool::new(false)),
            window_visible: true,
            tray_command_rx: None,
            tray_manager: None,
        }
    }
}

impl MyTimeApp {
    fn initialize_tray(&mut self, ctx: &egui::Context) {
        if let Ok((tray_manager, tray_rx)) = tray::create_tray_icon(ctx.clone()) {
            self.tray_manager = Some(tray_manager);
            self.tray_command_rx = Some(tray_rx);
        }
    }

    fn start_tracking(&mut self) {
        if !self.is_tracking {
            self.is_tracking = true;
            self.current_session_start = Some(Instant::now());
            
            let entries = Arc::clone(&self.time_entries);
            let app_usage = Arc::clone(&self.app_usage);
            
            let handle = std::thread::spawn(move || {
                tracker::track_foreground_window(entries, app_usage);
            });
            
            self.tracking_thread = Some(handle);
        }
    }
    
    fn stop_tracking(&mut self) {
        if self.is_tracking {
            self.is_tracking = false;
            if let Some(start) = self.current_session_start.take() {
                self.total_tracked_time += start.elapsed();
            }
            
            if let Ok(entries) = self.time_entries.lock() {
                storage::save_to_csv(&entries).ok();
            }
        }
    }
}

impl eframe::App for MyTimeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize tray on first run
        if self.tray_manager.is_none() {
            self.initialize_tray(ctx);
        }

        // Process tray commands first
        let mut commands_to_process = Vec::new();
        if let Some(ref rx) = self.tray_command_rx {
            while let Ok(cmd) = rx.try_recv() {
                commands_to_process.push(cmd);
            }
        }

        // Process commands after releasing the borrow
        for cmd in commands_to_process {
            match cmd {
                tray::TrayCommand::Show => {
                    self.window_visible = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                tray::TrayCommand::Start => {
                    if !self.is_tracking {
                        self.start_tracking();
                    }
                }
                tray::TrayCommand::Stop => {
                    if self.is_tracking {
                        self.stop_tracking();
                    }
                }
                tray::TrayCommand::Exit => {
                    // Stop tracking before quitting
                    if self.is_tracking {
                        self.stop_tracking();
                    }
                    self.should_quit.store(true, Ordering::Relaxed);
                }
            }
        }

        // Update tray status
        if let Some(ref mut tray_manager) = self.tray_manager {
            let current_duration = if let Some(start) = self.current_session_start {
                self.total_tracked_time + start.elapsed()
            } else {
                self.total_tracked_time
            };

            // Update tray status (ignore errors to avoid disrupting the app)
            let _ = tray_manager.update_status(self.is_tracking, current_duration);
        }

        // Check if we should quit first
        if self.should_quit.load(Ordering::Relaxed) {
            // Stop tracking before quitting
            if self.is_tracking {
                self.stop_tracking();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return; // Exit early to avoid processing other events
        }

        // Handle window close event - minimize to tray instead of quitting (only if not quitting)
        if ctx.input(|i| i.viewport().close_requested()) {
            self.window_visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            // Don't actually close the window
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MyTime - Time Tracker");
            
            ui.separator();
            
            ui.horizontal(|ui| {
                // Start button - enabled only when not tracking
                let start_button = egui::Button::new("▶ Start");
                if ui.add_enabled(!self.is_tracking, start_button).clicked() {
                    self.start_tracking();
                }

                // Stop button - enabled only when tracking
                let stop_button = egui::Button::new("⏸ Stop");
                if ui.add_enabled(self.is_tracking, stop_button).clicked() {
                    self.stop_tracking();
                }

                ui.separator();
                ui.label(format!("Status: {}", if self.is_tracking { "Tracking" } else { "Stopped" }));
            });

            ui.separator();

            // Add quit button
            ui.horizontal(|ui| {
                if ui.button("❌ Quit").clicked() {
                    self.should_quit.store(true, Ordering::Relaxed);
                }
                ui.label("(Close window to minimize to tray)");
            });
            
            ui.separator();
            
            let current_duration = if let Some(start) = self.current_session_start {
                self.total_tracked_time + start.elapsed()
            } else {
                self.total_tracked_time
            };
            
            ui.label(format!("Total Time: {} hours {} minutes {} seconds", 
                current_duration.as_secs() / 3600,
                (current_duration.as_secs() % 3600) / 60,
                current_duration.as_secs() % 60
            ));
            
            ui.separator();
            
            if ui.button("📊 Show Chart").clicked() {
                self.show_chart(ui);
            }
            
            if let Ok(app_usage) = self.app_usage.lock() {
                if !app_usage.is_empty() {
                    ui.separator();
                    ui.heading("App Usage");
                    
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (app, duration) in app_usage.iter() {
                            ui.label(format!("{}: {} min", 
                                app, 
                                duration.as_secs() / 60
                            ));
                        }
                    });
                }
            }
        });
        
        if self.is_tracking {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

impl MyTimeApp {
    fn show_chart(&self, ui: &mut egui::Ui) {
        use egui_plot::{Bar, BarChart, Plot};
        
        if let Ok(app_usage) = self.app_usage.lock() {
            let bars: Vec<Bar> = app_usage
                .iter()
                .enumerate()
                .map(|(i, (app, duration))| {
                    Bar::new(i as f64, duration.as_secs() as f64 / 60.0)
                        .name(app)
                })
                .collect();
            
            let chart = BarChart::new(bars);
            
            Plot::new("app_usage_chart")
                .view_aspect(2.0)
                .show(ui, |plot_ui| plot_ui.bar_chart(chart));
        }
    }
}

mod tracker {
    use super::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::Threading::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    
    pub fn track_foreground_window(
        entries: Arc<Mutex<Vec<TimeEntry>>>,
        app_usage: Arc<Mutex<HashMap<String, Duration>>>
    ) {
        let mut last_window_info: Option<(String, String)> = None;
        let mut last_change_time = Instant::now();
        
        loop {
            if let Some((app_name, window_title)) = get_foreground_window_info() {
                let current_info = (app_name.clone(), window_title.clone());
                
                if last_window_info.as_ref() != Some(&current_info) {
                    let now = Instant::now();
                    let duration = now - last_change_time;
                    
                    if let Some((last_app, last_title)) = last_window_info {
                        let entry = TimeEntry {
                            app_name: last_app.clone(),
                            window_title: last_title,
                            start_time: Local::now() - chrono::Duration::seconds(duration.as_secs() as i64),
                            duration_seconds: duration.as_secs(),
                        };
                        
                        if let Ok(mut entries_lock) = entries.lock() {
                            entries_lock.push(entry);
                        }
                        
                        if let Ok(mut usage) = app_usage.lock() {
                            *usage.entry(last_app).or_insert(Duration::ZERO) += duration;
                        }
                    }
                    
                    last_window_info = Some(current_info);
                    last_change_time = now;
                }
            }
            
            std::thread::sleep(Duration::from_millis(100));
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
            
            // Find the actual length by looking for null terminator
            let actual_len = exe_buf.iter().position(|&c| c == 0).unwrap_or(result as usize);
            
            let exe_path = OsString::from_wide(&exe_buf[..actual_len])
                .to_string_lossy()
                .to_string();
            
            let app_name = exe_path
                .split('\\')
                .last()
                .unwrap_or("Unknown")
                .to_string();
            
            CloseHandle(process).ok();
            
            Some((app_name, window_title))
        }
    }
}

mod storage {
    use super::*;
    use std::fs::{OpenOptions, metadata};
    use std::io::Write;
    
    pub fn save_to_csv(entries: &[TimeEntry]) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = "mytime_data.csv";
        let file_exists = metadata(file_path).is_ok();
        
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .append(true)
            .open(file_path)?;
        
        // Write header if file is new
        if !file_exists {
            writeln!(&file, "app_name,window_title,start_time,duration_seconds")?;
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

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_min_inner_size([400.0, 300.0])
            .with_visible(true),
        ..Default::default()
    };

    let app = MyTimeApp::default();

    eframe::run_native(
        "MyTime",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}