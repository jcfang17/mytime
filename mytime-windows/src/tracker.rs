//! Window tracking module for MyTime
//!
//! Tracks foreground window changes and emits segments.
//! Implements the stable-title rule: wait 2s before finalizing a segment.

use crate::categorizer::create_heuristic_label;
use crate::models::Segment;
use crate::storage::{SqliteStorage, StorageAdapter};
use crate::utils::{compute_title_hash, now_ms};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::System::ProcessStatus::*;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Idle threshold in milliseconds (30 seconds)
const IDLE_THRESHOLD_MS: u32 = 30000;

/// Title stability threshold - wait this long before finalizing segment
const TITLE_STABILITY_MS: u64 = 2000;

/// Focus session gap - if same app returns after this, new focus session
const FOCUS_SESSION_GAP_MS: u64 = 30000;

/// Global keystroke counter (updated by keyboard hook)
static KEYSTROKE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Global mouse click counter (updated by mouse hook)
static CLICK_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A pending segment that hasn't been finalized yet
#[derive(Debug, Clone)]
struct PendingSegment {
    segment_id: String,
    app_name: String,
    window_title: String,
    title_hash: String,
    start_time_ms: i64,
    focus_session_id: String,
    // Accumulating metrics
    idle_seconds: u64,
    keystrokes: u64,
    mouse_clicks: u64,
}

impl PendingSegment {
    fn new(app_name: &str, window_title: &str, focus_session_id: &str) -> Self {
        Self {
            segment_id: uuid::Uuid::new_v4().to_string(),
            app_name: app_name.to_string(),
            window_title: window_title.to_string(),
            title_hash: compute_title_hash(app_name, window_title),
            start_time_ms: now_ms(),
            focus_session_id: focus_session_id.to_string(),
            idle_seconds: 0,
            keystrokes: 0,
            mouse_clicks: 0,
        }
    }

    fn finalize(self, device_id: Option<String>) -> Segment {
        let end_time_ms = now_ms();
        Segment {
            segment_id: self.segment_id,
            app_name: self.app_name,
            window_title: Some(self.window_title),
            title_hash: self.title_hash,
            start_time: self.start_time_ms,
            end_time: end_time_ms,
            idle_seconds: self.idle_seconds,
            keystrokes: self.keystrokes,
            mouse_clicks: self.mouse_clicks,
            focus_session_id: self.focus_session_id,
            device_id,
            schema_version: 1,
            created_at: end_time_ms,
        }
    }
}

/// Tracks title stability - pending title change
struct TitleStabilityTracker {
    pending_app: String,
    pending_title: String,
    pending_since: Instant,
}

impl TitleStabilityTracker {
    fn new(app: &str, title: &str) -> Self {
        Self {
            pending_app: app.to_string(),
            pending_title: title.to_string(),
            pending_since: Instant::now(),
        }
    }

    fn update(&mut self, app: &str, title: &str) {
        if self.pending_app != app || self.pending_title != title {
            self.pending_app = app.to_string();
            self.pending_title = title.to_string();
            self.pending_since = Instant::now();
        }
    }

    fn is_stable(&self) -> bool {
        self.pending_since.elapsed().as_millis() as u64 >= TITLE_STABILITY_MS
    }

    fn matches(&self, app: &str, title: &str) -> bool {
        self.pending_app == app && self.pending_title == title
    }
}

/// Callback for segment completion
pub type SegmentCallback = Box<dyn Fn(Segment) + Send + 'static>;

