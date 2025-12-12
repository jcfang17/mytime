# MyTime Windows - SQLite Migration & Segment Tracking

## Status: Ready for Implementation

**Date:** 2025-12-11
**Consensus:** Approved by team discussion
**Breaking Change:** Yes - storage format changes from CSV to SQLite

---

## Problem Statement

Current CSV-based storage loses granularity:
- `Edge.exe` shows 5 hours, but we can't distinguish YouTube (entertainment) vs Claude.ai (work)
- We only store the *last* window title per session, losing all intermediate title changes
- CSV becomes painful for querying/aggregating as data grows

---

## Architecture Decision

### Source of Truth: Segments (not Sessions)

```
segments (raw, append-only) → sessions (derived, recomputable) → insights (derived)
```

- **Segments**: Every window title change = new row
- **Sessions**: Computed by grouping contiguous segments by app + time gap
- **Labels**: Stored separately with provenance (user/heuristic/ai)

### Storage: SQLite (not CSV)

- Skip CSV expansion, go directly to SQLite
- `rusqlite` crate (sync, simple, no async overhead)
- Export CSV on demand for backward compatibility

### Data Location: User Choice

```
First run prompt:
  [1] Portable mode - store in app folder (default)
  [2] User data mode - store in %APPDATA%\MyTime\
```

Store choice in `config.json` next to executable.

---

## Schema (v1)

### Table: `segments`

| Column | Type | Notes |
|--------|------|-------|
| `segment_id` | TEXT PK | UUID v4, stable forever |
| `app_name` | TEXT NOT NULL | e.g., "msedge.exe" |
| `window_title` | TEXT | Raw title, nullable for redaction mode |
| `title_hash` | TEXT NOT NULL | BLAKE3(app_name + normalized_title), first 32 hex chars |
| `start_time` | INTEGER NOT NULL | Unix epoch milliseconds |
| `end_time` | INTEGER NOT NULL | Unix epoch milliseconds |
| `idle_seconds` | INTEGER DEFAULT 0 | |
| `keystrokes` | INTEGER DEFAULT 0 | |
| `mouse_clicks` | INTEGER DEFAULT 0 | |
| `focus_session_id` | TEXT NOT NULL | Groups segments from same app focus block |
| `schema_version` | INTEGER DEFAULT 1 | |
| `created_at` | INTEGER | Unix epoch ms |

**Indexes:**
- `idx_segments_start_time` on `start_time`
- `idx_segments_title_hash` on `title_hash`
- `idx_segments_focus_session` on `focus_session_id`

### Table: `labels`

| Column | Type | Notes |
|--------|------|-------|
| `title_hash` | TEXT NOT NULL | References segment title_hash |
| `category` | TEXT NOT NULL | entertainment, development, productivity, communication, unknown |
| `source` | TEXT NOT NULL | 'heuristic', 'user', 'ai' |
| `confidence` | REAL | 0.0-1.0, nullable for heuristic/user |
| `updated_at` | INTEGER | Unix epoch ms |
| **PK** | | `(title_hash, source)` |

### Table: `config`

| Column | Type | Notes |
|--------|------|-------|
| `key` | TEXT PK | |
| `value` | TEXT | JSON encoded |

Keys: `device_id`, `redact_titles`, `data_location`

### Table: `schema_migrations`

| Column | Type | Notes |
|--------|------|-------|
| `version` | INTEGER PK | |
| `applied_at` | INTEGER | Unix epoch ms |

---

## Key Concepts

### focus_session_id

A focus session = contiguous time on one app (regardless of title changes).

```
10:00:00 - Focus Edge → new focus_session_id: "abc-123"
10:00:00-10:05:00 - Title "YouTube - Video 1" → segment 1 (focus: abc-123)
10:05:00-10:30:00 - Title "Claude.ai" → segment 2 (focus: abc-123)
10:30:00 - Focus VS Code → new focus_session_id: "def-456"
```

This enables:
- Efficient "time per app" queries (group by focus_session_id)
- Fine-grained "time per title" queries (group by title_hash)

### title_hash Computation

```rust
fn compute_title_hash(app_name: &str, window_title: &str) -> String {
    let normalized = normalize_title(window_title);
    let input = format!("{}|{}", app_name.to_lowercase(), normalized);
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex()[..32].to_string()
}

fn normalize_title(title: &str) -> String {
    // 1. Lowercase
    // 2. Replace all digit sequences with "*"
    // 3. Trim whitespace
    // 4. Collapse multiple spaces
    let re = Regex::new(r"\d+").unwrap();
    re.replace_all(&title.to_lowercase(), "*").trim().to_string()
}
```

**Why app_name + title:** Prevents collisions ("Inbox" in Gmail vs Outlook).

### Noisy Title Coalescing

**Rule:** Don't drop raw data. Store all segments, but:
1. Skip segments shorter than 2 seconds
2. Use `title_hash` (normalized) for grouping - raw titles differ, hashes match
3. Merge micro-segments in derived views/queries, not at write time

```rust
fn should_create_segment(duration_ms: u64) -> bool {
    duration_ms >= 2000  // At least 2 seconds
}
```

---

