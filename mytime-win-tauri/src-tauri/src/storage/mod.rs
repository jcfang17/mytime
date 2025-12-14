//! Storage module for MyTime
//!
//! Provides a StorageAdapter trait and SQLite implementation.

mod sqlite;

pub use sqlite::SqliteStorage;

use crate::models::{AppSummary, DailySummary, Label, Segment};
use std::error::Error;

/// Result type for storage operations
pub type StorageResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Storage adapter trait - abstraction for future backend swaps
pub trait StorageAdapter: Send + Sync {
    // === Segments ===

    /// Insert a new segment
    fn insert_segment(&self, segment: &Segment) -> StorageResult<()>;

    /// Get segments within a time range
    fn get_segments_range(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<Segment>>;

    /// Get segments by focus session ID
    fn get_segments_by_focus_session(&self, focus_session_id: &str) -> StorageResult<Vec<Segment>>;

    // === Labels ===

    /// Get label for a title_hash (returns highest priority: user > ai > heuristic)
    fn get_label(&self, title_hash: &str) -> StorageResult<Option<Label>>;

    /// Get all labels for a title_hash (all sources)
    fn get_labels(&self, title_hash: &str) -> StorageResult<Vec<Label>>;

    /// Insert or update a label
    fn upsert_label(&self, label: &Label) -> StorageResult<()>;

    // === Derived Queries ===

    /// Get summary for a specific date
    fn get_daily_summary(&self, date: &str) -> StorageResult<DailySummary>;

    /// Get app breakdown for a time range
    fn get_app_breakdown(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<AppSummary>>;

    /// Get today's total active time in milliseconds
    fn get_today_active_ms(&self) -> StorageResult<i64>;

    // === Config ===

    /// Get a config value
    fn get_config(&self, key: &str) -> StorageResult<Option<String>>;

    /// Set a config value
    fn set_config(&self, key: &str, value: &str) -> StorageResult<()>;

    // === Maintenance ===

    /// Run pending migrations
    fn run_migrations(&self) -> StorageResult<()>;

    /// Get current schema version
    fn get_schema_version(&self) -> StorageResult<i32>;
}
