//! Data models for MyTime
//!
//! These models are the canonical representation of time tracking data.
//! Segments are the source of truth; sessions are derived.

use serde::{Deserialize, Serialize};

/// A segment represents a contiguous period of time on a single window title.
/// This is the atomic unit of time tracking - source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub segment_id: String,           // UUID v4
    pub app_name: String,             // e.g., "msedge.exe"
    pub window_title: Option<String>, // Raw title, None if redacted
    pub title_hash: String,           // BLAKE3(app_name + normalized_title)
    pub start_time: i64,              // Unix epoch milliseconds
    pub end_time: i64,                // Unix epoch milliseconds
    pub idle_seconds: u64,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
    pub focus_session_id: String, // Groups segments from same app focus block
    pub device_id: Option<String>, // Optional, for future multi-device
    pub schema_version: i32,
    pub created_at: i64, // Unix epoch milliseconds
}

impl Segment {
    pub fn duration_seconds(&self) -> u64 {
        ((self.end_time - self.start_time) / 1000) as u64
    }

    pub fn duration_ms(&self) -> i64 {
        self.end_time - self.start_time
    }
}

/// A label associates a category with a title_hash.
/// Stored separately with provenance for reclassification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub title_hash: String,
    pub category: String, // entertainment, development, productivity, communication, unknown
    pub source: LabelSource,
    pub confidence: Option<f64>, // 0.0-1.0, for AI labels
    pub updated_at: i64,         // Unix epoch milliseconds
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelSource {
    Heuristic,
    User,    // From classification rules
    Ai,
    Manual,  // Direct user assignment (highest priority)
}

impl LabelSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            LabelSource::Heuristic => "heuristic",
            LabelSource::User => "user",
            LabelSource::Ai => "ai",
            LabelSource::Manual => "manual",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "heuristic" => Some(LabelSource::Heuristic),
            "user" => Some(LabelSource::User),
            "ai" => Some(LabelSource::Ai),
            "manual" => Some(LabelSource::Manual),
            _ => None,
        }
    }
}

/// Category enum for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Entertainment,
    Development,
    Productivity,
    Communication,
    Unknown,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Entertainment => "entertainment",
            Category::Development => "development",
            Category::Productivity => "productivity",
            Category::Communication => "communication",
            Category::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "entertainment" => Category::Entertainment,
            "development" => Category::Development,
            "productivity" => Category::Productivity,
            "communication" => Category::Communication,
            _ => Category::Unknown,
        }
    }
}

/// Summary of time spent per app (derived from segments)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSummary {
    pub app_name: String,
    pub friendly_name: String,
    pub total_duration_ms: i64, // Total segment duration (includes idle time)
    pub idle_duration_ms: i64,  // Idle time (subset of total)
    pub segment_count: u32,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
    pub primary_category: Option<String>,
}

/// Summary of time spent per context within an app (e.g., sites within a browser)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSummary {
    pub context: String,                // Site/domain extracted from title (e.g., "youtube", "github")
    pub category: Option<String>,       // Category for this context
    pub total_duration_ms: i64,         // Total time
    pub idle_duration_ms: i64,          // Idle time
    pub segment_count: u32,
    pub sample_titles: Vec<String>,     // Up to 3 example window titles
}

/// Daily summary (derived from segments)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: String,                        // YYYY-MM-DD
    pub total_duration_ms: i64,              // Total segment duration
    pub total_idle_ms: i64,                  // Idle time (subset of total)
    pub app_summaries: Vec<AppSummary>,
    pub category_breakdown: Vec<(String, i64)>, // (category, duration_ms)
}

/// Bootstrap configuration - stored in bootstrap.json next to exe
/// Only contains data_location since it's needed before DB path is known
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    pub data_location: DataLocation,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            data_location: DataLocation::AppData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataLocation {
    Portable, // Store next to exe
    AppData,  // Store in %APPDATA%\MyTime\
}

/// Schema migration record
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i32,
    pub applied_at: i64,
}

/// Tracking state sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingState {
    pub is_tracking: bool,
    pub session_start_ms: Option<i64>,
    pub total_time_ms: i64,     // Live total from DB
    pub baseline_ms: Option<i64>, // Total at session start (for avoiding double-count)
}

/// Old CSV format for import/export compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTimeEntry {
    pub app_name: String,
    pub window_title: String,
    pub start_time: String, // ISO8601
    pub end_time: String,   // ISO8601
    pub duration_seconds: u64,
    pub idle_seconds: u64,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
}

// === Classification Rules ===

/// How to match app_pattern and title_pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    Contains,  // Simple substring match (case-insensitive)
    Prefix,    // Starts with (case-insensitive)
    Exact,     // Exact match (case-insensitive)
    Regex,     // Regular expression
}

