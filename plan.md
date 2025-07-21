# MyTime Enhancement Plan

## CSV Schema Enhancements

### Current Schema Issues
- Windows and macOS have inconsistent CSV formats (quoting, timezone)
- Missing critical fields for productivity analysis
- No explicit end_time (must be calculated from start_time + duration)
- No idle detection to distinguish active work from passive time
- No activity metrics for deeper productivity insights

### Required Schema Changes

#### 1. Add end_time field
- **Why**: Easier database queries and time range analysis
- **Implementation**: Calculate and store when tracking window changes
- **Format**: ISO8601 with consistent timezone (UTC recommended)

#### 2. Add idle_seconds field
- **Why**: Distinguish active work from passive reading/thinking time
- **Implementation**: 
  - Windows: Use `GetLastInputInfo()` API
  - macOS: Use `CGEventSourceSecondsSinceLastEventType()`
  - Track seconds of inactivity within each time entry
- **Threshold**: Consider idle after 30 seconds of no input

#### 3. Add activity metrics (keystrokes, mouse_clicks)
- **Why**: Quantify activity level for productivity analysis
- **Implementation**:
  - Count keyboard and mouse events during active periods
  - Reset counters when window changes
  - Store as simple integers per time entry
- **Privacy**: Count events only, don't capture actual keystrokes

### New CSV Schema
```csv
app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks
```

### Implementation Notes
- User/device info will be attached during bulk database upserts (not stored locally)
- Both platforms must use identical CSV format (fix Windows vs macOS differences)
- Use UTC timezone for all timestamps
- Quote all string fields consistently

### Benefits for AI/Analytics
- Calculate true "active time" (duration_seconds - idle_seconds)
- Identify productivity patterns (high keystroke periods = coding, high clicks = design work)
- Better time estimates for similar future tasks
- Detect and filter out AFK (away from keyboard) periods