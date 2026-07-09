# MyTime Enhancement Plan

## Implementation Status

### Completed (Windows)

#### Phase 1: Storage Foundation
- [x] SQLite database with segments and labels tables
- [x] Epoch milliseconds for all timestamps
- [x] BLAKE3 title hashing (app + normalized_title)
- [x] Bootstrap config for portable vs AppData storage
- [x] Schema migrations support

#### Phase 2: Segment Tracking
- [x] Stable-title rule (2s wait before finalizing segment)
- [x] Focus session grouping (new session on app change or 30s gap)
- [x] Idle detection via GetLastInputInfo
- [x] Keystroke and mouse click counting via hooks
- [x] Raw window title preserved in DB

#### Phase 3: Labels & Categories
- [x] Heuristic categorizer (entertainment, development, productivity, communication)
- [x] Labels table with provenance (source: heuristic/user/ai)
- [x] Category breakdown in UI
- [x] Dominant category by duration (not arbitrary segment)

#### Phase 4: Migration & Compatibility
- [ ] CSV import on first run — NOT implemented (previously mis-marked as done; no import code exists)
- [x] CSV export for today's data
- [ ] Backup old CSV after import — NOT implemented (depends on CSV import)
- [x] Configurable day start hour (default 6 AM)

### Pending / Future Work

#### Analytics Improvements
- [x] Segment-level "Selected Breakdown" (per app/site/category)
- [ ] Segment-level breakdown for main app list (currently dominant-category per app)
- [ ] Apply best-label selection (manual > user > ai > heuristic) to all aggregate queries
- [x] Historical data views (week, month) — History tab: per-day stacked category chart, period comparison, range top apps (2026-07-09)

#### macOS Parity
- [ ] Port SQLite storage to macOS version
- [ ] Port segment tracking with stable-title rule
- [ ] Port heuristic categorizer

#### Future Features
- [ ] User label editing UI
- [x] AI-powered categorization — `generate_suggestions` analyzes the unknown queue with Claude Haiku (Anthropic API, structured outputs) and files suggestions into the approve/reject pipeline; AI period insights card in History tab (2026-07-09)
- [ ] Cloud sync (optional)
- [ ] Cross-device deduplication

---

## Architecture Notes

### Data Model

```
segments (source of truth)
├── segment_id (UUID)
├── app_name, window_title, title_hash
├── start_time, end_time (epoch ms)
├── idle_seconds, keystrokes, mouse_clicks
├── focus_session_id
└── device_id, schema_version, created_at

labels (category provenance)
├── title_hash (FK to normalized title)
├── category (entertainment/development/productivity/communication/unknown)
├── source (heuristic/user/ai)
├── confidence (for AI labels)
└── updated_at

config (key-value settings)
├── device_id
├── day_start_hour
└── ... future settings
```

### Key Design Decisions

1. **Segments as source of truth** - Sessions are derived, not stored
2. **title_hash = BLAKE3(app + normalized_title)** - Privacy-safe grouping key
3. **Raw title preserved** - Allows AI reclassification later
4. **Labels with provenance** - User > AI > Heuristic priority
5. **Epoch milliseconds** - Consistent across timezones
6. **Configurable day boundary** - 6 AM default, user-adjustable

### Reviewer Feedback Addressed

| Issue | Status | Notes |
|-------|--------|-------|
| Focus session bug (title change = new session) | Fixed | Now only on app change |
| get_daily_summary ignores day_start_hour | Fixed | Uses config value |
| Category picks arbitrary segment | Fixed | Dominant by duration |
| active_duration_ms naming | Fixed | Renamed to total_duration_ms |
| Legacy CSV save when using SQLite | Fixed | Gated with is_none() check |

### Known Limitations (OK for v1)

- 2s shift at session start (stable-title tradeoff)
- Main app list uses dominant category (browsers can be mixed-use; use Selected Breakdown for accurate per-category accounting)
- Some aggregate queries may double-count labels if multiple label sources exist (Selected Breakdown already picks the best label)

---

## Original Schema Enhancement Notes

### CSV Schema (Legacy - for import/export)
```csv
app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks
```

### Benefits for AI/Analytics
- Calculate true "active time" (total - idle)
- Identify productivity patterns
- Better time estimates for similar tasks
- Detect and filter AFK periods
