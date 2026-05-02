import { useEffect, useState } from "react";
import {
  approveSuggestion,
  createRule,
  deleteRule,
  rejectSuggestion,
  setAppCategory,
  updateRule,
} from "./api";
import { Sidebar } from "./components/Sidebar";
import { DashboardHeader } from "./components/DashboardHeader";
import { DateNav } from "./components/DateNav";
import { DashboardTabs } from "./components/DashboardTabs";
import type { DashboardTab } from "./components/DashboardTabs";
import { OverviewTab } from "./components/OverviewTab";
import { CleanupTab } from "./components/CleanupTab";
import { DigestTab } from "./components/DigestTab";
import { SettingsPage } from "./components/SettingsPage";
import { ContextMenu } from "./components/ContextMenu";
import type { ContextMenuState } from "./components/ContextMenu";
import { RuleFormModal } from "./components/RuleFormModal";
import type { RuleFormState } from "./components/RuleFormModal";
import { useTrackingState } from "./hooks/useTrackingState";
import { useDayBreakdown } from "./hooks/useDayBreakdown";
import { useSelectedBreakdown } from "./hooks/useSelectedBreakdown";
import { useTimeline } from "./hooks/useTimeline";
import { useUnknownQueue } from "./hooks/useUnknownQueue";
import { useDigest } from "./hooks/useDigest";
import { useSettings } from "./hooks/useSettings";
import { useRules } from "./hooks/useRules";
import { useSuggestions } from "./hooks/useSuggestions";
import { useAppContexts } from "./hooks/useAppContexts";
import { useProvenance } from "./hooks/useProvenance";
import type {
  ClassificationRule,
  ContextSummary,
  UnknownQueueItem,
} from "./types";
import "./App.css";

type Page = "dashboard" | "settings";

const DEFAULT_RULE_FORM: RuleFormState = {
  appPattern: "",
  titlePattern: "",
  matchType: "contains",
  category: "productivity",
};

