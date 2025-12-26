//! SQLite storage implementation for MyTime

use crate::models::{
    AiSuggestion, AppSummary, BootstrapConfig, ClassificationRule, ContextSummary, DailySummary,
    DataLocation, Label, LabelSource, MatchType, RuleSource, Segment, SuggestionStatus,
};
use crate::storage::{StorageAdapter, StorageResult};
use crate::utils;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// SQLite storage implementation
pub struct SqliteStorage {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

impl SqliteStorage {
    /// Create a new SqliteStorage instance
    /// This handles bootstrap config, data location, and migrations
    pub fn new() -> StorageResult<Self> {
        let data_dir = Self::determine_data_dir()?;

        // Ensure data directory exists
        fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("mytime.db");
        let conn = Connection::open(&db_path)?;

        // Enable foreign keys and WAL mode for better performance
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        let storage = Self {
            conn: Mutex::new(conn),
            data_dir,
        };

        // Run migrations
        storage.run_migrations()?;

        // Ensure device_id exists
        storage.ensure_device_id()?;

        Ok(storage)
    }

    /// Determine the data directory based on bootstrap config
    fn determine_data_dir() -> StorageResult<PathBuf> {
        let exe_dir = std::env::current_exe()?
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let bootstrap_path = exe_dir.join("bootstrap.json");

        // Try to read existing bootstrap config
        let config = if bootstrap_path.exists() {
            let content = fs::read_to_string(&bootstrap_path)?;
            serde_json::from_str::<BootstrapConfig>(&content).unwrap_or_default()
        } else {
            // First run - check if exe dir is writable
            let test_file = exe_dir.join(".mytime_write_test");
            let is_writable = fs::write(&test_file, "test").is_ok();
            if is_writable {
                let _ = fs::remove_file(&test_file);
            }

            // Default to AppData (safer)
            let config = BootstrapConfig {
                data_location: DataLocation::AppData,
            };

            // Save bootstrap config
            let content = serde_json::to_string_pretty(&config)?;
            let _ = fs::write(&bootstrap_path, content);

            config
        };

        // Resolve data directory
        let data_dir = match config.data_location {
            DataLocation::Portable => exe_dir.join("data"),
            DataLocation::AppData => {
                let appdata = std::env::var("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| exe_dir.clone());
                appdata.join("MyTime")
            }
        };

        Ok(data_dir)
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Ensure device_id exists in config
    fn ensure_device_id(&self) -> StorageResult<()> {
        if self.get_config("device_id")?.is_none() {
            let device_id = uuid::Uuid::new_v4().to_string();
            self.set_config("device_id", &device_id)?;
        }
        Ok(())
    }

    /// Get the device_id
    pub fn get_device_id(&self) -> StorageResult<String> {
        self.get_config("device_id")?
            .ok_or_else(|| "device_id not found".into())
    }

    /// Get the day start hour (0-23), defaults to 6 (6 AM)
    pub fn get_day_start_hour(&self) -> StorageResult<u32> {
        match self.get_config("day_start_hour")? {
            Some(value) => Ok(value.parse().unwrap_or(utils::DEFAULT_DAY_START_HOUR)),
            None => Ok(utils::DEFAULT_DAY_START_HOUR),
        }
    }

    /// Set the day start hour (0-23)
    pub fn set_day_start_hour(&self, hour: u32) -> StorageResult<()> {
        let hour = hour.min(23); // Clamp to valid range
        self.set_config("day_start_hour", &hour.to_string())
    }

    /// Run SQL migration v001 - initial schema
    fn run_migration_v001(conn: &Connection) -> StorageResult<()> {
        conn.execute_batch(
            r#"
            -- Segments table: source of truth
            CREATE TABLE IF NOT EXISTS segments (
                segment_id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                window_title TEXT,
                title_hash TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                idle_seconds INTEGER DEFAULT 0,
                keystrokes INTEGER DEFAULT 0,
                mouse_clicks INTEGER DEFAULT 0,
                focus_session_id TEXT NOT NULL,
                device_id TEXT,
                schema_version INTEGER DEFAULT 1,
                created_at INTEGER NOT NULL
            );

            -- Indexes for common queries
            CREATE INDEX IF NOT EXISTS idx_segments_start_time ON segments(start_time);
            CREATE INDEX IF NOT EXISTS idx_segments_title_hash ON segments(title_hash);
            CREATE INDEX IF NOT EXISTS idx_segments_focus_session ON segments(focus_session_id);
            CREATE INDEX IF NOT EXISTS idx_segments_app_name ON segments(app_name);

            -- Labels table: category assignments with provenance
            CREATE TABLE IF NOT EXISTS labels (
                title_hash TEXT NOT NULL,
                category TEXT NOT NULL,
                source TEXT NOT NULL,
                confidence REAL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (title_hash, source)
            );

            CREATE INDEX IF NOT EXISTS idx_labels_title_hash ON labels(title_hash);

            -- Config table: app settings
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- Schema migrations tracking
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );
            "#,
        )?;

        Ok(())
    }

    /// Run SQL migration v002 - classification rules
    fn run_migration_v002(conn: &Connection) -> StorageResult<()> {
        conn.execute_batch(
            r#"
            -- Classification rules table: pattern-based categorization
            CREATE TABLE IF NOT EXISTS classification_rules (
                rule_id TEXT PRIMARY KEY,
                app_pattern TEXT,           -- NULL = match any app
                title_pattern TEXT,         -- NULL = match any title
                match_type TEXT NOT NULL DEFAULT 'contains',  -- contains, prefix, exact, regex
                category TEXT NOT NULL,
                tags TEXT,                  -- JSON array of tags
                source TEXT NOT NULL DEFAULT 'user',  -- builtin, user, ai-approved
                priority INTEGER DEFAULT 0, -- Additional priority within same source
                enabled INTEGER DEFAULT 1,
                created_at INTEGER NOT NULL
            );

            -- Index for enabled rules
            CREATE INDEX IF NOT EXISTS idx_rules_enabled ON classification_rules(enabled);
            CREATE INDEX IF NOT EXISTS idx_rules_source ON classification_rules(source);
            "#,
        )?;

        Ok(())
    }

    /// Run SQL migration v003 - AI suggestions
    fn run_migration_v003(conn: &Connection) -> StorageResult<()> {
        conn.execute_batch(
            r#"
            -- AI suggestions table: pending AI categorization suggestions
            CREATE TABLE IF NOT EXISTS ai_suggestions (
                suggestion_id TEXT PRIMARY KEY,
                app_pattern TEXT,
                title_pattern TEXT,
                match_type TEXT NOT NULL DEFAULT 'contains',
                suggested_category TEXT NOT NULL,
                confidence REAL NOT NULL,
                reason TEXT NOT NULL,
                sample_titles TEXT,         -- JSON array
                match_count INTEGER DEFAULT 0,
                total_duration_ms INTEGER DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',  -- pending, approved, rejected, expired
                created_at INTEGER NOT NULL,
                reviewed_at INTEGER
            );

            -- Index for pending suggestions
            CREATE INDEX IF NOT EXISTS idx_suggestions_status ON ai_suggestions(status);
            CREATE INDEX IF NOT EXISTS idx_suggestions_created ON ai_suggestions(created_at);
            "#,
        )?;

        Ok(())
    }
}

impl StorageAdapter for SqliteStorage {
    fn insert_segment(&self, segment: &Segment) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute(
            r#"
            INSERT INTO segments (
                segment_id, app_name, window_title, title_hash,
                start_time, end_time, idle_seconds, keystrokes, mouse_clicks,
                focus_session_id, device_id, schema_version, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                segment.segment_id,
                segment.app_name,
                segment.window_title,
                segment.title_hash,
                segment.start_time,
                segment.end_time,
                segment.idle_seconds,
                segment.keystrokes,
                segment.mouse_clicks,
                segment.focus_session_id,
                segment.device_id,
                segment.schema_version,
                segment.created_at,
            ],
        )?;

