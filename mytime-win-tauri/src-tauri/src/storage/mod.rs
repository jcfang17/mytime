//! Storage module for MyTime
//!
//! Provides a StorageAdapter trait and SQLite implementation.

mod sqlite;

pub use sqlite::SqliteStorage;

use crate::models::{AiSuggestion, AppSummary, ClassificationRule, ContextSummary, DailyDigest, DailySummary, Label, LabelProvenance, Segment, TimelineSegment, UnknownQueueItem};
use crate::models::SelectedBreakdownRow;
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

    /// Get category breakdown at segment level (not app level)
    /// This properly handles apps like browsers where different titles have different categories
    /// Returns (category, total_ms, idle_ms) for each category
    fn get_segment_category_breakdown(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<(String, i64, i64)>>;

    /// Backfill labels for all segments matching a rule
    /// Used when a rule is created/edited to update historical data
    fn backfill_labels_for_rule(&self, rule: &ClassificationRule, days_back: u32) -> StorageResult<u32>;

    /// Get context breakdown within an app (e.g., sites within a browser)
    fn get_app_contexts(
        &self,
        app_name: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<ContextSummary>>;

    /// Get a breakdown of segments matching selected categories.
    ///
    /// For browsers, the result is grouped by (app, context, category).
    /// For non-browsers, the result is grouped by (app, category).
    fn get_selected_breakdown(
        &self,
        start_ms: i64,
        end_ms: i64,
        categories: &[String],
    ) -> StorageResult<Vec<SelectedBreakdownRow>>;

    /// Get today's total tracked time in milliseconds (includes idle)
    fn get_today_total_ms(&self) -> StorageResult<i64>;

    /// Get timeline segments for a day range, each annotated with its best category
    fn get_timeline_segments(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<TimelineSegment>>;

    /// Get unknown-category segments grouped by (app_name, context) for the cleanup queue
    fn get_unknown_queue(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<UnknownQueueItem>>;

    /// Get daily digest statistics for a time range
    fn get_daily_digest(&self, start_ms: i64, end_ms: i64) -> StorageResult<DailyDigest>;

    /// Get label provenance for a title_hash (explains why it has a particular category)
    fn get_label_provenance(&self, title_hash: &str) -> StorageResult<LabelProvenance>;

    // === Config ===

    /// Get a config value
    fn get_config(&self, key: &str) -> StorageResult<Option<String>>;

    /// Set a config value
    fn set_config(&self, key: &str, value: &str) -> StorageResult<()>;

    // === Classification Rules ===

    /// Get all enabled rules, sorted by effective priority (highest first)
    /// Used for rule matching during categorization
    fn get_rules(&self) -> StorageResult<Vec<ClassificationRule>>;

    /// Get all rules (including disabled), sorted by effective priority
    /// Used for the UI to show/edit all rules
    fn get_all_rules(&self) -> StorageResult<Vec<ClassificationRule>>;

    /// Get a rule by ID
    fn get_rule(&self, rule_id: &str) -> StorageResult<Option<ClassificationRule>>;

    /// Insert or update a rule
    fn upsert_rule(&self, rule: &ClassificationRule) -> StorageResult<()>;

    /// Delete a rule by ID
    fn delete_rule(&self, rule_id: &str) -> StorageResult<()>;

    /// Find the first matching rule for an app/title pair
    /// Returns the highest-priority matching rule
    fn find_matching_rule(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> StorageResult<Option<ClassificationRule>>;

    // === AI Suggestions ===

    /// Get all pending AI suggestions
    fn get_pending_suggestions(&self) -> StorageResult<Vec<AiSuggestion>>;

    /// Get a suggestion by ID
    fn get_suggestion(&self, suggestion_id: &str) -> StorageResult<Option<AiSuggestion>>;

    /// Insert a new AI suggestion
    fn insert_suggestion(&self, suggestion: &AiSuggestion) -> StorageResult<()>;

    /// Update suggestion status (approve/reject/expire)
    fn update_suggestion_status(
        &self,
        suggestion_id: &str,
        status: crate::models::SuggestionStatus,
    ) -> StorageResult<()>;

    /// Delete old/expired suggestions
    fn cleanup_old_suggestions(&self, max_age_days: u32) -> StorageResult<u32>;

    // === Maintenance ===

    /// Run pending migrations
    fn run_migrations(&self) -> StorageResult<()>;

    /// Get current schema version
    fn get_schema_version(&self) -> StorageResult<i32>;
}
