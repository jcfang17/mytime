# MyTime Tauri - Implementation Details

## Current Architecture (v0.1)

### Technology Stack

| Component | Technology | Version |
|-----------|------------|---------|
| Framework | Tauri | 2.x |
| Frontend | React + TypeScript | 18.x |
| Build Tool | Vite | 6.x |
| Package Manager | pnpm | - |
| Database | SQLite (rusqlite) | 0.31 |
| Hashing | BLAKE3 | 1.5 |
| Windows API | windows-rs | 0.58 |

---

## Backend (Rust)

### lib.rs - Main Entry

**App State:**
```rust
pub struct AppState {
    storage: Arc<SqliteStorage>,      // Thread-safe storage
    is_tracking: AtomicBool,          // Tracking status
    session_start_ms: Mutex<Option<i64>>,  // Session start time
    baseline_ms: Mutex<Option<i64>>,  // Total at session start
    should_stop: Arc<AtomicBool>,     // Stop signal for thread
    tracking_thread: Mutex<Option<JoinHandle<()>>>,
}
```

**Key Commands:**
```rust
#[tauri::command]
fn start_tracking(state: State<AppState>) -> Result<TrackingState, String>

#[tauri::command]
fn stop_tracking(state: State<AppState>) -> Result<TrackingState, String>

#[tauri::command]
fn get_app_breakdown(state: State<AppState>, day_offset: i32) -> Result<Vec<AppSummary>, String>

#[tauri::command]
async fn export_csv(app: AppHandle, state: State<'_, AppState>, day_offset: i32) -> Result<usize, String>
```

**Important Pattern - Mutex Release Before Join:**
```rust
// WRONG: Mutex held during join()
if let Some(handle) = state.tracking_thread.lock().unwrap().take() {
    let _ = handle.join();
}

// CORRECT: Extract handle first
let handle = state.tracking_thread.lock().unwrap().take();
if let Some(h) = handle {
    let _ = h.join();
}
```

---

### tracker.rs - Window Tracking

**Global State:**
```rust
static KEYSTROKE_COUNTER: AtomicU64 = AtomicU64::new(0);
static CLICK_COUNTER: AtomicU64 = AtomicU64::new(0);
static IS_ACTIVITY_TRACKING: AtomicBool = AtomicBool::new(false);
```

**Stable-Title Rule:**

Wait 2 seconds before finalizing segment to avoid noise:

```rust
const TITLE_STABILITY_MS: u64 = 2000;

struct TitleStabilityTracker {
    pending_app: String,
    pending_title: String,
    pending_since: Instant,
}

impl TitleStabilityTracker {
    fn is_stable(&self) -> bool {
        self.pending_since.elapsed().as_millis() as u64 >= TITLE_STABILITY_MS
    }
}
```

**Focus Session Logic:**

New session when app changes OR same app returns after 30s gap:

```rust
const FOCUS_SESSION_GAP_MS: u64 = 30000;

let is_new_focus_session = if app_changed {
    current_app.as_ref() != Some(&app_name) || gap_exceeded
} else {
    false
};
```

**Activity Hook Lifecycle:**

Hooks only count when tracking is active:

```rust
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && wparam.0 == WM_KEYDOWN as usize
        && IS_ACTIVITY_TRACKING.load(Ordering::SeqCst)  // Check flag
    {
        KEYSTROKE_COUNTER.fetch_add(1, Ordering::SeqCst);
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}
```

---

### storage/sqlite.rs - Database

**Data Location:**
```rust
// Default: %APPDATA%\MyTime\mytime.db
// Portable: Next to executable (if bootstrap.json exists)
```

**Key Queries:**

```rust
// Get today's active time
fn get_today_active_ms(&self) -> StorageResult<i64> {
    let day_start = today_start_ms_with_hour(day_start_hour);
    SELECT COALESCE(SUM(end_time - start_time - (idle_seconds * 1000)), 0)
    FROM segments WHERE start_time >= ?
}

// Get app breakdown with dominant category
fn get_app_breakdown(&self, start_ms: i64, end_ms: i64) -> StorageResult<Vec<AppSummary>> {
    // 1. Aggregate segments by app_name
    // 2. For each app, find dominant category from labels
    // 3. Return sorted by duration descending
}
```

---

### categorizer.rs - Heuristic Classification

**Category Rules:**