        Ok(())
    }

    fn get_segments_range(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<Segment>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT segment_id, app_name, window_title, title_hash,
                   start_time, end_time, idle_seconds, keystrokes, mouse_clicks,
                   focus_session_id, device_id, schema_version, created_at
            FROM segments
            WHERE start_time >= ?1 AND start_time < ?2
            ORDER BY start_time ASC
            "#,
        )?;

        let segments = stmt
            .query_map(params![start_ms, end_ms], |row| {
                Ok(Segment {
                    segment_id: row.get(0)?,
                    app_name: row.get(1)?,
                    window_title: row.get(2)?,
                    title_hash: row.get(3)?,
                    start_time: row.get(4)?,
                    end_time: row.get(5)?,
                    idle_seconds: row.get(6)?,
                    keystrokes: row.get(7)?,
                    mouse_clicks: row.get(8)?,
                    focus_session_id: row.get(9)?,
                    device_id: row.get(10)?,
                    schema_version: row.get(11)?,
                    created_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(segments)
    }

    fn get_segments_by_focus_session(&self, focus_session_id: &str) -> StorageResult<Vec<Segment>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT segment_id, app_name, window_title, title_hash,
                   start_time, end_time, idle_seconds, keystrokes, mouse_clicks,
                   focus_session_id, device_id, schema_version, created_at
            FROM segments
            WHERE focus_session_id = ?1
            ORDER BY start_time ASC
            "#,
        )?;

        let segments = stmt
            .query_map(params![focus_session_id], |row| {
                Ok(Segment {
                    segment_id: row.get(0)?,
                    app_name: row.get(1)?,
                    window_title: row.get(2)?,
                    title_hash: row.get(3)?,
                    start_time: row.get(4)?,
                    end_time: row.get(5)?,
                    idle_seconds: row.get(6)?,
                    keystrokes: row.get(7)?,
                    mouse_clicks: row.get(8)?,
                    focus_session_id: row.get(9)?,
                    device_id: row.get(10)?,
                    schema_version: row.get(11)?,
                    created_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(segments)
    }

    fn get_label(&self, title_hash: &str) -> StorageResult<Option<Label>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Priority: user > ai > heuristic
        let label = conn
            .query_row(
                r#"
                SELECT title_hash, category, source, confidence, updated_at
                FROM labels
                WHERE title_hash = ?1
                ORDER BY
                    CASE source
                        WHEN 'manual' THEN 0
                        WHEN 'user' THEN 1
                        WHEN 'ai' THEN 2
                        WHEN 'heuristic' THEN 3
                        ELSE 4
                    END
                LIMIT 1
                "#,
                params![title_hash],
                |row| {
                    let source_str: String = row.get(2)?;
                    Ok(Label {
                        title_hash: row.get(0)?,
                        category: row.get(1)?,
                        source: LabelSource::from_str(&source_str).unwrap_or(LabelSource::Heuristic),
                        confidence: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()?;

        Ok(label)
    }

    fn get_labels(&self, title_hash: &str) -> StorageResult<Vec<Label>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT title_hash, category, source, confidence, updated_at
            FROM labels
            WHERE title_hash = ?1
            "#,
        )?;

        let labels = stmt
            .query_map(params![title_hash], |row| {
                let source_str: String = row.get(2)?;
                Ok(Label {
                    title_hash: row.get(0)?,
                    category: row.get(1)?,
                    source: LabelSource::from_str(&source_str).unwrap_or(LabelSource::Heuristic),
                    confidence: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(labels)
    }

    fn upsert_label(&self, label: &Label) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute(
            r#"
            INSERT INTO labels (title_hash, category, source, confidence, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(title_hash, source) DO UPDATE SET
                category = excluded.category,
                confidence = excluded.confidence,
                updated_at = excluded.updated_at
            "#,
            params![
                label.title_hash,
                label.category,
                label.source.as_str(),
                label.confidence,
                label.updated_at,
            ],
        )?;

        Ok(())
    }

    fn get_daily_summary(&self, date: &str) -> StorageResult<DailySummary> {
        // Get configured day start hour
        let day_start_hour = self.get_day_start_hour()?;

        // Parse date to get start/end timestamps using configured day start hour
        let parsed_date =
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|e| e.to_string())?;
        let start_of_day = parsed_date.and_hms_opt(day_start_hour, 0, 0).unwrap();

        let local_offset = *chrono::Local::now().offset();
        let start_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
            start_of_day - local_offset,
            local_offset,
        );
        let end_dt = start_dt + chrono::Duration::days(1);

        let start_ms = start_dt.timestamp_millis();
        let end_ms = end_dt.timestamp_millis();

        let app_summaries = self.get_app_breakdown(start_ms, end_ms)?;

        let total_duration_ms: i64 = app_summaries.iter().map(|s| s.total_duration_ms).sum();
        let total_idle_ms: i64 = app_summaries.iter().map(|s| s.idle_duration_ms).sum();

        // Calculate category breakdown
        let mut category_totals: HashMap<String, i64> = HashMap::new();
        for summary in &app_summaries {
            if let Some(cat) = &summary.primary_category {
                *category_totals.entry(cat.clone()).or_default() += summary.total_duration_ms;
            }
        }
        let category_breakdown: Vec<(String, i64)> = category_totals.into_iter().collect();

        Ok(DailySummary {
            date: date.to_string(),
            total_duration_ms,
            total_idle_ms,
            app_summaries,
            category_breakdown,
        })
    }

    fn get_app_breakdown(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<AppSummary>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // First, get basic app stats
        let mut stmt = conn.prepare(
            r#"
            SELECT
                s.app_name,
                SUM(s.end_time - s.start_time) as total_duration_ms,
                SUM(s.idle_seconds * 1000) as total_idle_ms,
                COUNT(*) as segment_count,
                SUM(s.keystrokes) as total_keystrokes,
                SUM(s.mouse_clicks) as total_clicks
            FROM segments s
            WHERE s.start_time >= ?1 AND s.start_time < ?2
            GROUP BY s.app_name
            ORDER BY total_duration_ms DESC
            "#,
        )?;

        let mut summaries: Vec<AppSummary> = stmt
            .query_map(params![start_ms, end_ms], |row| {
                let app_name: String = row.get(0)?;
                Ok(AppSummary {
                    friendly_name: utils::to_friendly_name(&app_name),
                    app_name,
                    total_duration_ms: row.get(1)?,
                    idle_duration_ms: row.get(2)?,
                    segment_count: row.get(3)?,
                    keystrokes: row.get(4)?,
                    mouse_clicks: row.get(5)?,
                    primary_category: None, // Will compute below
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Now compute dominant category by duration for each app
        // Use a subquery to get the best label per title_hash (user > ai > heuristic)
        let mut cat_stmt = conn.prepare(
            r#"
            WITH best_labels AS (
                SELECT title_hash, category,
                       ROW_NUMBER() OVER (
                           PARTITION BY title_hash
                           ORDER BY CASE source
                               WHEN 'manual' THEN 0
                               WHEN 'user' THEN 1
                               WHEN 'ai' THEN 2
                               WHEN 'heuristic' THEN 3
                               ELSE 4
                           END
                       ) as rn
                FROM labels
            )
            SELECT bl.category, SUM(s.end_time - s.start_time) as cat_duration
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time >= ?1 AND s.start_time < ?2
              AND s.app_name = ?3
              AND bl.category IS NOT NULL
            GROUP BY bl.category
            ORDER BY cat_duration DESC
            LIMIT 1
            "#,
        )?;

        for summary in &mut summaries {
            if let Ok(category) = cat_stmt.query_row(
                params![start_ms, end_ms, &summary.app_name],
                |row| row.get::<_, String>(0),
            ) {
                summary.primary_category = Some(category);
            }
        }

        Ok(summaries)
    }

    fn get_segment_category_breakdown(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<(String, i64, i64)>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Aggregate duration by segment-level category (using best label per title_hash)
        // This properly handles browsers where YouTube=entertainment and Overleaf=productivity
        // Returns (category, total_ms, idle_ms)
        let mut stmt = conn.prepare(
            r#"
            WITH best_labels AS (
                SELECT title_hash, category,
                       ROW_NUMBER() OVER (
                           PARTITION BY title_hash
                           ORDER BY CASE source
                               WHEN 'manual' THEN 0
                               WHEN 'user' THEN 1
                               WHEN 'ai' THEN 2
                               WHEN 'heuristic' THEN 3
                               ELSE 4
                           END
                       ) as rn
                FROM labels
            )
            SELECT COALESCE(bl.category, 'unknown') as cat,
                   SUM(s.end_time - s.start_time) as total_ms,
                   SUM(s.idle_seconds * 1000) as idle_ms
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time >= ?1 AND s.start_time < ?2
            GROUP BY cat
            ORDER BY total_ms DESC
            "#,
        )?;

        let categories = stmt
            .query_map(params![start_ms, end_ms], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2).unwrap_or(0),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(categories)
    }

    fn get_app_contexts(
        &self,
        app_name: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<ContextSummary>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Get all segments for this app in the time range
        // Use a CTE to get the best label per title_hash (user > ai > heuristic)
        let mut stmt = conn.prepare(
            r#"
            WITH best_labels AS (
                SELECT title_hash, category,
                       ROW_NUMBER() OVER (
                           PARTITION BY title_hash
                           ORDER BY CASE source
                               WHEN 'manual' THEN 0
                               WHEN 'user' THEN 1
                               WHEN 'ai' THEN 2
                               WHEN 'heuristic' THEN 3
                               ELSE 4
                           END
                       ) as rn
                FROM labels
            )
            SELECT s.window_title, s.title_hash, s.end_time - s.start_time as duration_ms,
                   s.idle_seconds * 1000 as idle_ms, bl.category
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.app_name = ?1 AND s.start_time >= ?2 AND s.start_time < ?3
            ORDER BY s.start_time DESC
            "#,
        )?;

        // Aggregate by extracted context, tracking category durations
        struct ContextData {
            total_duration_ms: i64,
            idle_duration_ms: i64,
            segment_count: u32,
            sample_titles: Vec<String>,
            category_durations: HashMap<String, i64>, // Track duration per category
        }
        let mut context_map: HashMap<String, ContextData> = HashMap::new();

        let rows = stmt.query_map(params![app_name, start_ms, end_ms], |row| {
            let title: Option<String> = row.get(0)?;
            let duration_ms: i64 = row.get(2)?;
            let idle_ms: i64 = row.get(3)?;
            let category: Option<String> = row.get(4)?;
            Ok((title, duration_ms, idle_ms, category))
        })?;

        for row in rows {
            let (title, duration_ms, idle_ms, category) = row?;
            let title_str = title.as_deref().unwrap_or("");

            // Extract context from title
            let context = utils::extract_context(app_name, title_str)
                .unwrap_or_else(|| "other".to_string());

            let entry = context_map.entry(context).or_insert_with(|| ContextData {
                total_duration_ms: 0,
                idle_duration_ms: 0,
                segment_count: 0,
                sample_titles: Vec::new(),
                category_durations: HashMap::new(),
            });

            entry.total_duration_ms += duration_ms;
            entry.idle_duration_ms += idle_ms;
            entry.segment_count += 1;

            // Track duration per category for this context
            if let Some(ref cat) = category {
                *entry.category_durations.entry(cat.clone()).or_default() += duration_ms;
            }

            // Collect sample titles (up to 3 unique)
            if let Some(ref t) = title {
                if entry.sample_titles.len() < 3 && !entry.sample_titles.contains(t) {
                    entry.sample_titles.push(t.clone());
                }
            }
        }

        // Convert to vec, picking dominant category by duration
        let mut contexts: Vec<ContextSummary> = context_map
            .into_iter()
            .map(|(ctx, data)| {
                // Pick category with max duration
                let category = data
                    .category_durations
                    .into_iter()
                    .max_by_key(|(_, dur)| *dur)
                    .map(|(cat, _)| cat);

                ContextSummary {
                    context: ctx,
                    category,
                    total_duration_ms: data.total_duration_ms,
                    idle_duration_ms: data.idle_duration_ms,
                    segment_count: data.segment_count,
                    sample_titles: data.sample_titles,
                }
            })
            .collect();
        contexts.sort_by(|a, b| b.total_duration_ms.cmp(&a.total_duration_ms));

        Ok(contexts)
    }

    fn get_today_active_ms(&self) -> StorageResult<i64> {
        let day_start_hour = self.get_day_start_hour()?;
        let start_ms = utils::today_start_ms_with_hour(day_start_hour);

        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(end_time - start_time), 0) FROM segments WHERE start_time >= ?1",
            params![start_ms],
            |row| row.get(0),
        )?;

        Ok(total)
    }

    fn get_config(&self, key: &str) -> StorageResult<Option<String>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let value = conn
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        Ok(value)
    }

    fn set_config(&self, key: &str, value: &str) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;

        Ok(())
    }

    // === Classification Rules ===

    fn get_rules(&self) -> StorageResult<Vec<ClassificationRule>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT rule_id, app_pattern, title_pattern, match_type, category,
                   tags, source, priority, enabled, created_at
            FROM classification_rules
            WHERE enabled = 1
            ORDER BY
                -- Primary: effective priority (source score + priority)
                CASE source
                    WHEN 'user' THEN 100
                    WHEN 'ai-approved' THEN 50
                    WHEN 'builtin' THEN 0
                    ELSE 0
                END + priority DESC,
                -- Tie-breaker 1: specificity (longer patterns are more specific)
                LENGTH(COALESCE(app_pattern, '')) + LENGTH(COALESCE(title_pattern, '')) DESC,
                -- Tie-breaker 2: newer rules win
                created_at DESC
            "#,
        )?;

        let rules = stmt
            .query_map([], |row| {
                let tags_json: Option<String> = row.get(5)?;
                let tags: Option<Vec<String>> = tags_json
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok());

                let match_type_str: String = row.get(3)?;
                let source_str: String = row.get(6)?;

                Ok(ClassificationRule {
                    rule_id: row.get(0)?,
                    app_pattern: row.get(1)?,
                    title_pattern: row.get(2)?,
                    match_type: MatchType::from_str(&match_type_str),
                    category: row.get(4)?,
                    tags,
                    source: RuleSource::from_str(&source_str),
                    priority: row.get(7)?,
                    enabled: row.get::<_, i32>(8)? != 0,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rules)
    }

    fn get_all_rules(&self) -> StorageResult<Vec<ClassificationRule>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Return ALL rules (including disabled) for UI management
        let mut stmt = conn.prepare(
            r#"
            SELECT rule_id, app_pattern, title_pattern, match_type, category,
                   tags, source, priority, enabled, created_at
            FROM classification_rules
            ORDER BY
                -- Primary: effective priority (source score + priority)
                CASE source
                    WHEN 'user' THEN 100
                    WHEN 'ai-approved' THEN 50
                    WHEN 'builtin' THEN 0
                    ELSE 0
                END + priority DESC,
                -- Tie-breaker 1: specificity (longer patterns are more specific)
                LENGTH(COALESCE(app_pattern, '')) + LENGTH(COALESCE(title_pattern, '')) DESC,
                -- Tie-breaker 2: newer rules win
                created_at DESC
            "#,
        )?;

        let rules = stmt
            .query_map([], |row| {
                let tags_json: Option<String> = row.get(5)?;
                let tags: Option<Vec<String>> = tags_json
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok());

                let match_type_str: String = row.get(3)?;
                let source_str: String = row.get(6)?;

                Ok(ClassificationRule {
                    rule_id: row.get(0)?,
                    app_pattern: row.get(1)?,
                    title_pattern: row.get(2)?,
                    match_type: MatchType::from_str(&match_type_str),
                    category: row.get(4)?,
                    tags,
                    source: RuleSource::from_str(&source_str),
                    priority: row.get(7)?,
                    enabled: row.get::<_, i32>(8)? != 0,
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rules)
    }

    fn get_rule(&self, rule_id: &str) -> StorageResult<Option<ClassificationRule>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let rule = conn
            .query_row(
                r#"
                SELECT rule_id, app_pattern, title_pattern, match_type, category,
                       tags, source, priority, enabled, created_at
                FROM classification_rules
                WHERE rule_id = ?1
                "#,
                params![rule_id],
                |row| {
                    let tags_json: Option<String> = row.get(5)?;
                    let tags: Option<Vec<String>> = tags_json
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok());

                    let match_type_str: String = row.get(3)?;
                    let source_str: String = row.get(6)?;

                    Ok(ClassificationRule {
                        rule_id: row.get(0)?,
                        app_pattern: row.get(1)?,
                        title_pattern: row.get(2)?,
                        match_type: MatchType::from_str(&match_type_str),
                        category: row.get(4)?,
                        tags,
                        source: RuleSource::from_str(&source_str),
                        priority: row.get(7)?,
                        enabled: row.get::<_, i32>(8)? != 0,
                        created_at: row.get(9)?,
                    })
                },
            )
            .optional()?;

        Ok(rule)
    }

    fn upsert_rule(&self, rule: &ClassificationRule) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let tags_json = rule.tags.as_ref().map(|t| serde_json::to_string(t).ok()).flatten();

        conn.execute(
            r#"
            INSERT INTO classification_rules (
                rule_id, app_pattern, title_pattern, match_type, category,
                tags, source, priority, enabled, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(rule_id) DO UPDATE SET
                app_pattern = excluded.app_pattern,
                title_pattern = excluded.title_pattern,
                match_type = excluded.match_type,
                category = excluded.category,
                tags = excluded.tags,
                source = excluded.source,
                priority = excluded.priority,
                enabled = excluded.enabled
            "#,
            params![
                rule.rule_id,
                rule.app_pattern,
                rule.title_pattern,
                rule.match_type.as_str(),
                rule.category,
                tags_json,
                rule.source.as_str(),
                rule.priority,
                rule.enabled as i32,
                rule.created_at,
            ],
        )?;

        Ok(())
    }

    fn delete_rule(&self, rule_id: &str) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute(
            "DELETE FROM classification_rules WHERE rule_id = ?1",
            params![rule_id],
        )?;

        Ok(())
    }

    fn find_matching_rule(
        &self,
        app_name: &str,
        window_title: &str,
    ) -> StorageResult<Option<ClassificationRule>> {
        // Get all enabled rules sorted by priority
        let rules = self.get_rules()?;

        // Find first matching rule (already sorted by priority)
        for rule in rules {
            if rule.matches(app_name, window_title) {
                return Ok(Some(rule));
            }
        }

        Ok(None)
    }

    fn backfill_labels_for_rule(&self, rule: &ClassificationRule, days_back: u32) -> StorageResult<u32> {
        use crate::utils::{day_range_ms_with_offset, now_ms, DEFAULT_DAY_START_HOUR};
        use std::collections::HashSet;

        // Get the day start hour from config (or use default)
        let day_start_hour = self.get_day_start_hour().unwrap_or(DEFAULT_DAY_START_HOUR);

        // Calculate time range (last N days)
        let (start_ms, _) = day_range_ms_with_offset(day_start_hour, -(days_back as i32));
        let end_ms = now_ms();

        // Get all segments in range
        let segments = self.get_segments_range(start_ms, end_ms)?;

        let mut updated_count = 0u32;
        let mut processed_hashes = HashSet::new();

        for segment in segments {
            // Skip already processed title_hashes
            if processed_hashes.contains(&segment.title_hash) {
                continue;
            }

            // Check if this segment matches the rule
            let window_title = segment.window_title.as_deref().unwrap_or("");
            if !rule.matches(&segment.app_name, window_title) {
                continue;
            }

            // Check if there's already a manual label (don't overwrite)
            if let Ok(Some(existing)) = self.get_label(&segment.title_hash) {
                if existing.source == LabelSource::Manual {
                    processed_hashes.insert(segment.title_hash.clone());
                    continue;
                }
            }

            // Create/update label with source='user' (from rule)
            let label = Label {
                title_hash: segment.title_hash.clone(),
                category: rule.category.clone(),
                source: LabelSource::User,
                confidence: None,
                updated_at: now_ms(),
            };

            if self.upsert_label(&label).is_ok() {
                updated_count += 1;
            }

            processed_hashes.insert(segment.title_hash);
        }

        Ok(updated_count)
    }

    // === AI Suggestions ===

    fn get_pending_suggestions(&self) -> StorageResult<Vec<AiSuggestion>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT suggestion_id, app_pattern, title_pattern, match_type,
                   suggested_category, confidence, reason, sample_titles,
                   match_count, total_duration_ms, status, created_at, reviewed_at
            FROM ai_suggestions
            WHERE status = 'pending'
            ORDER BY confidence DESC, total_duration_ms DESC
            "#,
        )?;

        let suggestions = stmt
            .query_map([], |row| {
                let sample_titles_json: Option<String> = row.get(7)?;
                let sample_titles: Vec<String> = sample_titles_json
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                let match_type_str: String = row.get(3)?;
                let status_str: String = row.get(10)?;

                Ok(AiSuggestion {
                    suggestion_id: row.get(0)?,
                    app_pattern: row.get(1)?,
                    title_pattern: row.get(2)?,
                    match_type: MatchType::from_str(&match_type_str),
                    suggested_category: row.get(4)?,
                    confidence: row.get(5)?,
                    reason: row.get(6)?,
                    sample_titles,
                    match_count: row.get(8)?,
                    total_duration_ms: row.get(9)?,
                    status: SuggestionStatus::from_str(&status_str),
                    created_at: row.get(11)?,
                    reviewed_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(suggestions)
    }

    fn get_suggestion(&self, suggestion_id: &str) -> StorageResult<Option<AiSuggestion>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let suggestion = conn
            .query_row(
                r#"
                SELECT suggestion_id, app_pattern, title_pattern, match_type,
                       suggested_category, confidence, reason, sample_titles,
                       match_count, total_duration_ms, status, created_at, reviewed_at
                FROM ai_suggestions
                WHERE suggestion_id = ?1
                "#,
                params![suggestion_id],
                |row| {
                    let sample_titles_json: Option<String> = row.get(7)?;
                    let sample_titles: Vec<String> = sample_titles_json
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();

                    let match_type_str: String = row.get(3)?;
                    let status_str: String = row.get(10)?;

                    Ok(AiSuggestion {
                        suggestion_id: row.get(0)?,
                        app_pattern: row.get(1)?,
                        title_pattern: row.get(2)?,
                        match_type: MatchType::from_str(&match_type_str),
                        suggested_category: row.get(4)?,
                        confidence: row.get(5)?,
                        reason: row.get(6)?,
                        sample_titles,
                        match_count: row.get(8)?,
                        total_duration_ms: row.get(9)?,
                        status: SuggestionStatus::from_str(&status_str),
                        created_at: row.get(11)?,
                        reviewed_at: row.get(12)?,
                    })
                },
            )
            .optional()?;

        Ok(suggestion)
    }

    fn insert_suggestion(&self, suggestion: &AiSuggestion) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let sample_titles_json = serde_json::to_string(&suggestion.sample_titles).ok();

        conn.execute(
            r#"
            INSERT INTO ai_suggestions (
                suggestion_id, app_pattern, title_pattern, match_type,
                suggested_category, confidence, reason, sample_titles,
                match_count, total_duration_ms, status, created_at, reviewed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                suggestion.suggestion_id,
                suggestion.app_pattern,
                suggestion.title_pattern,
                suggestion.match_type.as_str(),
                suggestion.suggested_category,
                suggestion.confidence,
                suggestion.reason,
                sample_titles_json,
                suggestion.match_count,
                suggestion.total_duration_ms,
                suggestion.status.as_str(),
                suggestion.created_at,
                suggestion.reviewed_at,
            ],
        )?;

        Ok(())
    }

    fn update_suggestion_status(
        &self,
        suggestion_id: &str,
        status: SuggestionStatus,
    ) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE ai_suggestions SET status = ?1, reviewed_at = ?2 WHERE suggestion_id = ?3",
            params![status.as_str(), now, suggestion_id],
        )?;

        Ok(())
    }

    fn cleanup_old_suggestions(&self, max_age_days: u32) -> StorageResult<u32> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let cutoff_ms = chrono::Utc::now().timestamp_millis()
            - (max_age_days as i64 * 24 * 60 * 60 * 1000);

        let deleted = conn.execute(
            "DELETE FROM ai_suggestions WHERE created_at < ?1 AND status IN ('rejected', 'expired')",
            params![cutoff_ms],
        )?;

        Ok(deleted as u32)
    }

    fn run_migrations(&self) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Create schema_migrations table first if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Check current version
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Run migrations
        if current_version < 1 {
            Self::run_migration_v001(&conn)?;

            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![1, now],
            )?;
        }

        if current_version < 2 {
            Self::run_migration_v002(&conn)?;

            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![2, now],
            )?;
        }

        if current_version < 3 {
            Self::run_migration_v003(&conn)?;

            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![3, now],
            )?;
        }

        Ok(())
    }

    fn get_schema_version(&self) -> StorageResult<i32> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(version)
    }
}
