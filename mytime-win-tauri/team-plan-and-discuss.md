# MyTime Tauri - Implementation Plan & Architecture

## Status: Implemented

**Date:** 2025-12-13
**Platform:** Windows (Tauri 2.x)
**Port From:** mytime-windows (Rust/native-windows-gui)

---

## Why Tauri?

### Problems with native-windows-gui

The original Windows app used native-windows-gui, which works well but:
- Win32 API is verbose and Windows-only
- Styling is limited to classic Windows appearance
- No easy path to cross-platform

### Tauri Advantages

- **Cross-platform**: Same codebase for Windows, macOS, Linux
- **Modern UI**: React/TypeScript frontend with full CSS styling
- **Smaller bundle**: ~10MB vs Electron's ~100MB+
- **Rust backend**: Same tracking/storage logic, just different UI layer
- **Active ecosystem**: Plugins for dialog, autostart, etc.

---

## Architecture

### Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 18 + TypeScript + Vite |
| Backend | Rust (Tauri 2.x) |
| Database | SQLite (rusqlite) |
| Styling | CSS (dark theme) |
| Package Manager | pnpm |

### Module Structure

```
mytime-win-tauri/
├── src/                        # Frontend (React)
│   ├── App.tsx                 # Main component
│   ├── App.css                 # Dark theme styles
│   ├── api.ts                  # Tauri invoke wrappers
│   └── types.ts                # TypeScript types
├── src-tauri/                  # Backend (Rust)
│   ├── src/
│   │   ├── lib.rs              # Tauri commands, app state
│   │   ├── models.rs           # Segment, Label, TrackingState
│   │   ├── utils.rs            # Hashing, time formatting
│   │   ├── tracker.rs          # Window tracking, stable-title rule
│   │   ├── categorizer.rs      # Heuristic classification
│   │   └── storage/
│   │       ├── mod.rs          # StorageAdapter trait
│   │       └── sqlite.rs       # SQLite implementation
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # Tauri configuration
└── package.json                # Node dependencies
```

---

## Data Model (Same as Windows)

### Source of Truth: Segments

```
segments (raw, append-only) → derived queries → UI
```

- **Segments**: Every window title change = new row
- **Labels**: Category assignment with provenance (heuristic/user/ai)
- **Config**: Key-value storage for settings

### SQLite Schema

```sql
CREATE TABLE segments (
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

CREATE TABLE labels (
    title_hash TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

---

## Key Implementation Details

### Tauri Commands

All backend functionality exposed via `#[tauri::command]`:

| Command | Purpose |
|---------|---------|
| `start_tracking` | Begin window monitoring |
| `stop_tracking` | Stop monitoring, finalize segment |
| `get_tracking_state` | Current status, time, baseline |
| `get_app_breakdown` | Per-app time summary |
| `get_category_breakdown` | Per-category totals |
| `set_app_category` | User category override |
| `get_day_start_hour` / `set_day_start_hour` | Day boundary config |
| `export_csv` | Export with save dialog |
| `get_autostart_enabled` / `set_autostart_enabled` | Launch on login |

### State Management

```rust
pub struct AppState {
    storage: Arc<SqliteStorage>,
    is_tracking: AtomicBool,
    session_start_ms: Mutex<Option<i64>>,
    baseline_ms: Mutex<Option<i64>>,  // Avoids double-counting
    should_stop: Arc<AtomicBool>,
    tracking_thread: Mutex<Option<JoinHandle<()>>>,
}
```

### Tracking Thread

Runs in background, separate from UI:

```rust
let handle = std::thread::spawn(move || {
    tracker::track_foreground_window(storage, should_stop, None);
});
```

---

## Frontend Architecture

### Polling Strategy

Optimized to reduce DB load:

| Data | Poll Interval | Reason |
|------|---------------|--------|
| Tracking state | 1 second | Timer display needs fast update |
| App/category breakdown | 5 seconds | Data changes slowly |
| Settings | Once on load | Static until user changes |