| Category | App Patterns | Title Patterns |
|----------|--------------|----------------|
| Entertainment | - | youtube, netflix, twitch, reddit, spotify, steam |
| Development | code.exe, devenv.exe, idea | github, gitlab, stackoverflow, localhost |
| Productivity | - | claude, chatgpt, notion, docs.google, word |
| Communication | slack, discord, teams | gmail, mail, zoom |
| Unknown | (fallback) | - |

```rust
pub fn categorize_heuristic(app_name: &str, window_title: &str) -> &'static str {
    let title_lower = window_title.to_lowercase();
    let app_lower = app_name.to_lowercase();

    // Check patterns in order of priority
    if is_entertainment(&app_lower, &title_lower) { "entertainment" }
    else if is_development(&app_lower, &title_lower) { "development" }
    else if is_productivity(&title_lower) { "productivity" }
    else if is_communication(&app_lower, &title_lower) { "communication" }
    else { "unknown" }
}
```

---

### utils.rs - Utilities

**Title Hash Computation:**
```rust
pub fn compute_title_hash(app_name: &str, window_title: &str) -> String {
    let normalized = normalize_title(window_title);
    let input = format!("{}|{}", app_name.to_lowercase(), normalized);
    let hash = blake3::hash(input.as_bytes());
    hash.to_hex()[..32].to_string()
}

pub fn normalize_title(title: &str) -> String {
    // Replace digits with * to group similar titles
    // "Video 1 of 10" and "Video 2 of 10" -> same hash
    DIGIT_REGEX.replace_all(&title.to_lowercase(), "*")
        .trim()
        .to_string()
}
```

**Day Boundary:**
```rust
pub fn today_start_ms_with_hour(day_start_hour: u32) -> i64 {
    // If current time < day_start_hour, use yesterday's date
    // Allows night owls to have "today" extend past midnight
}

pub fn day_range_ms_with_offset(day_start_hour: u32, offset: i32) -> (i64, i64) {
    // Returns (start_ms, end_ms) for day at offset
    // offset=0 is today, offset=-1 is yesterday
}
```

---

## Frontend (React)

### App.tsx - Main Component

**State:**
```typescript
const [trackingState, setTrackingState] = useState<TrackingState>({...});
const [appBreakdown, setAppBreakdown] = useState<AppSummary[]>([]);
const [categoryBreakdown, setCategoryBreakdown] = useState<[string, number][]>([]);
const [dayOffset, setDayOffset] = useState(0);
const [showActiveOnly, setShowActiveOnly] = useState(true);
const [dayStartHour, setDayStartHourState] = useState(6);
const [autostartEnabled, setAutostartEnabledState] = useState(false);
```

**Polling Architecture:**
```typescript
// Fast poll - tracking state (1s)
useEffect(() => {
    const interval = setInterval(loadTrackingState, 1000);
    return () => clearInterval(interval);
}, [loadTrackingState]);

// Slow poll - breakdown data (5s)
useEffect(() => {
    const interval = setInterval(loadBreakdown, 5000);
    return () => clearInterval(interval);
}, [loadBreakdown]);
```

**Display Time (Avoiding Double-Count):**
```typescript
const displayTimeMs = trackingState.is_tracking && trackingState.baseline_ms !== null
    ? trackingState.baseline_ms + currentSessionMs
    : trackingState.total_time_ms;
```

**Active Category Breakdown:**
```typescript
const activeCategoryBreakdown = useMemo(() => {
    if (!showActiveOnly) return categoryBreakdown;

    const catMap = new Map<string, number>();
    for (const app of appBreakdown) {
        const cat = app.primary_category || "unknown";
        const activeMs = app.total_duration_ms - app.idle_duration_ms;
        catMap.set(cat, (catMap.get(cat) || 0) + activeMs);
    }
    return Array.from(catMap.entries()).sort((a, b) => b[1] - a[1]);
}, [showActiveOnly, categoryBreakdown, appBreakdown]);
```

---

### api.ts - Tauri Invokes

```typescript
import { invoke } from "@tauri-apps/api/core";

export async function startTracking(): Promise<TrackingState> {
    return await invoke("start_tracking");
}

export async function getAppBreakdown(dayOffset: number): Promise<AppSummary[]> {
    return await invoke("get_app_breakdown", { dayOffset });
}

export async function exportCsv(dayOffset: number): Promise<number> {
    return await invoke("export_csv", { dayOffset });
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
    return await invoke("set_autostart_enabled", { enabled });
}
```

---

### types.ts - TypeScript Types