impl MatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchType::Contains => "contains",
            MatchType::Prefix => "prefix",
            MatchType::Exact => "exact",
            MatchType::Regex => "regex",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "contains" => MatchType::Contains,
            "prefix" => MatchType::Prefix,
            "exact" => MatchType::Exact,
            "regex" => MatchType::Regex,
            _ => MatchType::Contains, // Default
        }
    }
}

/// Source of a classification rule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSource {
    Builtin,    // Shipped with app, lowest priority
    User,       // Created by user, highest priority
    AiApproved, // AI suggestion approved by user
}

impl RuleSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleSource::Builtin => "builtin",
            RuleSource::User => "user",
            RuleSource::AiApproved => "ai-approved",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "builtin" => RuleSource::Builtin,
            "user" => RuleSource::User,
            "ai-approved" => RuleSource::AiApproved,
            _ => RuleSource::Builtin,
        }
    }

    /// Priority for rule ordering (higher = wins)
    pub fn priority(&self) -> i32 {
        match self {
            RuleSource::User => 100,
            RuleSource::AiApproved => 50,
            RuleSource::Builtin => 0,
        }
    }
}

/// A classification rule for categorizing windows
/// Rules are matched against (app_name, window_title) and output a category + optional tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRule {
    pub rule_id: String,
    pub app_pattern: Option<String>,   // NULL = match any app
    pub title_pattern: Option<String>, // NULL = match any title
    pub match_type: MatchType,
    pub category: String,
    pub tags: Option<Vec<String>>,     // Optional tags like ["site:overleaf", "work"]
    pub source: RuleSource,
    pub priority: i32,                 // Additional priority within same source
    pub enabled: bool,
    pub created_at: i64,
}

impl ClassificationRule {
    /// Compute effective priority (source priority + custom priority)
    pub fn effective_priority(&self) -> i32 {
        self.source.priority() + self.priority
    }

    /// Check if this rule matches the given app name and window title
    pub fn matches(&self, app_name: &str, window_title: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        let title_lower = window_title.to_lowercase();

        // Check app pattern (if specified)
        if let Some(ref pattern) = self.app_pattern {
            if !self.pattern_matches(pattern, &app_lower) {
                return false;
            }
        }

        // Check title pattern (if specified)
        if let Some(ref pattern) = self.title_pattern {
            if !self.pattern_matches(pattern, &title_lower) {
                return false;
            }
        }

        // If both patterns are None, rule matches everything (probably not intended)
        // Require at least one pattern
        self.app_pattern.is_some() || self.title_pattern.is_some()
    }

    fn pattern_matches(&self, pattern: &str, text: &str) -> bool {
        let pattern_lower = pattern.to_lowercase();
        match self.match_type {
            MatchType::Contains => text.contains(&pattern_lower),
            MatchType::Prefix => text.starts_with(&pattern_lower),
            MatchType::Exact => text == pattern_lower,
            MatchType::Regex => {
                // Compile regex (in production, consider caching compiled regexes)
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(text))
                    .unwrap_or(false)
            }
        }
    }
}

/// Result of rule matching - category and optional tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    pub rule_id: String,
    pub category: String,
    pub tags: Option<Vec<String>>,
}

// === AI Suggestions ===

/// Status of an AI suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionStatus {
    Pending,   // Waiting for user review
    Approved,  // User approved, rule created
    Rejected,  // User rejected
    Expired,   // Auto-expired (too old or pattern no longer matches)
}

impl SuggestionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SuggestionStatus::Pending => "pending",
            SuggestionStatus::Approved => "approved",
            SuggestionStatus::Rejected => "rejected",
            SuggestionStatus::Expired => "expired",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => SuggestionStatus::Pending,
            "approved" => SuggestionStatus::Approved,
            "rejected" => SuggestionStatus::Rejected,
            "expired" => SuggestionStatus::Expired,
            _ => SuggestionStatus::Pending,
        }
    }
}

/// An AI-generated categorization suggestion awaiting user approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub suggestion_id: String,
    pub app_pattern: Option<String>,
    pub title_pattern: Option<String>,
    pub match_type: MatchType,
    pub suggested_category: String,
    pub confidence: f64,           // 0.0-1.0
    pub reason: String,            // Why AI suggested this
    pub sample_titles: Vec<String>, // Example titles that would match
    pub match_count: u32,          // How many historical segments match
    pub total_duration_ms: i64,    // Total time of matching segments
    pub status: SuggestionStatus,
    pub created_at: i64,
    pub reviewed_at: Option<i64>,  // When user approved/rejected
}