## Heuristic Categories (v1)

Stored in `labels` table with `source = 'heuristic'`.

```rust
fn categorize_heuristic(app_name: &str, title: &str) -> &'static str {
    let title_lower = title.to_lowercase();
    let app_lower = app_name.to_lowercase();

    // Entertainment
    if matches_any(&title_lower, &["youtube", "netflix", "twitch", "reddit",
                                    "twitter", "x.com", "facebook", "instagram",
                                    "tiktok", "spotify", "music"]) {
        return "entertainment";
    }

    // Development
    if matches_any(&title_lower, &["github", "gitlab", "stackoverflow",
                                    "localhost", "vscode", "visual studio"])
       || matches_any(&app_lower, &["code.exe", "devenv.exe", "idea64.exe"]) {
        return "development";
    }

    // Productivity
    if matches_any(&title_lower, &["claude", "chatgpt", "notion", "obsidian",
                                    "overleaf", "docs.google", "sheets.google",
                                    "word", "excel", "powerpoint"]) {
        return "productivity";
    }

    // Communication
    if matches_any(&app_lower, &["slack", "discord", "teams", "outlook", "thunderbird"])
       || matches_any(&title_lower, &["gmail", "mail", "inbox"]) {
        return "communication";
    }

    "unknown"
}
```

---

## File Structure

```
mytime-windows/
├── src/
│   ├── main.rs                 # UI, event loop
│   ├── tracker.rs              # Window tracking (extract from main.rs)
│   ├── storage/
│   │   ├── mod.rs              # StorageAdapter trait
│   │   ├── sqlite.rs           # SQLite implementation
│   │   └── migrations/
│   │       └── v001_initial.sql
│   ├── categorizer.rs          # Heuristic rules
│   └── models.rs               # Segment, Label, Config structs
├── Cargo.toml                  # Add: rusqlite, blake3, uuid
└── data/                       # Created at runtime
    ├── mytime.db               # SQLite database
    └── config.json             # Device ID, settings
```

---

## Migration: CSV → SQLite

On first run with existing `mytime_data.csv`:

1. Prompt user: "Found existing data. Import into new format? [Yes/No]"
2. If yes:
   - Parse each CSV row
   - Create one segment per row (use row data for single segment)
   - Generate `focus_session_id` per row (they were already session-level)
   - Compute `title_hash` from app_name + window_title
   - Run heuristic categorizer, populate `labels` table
3. Backup CSV to `mytime_data.csv.backup`
4. Continue with SQLite as canonical store

---

## API: StorageAdapter Trait

```rust
pub trait StorageAdapter {
    // Segments
    fn insert_segment(&self, segment: &Segment) -> Result<()>;
    fn get_segments_range(&self, start: i64, end: i64) -> Result<Vec<Segment>>;
    fn get_segments_by_focus_session(&self, focus_session_id: &str) -> Result<Vec<Segment>>;

    // Labels
    fn get_label(&self, title_hash: &str) -> Result<Option<Label>>;
    fn upsert_label(&self, label: &Label) -> Result<()>;

    // Derived queries
    fn get_daily_summary(&self, date: NaiveDate) -> Result<DailySummary>;
    fn get_app_breakdown(&self, start: i64, end: i64) -> Result<Vec<AppBreakdown>>;

    // Config
    fn get_config(&self, key: &str) -> Result<Option<String>>;
    fn set_config(&self, key: &str, value: &str) -> Result<()>;

    // Migration
    fn run_migrations(&self) -> Result<()>;
}
```

---

## Compatibility: CSV Export

Menu option: "Export today's data as CSV"

Exports in old format for users who need it:
```csv
app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks
```

---

## Out of Scope (Defer)

| Feature | Reason |
|---------|--------|
| `device_id` in segments | Add when multi-device sync needed |
| `url_domain`, `browser_tab_id` | Requires browser extension |
| AI categorization | Need cloud backend first |
| Sync to Supabase | Future phase |
| Encryption at rest | Add when privacy mode implemented |

---

## Dependencies to Add

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
blake3 = "1.5"
uuid = { version = "1.0", features = ["v4"] }
```

---

## Testing Checklist

- [ ] New install: creates DB, runs migrations
- [ ] Existing CSV: prompts for import, migrates correctly
- [ ] Segment creation: new segment per title change
- [ ] Coalescing: segments < 2s are skipped
- [ ] title_hash: same normalized title → same hash
- [ ] Labels: heuristic categorizer populates labels table
- [ ] Daily summary: aggregates correctly from segments
- [ ] CSV export: produces valid file matching old format
- [ ] Data location: portable vs user data folder works

---

## Open Questions

1. **Segment batching:** Write segments immediately or batch every N seconds?
   - Recommendation: Batch every 5 seconds to reduce disk I/O

2. **Focus session gap threshold:** How long until new focus_session_id?
   - Recommendation: Same app returning after > 30 seconds = new focus session

3. **Title redaction mode:** Store null + hash, or encrypted title?
   - Recommendation: Store null + hash (simpler, no key management)

---

## Revision History

| Date | Change |
|------|--------|
| 2025-12-11 | Initial draft based on team discussion |
