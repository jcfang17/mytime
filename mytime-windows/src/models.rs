//! Data models for MyTime
//!
//! These models are the canonical representation of time tracking data.
//! Segments are the source of truth; sessions are derived.

#![allow(dead_code)] // Models will be used in Phase 2

use serde::{Deserialize, Serialize};

/// A segment represents a contiguous period of time on a single window title.
/// This is the atomic unit of time tracking - source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub segment_id: String,        // UUID v4
    pub app_name: String,          // e.g., "msedge.exe"
    pub window_title: Option<String>, // Raw title, None if redacted
    pub title_hash: String,        // BLAKE3(app_name + normalized_title)
    pub start_time: i64,           // Unix epoch milliseconds
    pub end_time: i64,             // Unix epoch milliseconds
    pub idle_seconds: u64,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
    pub focus_session_id: String,  // Groups segments from same app focus block
    pub device_id: Option<String>, // Optional, for future multi-device
    pub schema_version: i32,
    pub created_at: i64,           // Unix epoch milliseconds
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
    pub category: String,          // entertainment, development, productivity, communication, unknown
    pub source: LabelSource,
    pub confidence: Option<f64>,   // 0.0-1.0, for AI labels
    pub updated_at: i64,           // Unix epoch milliseconds
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelSource {
    Heuristic,
    User,
    Ai,
}

impl LabelSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            LabelSource::Heuristic => "heuristic",
            LabelSource::User => "user",
            LabelSource::Ai => "ai",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "heuristic" => Some(LabelSource::Heuristic),
            "user" => Some(LabelSource::User),
            "ai" => Some(LabelSource::Ai),
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
#[derive(Debug, Clone, Default)]
pub struct AppSummary {
    pub app_name: String,
    pub friendly_name: String,
    pub total_duration_ms: i64,    // Total segment duration (includes idle time)
    pub idle_duration_ms: i64,     // Idle time (subset of total)
    pub segment_count: u32,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
    pub primary_category: Option<String>,
}

/// Daily summary (derived from segments)
#[derive(Debug, Clone, Default)]
pub struct DailySummary {
    pub date: String,              // YYYY-MM-DD
    pub total_duration_ms: i64,    // Total segment duration
    pub total_idle_ms: i64,        // Idle time (subset of total)
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
    Portable,  // Store next to exe
    AppData,   // Store in %APPDATA%\MyTime\
}

/// Schema migration record
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i32,
    pub applied_at: i64,
}

/// Old CSV format for import/export compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTimeEntry {
    pub app_name: String,
    pub window_title: String,
    pub start_time: String,        // ISO8601
    pub end_time: String,          // ISO8601
    pub duration_seconds: u64,
    pub idle_seconds: u64,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
}
