//! SQLite storage implementation for MyTime

use crate::models::{
    AppSummary, BootstrapConfig, DailySummary, DataLocation, Label, LabelSource, Segment,
};
use crate::storage::{StorageAdapter, StorageResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Current schema version
const SCHEMA_VERSION: i32 = 1;

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

            // Default to AppData (safer), user can change later
            let config = BootstrapConfig {
                data_location: if is_writable {
                    // Could prompt user here, but for now default to AppData
                    DataLocation::AppData
                } else {
                    DataLocation::AppData
                },
            };

            // Save bootstrap config
            let content = serde_json::to_string_pretty(&config)?;
            fs::write(&bootstrap_path, content)?;

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

    /// Run SQL migration
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
        // Parse date to get start/end timestamps
        let start_of_day = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|e| e.to_string())?
            .and_hms_opt(0, 0, 0)
            .unwrap();

        let local_offset = chrono::Local::now().offset().clone();
        let start_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
            start_of_day - local_offset,
            local_offset,
        );
        let end_dt = start_dt + chrono::Duration::days(1);

        let start_ms = start_dt.timestamp_millis();
        let end_ms = end_dt.timestamp_millis();

        let app_summaries = self.get_app_breakdown(start_ms, end_ms)?;

        let total_active_ms: i64 = app_summaries.iter().map(|s| s.active_duration_ms).sum();
        let total_idle_ms: i64 = app_summaries.iter().map(|s| s.idle_duration_ms).sum();

        // Calculate category breakdown
        let mut category_totals: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        for summary in &app_summaries {
            if let Some(cat) = &summary.primary_category {
                *category_totals.entry(cat.clone()).or_default() += summary.active_duration_ms;
            }
        }
        let category_breakdown: Vec<(String, i64)> = category_totals.into_iter().collect();

        Ok(DailySummary {
            date: date.to_string(),
            total_active_ms,
            total_idle_ms,
            app_summaries,
            category_breakdown,
        })
    }

    fn get_app_breakdown(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<AppSummary>> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                s.app_name,
                SUM(s.end_time - s.start_time) as total_duration_ms,
                SUM(s.idle_seconds * 1000) as total_idle_ms,
                COUNT(*) as segment_count,
                SUM(s.keystrokes) as total_keystrokes,
                SUM(s.mouse_clicks) as total_clicks,
                (
                    SELECT l.category
                    FROM labels l
                    WHERE l.title_hash = s.title_hash
                    ORDER BY
                        CASE l.source
                            WHEN 'user' THEN 1
                            WHEN 'ai' THEN 2
                            WHEN 'heuristic' THEN 3
                            ELSE 4
                        END
                    LIMIT 1
                ) as primary_category
            FROM segments s
            WHERE s.start_time >= ?1 AND s.start_time < ?2
            GROUP BY s.app_name
            ORDER BY total_duration_ms DESC
            "#,
        )?;

        let summaries = stmt
            .query_map(params![start_ms, end_ms], |row| {
                let app_name: String = row.get(0)?;
                Ok(AppSummary {
                    friendly_name: crate::utils::to_friendly_name(&app_name),
                    app_name,
                    active_duration_ms: row.get(1)?,
                    idle_duration_ms: row.get(2)?,
                    segment_count: row.get(3)?,
                    keystrokes: row.get(4)?,
                    mouse_clicks: row.get(5)?,
                    primary_category: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(summaries)
    }

    fn get_today_active_ms(&self) -> StorageResult<i64> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let today = chrono::Local::now().date_naive();
        let start_of_day = today.and_hms_opt(0, 0, 0).unwrap();
        let local_offset = chrono::Local::now().offset().clone();
        let start_dt = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
            start_of_day - local_offset,
            local_offset,
        );
        let start_ms = start_dt.timestamp_millis();

        let total: i64 = conn
            .query_row(
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

    fn run_migrations(&self) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

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

        // Future migrations go here:
        // if current_version < 2 { ... }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    #[test]
    fn test_segment_insert_and_query() {
        // This would need a temp DB setup
        // For now, just ensure it compiles
    }
}
