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

### Data Location: Safe Default with Portable Option

**Default:** `%APPDATA%\MyTime\` (always writable)
**Portable:** App folder, only if writable (test on startup)

```
Bootstrap logic:
1. Check for bootstrap.json next to exe (portable marker)
2. If exists and folder writable → portable mode
3. Otherwise → %APPDATA%\MyTime\ (create if needed)
```

**bootstrap.json** (next to exe, ONLY file outside DB):
```json
{
  "data_location": "portable" | "appdata"
}
```

First run: if exe folder is writable, prompt user for preference. Otherwise, silently use %APPDATA%.

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
| `device_id` | TEXT | Optional, set on first run, enables future multi-device |
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

Keys: `device_id`, `redact_titles`

**Note:** `data_location` is NOT in this table - it's in `bootstrap.json` (needed before DB path is known).

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

### Noisy Title Coalescing (Stable Title Rule)

**Rule:** Don't lose time from short title flickers. Keep previous segment open until new title is stable.

```
Timeline:
  00:00 - Title "YouTube" starts
  00:05 - Title changes to "Ad - 0:05" (don't close YouTube yet, start stability timer)
  00:06 - Title changes to "Ad - 0:04" (reset stability timer)
  00:07 - Title changes back to "YouTube" (reset stability timer)
  00:09 - 2 seconds stable on "YouTube" → no segment created for ads, YouTube continues
  00:30 - Title changes to "Claude.ai"
  00:32 - 2 seconds stable → NOW close YouTube segment (00:00-00:32), open Claude segment
```

**Implementation:**
```rust
struct PendingSegment {
    app_name: String,
    window_title: String,
    start_time: i64,
    // Accumulating metrics
    idle_seconds: u64,
    keystrokes: u64,
    mouse_clicks: u64,
}

struct TitleStabilityTracker {
    pending_title: String,
    pending_since: Instant,
}

fn on_title_change(new_title: &str, tracker: &mut TitleStabilityTracker) {
    tracker.pending_title = new_title.to_string();
    tracker.pending_since = Instant::now();
}

fn on_tick(tracker: &TitleStabilityTracker, current_segment: &mut PendingSegment) -> Option<Segment> {
    if tracker.pending_title != current_segment.window_title
       && tracker.pending_since.elapsed() >= Duration::from_secs(2) {
        // Title stable for 2s, finalize previous segment
        let completed = finalize_segment(current_segment);
        // Start new segment with pending_title
        *current_segment = PendingSegment::new(&tracker.pending_title);
        return Some(completed);
    }
    None
}
```

**Key insight:** Time is never lost. The 2s stability window just delays segment boundaries, doesn't drop time.

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
| `url_domain`, `browser_tab_id` | Requires browser extension |
| AI categorization | Need cloud backend first |
| Sync to Supabase | Future phase |
| Encryption at rest | Add when privacy mode implemented |

**Note:** `device_id` IS included in schema (optional field, set on first run) for cheap future-proofing.

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
- [ ] Coalescing: stable-title rule works (no time lost on flickers)
- [ ] title_hash: same normalized title → same hash
- [ ] Labels: heuristic categorizer populates labels table
- [ ] Daily summary: aggregates correctly from segments
- [ ] CSV export: produces valid file matching old format
- [ ] Data location: portable vs user data folder works

---

## Resolved Questions

1. **Segment writes:** ~~Batch every N seconds~~ → **Write on segment close (title change/app switch)** inside a transaction. Low frequency, avoids losing data on crash.

2. **Focus session gap threshold:** Same app returning after > 30 seconds = new focus session. ✓

3. **Title redaction mode:** Store null + hash (simpler, no key management). ✓

4. **Config storage:** `bootstrap.json` for data_location only (needed before DB path known). Everything else in SQLite `config` table. ✓

5. **device_id:** Include in schema as optional field, generate UUID on first run, store in `config` table. Cheap future-proofing. ✓

6. **Short segment handling:** Don't skip/drop. Use stable-title rule: keep previous segment open until new title is stable for 2s. No time is lost. ✓

7. **Portable mode:** NOT default. Use `%APPDATA%\MyTime\` as safe default. Portable only if exe folder is writable AND user explicitly chooses it. ✓

---

## Implementation Phases

### Phase 1: Storage Foundation
1. Add dependencies (rusqlite, blake3, uuid)
2. Create `storage/` module structure
3. Implement bootstrap.json + data location logic
4. Create SQLite schema (migration v001)
5. Implement StorageAdapter trait + SqliteStorage
6. Generate device_id on first run

### Phase 2: Segment Tracking
1. Extract tracker logic from main.rs to tracker.rs
2. Implement PendingSegment + TitleStabilityTracker
3. Change tracker to emit segments (not sessions)
4. Write segments to SQLite on close (in transaction)
5. Update UI to query from SQLite

### Phase 3: Labels & Categories
1. Implement heuristic categorizer
2. Populate labels table on segment insert
3. Update UI to show category breakdown

### Phase 4: Migration & Compatibility
1. CSV import on first run (if exists)
2. CSV export menu option
3. Backup old CSV after import

---

## Revision History

| Date | Change |
|------|--------|
| 2025-12-11 | Initial draft based on team discussion |
| 2025-12-11 | Rev 2: Fixed config duplication, added device_id, safe default for data location, stable-title rule for coalescing, write-on-close instead of batching |