function App() {
  const [currentPage, setCurrentPage] = useState<Page>("dashboard");
  const [dashboardTab, setDashboardTab] = useState<DashboardTab>("overview");
  const [dayOffset, setDayOffset] = useState(0);
  const [showActiveOnly, setShowActiveOnly] = useState(true);
  const [selectedCategories, setSelectedCategories] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // Rule form modal state
  const [showRuleForm, setShowRuleForm] = useState(false);
  const [editingRule, setEditingRule] = useState<ClassificationRule | null>(null);
  const [ruleFormInitial, setRuleFormInitial] = useState<RuleFormState>(DEFAULT_RULE_FORM);

  // Data hooks
  const tracking = useTrackingState();
  const breakdown = useDayBreakdown(dayOffset);
  const selectedBreakdown = useSelectedBreakdown(dayOffset, selectedCategories);
  const timeline = useTimeline(dayOffset);
  const unknownQueue = useUnknownQueue(dayOffset);
  const digest = useDigest(dayOffset);
  const settings = useSettings();
  const rules = useRules();
  const suggestions = useSuggestions();
  const contexts = useAppContexts(dayOffset);
  const provenance = useProvenance();

  // Close context menu on click outside
  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, []);

  // Reload helpers (used after mutations)
  const reloadDayData = async () => {
    await Promise.all([
      breakdown.reload(),
      timeline.reload(),
      unknownQueue.reload(),
      digest.reload(),
      selectedBreakdown.reload(),
      contexts.reloadExpanded(),
    ]);
    contexts.invalidate();
  };

  // Live timer display
  const currentSessionMs =
    tracking.trackingState.is_tracking && tracking.trackingState.session_start_ms
      ? Date.now() - tracking.trackingState.session_start_ms
      : 0;
  const displayTimeMs =
    tracking.trackingState.is_tracking && tracking.trackingState.baseline_ms !== null
      ? tracking.trackingState.baseline_ms + currentSessionMs
      : tracking.trackingState.total_time_ms;

  // Handlers
  const handleAppContextMenu = (e: React.MouseEvent, appName: string) => {
    e.preventDefault();
    setContextMenu({ kind: "app", x: e.clientX, y: e.clientY, appName });
  };

  const handleContextRowMenu = (
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
        await rules.reload();
      }
      setContextMenu(null);
      await reloadDayData();
    } catch (err) {
      console.error("Failed to set category:", err);
    }
  };

  const handleCategoryClick = (category: string) => {
    setSelectedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) next.delete(category);
      else next.add(category);
      return next;
    });
  };

  const handleDayStartHourChange = async (hour: number) => {
    await settings.updateDayStartHour(hour);
    await reloadDayData();
  };

  const handleAddRule = () => {
    setEditingRule(null);
    setRuleFormInitial(DEFAULT_RULE_FORM);
    setShowRuleForm(true);
  };

  const handleEditRule = (rule: ClassificationRule) => {
    setEditingRule(rule);
    setRuleFormInitial({
      appPattern: rule.app_pattern || "",
      titlePattern: rule.title_pattern || "",
      matchType: rule.match_type,
      category: rule.category,
    });
    setShowRuleForm(true);
  };

  const handleDeleteRule = async (ruleId: string) => {
    try {
      await deleteRule(ruleId);
      await rules.reload();
      await reloadDayData();
    } catch (err) {
      console.error("Failed to delete rule:", err);
    }
  };

  const handleSaveRule = async (form: RuleFormState) => {
    try {
      const appPattern = form.appPattern.trim() || null;
      const titlePattern = form.titlePattern.trim() || null;

      if (editingRule) {
        await updateRule(
          editingRule.rule_id,
          appPattern,
          titlePattern,
          form.matchType,
          form.category,
          null,
          editingRule.enabled,
          editingRule.priority
        );
      } else {
        await createRule(appPattern, titlePattern, form.matchType, form.category, null);
      }
      setShowRuleForm(false);
      await rules.reload();
      await reloadDayData();
    } catch (err) {
      console.error("Failed to save rule:", err);
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
      await rules.reload();
      await reloadDayData();
    } catch (err) {
      console.error("Failed to toggle rule:", err);
    }
  };

  const handleApproveSuggestion = async (suggestionId: string) => {
    try {
      await approveSuggestion(suggestionId);
      await suggestions.reload();
      await rules.reload();
      await reloadDayData();
    } catch (err) {
      console.error("Failed to approve suggestion:", err);
    }
  };

  const handleRejectSuggestion = async (suggestionId: string) => {
    try {
      await rejectSuggestion(suggestionId);
      await suggestions.reload();
    } catch (err) {
      console.error("Failed to reject suggestion:", err);
    }
  };

  const handleCreateRuleFromContext = (context: ContextSummary, appName: string) => {
    setEditingRule(null);
    setRuleFormInitial({
      appPattern: appName,
      titlePattern: context.context,
      matchType: "contains",
      category: context.category || "productivity",
    });
    setShowRuleForm(true);
  };

  const handleCreateRuleFromCleanup = (item: UnknownQueueItem) => {
    setEditingRule(null);
    setRuleFormInitial({
      appPattern: item.app_name,
      titlePattern: item.context || "",
      matchType: "contains",
      category: "productivity",
    });
    setShowRuleForm(true);
  };

  return (
    <div className="app-container">
      <Sidebar
        currentPage={currentPage}
        onPageChange={setCurrentPage}
        isTracking={tracking.trackingState.is_tracking}
      />

      <main className="main-content">
        {currentPage === "dashboard" && (
          <div className="dashboard">
            <DashboardHeader
              isTracking={tracking.trackingState.is_tracking}
              displayTimeMs={displayTimeMs}
              onStart={tracking.start}
              onStop={tracking.stop}
            />

            <DateNav
              dayLabel={breakdown.dayLabel}
              dayOffset={dayOffset}
              showActiveOnly={showActiveOnly}
              onPrev={() => setDayOffset((prev) => prev - 1)}
              onNext={() => setDayOffset((prev) => Math.min(prev + 1, 0))}
              onToggleActiveOnly={setShowActiveOnly}
            />

            <DashboardTabs
              active={dashboardTab}
              onChange={setDashboardTab}
              cleanupBadgeCount={unknownQueue.queue.length}
            />

            {dashboardTab === "overview" && (
              <OverviewTab
                appBreakdown={breakdown.appBreakdown}
                categoryBreakdown={breakdown.categoryBreakdown}
                showActiveOnly={showActiveOnly}
                selectedCategories={selectedCategories}
                onCategoryClick={handleCategoryClick}
                onClearSelection={() => setSelectedCategories(new Set())}
                selectedBreakdown={selectedBreakdown.rows}
                selectedBreakdownLoading={selectedBreakdown.loading}
                timelineSegments={timeline.segments}
                dayRange={timeline.dayRange}
                expandedApp={contexts.expandedApp}
                appContexts={contexts.contexts}
                contextsLoading={contexts.loading}
                onToggleAppExpand={contexts.toggle}
                onAppContextMenu={handleAppContextMenu}
                onContextRowMenu={handleContextRowMenu}
                onCreateRuleFromContext={handleCreateRuleFromContext}
                provenanceTitleHash={provenance.titleHash}
                provenance={provenance.provenance}
                provenanceLoading={provenance.loading}
                onShowProvenance={provenance.show}
                onClearProvenance={provenance.clear}
              />
            )}

            {dashboardTab === "cleanup" && (
              <CleanupTab
                queue={unknownQueue.queue}
                loading={unknownQueue.loading}
                onCreateRule={handleCreateRuleFromCleanup}
              />
            )}

            {dashboardTab === "digest" && (
              <DigestTab
                digest={digest.digest}
                loading={digest.loading}
                showActiveOnly={showActiveOnly}
              />
            )}
          </div>
        )}

        {currentPage === "settings" && (
          <SettingsPage
            dayLabel={breakdown.dayLabel}
            dayOffset={dayOffset}
            dayStartHour={settings.dayStartHour}
            autostartEnabled={settings.autostartEnabled}
            rules={rules.rules}
            suggestions={suggestions.suggestions}
            onDayStartHourChange={handleDayStartHourChange}
            onAutostartToggle={settings.updateAutostart}
            onAddRule={handleAddRule}
            onEditRule={handleEditRule}
            onDeleteRule={handleDeleteRule}
            onToggleRule={handleToggleRule}
            onApproveSuggestion={handleApproveSuggestion}
            onRejectSuggestion={handleRejectSuggestion}
          />
        )}

        {showRuleForm && (
          <RuleFormModal
            editingRule={editingRule}
            initialForm={ruleFormInitial}
            onSave={handleSaveRule}
            onCancel={() => setShowRuleForm(false)}
          />
        )}
      </main>

      {contextMenu && (
        <ContextMenu menu={contextMenu} onSelect={handleSetCategory} />
      )}
    </div>
  );
}

export default App;
