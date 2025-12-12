# MyTime Windows Implementation Details

## Current Architecture (v0.2)

### Module Structure

```
src/
├── main.rs           # UI, tray, event handlers, legacy storage
├── models.rs         # Data structures (Segment, Label, AppSummary, etc.)
├── utils.rs          # Title hashing, time formatting, day boundary
├── tracker.rs        # Foreground window tracking, stable-title rule
├── categorizer.rs    # Heuristic category classification
└── storage/
    ├── mod.rs        # StorageAdapter trait
    └── sqlite.rs     # SQLite implementation
```

### Technology Stack

- **GUI Framework**: native-windows-gui (nwg) 1.0
- **Database**: SQLite via rusqlite
- **Hashing**: BLAKE3 for title normalization
- **Language**: Rust (edition 2021)
- **Windows API**: windows-rs 0.58

---

## Why native-windows-gui (Not egui)

The original implementation used **egui/eframe** but was abandoned due to:

**Problem**: egui's event loop stops when window is hidden/minimized.
- `request_repaint()` ignored when window not visible
- Tray menu clicks don't work (ghost tray icon)
- App can only be killed via Task Manager

**Root Cause**: Known limitation - egui doesn't update once window is not interactable.

**Solution**: native-windows-gui provides proper Win32 message loop that works when hidden.

---

## Data Storage

### SQLite Schema

```sql
-- Source of truth for time tracking
CREATE TABLE segments (
    segment_id TEXT PRIMARY KEY,
    app_name TEXT NOT NULL,
    window_title TEXT,
    title_hash TEXT NOT NULL,
    start_time INTEGER NOT NULL,  -- epoch ms
    end_time INTEGER NOT NULL,    -- epoch ms
    idle_seconds INTEGER DEFAULT 0,
    keystrokes INTEGER DEFAULT 0,
    mouse_clicks INTEGER DEFAULT 0,
    focus_session_id TEXT NOT NULL,
    device_id TEXT,
    schema_version INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);

-- Category labels with provenance
CREATE TABLE labels (
    title_hash TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    source TEXT NOT NULL,  -- 'heuristic', 'user', 'ai'
    confidence REAL,
    updated_at INTEGER NOT NULL
);

-- Key-value config storage
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### Data Location

Determined by `bootstrap.json` next to executable:
- **AppData** (default): `%APPDATA%\MyTime\mytime.db`
- **Portable**: Same directory as executable

---

## Tracking Logic (tracker.rs)

### Stable-Title Rule

Wait 2 seconds before finalizing a segment to avoid noise from rapidly changing titles:

```rust
const TITLE_STABILITY_MS: u64 = 2000;

// Only finalize when:
// 1. Title changed AND
// 2. New title stable for 2s AND
// 3. Tracker matches new title
if title_changed && tracker_stable && tracker_matches_new {
    finalize_segment();
    start_new_segment();
}
```

### Focus Session Logic

New focus session created when:
1. **App changes** (not just title), OR
2. **Same app returns after 30s gap**

```rust
let app_changed = prev_app_name != app_name;
let is_new_focus_session = if app_changed {
    current_app.as_ref() != Some(&app_name) || gap_exceeded
} else {
    false  // Same app, title change = same session
};
```

### Idle Detection

Uses `GetLastInputInfo()` to track seconds without keyboard/mouse input:
- Threshold: 30 seconds
- Accumulates in `idle_seconds` field per segment

### Activity Metrics

Global hooks count keystrokes and mouse clicks:
```rust
static KEYSTROKE_COUNTER: AtomicU64;
static CLICK_COUNTER: AtomicU64;
```

---

## Categorization (categorizer.rs)

### Heuristic Rules

Pattern matching on app name and window title:

| Category | Examples |
|----------|----------|
| Entertainment | YouTube, Netflix, Reddit, Spotify, Steam |
| Development | VS Code, GitHub, Terminal, localhost |
| Productivity | Claude, Notion, Google Docs, Figma |
| Communication | Slack, Teams, Gmail, Zoom |
| Unknown | Anything else |

### Label Priority

When multiple labels exist: **User > AI > Heuristic**

```sql
ORDER BY CASE source
    WHEN 'user' THEN 1
    WHEN 'ai' THEN 2
    WHEN 'heuristic' THEN 3
END
```

---

## UI Components

### Main Window

```rust
#[derive(Default, NwgUi)]
pub struct MyTimeApp {
    window: nwg::Window,
    status_label: nwg::Label,      // "Tracking" / "Stopped"
    time_label: nwg::Label,        // "02:34:15"
    summary_label: nwg::Label,     // "Top: VS Code (1:23) - 5 apps"
    category_label: nwg::Label,    // Category breakdown
    app_list: nwg::ListView,       // App | Active | Idle columns
    // ... buttons, tray, timer
}
```

### System Tray Menu

- Show Window
- Start Tracking / Stop Tracking
- Export Today's Data
- Start with Windows (checkbox)
- Day Starts At... (config dialog)
- Exit

### Configurable Day Boundary

Default: 6 AM (user can change via tray menu)

```rust
pub fn today_start_ms_with_hour(day_start_hour: u32) -> i64 {
    // If current hour < day_start_hour, use yesterday's date
    let effective_date = if now.hour() < day_start_hour {
        today - Duration::days(1)
    } else {
        today
    };
    // ...
}
```

---

## Build & Run

```powershell
cd mytime-windows
cargo build --release
.\target\release\mytime-win.exe

# Run tests
cargo test
```

### Required: Windows Manifest

The app requires Common Controls v6 manifest in `build.rs`:

```rust
res.set_manifest(r#"
<assembly ...>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity name="Microsoft.Windows.Common-Controls" version="6.0.0.0" .../>
    </dependentAssembly>
  </dependency>
</assembly>
"#);
```

---

## Migration from CSV

On first run with existing `mytime_data.csv`:
1. Prompt user to import
2. Convert each row to Segment
3. Create heuristic labels
4. Backup CSV to `.csv.backup`

---

## Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| STATUS_ENTRYPOINT_NOT_FOUND | Missing manifest | Add winres + manifest in build.rs |
| Win95 look | Missing visual styles | Call `nwg::enable_visual_styles()` before init |
| Tray icon missing | No icon resource | Use system icon fallback |
| Column text truncated | Width too narrow | Columns are user-resizable |

---

## Files Changed This Phase

| File | Changes |
|------|---------|
| `models.rs` | Renamed active_duration_ms -> total_duration_ms |
| `utils.rs` | Added day_start_hour support, Timelike import |
| `tracker.rs` | Fixed focus session logic (app change only) |
| `categorizer.rs` | NEW - heuristic classification |
| `storage/sqlite.rs` | Day start config, dominant category query |
| `main.rs` | Category UI, export, day config dialog, import |
