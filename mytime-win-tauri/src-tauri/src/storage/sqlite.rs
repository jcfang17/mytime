//! SQLite storage implementation for MyTime

use crate::models::{
    AiSuggestion, AppSummary, BootstrapConfig, ClassificationRule, ContextSummary, DailyDigest,
    DataLocation, DigestAppEntry, DigestCategoryEntry, DigestFocusBlock,
    DigestIdleEntry, Label, LabelProvenance, LabelSource, MatchType, RuleSource, Segment,
    SelectedBreakdownRow, SuggestionStatus, TimelineSegment, UnknownQueueItem,
};
use crate::storage::{StorageAdapter, StorageResult};
use crate::utils;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// SQLite storage implementation
pub struct SqliteStorage {
    conn: Mutex<Connection>,
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
            WHERE start_time < ?2 AND end_time > ?1
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

    fn get_app_breakdown(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<AppSummary>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // First, get basic app stats
        // Use overlap-aware filtering: include segments that cross day boundaries
        // and clamp durations to the query window [start_ms, end_ms).
        let mut stmt = conn.prepare(
            r#"
            SELECT
                s.app_name,
                SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as total_duration_ms,
                SUM(CAST(s.idle_seconds * 1000.0
                    * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                    / MAX(s.end_time - s.start_time, 1) AS INTEGER)) as total_idle_ms,
                COUNT(*) as segment_count,
                SUM(s.keystrokes) as total_keystrokes,
                SUM(s.mouse_clicks) as total_clicks
            FROM segments s
            WHERE s.start_time < ?2 AND s.end_time > ?1
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
            SELECT bl.category, SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as cat_duration
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
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
                   SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as total_ms,
                   SUM(CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER)) as idle_ms
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
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
            SELECT s.window_title, s.title_hash,
                   MIN(s.end_time, ?3) - MAX(s.start_time, ?2) as duration_ms,
                   CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?3) - MAX(s.start_time, ?2))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER) as idle_ms,
                   bl.category
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.app_name = ?1 AND s.start_time < ?3 AND s.end_time > ?2
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

    fn get_selected_breakdown(
        &self,
        start_ms: i64,
        end_ms: i64,
        categories: &[String],
    ) -> StorageResult<Vec<SelectedBreakdownRow>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let wanted: HashSet<String> = categories.iter().map(|c| c.to_lowercase()).collect();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch segments with their best category label (manual > user > ai > heuristic).
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
            SELECT s.app_name,
                   s.window_title,
                   (MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as duration_ms,
                   CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER) as idle_ms,
                   COALESCE(bl.category, 'unknown') as category
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
            "#,
        )?;

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        struct GroupKey {
            app_name: String,
            context: Option<String>,
            category: String,
        }

        #[derive(Debug, Clone)]
        struct GroupData {
            total_duration_ms: i64,
            idle_duration_ms: i64,
            segment_count: u32,
        }

        let mut groups: HashMap<GroupKey, GroupData> = HashMap::new();

        let rows = stmt.query_map(params![start_ms, end_ms], |row| {
            let app_name: String = row.get(0)?;
            let window_title: Option<String> = row.get(1)?;
            let duration_ms: i64 = row.get(2)?;
            let idle_ms: i64 = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
            let category: String = row.get(4)?;
            Ok((app_name, window_title, duration_ms, idle_ms, category))
        })?;

        for row in rows {
            let (app_name, window_title, duration_ms, idle_ms, category) = row?;
            let category = category.to_lowercase();

            if !wanted.contains(&category) {
                continue;
            }

            let title_str = window_title.as_deref().unwrap_or("");
            let is_browser = utils::is_browser(&app_name);
            let context = if is_browser {
                Some(
                    utils::extract_context(&app_name, title_str)
                        .unwrap_or_else(|| "other".to_string()),
                )
            } else {
                None
            };

            let key = GroupKey {
                app_name,
                context,
                category,
            };

            let entry = groups.entry(key).or_insert(GroupData {
                total_duration_ms: 0,
                idle_duration_ms: 0,
                segment_count: 0,
            });

            entry.total_duration_ms += duration_ms;
            entry.idle_duration_ms += idle_ms;
            entry.segment_count += 1;
        }

        let mut out: Vec<SelectedBreakdownRow> = groups
            .into_iter()
            .map(|(key, data)| SelectedBreakdownRow {
                friendly_name: utils::to_friendly_name(&key.app_name),
                app_name: key.app_name,
                context: key.context,
                category: key.category,
                total_duration_ms: data.total_duration_ms,
                idle_duration_ms: data.idle_duration_ms,
                segment_count: data.segment_count,
            })
            .collect();

        out.sort_by(|a, b| b.total_duration_ms.cmp(&a.total_duration_ms));
        Ok(out)
    }

    fn get_today_total_ms(&self) -> StorageResult<i64> {
        let day_start_hour = self.get_day_start_hour()?;
        let start_ms = utils::today_start_ms_with_hour(day_start_hour);

        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // Include segments that overlap today's boundary and clamp their start
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(end_time - MAX(start_time, ?1)), 0) FROM segments WHERE end_time > ?1",
            params![start_ms],
            |row| row.get(0),
        )?;

        Ok(total)
    }

    fn get_timeline_segments(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<TimelineSegment>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

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
            SELECT s.segment_id, s.app_name, s.window_title, s.title_hash,
                   MAX(s.start_time, ?1) as clamped_start,
                   MIN(s.end_time, ?2) as clamped_end,
                   COALESCE(bl.category, 'unknown') as category,
                   CAST(s.idle_seconds
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER) as clamped_idle
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
            ORDER BY clamped_start ASC
            "#,
        )?;

        let segments = stmt
            .query_map(params![start_ms, end_ms], |row| {
                let app_name: String = row.get(1)?;
                Ok(TimelineSegment {
                    segment_id: row.get(0)?,
                    friendly_name: utils::to_friendly_name(&app_name),
                    app_name,
                    window_title: row.get(2)?,
                    title_hash: row.get(3)?,
                    start_time: row.get(4)?,
                    end_time: row.get(5)?,
                    category: row.get(6)?,
                    idle_seconds: row.get::<_, i64>(7).unwrap_or(0) as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(segments)
    }

    fn get_unknown_queue(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<UnknownQueueItem>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

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
            SELECT s.app_name,
                   s.window_title,
                   (MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as duration_ms,
                   CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER) as idle_ms
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
              AND (bl.category IS NULL OR bl.category = 'unknown')
            ORDER BY duration_ms DESC
            "#,
        )?;

        struct QueueData {
            total_duration_ms: i64,
            idle_duration_ms: i64,
            segment_count: u32,
            sample_titles: Vec<String>,
        }

        let mut grouped: HashMap<(String, Option<String>), QueueData> = HashMap::new();

        let rows = stmt.query_map(params![start_ms, end_ms], |row| {
            let app_name: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let duration_ms: i64 = row.get(2)?;
            let idle_ms: i64 = row.get(3)?;
            Ok((app_name, title, duration_ms, idle_ms))
        })?;

        for row in rows {
            let (app_name, title, duration_ms, idle_ms) = row?;
            let title_str = title.as_deref().unwrap_or("");

            let context = if utils::is_browser(&app_name) {
                Some(utils::extract_context(&app_name, title_str)
                    .unwrap_or_else(|| "other".to_string()))
            } else {
                None
            };

            let key = (app_name, context);
            let entry = grouped.entry(key).or_insert_with(|| QueueData {
                total_duration_ms: 0,
                idle_duration_ms: 0,
                segment_count: 0,
                sample_titles: Vec::new(),
            });

            entry.total_duration_ms += duration_ms;
            entry.idle_duration_ms += idle_ms;
            entry.segment_count += 1;

            if let Some(ref t) = title {
                if entry.sample_titles.len() < 3 && !entry.sample_titles.contains(t) {
                    entry.sample_titles.push(t.clone());
                }
            }
        }

        let mut items: Vec<UnknownQueueItem> = grouped
            .into_iter()
            .filter(|(_, data)| data.total_duration_ms >= 5000)
            .map(|((app_name, context), data)| UnknownQueueItem {
                friendly_name: utils::to_friendly_name(&app_name),
                app_name,
                context,
                total_duration_ms: data.total_duration_ms,
                idle_duration_ms: data.idle_duration_ms,
                segment_count: data.segment_count,
                sample_titles: data.sample_titles,
            })
            .collect();

        items.sort_by(|a, b| b.total_duration_ms.cmp(&a.total_duration_ms));

        Ok(items)
    }

    fn get_daily_digest(&self, start_ms: i64, end_ms: i64) -> StorageResult<DailyDigest> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        // 1. Category breakdown (reuse same CTE pattern)
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
            SELECT COALESCE(bl.category, 'unknown') as category,
                   SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as total_ms,
                   SUM(CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER)) as idle_ms
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
            GROUP BY COALESCE(bl.category, 'unknown')
            ORDER BY total_ms DESC
            "#,
        )?;

        let cat_rows = cat_stmt.query_map(params![start_ms, end_ms], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })?;

        let mut total_tracked_ms: i64 = 0;
        let mut total_idle_ms: i64 = 0;
        let mut all_categories: Vec<(String, i64, i64)> = Vec::new(); // (cat, total_ms, idle_ms)

        for row in cat_rows {
            let (cat, total_ms, idle_ms) = row?;
            total_tracked_ms += total_ms;
            total_idle_ms += idle_ms;
            all_categories.push((cat, total_ms, idle_ms));
        }

        let top_categories: Vec<DigestCategoryEntry> = all_categories
            .iter()
            .take(3)
            .map(|(cat, ms, idle)| DigestCategoryEntry {
                category: cat.clone(),
                duration_ms: *ms,
                idle_ms: *idle,
                percentage: if total_tracked_ms > 0 {
                    (*ms as f64 / total_tracked_ms as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        // 2. Top 5 apps by duration
        let mut app_stmt = conn.prepare(
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
            SELECT s.app_name,
                   SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as total_ms,
                   SUM(CAST(s.idle_seconds * 1000.0
                       * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       / MAX(s.end_time - s.start_time, 1) AS INTEGER)) as idle_ms,
                   (SELECT bl2.category FROM best_labels bl2
                    WHERE bl2.title_hash = s.title_hash AND bl2.rn = 1
                    LIMIT 1) as category
            FROM segments s
            LEFT JOIN best_labels bl ON bl.title_hash = s.title_hash AND bl.rn = 1
            WHERE s.start_time < ?2 AND s.end_time > ?1
            GROUP BY s.app_name
            ORDER BY total_ms DESC
            LIMIT 5
            "#,
        )?;

        let top_apps: Vec<DigestAppEntry> = app_stmt
            .query_map(params![start_ms, end_ms], |row| {
                let app_name: String = row.get(0)?;
                Ok(DigestAppEntry {
                    friendly_name: utils::to_friendly_name(&app_name),
                    app_name,
                    duration_ms: row.get(1)?,
                    idle_ms: row.get(2)?,
                    category: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // 3. Longest focus block (group by focus_session_id, subtract idle)
        let longest_focus: Option<DigestFocusBlock> = conn
            .query_row(
                r#"
                SELECT s.app_name,
                       SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                       - SUM(CAST(s.idle_seconds * 1000.0
                           * (MIN(s.end_time, ?2) - MAX(s.start_time, ?1))
                           / MAX(s.end_time - s.start_time, 1) AS INTEGER)) as active_ms
                FROM segments s
                WHERE s.start_time < ?2 AND s.end_time > ?1
                GROUP BY s.focus_session_id
                HAVING active_ms > 0
                ORDER BY active_ms DESC
                LIMIT 1
                "#,
                params![start_ms, end_ms],
                |row| {
                    let app_name: String = row.get(0)?;
                    Ok(DigestFocusBlock {
                        friendly_name: utils::to_friendly_name(&app_name),
                        app_name,
                        duration_ms: row.get(1)?,
                    })
                },
            )
            .optional()?;

        // 4. Most idle segment
        let most_idle: Option<DigestIdleEntry> = conn
            .query_row(
                r#"
                SELECT s.app_name, COALESCE(s.window_title, ''),
                       s.idle_seconds,
                       (MIN(s.end_time, ?2) - MAX(s.start_time, ?1)) as duration_ms
                FROM segments s
                WHERE s.start_time < ?2 AND s.end_time > ?1
                  AND s.idle_seconds > 0
                ORDER BY s.idle_seconds DESC
                LIMIT 1
                "#,
                params![start_ms, end_ms],
                |row| {
                    let app_name: String = row.get(0)?;
                    Ok(DigestIdleEntry {
                        friendly_name: utils::to_friendly_name(&app_name),
                        app_name,
                        window_title: row.get(1)?,
                        idle_seconds: row.get::<_, i64>(2)? as u64,
                        duration_ms: row.get(3)?,
                    })
                },
            )
            .optional()?;

        Ok(DailyDigest {
            total_tracked_ms,
            total_active_ms: total_tracked_ms - total_idle_ms,
            top_categories,
            top_apps,
            longest_focus,
            most_idle,
        })
    }

    fn get_label_provenance(&self, title_hash: &str) -> StorageResult<LabelProvenance> {
        // Get the best label for this title_hash
        let best_label = self.get_label(title_hash)?;

        // If label is from a rule source, try to find which rule matched
        let matching_rule = if let Some(ref label) = best_label {
            match label.source {
                LabelSource::User | LabelSource::Ai => {
                    // Look up a segment with this title_hash to get app_name + window_title
                    let conn = self.conn.lock().map_err(|e| e.to_string())?;
                    let seg_info: Option<(String, Option<String>)> = conn
                        .query_row(
                            "SELECT app_name, window_title FROM segments WHERE title_hash = ?1 LIMIT 1",
                            params![title_hash],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .optional()?;

                    if let Some((app_name, window_title)) = seg_info {
                        let title = window_title.as_deref().unwrap_or("");
                        self.find_matching_rule(&app_name, title)?
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        Ok(LabelProvenance {
            best_label,
            matching_rule,
        })
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

}

#[cfg(test)]
impl SqliteStorage {
    /// Test-only constructor backed by an in-memory SQLite connection.
    /// Skips bootstrap config / data-dir resolution so tests don't touch the filesystem.
    pub(crate) fn new_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        storage.ensure_device_id()?;
        Ok(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LabelSource, Segment};

    const HOUR_MS: i64 = 3_600_000;

    fn make_segment(
        app: &str,
        title_hash: &str,
        start_ms: i64,
        end_ms: i64,
        idle_seconds: u64,
    ) -> Segment {
        Segment {
            segment_id: uuid::Uuid::new_v4().to_string(),
            app_name: app.to_string(),
            window_title: Some(format!("{}-title", title_hash)),
            title_hash: title_hash.to_string(),
            start_time: start_ms,
            end_time: end_ms,
            idle_seconds,
            keystrokes: 0,
            mouse_clicks: 0,
            focus_session_id: "test-session".to_string(),
            device_id: None,
            schema_version: 1,
            created_at: end_ms,
        }
    }

    fn make_label(title_hash: &str, category: &str, source: LabelSource) -> Label {
        Label {
            title_hash: title_hash.to_string(),
            category: category.to_string(),
            source,
            confidence: None,
            updated_at: 0,
        }
    }

    /// Window: [6h, 30h). Segment from 5h to 7h with 60s idle over 2h total.
    /// Expect: duration clamped to 1h (6h–7h); idle pro-rated to 30s.
    #[test]
    fn app_breakdown_clamps_segment_overlapping_window_start() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let day_start = 6 * HOUR_MS;
        let day_end = 30 * HOUR_MS;

        let seg = make_segment("notepad.exe", "h1", 5 * HOUR_MS, 7 * HOUR_MS, 60);
        storage.insert_segment(&seg).unwrap();

        let breakdown = storage.get_app_breakdown(day_start, day_end).unwrap();
        assert_eq!(breakdown.len(), 1);
        assert_eq!(breakdown[0].app_name, "notepad.exe");
        assert_eq!(breakdown[0].total_duration_ms, HOUR_MS);
        // 60s * 1000 * (1h / 2h) = 30_000ms
        assert_eq!(breakdown[0].idle_duration_ms, 30_000);
    }

    /// Window: [4h, 6h). Segment from 5h to 7h with 60s idle.
    /// Expect: duration clamped to 1h (5h–6h); idle pro-rated to 30s.
    #[test]
    fn app_breakdown_clamps_segment_overlapping_window_end() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 4 * HOUR_MS;
        let win_end = 6 * HOUR_MS;

        let seg = make_segment("code.exe", "h1", 5 * HOUR_MS, 7 * HOUR_MS, 60);
        storage.insert_segment(&seg).unwrap();

        let breakdown = storage.get_app_breakdown(win_start, win_end).unwrap();
        assert_eq!(breakdown.len(), 1);
        assert_eq!(breakdown[0].total_duration_ms, HOUR_MS);
        assert_eq!(breakdown[0].idle_duration_ms, 30_000);
    }

    /// Segments fully before or after the window must not appear.
    #[test]
    fn app_breakdown_excludes_segments_outside_window() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 10 * HOUR_MS;
        let win_end = 12 * HOUR_MS;

        // Fully before
        storage
            .insert_segment(&make_segment("a.exe", "h1", 1 * HOUR_MS, 2 * HOUR_MS, 0))
            .unwrap();
        // Fully after
        storage
            .insert_segment(&make_segment("b.exe", "h2", 20 * HOUR_MS, 21 * HOUR_MS, 0))
            .unwrap();
        // Touching boundary exactly (end == win_start) — overlap predicate is end > start
        storage
            .insert_segment(&make_segment("c.exe", "h3", 9 * HOUR_MS, 10 * HOUR_MS, 0))
            .unwrap();

        let breakdown = storage.get_app_breakdown(win_start, win_end).unwrap();
        assert!(breakdown.is_empty(), "got: {:?}", breakdown);
    }

    /// Segment fully inside window: full duration counted, idle not pro-rated down.
    #[test]
    fn app_breakdown_segment_fully_inside_is_unchanged() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 6 * HOUR_MS;
        let win_end = 30 * HOUR_MS;

        let seg = make_segment("foo.exe", "h1", 8 * HOUR_MS, 10 * HOUR_MS, 120);
        storage.insert_segment(&seg).unwrap();

        let breakdown = storage.get_app_breakdown(win_start, win_end).unwrap();
        assert_eq!(breakdown.len(), 1);
        assert_eq!(breakdown[0].total_duration_ms, 2 * HOUR_MS);
        assert_eq!(breakdown[0].idle_duration_ms, 120 * 1000);
    }

    /// Category breakdown should clamp duration AND prefer manual > heuristic label.
    #[test]
    fn category_breakdown_clamps_and_picks_best_label() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 6 * HOUR_MS;
        let win_end = 30 * HOUR_MS;

        // Segment straddling window start: 5h → 7h, so 1h falls in window
        storage
            .insert_segment(&make_segment("msedge.exe", "h1", 5 * HOUR_MS, 7 * HOUR_MS, 0))
            .unwrap();

        // Heuristic says entertainment, manual says productivity — manual must win
        storage
            .upsert_label(&make_label("h1", "entertainment", LabelSource::Heuristic))
            .unwrap();
        storage
            .upsert_label(&make_label("h1", "productivity", LabelSource::Manual))
            .unwrap();

        let cats = storage
            .get_segment_category_breakdown(win_start, win_end)
            .unwrap();

        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].0, "productivity");
        assert_eq!(cats[0].1, HOUR_MS); // clamped duration
    }

    /// Segments without any label fall through as `unknown`.
    #[test]
    fn category_breakdown_unlabeled_falls_to_unknown() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 0;
        let win_end = 100 * HOUR_MS;

        storage
            .insert_segment(&make_segment("mystery.exe", "h1", 1 * HOUR_MS, 2 * HOUR_MS, 0))
            .unwrap();

        let cats = storage
            .get_segment_category_breakdown(win_start, win_end)
            .unwrap();

        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].0, "unknown");
        assert_eq!(cats[0].1, HOUR_MS);
    }

    /// Multiple segments aggregated by app, with one straddling — totals should sum cleanly.
    #[test]
    fn app_breakdown_aggregates_multiple_segments_per_app() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 6 * HOUR_MS;
        let win_end = 30 * HOUR_MS;

        // Fully inside: 2h
        storage
            .insert_segment(&make_segment("code.exe", "h1", 8 * HOUR_MS, 10 * HOUR_MS, 0))
            .unwrap();
        // Straddling start: contributes 1h (6h–7h)
        storage
            .insert_segment(&make_segment("code.exe", "h2", 5 * HOUR_MS, 7 * HOUR_MS, 0))
            .unwrap();

        let breakdown = storage.get_app_breakdown(win_start, win_end).unwrap();
        assert_eq!(breakdown.len(), 1);
        assert_eq!(breakdown[0].segment_count, 2);
        assert_eq!(breakdown[0].total_duration_ms, 3 * HOUR_MS);
    }

    /// Timeline segments should report clamped start/end, not raw segment bounds.
    #[test]
    fn timeline_segments_clamp_to_window() {
        let storage = SqliteStorage::new_in_memory().unwrap();
        let win_start = 6 * HOUR_MS;
        let win_end = 30 * HOUR_MS;

        storage
            .insert_segment(&make_segment("foo.exe", "h1", 5 * HOUR_MS, 7 * HOUR_MS, 60))
            .unwrap();

        let timeline = storage.get_timeline_segments(win_start, win_end).unwrap();
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].start_time, win_start, "clamped to window start");
        assert_eq!(timeline[0].end_time, 7 * HOUR_MS);
        // idle pro-rated: 60s * (1h/2h) = 30s
        assert_eq!(timeline[0].idle_seconds, 30);
    }
}