/// Track foreground window and emit segments
///
/// This is the main tracking function that runs in a separate thread.
/// It implements the stable-title rule and emits segments to the callback.
pub fn track_foreground_window(
    storage: Arc<SqliteStorage>,
    should_stop: Arc<AtomicBool>,
    on_segment: Option<SegmentCallback>,
) {
    // Get device ID for segments
    let device_id = storage.get_device_id().ok();

    // Current state
    let mut current_segment: Option<PendingSegment> = None;
    let mut stability_tracker: Option<TitleStabilityTracker> = None;
    let mut current_focus_session_id: Option<String> = None;
    let mut current_app: Option<String> = None; // Track current app for focus session logic
    let mut last_app_change_time = Instant::now(); // Only updated when APP changes, not title
    let mut last_activity_check = Instant::now();

    // Reset counters
    KEYSTROKE_COUNTER.store(0, Ordering::SeqCst);
    CLICK_COUNTER.store(0, Ordering::SeqCst);

    // Start activity monitor (keyboard/mouse hooks)
    static ACTIVITY_MONITOR_STARTED: std::sync::Once = std::sync::Once::new();
    ACTIVITY_MONITOR_STARTED.call_once(|| {
        std::thread::spawn(monitor_activity);
    });

    loop {
        if should_stop.load(Ordering::SeqCst) {
            // Finalize current segment on stop
            if let Some(segment) = current_segment.take() {
                let duration_ms = now_ms() - segment.start_time_ms;
                if duration_ms >= TITLE_STABILITY_MS as i64 {
                    let final_segment = segment.finalize(device_id.clone());
                    save_segment(&storage, &final_segment, on_segment.as_ref());
                }
            }
            break;
        }

        let now = Instant::now();

        // Check idle time every second
        if now - last_activity_check >= Duration::from_secs(1) {
            if let Some(idle_ms) = get_idle_time() {
                if idle_ms > IDLE_THRESHOLD_MS {
                    if let Some(ref mut seg) = current_segment {
                        seg.idle_seconds += 1;
                    }
                }
            }
            last_activity_check = now;
        }

        // Get current foreground window
        if let Some((app_name, window_title)) = get_foreground_window_info() {
            // Update or create stability tracker
            match &mut stability_tracker {
                Some(tracker) => {
                    tracker.update(&app_name, &window_title);
                }
                None => {
                    stability_tracker = Some(TitleStabilityTracker::new(&app_name, &window_title));
                }
            }

            let tracker = stability_tracker.as_ref().unwrap();

            // Check if we need to finalize current segment
            if let Some(ref seg) = current_segment {
                let title_changed = seg.app_name != app_name || seg.window_title != window_title;
                let tracker_stable = tracker.is_stable();
                let tracker_matches_new = tracker.matches(&app_name, &window_title);
                let prev_app_name = seg.app_name.clone(); // Clone before taking

                // Finalize if: title changed AND new title is stable AND tracker matches new title
                if title_changed && tracker_stable && tracker_matches_new {
                    // Collect metrics before finalizing
                    let mut segment = current_segment.take().unwrap();
                    segment.keystrokes += KEYSTROKE_COUNTER.swap(0, Ordering::SeqCst);
                    segment.mouse_clicks += CLICK_COUNTER.swap(0, Ordering::SeqCst);

                    // Only save if segment is long enough
                    let duration_ms = now_ms() - segment.start_time_ms;
                    if duration_ms >= TITLE_STABILITY_MS as i64 {
                        let final_segment = segment.finalize(device_id.clone());
                        save_segment(&storage, &final_segment, on_segment.as_ref());
                    }

                    // Check if this is a new focus session
                    // Only consider it a new session if:
                    // 1. The APP changed (not just title), OR
                    // 2. We returned to the same app after being away for > FOCUS_SESSION_GAP_MS
                    let app_changed = prev_app_name != app_name;
                    let is_new_focus_session = if app_changed {
                        // App changed - check if we're returning to a previous app after a gap
                        let gap_exceeded = now.duration_since(last_app_change_time).as_millis() as u64
                            > FOCUS_SESSION_GAP_MS;
                        // New focus session if: different app OR returning after gap
                        current_app.as_ref() != Some(&app_name) || gap_exceeded
                    } else {
                        false // Same app, same title change = same focus session
                    };

                    // Only update app change time when APP actually changes
                    if app_changed {
                        last_app_change_time = now;
                        current_app = Some(app_name.clone());
                    }

                    if is_new_focus_session {
                        current_focus_session_id = Some(uuid::Uuid::new_v4().to_string());
                    }

                    // Start new segment
                    let focus_session_id = current_focus_session_id
                        .clone()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    current_segment =
                        Some(PendingSegment::new(&app_name, &window_title, &focus_session_id));
                }
            } else {
                // No current segment, start one if tracker is stable
                if tracker.is_stable() {
                    let focus_session_id = current_focus_session_id
                        .clone()
                        .unwrap_or_else(|| {
                            let id = uuid::Uuid::new_v4().to_string();
                            current_focus_session_id = Some(id.clone());
                            id
                        });
                    current_segment =
                        Some(PendingSegment::new(&app_name, &window_title, &focus_session_id));
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Save segment to storage and call callback
fn save_segment(
    storage: &SqliteStorage,
    segment: &Segment,
    on_segment: Option<&SegmentCallback>,
) {
    // Save segment to SQLite
    if let Err(e) = storage.insert_segment(segment) {
        eprintln!("Failed to save segment: {}", e);
    }

    // Create and save heuristic label (only if we have a window title)
    if let Some(ref title) = segment.window_title {
        let label = create_heuristic_label(&segment.title_hash, &segment.app_name, title);
        // Only insert if no label exists yet for this title_hash (don't overwrite user labels)
        if let Ok(None) = storage.get_label(&segment.title_hash) {
            if let Err(e) = storage.upsert_label(&label) {
                eprintln!("Failed to save label: {}", e);
            }
        }
    }

    // Call callback if provided
    if let Some(callback) = on_segment {
        callback(segment.clone());
    }
}

/// Get system idle time in milliseconds
fn get_idle_time() -> Option<u32> {
    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut last_input).as_bool() {
            let current_tick = GetTickCount64() as u32;
            Some(current_tick.wrapping_sub(last_input.dwTime))
        } else {
            None
        }
    }
}

/// Start keyboard and mouse hooks for activity monitoring
fn monitor_activity() {
    unsafe {
        let keyboard_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            HINSTANCE::default(),
            0,
        )
        .ok();

        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE::default(), 0).ok();

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

/// Get foreground window app name and title
fn get_foreground_window_info() -> Option<(String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }

        // Get window title
        let mut title_buf = vec![0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        let window_title = std::ffi::OsString::from(
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        ).to_string_lossy().to_string();

        // Get process ID
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        // Get process handle
        let process =
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id).ok()?;

        // Get executable path
        let mut exe_buf = vec![0u16; 512];
        let result = GetModuleFileNameExW(process, HMODULE::default(), &mut exe_buf);

        let actual_len = exe_buf.iter().position(|&c| c == 0).unwrap_or(result as usize);
        let exe_path = String::from_utf16_lossy(&exe_buf[..actual_len]);

        // Extract just the filename
        let app_name = exe_path
            .split('\\')
            .next_back()
            .unwrap_or("Unknown")
            .to_string();

        let _ = CloseHandle(process);

        Some((app_name, window_title))
    }
}

/// Keyboard hook callback
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 == WM_KEYDOWN as usize {
        KEYSTROKE_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

/// Mouse hook callback
unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 == WM_LBUTTONDOWN as usize || wparam.0 == WM_RBUTTONDOWN as usize) {
        CLICK_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_segment_creation() {
        let segment = PendingSegment::new("test.exe", "Test Window", "focus-123");
        assert_eq!(segment.app_name, "test.exe");
        assert_eq!(segment.window_title, "Test Window");
        assert_eq!(segment.focus_session_id, "focus-123");
        assert!(!segment.segment_id.is_empty());
    }

    #[test]
    fn test_title_stability_tracker() {
        let mut tracker = TitleStabilityTracker::new("app.exe", "Title 1");
        assert!(!tracker.is_stable()); // Just created, not stable yet

        // Simulate time passing would be needed for full test
        assert!(tracker.matches("app.exe", "Title 1"));
        assert!(!tracker.matches("app.exe", "Title 2"));

        tracker.update("app.exe", "Title 2");
        assert!(!tracker.matches("app.exe", "Title 1"));
        assert!(tracker.matches("app.exe", "Title 2"));
    }
}
