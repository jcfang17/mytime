//! Utility functions for MyTime

use chrono::Timelike;
use regex::Regex;
use std::sync::LazyLock;

/// Regex for normalizing titles (replace digits with *)
static DIGIT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// Convert app name (e.g., "msedge.exe") to friendly name (e.g., "Microsoft Edge")
pub fn to_friendly_name(app_name: &str) -> String {
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
                        Some(first) => first.to_uppercase().chain(chars).collect(),
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

    let start_of_day = effective_date
        .and_hms_opt(day_start_hour, 0, 0)
        .unwrap();
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
    let end_of_day = (target_date + chrono::Duration::days(1))
        .and_hms_opt(day_start_hour, 0, 0)
        .unwrap();

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

// === Context Extraction for Browsers ===

/// Known browser app names (lowercase, without .exe)
const BROWSER_APPS: &[&str] = &[
    "msedge", "chrome", "firefox", "brave", "opera", "vivaldi", "arc",
    "safari", "chromium", "edge", "iexplore", "waterfox", "librewolf",
];

/// Check if an app is a browser
pub fn is_browser(app_name: &str) -> bool {
    let app_lower = app_name.to_lowercase();
    let name = app_lower.trim_end_matches(".exe");
    BROWSER_APPS.iter().any(|b| name.contains(b))
}

/// Extract context (site/domain) from a browser window title
///
/// Browser titles typically follow patterns like:
/// - "Page Title - Site Name"
/// - "Page Title | Site Name"
/// - "Page Title – Site Name" (en-dash)
/// - "Site Name: Page Title" (less common)
///
/// Returns the extracted site name, or None if no clear pattern found
pub fn extract_browser_context(window_title: &str) -> Option<String> {
    let title = window_title.trim();
    if title.is_empty() {
        return None;
    }

    // Try to extract from common patterns
    // Pattern 1: "... - Site Name" or "... | Site Name" or "... – Site Name"
    // Work backwards through separators to skip browser name suffixes
    let separators = [" - ", " | ", " – ", " — ", " · "];

    for sep in separators {
        // Find all occurrences and try from rightmost, skipping invalid contexts
        let mut search_str = title;
        while let Some(pos) = search_str.rfind(sep) {
            let site = search_str[pos + sep.len()..].trim();
            if is_valid_context(site) {
                // Normalize, then validate again; this avoids returning values like
                // "Microsoft Edge - Personal" which normalize down to "microsoft edge".
                let normalized = normalize_context(site);
                if is_valid_context(&normalized) {
                    return Some(normalized);
                }
            }
            // Try earlier occurrence
            search_str = &search_str[..pos];
        }
    }

    // Pattern 2: "Site Name: ..." (check first segment)
    if let Some(pos) = title.find(": ") {
        let site = title[..pos].trim();
        // Only use if it looks like a site name (short, no spaces or few)
        if site.len() < 30 && site.split_whitespace().count() <= 3 {
            if is_valid_context(site) {
                return Some(normalize_context(site));
            }
        }
    }

    // No pattern found - try to extract domain-like strings
    extract_domain_from_title(title)
}

/// Check if a string looks like a valid site/context name
fn is_valid_context(s: &str) -> bool {
    fn normalize_for_compare(s: &str) -> String {
        // Some apps insert invisible separators (notably Edge can include U+200B).
        // Remove common zero-width characters and collapse whitespace so browser-name
        // detection is stable.
        let cleaned: String = s
            .chars()
            .filter(|c| !matches!(*c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
            .collect();
        cleaned
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    let lower = normalize_for_compare(s);

    // Must have some content
    if lower.is_empty() || lower.len() < 2 {
        return false;
    }
    // Reject if too long (probably not a site name)
    if lower.len() > 50 {
        return false;
    }

    // Reject common non-site strings (partial match)
    let reject_patterns = [
        "untitled", "new tab", "loading", "and more",
        "more page",
        "search results", "google search", "bing search",
    ];
    if reject_patterns.iter().any(|p| lower.contains(p)) {
        return false;
    }

    // Reject browser names (exact or with profile suffix like "Microsoft Edge - Work")
    let browser_names = [
        "microsoft edge", "google chrome", "mozilla firefox",
        "brave", "opera", "vivaldi", "safari", "chromium",
        "edge", "chrome", "firefox",
    ];
    // Check if it IS a browser name or STARTS WITH a browser name (handles "Microsoft Edge - Work")
    if browser_names.iter().any(|b| lower == *b || lower.starts_with(&format!("{} -", b))) {
        return false;
    }

    // Also reject variants that contain a browser brand name but don't use the " - " separator,
    // e.g. "Microsoft Edge (Work)".
    let browser_brands = ["microsoft edge", "google chrome", "mozilla firefox"];
    if browser_brands.iter().any(|b| lower.contains(b)) {
        return false;
    }

    // Reject exact profile names (case-insensitive)
    let profile_names = [
        "personal", "work", "default", "profile 1", "profile 2",
        "guest", "incognito", "private",
    ];
    if profile_names.iter().any(|p| lower == *p) {
        return false;
    }

    true
}

/// Normalize a context string for consistent storage/comparison
fn normalize_context(s: &str) -> String {
    // Match the same normalization as is_valid_context.
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(*c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
        .collect();

    cleaned
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        // Remove common suffixes
        .trim_end_matches(" - google chrome")
        .trim_end_matches(" - microsoft edge")
        .trim_end_matches(" - mozilla firefox")
        .trim_end_matches(" - personal")
        .trim_end_matches(" - work")
        .trim()
        .to_string()
}

/// Try to extract a domain-like pattern from title
fn extract_domain_from_title(title: &str) -> Option<String> {
    // Look for common domain patterns in the title
    let known_sites = [
        ("youtube", "youtube"),
        ("youtu.be", "youtube"),
        ("github.com", "github"),
        ("github", "github"),
        ("gitlab", "gitlab"),
        ("stackoverflow", "stackoverflow"),
        ("stack overflow", "stackoverflow"),
        ("reddit", "reddit"),
        ("twitter", "twitter"),
        ("x.com", "twitter"),
        ("bilibili", "bilibili"),
        ("netflix", "netflix"),
        ("twitch", "twitch"),
        ("discord", "discord"),
        ("slack", "slack"),
        ("notion", "notion"),
        ("figma", "figma"),
        ("google docs", "google docs"),
        ("google sheets", "google sheets"),
        ("google drive", "google drive"),
        ("gmail", "gmail"),
        ("outlook", "outlook"),
        ("overleaf", "overleaf"),
        ("chatgpt", "chatgpt"),
        ("grok", "grok"),
        ("claude", "claude"),
        ("linkedin", "linkedin"),
        ("facebook", "facebook"),
        ("instagram", "instagram"),
        ("tiktok", "tiktok"),
        ("amazon", "amazon"),
        ("wikipedia", "wikipedia"),
        ("medium", "medium"),
        ("dev.to", "dev.to"),
        ("hacker news", "hacker news"),
        ("localhost", "localhost"),
    ];

    let title_lower = title.to_lowercase();
    for (pattern, site) in known_sites {
        if title_lower.contains(pattern) {
            return Some(site.to_string());
        }
    }

    None
}

/// Extract context for any app (browser or not)
/// For browsers, extracts site. For other apps, returns None.
pub fn extract_context(app_name: &str, window_title: &str) -> Option<String> {
    if is_browser(app_name) {
        extract_browser_context(window_title)
    } else {
        // For non-browsers, we could potentially extract project names
        // from IDEs, etc. For now, return None.
        None
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

    #[test]
    fn test_is_browser() {
        assert!(is_browser("msedge.exe"));
        assert!(is_browser("chrome.exe"));
        assert!(is_browser("firefox.exe"));
        assert!(!is_browser("code.exe"));
        assert!(!is_browser("slack.exe"));
    }

    #[test]
    fn test_extract_browser_context() {
        // Standard "Title - Site" pattern
        assert_eq!(
            extract_browser_context("My Project - Overleaf"),
            Some("overleaf".to_string())
        );
        assert_eq!(
            extract_browser_context("Watch Video - YouTube"),
            Some("youtube".to_string())
        );

        // Pipe separator
        assert_eq!(
            extract_browser_context("repo | GitHub"),
            Some("github".to_string())
        );

        // Known site detection
        assert_eq!(
            extract_browser_context("Some random video on bilibili"),
            Some("bilibili".to_string())
        );

        // Empty/invalid
        assert_eq!(extract_browser_context(""), None);
        assert_eq!(extract_browser_context("New Tab"), None);

        // Browser names should be rejected as contexts
        assert_eq!(extract_browser_context("New Tab - Microsoft Edge"), None);
        assert_eq!(
            extract_browser_context("New Tab - Microsoft\u{200b} Edge"),
            None
        );
        assert_eq!(extract_browser_context("Settings - Google Chrome"), None);
        assert_eq!(extract_browser_context("Microsoft Edge"), None);

        // But pages about browsers should work
        assert_eq!(
            extract_browser_context("Edge computing - Wikipedia"),
            Some("wikipedia".to_string())
        );

        // Multi-separator: skip browser name suffix and profile name
        assert_eq!(
            extract_browser_context("anthropics/claude-code - GitHub - Personal - Microsoft Edge"),
            Some("github".to_string())
        );
        assert_eq!(
            extract_browser_context(
                "anthropics/claude-code - GitHub - Personal - Microsoft\u{200b} Edge"
            ),
            Some("github".to_string())
        );
        assert_eq!(
            extract_browser_context("YouTube - Work - Google Chrome"),
            Some("youtube".to_string())
        );

        // Edge multi-tab titles: don't treat "and N more pages" as the site.
        assert_eq!(
            extract_browser_context(
                "ChatGPT - AI4PS and 6 more pages - Personal - Microsoft\u{200b} Edge"
            ),
            Some("chatgpt".to_string())
        );
        assert_eq!(
            extract_browser_context(
                "Efficient Token Reduction in Vision Transformers - Grok and 6 more pages - Personal - Microsoft\u{200b} Edge"
            ),
            Some("grok".to_string())
        );

        // Browser name with profile suffix should be rejected
        assert_eq!(extract_browser_context("Page - Microsoft Edge - Work"), None);
        assert_eq!(extract_browser_context("Page - Microsoft Edge - Personal"), None);
        assert_eq!(extract_browser_context("Tab - Google Chrome - Profile 1"), None);
    }
}