```typescript
export interface TrackingState {
    is_tracking: boolean;
    session_start_ms: number | null;
    total_time_ms: number;
    baseline_ms: number | null;
}

export interface AppSummary {
    app_name: string;
    friendly_name: string;
    total_duration_ms: number;
    idle_duration_ms: number;
    segment_count: number;
    keystrokes: number;
    mouse_clicks: number;
    primary_category: string | null;
}

export type Category = "entertainment" | "development" | "productivity" | "communication" | "unknown";

export const CATEGORY_INFO: Record<Category, { emoji: string; label: string; color: string }> = {
    entertainment: { emoji: "🎬", label: "Entertainment", color: "#ef4444" },
    development: { emoji: "💻", label: "Development", color: "#3b82f6" },
    productivity: { emoji: "📝", label: "Productivity", color: "#22c55e" },
    communication: { emoji: "💬", label: "Communication", color: "#a855f7" },
    unknown: { emoji: "📁", label: "Other", color: "#6b7280" },
};
```

---

## Configuration

### tauri.conf.json

```json
{
    "productName": "MyTime",
    "version": "0.1.0",
    "identifier": "com.mytime.app",
    "build": {
        "beforeDevCommand": "pnpm dev",
        "devUrl": "http://localhost:1420",
        "beforeBuildCommand": "pnpm build",
        "frontendDist": "../dist"
    },
    "app": {
        "windows": [{
            "title": "MyTime - Time Tracker",
            "width": 900,
            "height": 650,
            "minWidth": 700,
            "minHeight": 500,
            "center": true,
            "resizable": true,
            "decorations": true
        }],
        "security": {
            "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
        }
    }
}
```

---

## System Tray

**Setup (lib.rs):**
```rust
fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::new("Show").id("show").build(app)?;
    let start_item = MenuItemBuilder::new("Start Tracking").id("start").build(app)?;
    let stop_item = MenuItemBuilder::new("Stop Tracking").id("stop").build(app)?;
    let quit_item = MenuItemBuilder::new("Quit").id("quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&start_item)
        .item(&stop_item)
        .separator()
        .item(&quit_item)
        .build()?;

    // Tauri 2.x manages tray lifecycle - no need to store handle
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("MyTime - Time Tracker")
        .icon(load_icon())
        .on_menu_event(move |app, event| { ... })
        .on_tray_icon_event(|tray, event| { ... })
        .build(app)?;

    Ok(())
}
```

**Close-to-Tray Behavior:**
```rust
.on_window_event(|window, event| {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let _ = window.hide();
    }
})
```

---

## CSS Theming (App.css)

**CSS Variables:**
```css
:root {
    --bg-primary: #0f172a;
    --bg-secondary: #1e293b;
    --bg-tertiary: #334155;
    --text-primary: #f8fafc;
    --text-secondary: #94a3b8;
    --text-muted: #64748b;
    --accent-primary: #3b82f6;
    --accent-success: #22c55e;
    --accent-warning: #f59e0b;
    --accent-danger: #ef4444;
    --border-color: #334155;
}
```

**Layout:**
- Sidebar: 200px fixed width
- Main content: Flexible, scrollable
- Category chips: Flexbox wrap
- App list: CSS Grid (3 columns)

---

## Build Commands

```bash
# Development
pnpm tauri dev

# Build release
pnpm tauri build

# Check Rust code
cd src-tauri && cargo check

# Build frontend only
pnpm build

# Run tests
cd src-tauri && cargo test
```

---

## Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| Two tray icons | Manual + config tray | Remove trayIcon from tauri.conf.json |
| Close kills app | No close handler | Add `on_window_event` with `prevent_close()` |
| Double-counted time | baseline_ms not used | Store baseline at session start |
| Mutex deadlock | join() while locked | Extract handle before join() |
| Hooks count when stopped | No tracking flag | Check IS_ACTIVITY_TRACKING in hooks |

---

## Files Changed Summary

| File | Purpose |
|------|---------|
| `lib.rs` | App state, Tauri commands, tray setup |
| `models.rs` | Segment, Label, TrackingState structs |
| `tracker.rs` | Window tracking, stable-title rule, hooks |
| `categorizer.rs` | Heuristic classification |
| `storage/sqlite.rs` | Database operations |
| `utils.rs` | Hashing, time utilities |
| `App.tsx` | React UI, state management |
| `App.css` | Dark theme styling |
| `api.ts` | Tauri invoke wrappers |
| `types.ts` | TypeScript interfaces |
