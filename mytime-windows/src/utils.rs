//! Utility functions for MyTime

#![allow(dead_code)] // Utils will be used in Phase 2

use chrono::Timelike;
use regex::Regex;
use std::sync::LazyLock;

/// Regex for normalizing titles (replace digits with *)
static DIGIT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// Convert app name (e.g., "msedge.exe") to friendly name (e.g., "Microsoft Edge")
pub fn to_friendly_name(app_name: &str) -> String {
    // Remove .exe extension
    let name = app_name
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE");

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
        "pwsh" => "PowerShell".to_string(),
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
        "wechat" | "weixin" => "WeChat".to_string(),
        "telegram" => "Telegram".to_string(),
        "signal" => "Signal".to_string(),
        "zoom" => "Zoom".to_string(),
        "obs64" | "obs" => "OBS Studio".to_string(),
        "vlc" => "VLC".to_string(),
        "acrobat" => "Adobe Acrobat".to_string(),
        "photoshop" => "Adobe Photoshop".to_string(),
        // Microsoft New Outlook
        "olk" => "Outlook".to_string(),
        // Games
        "bf6" | "bf2042" | "battlefield" => "Battlefield".to_string(),
        "cod" | "modernwarfare" | "warzone" => "Call of Duty".to_string(),
        "valorant" => "Valorant".to_string(),
        "leagueoflegends" | "league of legends" => "League of Legends".to_string(),
        "csgo" | "cs2" | "counterstrike" => "Counter-Strike".to_string(),
        "dota2" => "Dota 2".to_string(),
        "overwatch" => "Overwatch".to_string(),
        "fortnite" | "fortniteclient" => "Fortnite".to_string(),
        "minecraft" | "javaw" => "Minecraft".to_string(),
        "gta5" | "gtav" | "playgtav" => "GTA V".to_string(),
        "rdr2" => "Red Dead Redemption 2".to_string(),
        "eldenring" => "Elden Ring".to_string(),
        "cyberpunk2077" => "Cyberpunk 2077".to_string(),
        // More apps
        "explorer" => "File Explorer".to_string(),
        "searchhost" | "searchapp" => "Windows Search".to_string(),
        "mstsc" => "Remote Desktop".to_string(),
        "systemsettings" => "Settings".to_string(),
        _ => {
            // Capitalize first letter of each word
            name.split(|c: char| c == '-' || c == '_' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().chain(chars).collect()
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// Normalize a window title for hashing
/// - Lowercase
/// - Replace digit sequences with "*"
/// - Trim whitespace
pub fn normalize_title(title: &str) -> String {
    let lowered = title.to_lowercase();
    let normalized = DIGIT_REGEX.replace_all(&lowered, "*");
    normalized.trim().to_string()
}

/// Compute title hash using BLAKE3
/// Uses app_name + normalized_title to prevent collisions
pub fn compute_title_hash(app_name: &str, window_title: &str) -> String {
    let normalized = normalize_title(window_title);
    let input = format!("{}|{}", app_name.to_lowercase(), normalized);
    let hash = blake3::hash(input.as_bytes());
    // Return first 32 hex characters
    hash.to_hex()[..32].to_string()
}

/// Format duration in milliseconds to human-readable string
pub fn format_duration_ms(ms: i64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

/// Get current timestamp in milliseconds
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Default day start hour (6 AM) - day begins at 6 AM, not midnight
pub const DEFAULT_DAY_START_HOUR: u32 = 6;

/// Get start of today in local timezone as milliseconds
/// Uses the provided day_start_hour (0-23) to determine when "today" starts
/// If current time is before day_start_hour, we consider it still "yesterday"
pub fn today_start_ms_with_hour(day_start_hour: u32) -> i64 {
    let now = chrono::Local::now();
    let today = now.date_naive();

    // If current hour is before day_start_hour, use yesterday's date
    let effective_date = if now.hour() < day_start_hour {
        today - chrono::Duration::days(1)
    } else {
        today
    };

    let start_of_day = effective_date.and_hms_opt(day_start_hour, 0, 0).unwrap();
    let local_offset = *now.offset();
    let start_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
        start_of_day - local_offset,
        local_offset,
    );
    start_dt.timestamp_millis()
}

/// Get start of today using default day start hour (6 AM)
pub fn today_start_ms() -> i64 {
    today_start_ms_with_hour(DEFAULT_DAY_START_HOUR)
}

/// Get start and end time for a day with offset from today
/// offset: 0 = today, -1 = yesterday, -2 = two days ago, etc.
/// Returns (start_ms, end_ms) tuple
pub fn day_range_ms_with_offset(day_start_hour: u32, offset: i32) -> (i64, i64) {
    let now = chrono::Local::now();
    let today = now.date_naive();

    // Determine the "effective today" based on day_start_hour
    let effective_today = if now.hour() < day_start_hour {
        today - chrono::Duration::days(1)
    } else {
        today
    };

    // Apply offset
    let target_date = effective_today + chrono::Duration::days(offset as i64);

    let start_of_day = target_date.and_hms_opt(day_start_hour, 0, 0).unwrap();
    let end_of_day = (target_date + chrono::Duration::days(1)).and_hms_opt(day_start_hour, 0, 0).unwrap();

    let local_offset = *now.offset();
    let start_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
        start_of_day - local_offset,
        local_offset,
    );
    let end_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
        end_of_day - local_offset,
        local_offset,
    );

    (start_dt.timestamp_millis(), end_dt.timestamp_millis())
}

/// Format a date for display based on day offset
/// Returns "Today", "Yesterday", or date like "Dec 11"
pub fn format_day_label(offset: i32) -> String {
    match offset {
        0 => "Today".to_string(),
        -1 => "Yesterday".to_string(),
        _ => {
            let now = chrono::Local::now();
            let target = now.date_naive() + chrono::Duration::days(offset as i64);
            target.format("%b %d").to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_friendly_name() {
        assert_eq!(to_friendly_name("msedge.exe"), "Microsoft Edge");
        assert_eq!(to_friendly_name("code.exe"), "Visual Studio Code");
        assert_eq!(to_friendly_name("unknown-app.exe"), "Unknown App");
    }

    #[test]
    fn test_normalize_title() {
        assert_eq!(
            normalize_title("Slack | 3 new messages"),
            "slack | * new messages"
        );
        assert_eq!(
            normalize_title("YouTube - 2:34 / 5:00"),
            "youtube - *:* / *:*"
        );
    }

    #[test]
    fn test_compute_title_hash() {
        let hash1 = compute_title_hash("msedge.exe", "YouTube - Video 1");
        let hash2 = compute_title_hash("msedge.exe", "YouTube - Video 2");
        // Same normalized title (digits replaced) should produce same hash
        assert_eq!(hash1, hash2);

        let hash3 = compute_title_hash("chrome.exe", "YouTube - Video 1");
        // Different app should produce different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0), "00:00");
        assert_eq!(format_duration_ms(65000), "01:05");
        assert_eq!(format_duration_ms(3665000), "01:01:05");
    }
}
