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
  getRules,
  createRule,
  updateRule,
  deleteRule,
  previewRuleMatches,
  getSuggestions,
  approveSuggestion,
  rejectSuggestion,
  getAppContexts,
  getSelectedBreakdown,
  getTimelineSegments,
  getDayRange,
  getUnknownQueue,
  getDailyDigest,
  getLabelProvenance,
} from "./api";
import type {
  TrackingState,
  AppSummary,
  Category,
  ClassificationRule,
  MatchType,
  RulePreview,
  AiSuggestion,
  ContextSummary,
  CategoryBreakdownEntry,
  SelectedBreakdownRow,
  TimelineSegment,
  UnknownQueueItem,
  DailyDigest,
  LabelProvenance,
} from "./types";
import { getCategoryInfo, CATEGORY_INFO } from "./types";
import "./App.css";

type Page = "dashboard" | "settings";
type DashboardTab = "overview" | "cleanup" | "digest";

type ContextMenuState =
  | { kind: "app"; x: number; y: number; appName: string }
  | { kind: "context"; x: number; y: number; appName: string; context: string };

const BROWSER_APP_RE =
  /^(msedge|chrome|firefox|brave|opera|vivaldi|arc|safari)/i;

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("dashboard");
  const [dashboardTab, setDashboardTab] = useState<DashboardTab>("overview");
  const [trackingState, setTrackingState] = useState<TrackingState>({
    is_tracking: false,
    session_start_ms: null,
    total_time_ms: 0,
    baseline_ms: null,
  });
  const [appBreakdown, setAppBreakdown] = useState<AppSummary[]>([]);
  const [categoryBreakdown, setCategoryBreakdown] = useState<CategoryBreakdownEntry[]>([]);
  const [dayOffset, setDayOffset] = useState(0);
  const [dayLabel, setDayLabel] = useState("Today");
  const [showActiveOnly, setShowActiveOnly] = useState(true);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [dayStartHour, setDayStartHourState] = useState(6);
  const [autostartEnabled, setAutostartEnabledState] = useState(false);
  const [exportStatus, setExportStatus] = useState<string | null>(null);

  // Rules state
  const [rules, setRules] = useState<ClassificationRule[]>([]);
  const [editingRule, setEditingRule] = useState<ClassificationRule | null>(null);
  const [showRuleForm, setShowRuleForm] = useState(false);
  const [ruleForm, setRuleForm] = useState({
    appPattern: "",
    titlePattern: "",
    matchType: "contains" as MatchType,
    category: "productivity",
  });
  const [rulePreview, setRulePreview] = useState<RulePreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  // Suggestions state
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);

  // Context drill-down state
  const [expandedApp, setExpandedApp] = useState<string | null>(null);
  const [appContexts, setAppContexts] = useState<ContextSummary[]>([]);
  const [contextsLoading, setContextsLoading] = useState(false);

  // Cached contexts for browser drill-down (expanded app)
  const [contextsByApp, setContextsByApp] = useState<Record<string, ContextSummary[]>>({});

  // Category selection state (for adding up selected categories)
  const [selectedCategories, setSelectedCategories] = useState<Set<string>>(new Set());

  // Selected breakdown (segment-level, for selected category chips)
  const [selectedBreakdown, setSelectedBreakdown] = useState<SelectedBreakdownRow[]>([]);
  const [selectedBreakdownLoading, setSelectedBreakdownLoading] = useState(false);

  // Timeline state
  const [timelineSegments, setTimelineSegments] = useState<TimelineSegment[]>([]);
  const [dayRange, setDayRange] = useState<[number, number] | null>(null);
  const [hoveredSegment, setHoveredSegment] = useState<TimelineSegment | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  // Unknown cleanup queue state
  const [unknownQueue, setUnknownQueue] = useState<UnknownQueueItem[]>([]);
  const [unknownQueueLoading, setUnknownQueueLoading] = useState(false);

  // Daily digest state
  const [digest, setDigest] = useState<DailyDigest | null>(null);
  const [digestLoading, setDigestLoading] = useState(false);

  // Timeline drill-down state
  const [selectedSegment, setSelectedSegment] = useState<TimelineSegment | null>(null);

  // Label provenance state
  const [provenance, setProvenance] = useState<LabelProvenance | null>(null);
  const [provenanceLoading, setProvenanceLoading] = useState(false);
  const [provenanceTitleHash, setProvenanceTitleHash] = useState<string | null>(null);

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

  const loadSelectedBreakdown = useCallback(async () => {
    if (selectedCategories.size === 0) {
      setSelectedBreakdown([]);
      return;
    }

    try {
      setSelectedBreakdownLoading(true);
      const rows = await getSelectedBreakdown(dayOffset, Array.from(selectedCategories));
      setSelectedBreakdown(rows);
    } catch (err) {
      console.error("Failed to load selected breakdown:", err);
    } finally {
      setSelectedBreakdownLoading(false);
    }
  }, [dayOffset, selectedCategories]);

  // Load timeline data
  const loadTimeline = useCallback(async () => {
    try {
      const [segments, range] = await Promise.all([
        getTimelineSegments(dayOffset),
        getDayRange(dayOffset),
      ]);
      setTimelineSegments(segments);
      setDayRange(range);
    } catch (err) {
      console.error("Failed to load timeline:", err);
    }
  }, [dayOffset]);

  // Load unknown cleanup queue
  const loadUnknownQueue = useCallback(async () => {
    try {
      setUnknownQueueLoading(true);
      const queue = await getUnknownQueue(dayOffset);
      setUnknownQueue(queue);
    } catch (err) {
      console.error("Failed to load unknown queue:", err);
    } finally {
      setUnknownQueueLoading(false);
    }
  }, [dayOffset]);

  // Load daily digest
  const loadDigest = useCallback(async () => {
    try {
      setDigestLoading(true);
      const data = await getDailyDigest(dayOffset);
      setDigest(data);
    } catch (err) {
      console.error("Failed to load digest:", err);
    } finally {
      setDigestLoading(false);
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

  // Load rules
  const loadRules = useCallback(async () => {
    try {
      const rulesData = await getRules();
      setRules(rulesData);
    } catch (err) {
      console.error("Failed to load rules:", err);
    }
  }, []);

  // Load suggestions
  const loadSuggestions = useCallback(async () => {
    try {
      const suggestionsData = await getSuggestions();
      setSuggestions(suggestionsData);
    } catch (err) {
      console.error("Failed to load suggestions:", err);
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadTrackingState();
    loadBreakdown();
    loadTimeline();
    loadUnknownQueue();
    loadDigest();
    loadSettings();
    loadRules();
    loadSuggestions();
  }, [loadTrackingState, loadBreakdown, loadTimeline, loadUnknownQueue, loadDigest, loadSettings, loadRules, loadSuggestions]);

  // Fast poll for tracking state (1s)
  useEffect(() => {
    const interval = setInterval(loadTrackingState, 1000);
    return () => clearInterval(interval);
  }, [loadTrackingState]);

  // Slow poll for breakdown + timeline + unknown queue data (5s)
  useEffect(() => {
    const interval = setInterval(() => {
      loadBreakdown();
      loadTimeline();
      loadUnknownQueue();
    }, 5000);
    return () => clearInterval(interval);
  }, [loadBreakdown, loadTimeline, loadUnknownQueue]);

  // Keep selected breakdown in sync (immediate + slow poll when active)
  useEffect(() => {
    loadSelectedBreakdown();
  }, [loadSelectedBreakdown]);

  useEffect(() => {
    if (selectedCategories.size === 0) return;
    const interval = setInterval(loadSelectedBreakdown, 5000);
    return () => clearInterval(interval);
  }, [loadSelectedBreakdown, selectedCategories.size]);

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

  // Collapse expanded app when changing days (contexts are day-specific)
  useEffect(() => {
    setExpandedApp(null);
    setAppContexts([]);
    setContextsByApp({});
  }, [dayOffset]);

  const reloadExpandedAppContexts = useCallback(async () => {
    if (!expandedApp) return;
    setContextsLoading(true);
    try {
      const contexts = await getAppContexts(expandedApp, dayOffset);
      setAppContexts(contexts);
      setContextsByApp((prev) => ({ ...prev, [expandedApp]: contexts }));
    } catch (err) {
      console.error("Failed to reload app contexts:", err);
    } finally {
      setContextsLoading(false);
    }
  }, [dayOffset, expandedApp]);

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
    setContextMenu({ kind: "app", x: e.clientX, y: e.clientY, appName });
  };

  const handleContextRowContextMenu = (
    e: React.MouseEvent,
    appName: string,
    context: string
  ) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ kind: "context", x: e.clientX, y: e.clientY, appName, context });
  };

  const handleSetCategory = async (category: string) => {
    if (!contextMenu) return;
    try {
      if (contextMenu.kind === "app") {
        await setAppCategory(contextMenu.appName, category, dayOffset);
      } else {
        if (contextMenu.context === "other") {
          alert("Cannot create a rule for 'other'. Pick a specific site/context.");
          setContextMenu(null);
          return;
        }
        await createRule(
          contextMenu.appName,
          contextMenu.context,
          "contains",
          category,
          null
        );
        loadRules();
      }
      setContextMenu(null);
      loadBreakdown();
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
    } catch (err) {
      console.error("Failed to set category:", err);
    }
  };

  const handleDayStartHourChange = async (hour: number) => {
    try {
      await setDayStartHour(hour);
      setDayStartHourState(hour);
      loadBreakdown(); // Refresh data with new day boundary
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
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

  // Rule handlers
  const handleAddRule = () => {
    setEditingRule(null);
    setRuleForm({
      appPattern: "",
      titlePattern: "",
      matchType: "contains",
      category: "productivity",
    });
    setRulePreview(null);
    setShowRuleForm(true);
  };

  const handleEditRule = (rule: ClassificationRule) => {
    setEditingRule(rule);
    setRuleForm({
      appPattern: rule.app_pattern || "",
      titlePattern: rule.title_pattern || "",
      matchType: rule.match_type,
      category: rule.category,
    });
    setRulePreview(null);
    setShowRuleForm(true);
  };

  const handleDeleteRule = async (ruleId: string) => {
    try {
      await deleteRule(ruleId);
      loadRules();
      loadBreakdown(); // Refresh to show new categorizations
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
    } catch (err) {
      console.error("Failed to delete rule:", err);
    }
  };

  const handleSaveRule = async () => {
    try {
      const appPattern = ruleForm.appPattern.trim() || null;
      const titlePattern = ruleForm.titlePattern.trim() || null;

      if (!appPattern && !titlePattern) {
        alert("Please enter at least an app pattern or title pattern");
        return;
      }

      if (editingRule) {
        await updateRule(
          editingRule.rule_id,
          appPattern,
          titlePattern,
          ruleForm.matchType,
          ruleForm.category,
          null, // tags
          editingRule.enabled,
          editingRule.priority
        );
      } else {
        await createRule(
          appPattern,
          titlePattern,
          ruleForm.matchType,
          ruleForm.category,
          null // tags
        );
      }

      setShowRuleForm(false);
      loadRules();
      loadBreakdown(); // Refresh to show new categorizations
      loadUnknownQueue(); // Refresh cleanup queue after rule creation
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
    } catch (err) {
      console.error("Failed to save rule:", err);
    }
  };

  const handlePreviewRule = async () => {
    const appPattern = ruleForm.appPattern.trim() || null;
    const titlePattern = ruleForm.titlePattern.trim() || null;

    if (!appPattern && !titlePattern) {
      setRulePreview(null);
      return;
    }

    try {
      setPreviewLoading(true);
      const preview = await previewRuleMatches(
        appPattern,
        titlePattern,
        ruleForm.matchType,
        7 // Look back 7 days
      );
      setRulePreview(preview);
    } catch (err) {
      console.error("Failed to preview rule:", err);
    } finally {
      setPreviewLoading(false);
    }
  };

  const handleToggleRule = async (rule: ClassificationRule) => {
    try {
      await updateRule(
        rule.rule_id,
        rule.app_pattern,
        rule.title_pattern,
        rule.match_type,
        rule.category,
        rule.tags,
        !rule.enabled,
        rule.priority
      );
      loadRules();
      loadBreakdown(); // Categories might have changed
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
    } catch (err) {
      console.error("Failed to toggle rule:", err);
    }
  };

  // Suggestion handlers
  const handleApproveSuggestion = async (suggestionId: string) => {
    try {
      await approveSuggestion(suggestionId);
      loadSuggestions();
      loadRules(); // New rule was created
      loadBreakdown(); // Categories might have changed
      setContextsByApp({});
      await reloadExpandedAppContexts();
      await loadSelectedBreakdown();
    } catch (err) {
      console.error("Failed to approve suggestion:", err);
    }
  };

  const handleRejectSuggestion = async (suggestionId: string) => {
    try {
      await rejectSuggestion(suggestionId);
      loadSuggestions();
    } catch (err) {
      console.error("Failed to reject suggestion:", err);
    }
  };

  // Context drill-down handlers
  const handleToggleAppExpand = async (appName: string) => {
    if (expandedApp === appName) {
      // Collapse
      setExpandedApp(null);
      setAppContexts([]);
    } else {
      // Expand and load contexts
      setExpandedApp(appName);
      const cached = contextsByApp[appName];
      if (cached) {
        setAppContexts(cached);
      }

      setContextsLoading(true);
      try {
        const contexts = await getAppContexts(appName, dayOffset);
        setAppContexts(contexts);
        setContextsByApp((prev) => ({ ...prev, [appName]: contexts }));
      } catch (err) {
        console.error("Failed to load app contexts:", err);
        setAppContexts([]);
      } finally {
        setContextsLoading(false);
      }
    }
  };

  const handleCreateRuleFromContext = (context: ContextSummary, appName: string) => {
    // Pre-fill the rule form with this context
    setEditingRule(null);
    setRuleForm({
      appPattern: appName,
      titlePattern: context.context,
      matchType: "contains",
      category: context.category || "productivity",
    });
    setRulePreview(null);
    setShowRuleForm(true);
  };

  // Show label provenance (why a segment is labeled with a certain category)
  const handleShowProvenance = async (titleHash: string) => {
    if (provenanceTitleHash === titleHash) {
      // Toggle off
      setProvenanceTitleHash(null);
      setProvenance(null);
      return;
    }
    setProvenanceTitleHash(titleHash);
    setProvenanceLoading(true);
    try {
      const prov = await getLabelProvenance(titleHash);
      setProvenance(prov);
    } catch (err) {
      console.error("Failed to load provenance:", err);
    } finally {
      setProvenanceLoading(false);
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
  // Now uses segment-level categories from backend (properly handles mixed-use apps like browsers)
  const activeCategoryBreakdown: [string, number][] = useMemo(() => {
    if (!showActiveOnly) {
      // Return total_ms for each category
      return categoryBreakdown
        .map((entry) => [entry.category, entry.total_ms] as [string, number])
        .filter(([, ms]) => ms >= 5000)
        .sort((a, b) => b[1] - a[1]);
    }
    // Return active_ms (total - idle) for each category
    return categoryBreakdown
      .map((entry) => [entry.category, entry.total_ms - entry.idle_ms] as [string, number])
      .filter(([, ms]) => ms >= 5000)
      .sort((a, b) => b[1] - a[1]);
  }, [showActiveOnly, categoryBreakdown]);

  // Calculate total idle time across all categories
  const totalIdleMs = useMemo(() => {
    return categoryBreakdown.reduce((sum, entry) => sum + entry.idle_ms, 0);
  }, [categoryBreakdown]);

  // Calculate total tracked time (all categories)
  const totalTrackedMs = activeCategoryBreakdown.reduce((sum, [, ms]) => sum + ms, 0);

  // Unconditional totals for header (always show all three regardless of toggle)
  const unconditionalTotalMs = useMemo(() => {
    return categoryBreakdown.reduce((sum, entry) => sum + entry.total_ms, 0);
  }, [categoryBreakdown]);
  const unconditionalActiveMs = unconditionalTotalMs - totalIdleMs;

  // Calculate selected categories total
  const selectedTotalMs = useMemo(() => {
    if (selectedCategories.size === 0) return 0;
    return activeCategoryBreakdown
      .filter(([cat]) => selectedCategories.has(cat))
      .reduce((sum, [, ms]) => sum + ms, 0);
  }, [selectedCategories, activeCategoryBreakdown]);

  const selectedBreakdownView = useMemo(() => {
    const thresholdMs = 5000;

    const rows = selectedBreakdown
      .map((row) => {
        const displayMs = showActiveOnly
          ? row.total_duration_ms - row.idle_duration_ms
          : row.total_duration_ms;
        return { ...row, displayMs };
      })
      .filter((row) => row.displayMs > 0)
      .sort((a, b) => b.displayMs - a.displayMs);

    const visibleRows = rows.filter((row) => row.displayMs >= thresholdMs);
    const visibleTotalMs = visibleRows.reduce((sum, row) => sum + row.displayMs, 0);
    const totalMs = rows.reduce((sum, row) => sum + row.displayMs, 0);
    const otherMs = totalMs - visibleTotalMs;

    return { visibleRows, otherMs, totalMs };
  }, [selectedBreakdown, showActiveOnly]);

  // Toggle category selection
  const handleCategoryClick = (category: string) => {
    setSelectedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  };

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

            {/* Date navigation (shared across tabs) */}
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

            {/* Dashboard tabs */}
            <nav className="dashboard-tabs">
              <button
                className={`dashboard-tab ${dashboardTab === "overview" ? "active" : ""}`}
                onClick={() => setDashboardTab("overview")}
              >
                Overview
              </button>
              <button
                className={`dashboard-tab ${dashboardTab === "cleanup" ? "active" : ""}`}
                onClick={() => setDashboardTab("cleanup")}
              >
                Cleanup
                {unknownQueue.length > 0 && (
                  <span className="tab-badge">{unknownQueue.length}</span>
                )}
              </button>
              <button
                className={`dashboard-tab ${dashboardTab === "digest" ? "active" : ""}`}
                onClick={() => setDashboardTab("digest")}
              >
                Digest
              </button>
            </nav>

            {/* Overview tab */}
            {dashboardTab === "overview" && <>

            {/* Category breakdown */}
            {activeCategoryBreakdown.length > 0 && (
              <section className="category-section">
                {/* Summary row: total + idle + selected */}
                <div className="category-summary">
                  <span className="summary-total">
                    Total: <strong>{formatDurationLocal(unconditionalTotalMs)}</strong>
                  </span>
                  <span className="summary-active">
                    Active: <strong>{formatDurationLocal(unconditionalActiveMs)}</strong>
                  </span>
                  {totalIdleMs > 0 && (
                    <span className="summary-idle">
                      💤 Idle: {formatDurationLocal(totalIdleMs)}
                    </span>
                  )}
                  {selectedCategories.size > 0 && (
                    <span className="summary-selected">
                      Selected: <strong>{formatDurationLocal(selectedTotalMs)}</strong>
                      <button
                        className="btn btn-xs"
                        onClick={() => setSelectedCategories(new Set())}
                        title="Clear selection"
                      >
                        ✕
                      </button>
                    </span>
                  )}
                </div>
                <div className="category-chips">
                  {activeCategoryBreakdown.map(([cat, ms]) => {
                    const info = getCategoryInfo(cat);
                    const pct = totalTrackedMs > 0 ? Math.round((ms / totalTrackedMs) * 100) : 0;
                    const isSelected = selectedCategories.has(cat);
                    return (
                      <div
                        key={cat}
                        className={`category-chip ${isSelected ? "selected" : ""}`}
                        style={{ borderColor: info.color, backgroundColor: isSelected ? info.color + "20" : undefined }}
                        onClick={() => handleCategoryClick(cat)}
                      >
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

            {/* Timeline bar */}
            {dayRange && timelineSegments.length > 0 && (() => {
              const [rangeStart, rangeEnd] = dayRange;
              const rangeDuration = rangeEnd - rangeStart;
              // Compute hour labels
              const startHour = new Date(rangeStart).getHours();
              const hourLabels: number[] = [];
              for (let h = startHour; h < startHour + 24; h++) {
                const hour = h % 24;
                const hourMs = rangeStart + (h - startHour) * 3600000;
                if (hourMs >= rangeStart && hourMs <= rangeEnd) {
                  hourLabels.push(hour);
                }
              }
              return (
                <section className="timeline-section">
                  <div
                    className="timeline-bar"
                    onMouseLeave={() => setHoveredSegment(null)}
                  >
                    {timelineSegments.map((seg) => {
                      const left = ((seg.start_time - rangeStart) / rangeDuration) * 100;
                      const width = ((seg.end_time - seg.start_time) / rangeDuration) * 100;
                      const catInfo = getCategoryInfo(seg.category);
                      const durationMs = seg.end_time - seg.start_time;
                      const idleMs = seg.idle_seconds * 1000;
                      const idleRatio = durationMs > 0 ? idleMs / durationMs : 0;
                      const mostlyIdle = idleRatio > 0.5;
                      return (
                        <div
                          key={seg.segment_id}
                          className={`timeline-segment ${selectedSegment?.segment_id === seg.segment_id ? "selected" : ""} ${mostlyIdle ? "idle" : ""}`}
                          style={{
                            left: `${left}%`,
                            width: `${Math.max(width, 0.15)}%`,
                            backgroundColor: mostlyIdle ? "var(--bg-tertiary)" : catInfo.color,
                          }}
                          onMouseEnter={(e) => {
                            setHoveredSegment(seg);
                            setTooltipPos({ x: e.clientX, y: e.clientY });
                          }}
                          onMouseMove={(e) => {
                            setTooltipPos({ x: e.clientX, y: e.clientY });
                          }}
                          onClick={() => {
                            setSelectedSegment(
                              selectedSegment?.segment_id === seg.segment_id ? null : seg
                            );
                            setProvenanceTitleHash(null);
                            setProvenance(null);
                          }}
                        />
                      );
                    })}
                  </div>
                  <div className="timeline-labels">
                    {hourLabels.map((hour) => {
                      const hourMs = rangeStart + ((hour - startHour + 24) % 24) * 3600000;
                      const leftPct = ((hourMs - rangeStart) / rangeDuration) * 100;
                      return (
                        <span
                          key={hour}
                          className="timeline-hour-label"
                          style={{ left: `${leftPct}%` }}
                        >
                          {hour.toString().padStart(2, "0")}
                        </span>
                      );
                    })}
                  </div>
                  {hoveredSegment && (
                    <div
                      className="timeline-tooltip"
                      style={{ left: tooltipPos.x + 12, top: tooltipPos.y - 60 }}
                    >
                      <strong>{hoveredSegment.friendly_name}</strong>
                      {hoveredSegment.window_title && (
                        <div className="tooltip-title">{hoveredSegment.window_title}</div>
                      )}
                      <div className="tooltip-time">
                        {new Date(hoveredSegment.start_time).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                        {" - "}
                        {new Date(hoveredSegment.end_time).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                      </div>
                      <div className="tooltip-duration">
                        {formatDurationLocal(hoveredSegment.end_time - hoveredSegment.start_time)}
                        {" "}
                        <span style={{ color: getCategoryInfo(hoveredSegment.category).color }}>
                          {getCategoryInfo(hoveredSegment.category).label}
                        </span>
                      </div>
                      {hoveredSegment.idle_seconds > 0 && (
                        <div className="tooltip-idle">
                          💤 {formatDurationLocal(hoveredSegment.idle_seconds * 1000)} idle
                        </div>
                      )}
                    </div>
                  )}
                  {selectedSegment && (
                    <div className="timeline-detail">
                      <div className="timeline-detail-header">
                        <span className="timeline-detail-app">
                          {getCategoryInfo(selectedSegment.category).emoji}{" "}
                          {selectedSegment.friendly_name}
                        </span>
                        <button
                          className="btn btn-xs"
                          onClick={() => {
                            setSelectedSegment(null);
                            setProvenanceTitleHash(null);
                            setProvenance(null);
                          }}
                        >
                          ✕
                        </button>
                      </div>
                      {selectedSegment.window_title && (
                        <div className="timeline-detail-title">
                          {selectedSegment.window_title}
                        </div>
                      )}
                      <div className="timeline-detail-row">
                        <span>Time</span>
                        <span>
                          {new Date(selectedSegment.start_time).toLocaleTimeString([], {
                            hour: "2-digit",
                            minute: "2-digit",
                            second: "2-digit",
                          })}
                          {" - "}
                          {new Date(selectedSegment.end_time).toLocaleTimeString([], {
                            hour: "2-digit",
                            minute: "2-digit",
                            second: "2-digit",
                          })}
                        </span>
                      </div>
                      <div className="timeline-detail-row">
                        <span>Duration</span>
                        <span>
                          {formatDurationLocal(
                            selectedSegment.end_time - selectedSegment.start_time
                          )}
                        </span>
                      </div>
                      <div className="timeline-detail-row">
                        <span>Category</span>
                        <span
                          style={{
                            color: getCategoryInfo(selectedSegment.category).color,
                          }}
                        >
                          {getCategoryInfo(selectedSegment.category).label}
                        </span>
                      </div>
                      {selectedSegment.idle_seconds > 0 && (
                        <div className="timeline-detail-row">
                          <span>Idle</span>
                          <span>
                            {formatDurationLocal(selectedSegment.idle_seconds * 1000)}
                          </span>
                        </div>
                      )}
                      <button
                        className="btn btn-sm btn-secondary"
                        style={{ marginTop: 8 }}
                        onClick={() =>
                          handleShowProvenance(selectedSegment.title_hash)
                        }
                      >
                        Why "{getCategoryInfo(selectedSegment.category).label}"?
                      </button>
                      {provenanceTitleHash === selectedSegment.title_hash && (
                        <div className="provenance-panel">
                          {provenanceLoading ? (
                            <span>Loading...</span>
                          ) : provenance?.best_label ? (
                            <>
                              <div className="provenance-source">
                                {provenance.best_label.source === "manual" &&
                                  "Manually set by you"}
                                {provenance.best_label.source === "user" &&
                                  "Matched by classification rule"}
                                {provenance.best_label.source === "ai" &&
                                  `AI classified (${Math.round(
                                    (provenance.best_label.confidence || 0) * 100
                                  )}% confidence)`}
                                {provenance.best_label.source === "heuristic" &&
                                  "Heuristic classification (app name pattern)"}
                              </div>
                              {provenance.matching_rule && (
                                <div className="provenance-rule">
                                  {provenance.matching_rule.app_pattern && (
                                    <span className="pattern-badge">
                                      App: {provenance.matching_rule.app_pattern}
                                    </span>
                                  )}
                                  {provenance.matching_rule.title_pattern && (
                                    <span className="pattern-badge">
                                      Title: {provenance.matching_rule.title_pattern}
                                    </span>
                                  )}
                                  <span className="match-type">
                                    ({provenance.matching_rule.match_type})
                                  </span>
                                </div>
                              )}
                            </>
                          ) : (
                            <span className="provenance-none">
                              No label assigned (defaults to "Other")
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </section>
              );
            })()}

            {/* App list */}
            <section className="app-list">
              <h2>
                {selectedCategories.size > 0
                  ? "Selected Breakdown"
                  : "Application Usage"}
              </h2>
              {selectedCategories.size > 0 && (
                <p className="setting-description">
                  Click category chips above to filter what's shown here (browsers are broken down
                  by site).
                </p>
              )}
              {selectedCategories.size > 0 ? (
                <div className="app-table">
                  <div className="app-row header">
                    <span className="app-name">Activity</span>
                    <span className="app-time">Time</span>
                    <span className="app-idle">Idle</span>
                  </div>
                  {selectedBreakdownView.visibleRows.length === 0 ? (
                    <div className="app-row">
                      <span
                        className="app-name"
                        style={{ color: "var(--text-muted)" }}
                      >
                        {selectedBreakdownLoading
                          ? "Loading breakdown..."
                          : "No matching activity"}
                      </span>
                      <span className="app-time">-</span>
                      <span className="app-idle">-</span>
                    </div>
                  ) : (
                    <>
                      {selectedBreakdownView.visibleRows.map((row) => {
                        const catInfo = getCategoryInfo(row.category);
                        const contextLabel =
                          row.context === "other" ? "Other sites" : row.context;
                        const label = row.context
                          ? `${row.friendly_name} · ${contextLabel}`
                          : row.friendly_name;
                        return (
                          <div
                            key={`${row.app_name}:${row.context || ""}:${row.category}`}
                            className="app-row"
                            onContextMenu={(e) =>
                              row.context
                                ? handleContextRowContextMenu(
                                    e,
                                    row.app_name,
                                    row.context
                                  )
                                : handleContextMenu(e, row.app_name)
                            }
                          >
                            <span className="app-name">
                              <span className="app-icon">{catInfo.emoji}</span>
                              {label}
                            </span>
                            <span className="app-time">
                              {formatDurationLocal(row.displayMs)}
                            </span>
                            <span className="app-idle">
                              {row.idle_duration_ms > 0
                                ? `💤 ${formatDurationLocal(row.idle_duration_ms)}`
                                : "-"}
                            </span>
                          </div>
                        );
                      })}
                      {selectedBreakdownView.otherMs >= 1000 && (
                        <div key="other-small" className="app-row">
                          <span
                            className="app-name"
                            style={{ color: "var(--text-muted)" }}
                          >
                            Other (small items)
                          </span>
                          <span className="app-time">
                            {formatDurationLocal(selectedBreakdownView.otherMs)}
                          </span>
                          <span className="app-idle">-</span>
                        </div>
                      )}
                    </>
                  )}
                </div>
              ) : filteredApps.length === 0 ? (
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
                    const isBrowser = BROWSER_APP_RE.test(app.app_name);
                    const isExpanded = expandedApp === app.app_name;
                    return (
                      <div key={app.app_name} className="app-row-container">
                        <div
                          className={`app-row ${isExpanded ? "expanded" : ""}`}
                          onContextMenu={(e) => handleContextMenu(e, app.app_name)}
                          onClick={isBrowser ? () => handleToggleAppExpand(app.app_name) : undefined}
                          style={isBrowser ? { cursor: "pointer" } : undefined}
                        >
                          <span className="app-name">
                            {isBrowser && (
                              <span className="expand-icon">
                                {isExpanded ? "▼" : "▶"}
                              </span>
                            )}
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
                        {/* Context drill-down for browsers */}
                        {isExpanded && (
                          <div className="context-list">
                            {contextsLoading ? (
                              <div className="context-loading">Loading...</div>
                            ) : appContexts.length === 0 ? (
                              <div className="context-empty">No site data</div>
                            ) : (
                              appContexts
                                .filter((ctx) => {
                                  const activeMs = showActiveOnly
                                    ? ctx.total_duration_ms - ctx.idle_duration_ms
                                    : ctx.total_duration_ms;
                                  return activeMs >= 5000;
                                })
                                .map((ctx) => {
                                  const ctxCatInfo = getCategoryInfo(ctx.category);
                                  const ctxDisplayMs = showActiveOnly
                                    ? ctx.total_duration_ms - ctx.idle_duration_ms
                                    : ctx.total_duration_ms;
                                  return (
                                    <div
                                      key={ctx.context}
                                      className="context-row"
                                      onContextMenu={(e) =>
                                        handleContextRowContextMenu(e, app.app_name, ctx.context)
                                      }
                                    >
                                      <span className="context-name">
                                        <span className="context-icon">{ctxCatInfo.emoji}</span>
                                        {ctx.context}
                                      </span>
                                      <span className="context-time">
                                        {formatDurationLocal(ctxDisplayMs)}
                                      </span>
                                      {/* Hide + Rule for "other" since it won't match anything */}
                                      {ctx.context !== "other" && (
                                        <button
                                          className="btn btn-sm"
                                          onClick={() => handleCreateRuleFromContext(ctx, app.app_name)}
                                          title="Create rule for this site"
                                        >
                                          + Rule
                                        </button>
                                      )}
                                    </div>
                                  );
                                })
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </section>

            </>}

            {/* Cleanup tab */}
            {dashboardTab === "cleanup" && (
              <div className="tab-content">
                <section className="cleanup-queue">
                  <h2>Unknown Activity</h2>
                  <p className="setting-description">
                    These activities have no category. Click "+ Rule" to create a classification rule.
                  </p>

                  {unknownQueueLoading && unknownQueue.length === 0 ? (
                    <p className="no-data">Loading...</p>
                  ) : unknownQueue.length === 0 ? (
                    <p className="no-data">All caught up! No unknown activity for this day.</p>
                  ) : (
                    <div className="app-table">
                      <div className="app-row header">
                        <span className="app-name">Activity</span>
                        <span className="app-time">Time</span>
                        <span className="app-idle">Action</span>
                      </div>
                      {unknownQueue.map((item) => {
                        const label = item.context
                          ? `${item.friendly_name} \u00b7 ${item.context}`
                          : item.friendly_name;
                        return (
                          <div
                            key={`${item.app_name}:${item.context || ""}`}
                            className="app-row"
                          >
                            <span className="app-name">
                              <span className="app-icon">📁</span>
                              {label}
                              <span
                                className="cleanup-sample"
                                title={item.sample_titles.join("\n")}
                              >
                                ({item.segment_count} segments)
                              </span>
                            </span>
                            <span className="app-time">
                              {formatDurationLocal(item.total_duration_ms)}
                            </span>
                            <span className="app-idle">
                              <button
                                className="btn btn-sm btn-primary"
                                onClick={() => {
                                  setEditingRule(null);
                                  setRuleForm({
                                    appPattern: item.app_name,
                                    titlePattern: item.context || "",
                                    matchType: "contains",
                                    category: "productivity",
                                  });
                                  setRulePreview(null);
                                  setShowRuleForm(true);
                                }}
                              >
                                + Rule
                              </button>
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </section>
              </div>
            )}

            {/* Digest tab */}
            {dashboardTab === "digest" && (
              <div className="tab-content">
                {digestLoading && !digest ? (
                  <p className="no-data">Loading digest...</p>
                ) : !digest || digest.total_tracked_ms === 0 ? (
                  <p className="no-data">No data for this day</p>
                ) : (
                  <div className="digest-grid">
                    <div className="digest-card">
                      <div className="digest-card-title">
                        {showActiveOnly ? "Active Time" : "Total Time"}
                      </div>
                      <div className="digest-card-value">
                        {formatDurationLocal(
                          showActiveOnly ? digest.total_active_ms : digest.total_tracked_ms
                        )}
                      </div>
                      <div className="digest-card-detail">
                        {showActiveOnly
                          ? `Total: ${formatDurationLocal(digest.total_tracked_ms)}`
                          : `Active: ${formatDurationLocal(digest.total_active_ms)}`}
                      </div>
                    </div>

                    <div className="digest-card">
                      <div className="digest-card-title">Top Categories</div>
                      {digest.top_categories.map((cat) => {
                        const info = getCategoryInfo(cat.category);
                        const displayMs = showActiveOnly
                          ? cat.duration_ms - cat.idle_ms
                          : cat.duration_ms;
                        const displayTotal = showActiveOnly
                          ? digest.total_active_ms
                          : digest.total_tracked_ms;
                        const pct = displayTotal > 0
                          ? Math.round((displayMs / displayTotal) * 100)
                          : 0;
                        return (
                          <div key={cat.category} className="digest-row">
                            <span>{info.emoji} {info.label}</span>
                            <span>
                              {formatDurationLocal(displayMs)} ({pct}%)
                            </span>
                          </div>
                        );
                      })}
                    </div>

                    <div className="digest-card">
                      <div className="digest-card-title">Top Apps</div>
                      {digest.top_apps.map((app) => {
                        const info = getCategoryInfo(app.category);
                        const displayMs = showActiveOnly
                          ? app.duration_ms - app.idle_ms
                          : app.duration_ms;
                        return (
                          <div key={app.app_name} className="digest-row">
                            <span>{info.emoji} {app.friendly_name}</span>
                            <span>{formatDurationLocal(displayMs)}</span>
                          </div>
                        );
                      })}
                    </div>

                    {digest.longest_focus && (
                      <div className="digest-card">
                        <div className="digest-card-title">Longest Focus Block</div>
                        <div className="digest-card-value">
                          {formatDurationLocal(digest.longest_focus.duration_ms)}
                        </div>
                        <div className="digest-card-detail">
                          {digest.longest_focus.friendly_name}
                        </div>
                      </div>
                    )}

                    {digest.most_idle && (
                      <div className="digest-card">
                        <div className="digest-card-title">Most Idle Window</div>
                        <div className="digest-card-value">
                          {formatDurationLocal(digest.most_idle.idle_seconds * 1000)}
                        </div>
                        <div className="digest-card-detail">
                          {digest.most_idle.friendly_name}
                          {digest.most_idle.window_title && (
                            <span className="digest-card-subtitle">
                              {" "}&mdash; {digest.most_idle.window_title}
                            </span>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
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

            {/* AI Suggestions */}
            {suggestions.length > 0 && (
              <section className="setting-section">
                <h3>AI Suggestions</h3>
                <p className="setting-description">
                  Review AI-generated categorization suggestions. Approve to create a rule, or reject to dismiss.
                </p>

                <div className="suggestions-list">
                  {suggestions.map((suggestion) => {
                    const catInfo = getCategoryInfo(suggestion.suggested_category);
                    return (
                      <div key={suggestion.suggestion_id} className="suggestion-item">
                        <div className="suggestion-info">
                          <div className="suggestion-header">
                            <span className="suggestion-category">
                              {catInfo.emoji} {catInfo.label}
                            </span>
                            <span className="suggestion-confidence">
                              {Math.round(suggestion.confidence * 100)}% confident
                            </span>
                          </div>
                          <div className="suggestion-pattern">
                            {suggestion.app_pattern && (
                              <span className="pattern-badge">
                                App: {suggestion.app_pattern}
                              </span>
                            )}
                            {suggestion.title_pattern && (
                              <span className="pattern-badge">
                                Title: {suggestion.title_pattern}
                              </span>
                            )}
                          </div>
                          <p className="suggestion-reason">{suggestion.reason}</p>
                          <div className="suggestion-stats">
                            <span>{suggestion.match_count} matches</span>
                            <span>{formatDurationLocal(suggestion.total_duration_ms)} total</span>
                          </div>
                          {suggestion.sample_titles.length > 0 && (
                            <ul className="suggestion-samples">
                              {suggestion.sample_titles.slice(0, 3).map((title, i) => (
                                <li key={i}>{title}</li>
                              ))}
                            </ul>
                          )}
                        </div>
                        <div className="suggestion-actions">
                          <button
                            className="btn btn-sm btn-success"
                            onClick={() => handleApproveSuggestion(suggestion.suggestion_id)}
                            title="Approve - create rule"
                          >
                            ✓ Approve
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() => handleRejectSuggestion(suggestion.suggestion_id)}
                            title="Reject - dismiss suggestion"
                          >
                            ✗ Reject
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            )}

            <section className="setting-section">
              <h3>Classification Rules</h3>
              <p className="setting-description">
                Create rules to automatically categorize apps and websites based on patterns.
              </p>

              {/* Rule list */}
              <div className="rules-list">
                {rules.length === 0 ? (
                  <p className="no-data">No rules defined yet</p>
                ) : (
                  rules.map((rule) => {
                    const catInfo = getCategoryInfo(rule.category);
                    return (
                      <div
                        key={rule.rule_id}
                        className={`rule-item ${!rule.enabled ? "disabled" : ""}`}
                      >
                        <div className="rule-info">
                          <span className="rule-category">
                            {catInfo.emoji} {catInfo.label}
                          </span>
                          <span className="rule-pattern">
                            {rule.app_pattern && (
                              <span className="pattern-badge">
                                App: {rule.app_pattern}
                              </span>
                            )}
                            {rule.title_pattern && (
                              <span className="pattern-badge">
                                Title: {rule.title_pattern}
                              </span>
                            )}
                            <span className="match-type">({rule.match_type})</span>
                          </span>
                          <span className="rule-source">{rule.source}</span>
                        </div>
                        <div className="rule-actions">
                          <button
                            className="btn btn-sm"
                            onClick={() => handleToggleRule(rule)}
                            title={rule.enabled ? "Disable" : "Enable"}
                          >
                            {rule.enabled ? "✓" : "○"}
                          </button>
                          <button
                            className="btn btn-sm"
                            onClick={() => handleEditRule(rule)}
                            title="Edit"
                          >
                            ✏️
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() => handleDeleteRule(rule.rule_id)}
                            title="Delete"
                          >
                            🗑️
                          </button>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>

              <button className="btn btn-primary" onClick={handleAddRule}>
                + Add Rule
              </button>
            </section>

            <section className="setting-section">
              <h3>About</h3>
              <p className="setting-description">
                MyTime v0.1.0 - Personal Time Tracking
              </p>
            </section>
          </div>
        )}

        {/* Rule form modal */}
        {showRuleForm && (
          <div className="modal-overlay" onClick={() => setShowRuleForm(false)}>
            <div className="modal" onClick={(e) => e.stopPropagation()}>
              <h3>{editingRule ? "Edit Rule" : "Add Rule"}</h3>

              <div className="form-group">
                <label>App Pattern</label>
                <input
                  type="text"
                  value={ruleForm.appPattern}
                  onChange={(e) =>
                    setRuleForm({ ...ruleForm, appPattern: e.target.value })
                  }
                  placeholder="e.g., msedge, chrome, code"
                />
                <span className="form-help">Match app name (exe filename)</span>
              </div>

              <div className="form-group">
                <label>Title Pattern</label>
                <input
                  type="text"
                  value={ruleForm.titlePattern}
                  onChange={(e) =>
                    setRuleForm({ ...ruleForm, titlePattern: e.target.value })
                  }
                  placeholder="e.g., YouTube, GitHub, Slack"
                />
                <span className="form-help">Match window title text</span>
              </div>

              <div className="form-group">
                <label>Match Type</label>
                <select
                  value={ruleForm.matchType}
                  onChange={(e) =>
                    setRuleForm({
                      ...ruleForm,
                      matchType: e.target.value as MatchType,
                    })
                  }
                >
                  <option value="contains">Contains</option>
                  <option value="prefix">Starts with</option>
                  <option value="exact">Exact match</option>
                  <option value="regex">Regex</option>
                </select>
              </div>

              <div className="form-group">
                <label>Category</label>
                <select
                  value={ruleForm.category}
                  onChange={(e) =>
                    setRuleForm({ ...ruleForm, category: e.target.value })
                  }
                >
                  {(Object.keys(CATEGORY_INFO) as Category[])
                    .filter((cat) => cat !== "unknown")
                    .map((cat) => {
                      const info = CATEGORY_INFO[cat];
                      return (
                        <option key={cat} value={cat}>
                          {info.emoji} {info.label}
                        </option>
                      );
                    })}
                </select>
              </div>

              {/* Preview */}
              <div className="rule-preview">
                <button
                  className="btn btn-secondary"
                  onClick={handlePreviewRule}
                  disabled={previewLoading}
                >
                  {previewLoading ? "Loading..." : "Preview Matches"}
                </button>
                {rulePreview && (
                  <div className="preview-result">
                    <p>
                      <strong>{rulePreview.match_count}</strong> matches (
                      {formatDurationLocal(rulePreview.total_duration_ms)} total)
                    </p>
                    {rulePreview.sample_titles.length > 0 && (
                      <ul className="sample-titles">
                        {rulePreview.sample_titles.slice(0, 3).map((title, i) => (
                          <li key={i}>{title}</li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </div>

              <div className="modal-actions">
                <button
                  className="btn btn-secondary"
                  onClick={() => setShowRuleForm(false)}
                >
                  Cancel
                </button>
                <button className="btn btn-primary" onClick={handleSaveRule}>
                  {editingRule ? "Update" : "Create"}
                </button>
              </div>
            </div>
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
