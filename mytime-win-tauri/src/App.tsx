import { useState, useEffect, useCallback, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getTrackingState,
  startTracking,
  stopTracking,
  getAppBreakdown,
  getCategoryBreakdown,
  getDayLabel,
  setAppCategory,
  getDayStartHour,
  setDayStartHour,
  exportCsv,
  getAutostartEnabled,
  setAutostartEnabled,
  formatDurationLocal,
} from "./api";
import type { TrackingState, AppSummary, Category } from "./types";
import { getCategoryInfo, CATEGORY_INFO } from "./types";
import "./App.css";

type Page = "dashboard" | "settings";

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("dashboard");
  const [trackingState, setTrackingState] = useState<TrackingState>({
    is_tracking: false,
    session_start_ms: null,
    total_time_ms: 0,
    baseline_ms: null,
  });
  const [appBreakdown, setAppBreakdown] = useState<AppSummary[]>([]);
  const [categoryBreakdown, setCategoryBreakdown] = useState<[string, number][]>([]);
  const [dayOffset, setDayOffset] = useState(0);
  const [dayLabel, setDayLabel] = useState("Today");
  const [showActiveOnly, setShowActiveOnly] = useState(true);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    appName: string;
  } | null>(null);
  const [dayStartHour, setDayStartHourState] = useState(6);
  const [autostartEnabled, setAutostartEnabledState] = useState(false);
  const [exportStatus, setExportStatus] = useState<string | null>(null);

  // Load tracking state (fast poll)
  const loadTrackingState = useCallback(async () => {
    try {
      const state = await getTrackingState();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to load tracking state:", err);
    }
  }, []);

  // Load breakdown data (slow poll)
  const loadBreakdown = useCallback(async () => {
    try {
      const [apps, categories, label] = await Promise.all([
        getAppBreakdown(dayOffset),
        getCategoryBreakdown(dayOffset),
        getDayLabel(dayOffset),
      ]);
      setAppBreakdown(apps);
      setCategoryBreakdown(categories);
      setDayLabel(label);
    } catch (err) {
      console.error("Failed to load breakdown:", err);
    }
  }, [dayOffset]);

  // Load settings
  const loadSettings = useCallback(async () => {
    try {
      const [hour, autostart] = await Promise.all([
        getDayStartHour(),
        getAutostartEnabled(),
      ]);
      setDayStartHourState(hour);
      setAutostartEnabledState(autostart);
    } catch (err) {
      console.error("Failed to load settings:", err);
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadTrackingState();
    loadBreakdown();
    loadSettings();
  }, [loadTrackingState, loadBreakdown, loadSettings]);

  // Fast poll for tracking state (1s)
  useEffect(() => {
    const interval = setInterval(loadTrackingState, 1000);
    return () => clearInterval(interval);
  }, [loadTrackingState]);

  // Slow poll for breakdown data (5s)
  useEffect(() => {
    const interval = setInterval(loadBreakdown, 5000);
    return () => clearInterval(interval);
  }, [loadBreakdown]);

  // Listen for tray events
  useEffect(() => {
    const unlistenStart = listen("tray-start", async () => {
      try {
        const state = await startTracking();
        setTrackingState(state);
      } catch (err) {
        console.error("Failed to start tracking:", err);
      }
    });

    const unlistenStop = listen("tray-stop", async () => {
      try {
        const state = await stopTracking();
        setTrackingState(state);
      } catch (err) {
        console.error("Failed to stop tracking:", err);
      }
    });

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, []);

  // Close context menu on click outside
  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  const handleStart = async () => {
    try {
      const state = await startTracking();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to start:", err);
    }
  };

  const handleStop = async () => {
    try {
      const state = await stopTracking();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to stop:", err);
    }
  };

  const handlePrevDay = () => setDayOffset((prev) => prev - 1);
  const handleNextDay = () => setDayOffset((prev) => Math.min(prev + 1, 0));

  const handleContextMenu = (e: React.MouseEvent, appName: string) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, appName });
  };

  const handleSetCategory = async (category: string) => {
    if (!contextMenu) return;
    try {
      await setAppCategory(contextMenu.appName, category, dayOffset);
      setContextMenu(null);
      loadBreakdown();
    } catch (err) {
      console.error("Failed to set category:", err);
    }
  };

  const handleDayStartHourChange = async (hour: number) => {
    try {
      await setDayStartHour(hour);
      setDayStartHourState(hour);
      loadBreakdown(); // Refresh data with new day boundary
    } catch (err) {
      console.error("Failed to set day start hour:", err);
    }
  };

  const handleAutostartToggle = async (enabled: boolean) => {
    try {
      await setAutostartEnabled(enabled);
      setAutostartEnabledState(enabled);
    } catch (err) {
      console.error("Failed to set autostart:", err);
    }
  };

  const handleExport = async () => {
    try {
      setExportStatus("Exporting...");
      const count = await exportCsv(dayOffset);
      if (count === 0) {
        setExportStatus("Export cancelled");
      } else {
        setExportStatus(`Exported ${count} records`);
      }
      setTimeout(() => setExportStatus(null), 3000);
    } catch (err) {
      console.error("Failed to export:", err);
      setExportStatus("Export failed");
      setTimeout(() => setExportStatus(null), 3000);
    }
  };

  // Calculate display time
  // When tracking: use baseline (time at session start) + elapsed session time
  // When stopped: use total_time_ms from database
  const currentSessionMs = trackingState.is_tracking && trackingState.session_start_ms
    ? Date.now() - trackingState.session_start_ms
    : 0;
  const displayTimeMs = trackingState.is_tracking && trackingState.baseline_ms !== null
    ? trackingState.baseline_ms + currentSessionMs
    : trackingState.total_time_ms;

  // Filter apps based on showActiveOnly
  const filteredApps = appBreakdown.map((app) => {
    const displayMs = showActiveOnly
      ? app.total_duration_ms - app.idle_duration_ms
      : app.total_duration_ms;
    return { ...app, displayMs };
  }).filter((app) => app.displayMs >= 5000);

  // Compute category breakdown that respects showActiveOnly filter
  const activeCategoryBreakdown = useMemo(() => {
    if (!showActiveOnly) {
      return categoryBreakdown;
    }
    // Compute from app breakdown with idle subtracted
    const catMap = new Map<string, number>();
    for (const app of appBreakdown) {
      const cat = app.primary_category || "unknown";
      const activeMs = app.total_duration_ms - app.idle_duration_ms;
      catMap.set(cat, (catMap.get(cat) || 0) + activeMs);
    }
    return Array.from(catMap.entries()).sort((a, b) => b[1] - a[1]);
  }, [showActiveOnly, categoryBreakdown, appBreakdown]);

  // Calculate total for percentage
  const totalMs = activeCategoryBreakdown
    .filter(([cat]) => cat !== "unknown")
    .reduce((sum, [, ms]) => sum + ms, 0);

  return (
    <div className="app-container">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <h1>⏱ MyTime</h1>
        </div>
        <nav className="sidebar-nav">
          <button
            className={`nav-item ${currentPage === "dashboard" ? "active" : ""}`}
            onClick={() => setCurrentPage("dashboard")}
          >
            📊 Dashboard
          </button>
          <button
            className={`nav-item ${currentPage === "settings" ? "active" : ""}`}
            onClick={() => setCurrentPage("settings")}
          >
            ⚙️ Settings
          </button>
        </nav>
        <div className="sidebar-footer">
          <div className={`tracking-status ${trackingState.is_tracking ? "active" : ""}`}>
            {trackingState.is_tracking ? "● Tracking" : "○ Stopped"}
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main className="main-content">
        {currentPage === "dashboard" && (
          <div className="dashboard">
            {/* Header with time and controls */}
            <header className="dashboard-header">
              <div className="time-display">
                <span className="time-value">{formatDurationLocal(displayTimeMs)}</span>
                <span className="time-label">
                  {trackingState.is_tracking ? "Tracking" : "Stopped"}
                </span>
              </div>
              <div className="controls">
                <button
                  className="btn btn-primary"
                  onClick={handleStart}
                  disabled={trackingState.is_tracking}
                >
                  ▶ Start
                </button>
                <button
                  className="btn btn-secondary"
                  onClick={handleStop}
                  disabled={!trackingState.is_tracking}
                >
                  ⏹ Stop
                </button>
              </div>
            </header>

            {/* Category breakdown */}
            {activeCategoryBreakdown.length > 0 && (
              <section className="category-section">
                <div className="category-chips">
                  {activeCategoryBreakdown
                    .filter(([cat, ms]) => ms >= 5000 && cat !== "unknown")
                    .slice(0, 4)
                    .map(([cat, ms]) => {
                      const info = getCategoryInfo(cat);
                      const pct = totalMs > 0 ? Math.round((ms / totalMs) * 100) : 0;
                      return (
                        <div key={cat} className="category-chip" style={{ borderColor: info.color }}>
                          <span className="category-emoji">{info.emoji}</span>
                          <span className="category-name">{info.label}</span>
                          <span className="category-time">{formatDurationLocal(ms)}</span>
                          <span className="category-pct">{pct}%</span>
                        </div>
                      );
                    })}
                </div>
              </section>
            )}

            {/* Date navigation */}
            <section className="date-nav">
              <button className="btn btn-icon" onClick={handlePrevDay}>
                ◀
              </button>
              <span className="date-label">{dayLabel}</span>
              <button
                className="btn btn-icon"
                onClick={handleNextDay}
                disabled={dayOffset >= 0}
              >
                ▶
              </button>
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={showActiveOnly}
                  onChange={(e) => setShowActiveOnly(e.target.checked)}
                />
                Active only
              </label>
            </section>

            {/* App list */}
            <section className="app-list">
              <h2>Application Usage</h2>
              {filteredApps.length === 0 ? (
                <p className="no-data">No activity tracked yet</p>
              ) : (
                <div className="app-table">
                  <div className="app-row header">
                    <span className="app-name">Application</span>
                    <span className="app-time">Time</span>
                    <span className="app-idle">Idle</span>
                  </div>
                  {filteredApps.map((app) => {
                    const catInfo = getCategoryInfo(app.primary_category);
                    return (
                      <div
                        key={app.app_name}
                        className="app-row"
                        onContextMenu={(e) => handleContextMenu(e, app.app_name)}
                      >
                        <span className="app-name">
                          <span className="app-icon">{catInfo.emoji}</span>
                          {app.friendly_name}
                        </span>
                        <span className="app-time">
                          {formatDurationLocal(app.displayMs)}
                        </span>
                        <span className="app-idle">
                          {app.idle_duration_ms > 0
                            ? `💤 ${formatDurationLocal(app.idle_duration_ms)}`
                            : "-"}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </section>
          </div>
        )}

        {currentPage === "settings" && (
          <div className="settings">
            <h2>Settings</h2>

            <section className="setting-section">
              <h3>Startup</h3>
              <label className="setting-toggle">
                <input
                  type="checkbox"
                  checked={autostartEnabled}
                  onChange={(e) => handleAutostartToggle(e.target.checked)}
                />
                <span>Launch MyTime when you log in</span>
              </label>
            </section>

            <section className="setting-section">
              <h3>Day Start Hour</h3>
              <p className="setting-description">
                When does your day start? Time tracked after midnight but before this hour
                will count toward the previous day.
              </p>
              <select
                className="setting-select"
                value={dayStartHour}
                onChange={(e) => handleDayStartHourChange(Number(e.target.value))}
              >
                {Array.from({ length: 24 }, (_, i) => i).map((hour) => {
                  const hour12 = hour === 0 ? 12 : hour > 12 ? hour - 12 : hour;
                  const ampm = hour < 12 ? "AM" : "PM";
                  const label = hour === 0 ? "12:00 AM (Midnight)"
                    : hour === 12 ? "12:00 PM (Noon)"
                    : `${hour12}:00 ${ampm}`;
                  return (
                    <option key={hour} value={hour}>
                      {label}
                    </option>
                  );
                })}
              </select>
            </section>

            <section className="setting-section">
              <h3>Export Data</h3>
              <p className="setting-description">
                Export time tracking data for {dayLabel} as a CSV file.
              </p>
              <div className="setting-row">
                <button className="btn btn-primary" onClick={handleExport}>
                  Export to CSV
                </button>
                {exportStatus && (
                  <span className="export-status">{exportStatus}</span>
                )}
              </div>
            </section>

            <section className="setting-section">
              <h3>About</h3>
              <p className="setting-description">
                MyTime v0.1.0 - Personal Time Tracking
              </p>
            </section>
          </div>
        )}
      </main>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="context-header">Set Category</div>
          {(Object.keys(CATEGORY_INFO) as Category[])
            .filter((cat) => cat !== "unknown")
            .map((cat) => {
              const info = CATEGORY_INFO[cat];
              return (
                <button
                  key={cat}
                  className="context-item"
                  onClick={() => handleSetCategory(cat)}
                >
                  {info.emoji} {info.label}
                </button>
              );
            })}
        </div>
      )}
    </div>
  );
}

export default App;