### Display Time Calculation

Avoids double-counting with baseline:

```typescript
const displayTimeMs = trackingState.is_tracking && trackingState.baseline_ms !== null
    ? trackingState.baseline_ms + currentSessionMs  // baseline + elapsed
    : trackingState.total_time_ms;                  // from DB when stopped
```

### Active-Only Filter

Category totals respect the "Active only" checkbox:

```typescript
const activeCategoryBreakdown = useMemo(() => {
    if (!showActiveOnly) return categoryBreakdown;
    // Compute from app breakdown with idle subtracted
    const catMap = new Map<string, number>();
    for (const app of appBreakdown) {
        const activeMs = app.total_duration_ms - app.idle_duration_ms;
        catMap.set(app.primary_category || "unknown", ...);
    }
    return Array.from(catMap.entries()).sort((a, b) => b[1] - a[1]);
}, [showActiveOnly, categoryBreakdown, appBreakdown]);
```

---

## System Tray

### Behavior

- **Left click**: Show/focus main window
- **Right click**: Context menu
- **Close button**: Hide to tray (not quit)

### Menu Items

- Show
- Start Tracking / Stop Tracking
- Quit

### Implementation Note

Tauri 2.x manages tray icon lifecycle automatically. The `TrayIcon` handle can be dropped after `build()` - the icon persists.

---

## Plugins Used

| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-opener` | Default Tauri plugin |
| `tauri-plugin-dialog` | Native save dialog for export |
| `tauri-plugin-autostart` | Launch on Windows login |

---

## Security Considerations

### Content Security Policy

```json
"security": {
    "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
}
```

### File Access

Export uses native save dialog (no arbitrary path from webview).

---

## Differences from Windows Version

| Aspect | Windows (nwg) | Tauri |
|--------|---------------|-------|
| UI Framework | native-windows-gui | React + CSS |
| Styling | Windows native | Custom dark theme |
| Window chrome | Native | Native (decorations: true) |
| State updates | Win32 messages | React state + polling |
| Export | Direct file write | Save dialog plugin |

### Same Components

- SQLite storage (identical schema)
- Tracking logic (stable-title rule)
- Categorization (heuristic rules)
- Activity hooks (keyboard/mouse)

---

## Build & Run

```bash
cd mytime-win-tauri

# Install dependencies
pnpm install

# Development
pnpm tauri dev

# Production build
pnpm tauri build
```

### Output

- Windows: `src-tauri/target/release/MyTime.exe`
- Installer: `src-tauri/target/release/bundle/`

---

## Testing Checklist

- [x] Tracking starts/stops correctly
- [x] Time display updates smoothly
- [x] App breakdown shows correct durations
- [x] Category chips filter with "Active only"
- [x] Date navigation works
- [x] Right-click category assignment
- [x] Export saves CSV via dialog
- [x] Settings persist (day start hour, autostart)
- [x] Close minimizes to tray
- [x] Tray menu works
- [x] Autostart registers/unregisters

---

## Known Limitations

1. **Activity hooks run for process lifetime**: Once started, keyboard/mouse hooks continue even when tracking stopped. Mitigated by checking `IS_ACTIVITY_TRACKING` flag before counting.

2. **No CSV import**: Unlike Windows version, no prompt to import existing CSV. Could add as future feature.

3. **Windows only**: While Tauri is cross-platform, this implementation uses Windows-specific APIs for window tracking. macOS/Linux would need platform-specific tracker implementations.

---

## Revision History

| Date | Change |
|------|--------|
| 2025-12-13 | Initial implementation - port from Windows app |
| 2025-12-13 | Added autostart feature |
| 2025-12-13 | Fixed: CSP, export dialog, double-counting, mutex during join |
| 2025-12-13 | Added: Active-only category filter, activity hook lifecycle |
