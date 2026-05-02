import { useMemo, useState } from "react";
import { CategoryChips } from "./CategoryChips";
import { Timeline } from "./Timeline";
import { AppList } from "./AppList";
import { SelectedBreakdownList } from "./SelectedBreakdownList";
import type {
  AppSummary,
  CategoryBreakdownEntry,
  ContextSummary,
  LabelProvenance,
  SelectedBreakdownRow,
  TimelineSegment,
} from "../types";

interface OverviewTabProps {
  appBreakdown: AppSummary[];
  categoryBreakdown: CategoryBreakdownEntry[];
  showActiveOnly: boolean;
  selectedCategories: Set<string>;
  onCategoryClick: (category: string) => void;
  onClearSelection: () => void;
  selectedBreakdown: SelectedBreakdownRow[];
  selectedBreakdownLoading: boolean;
  timelineSegments: TimelineSegment[];
  dayRange: [number, number] | null;
  expandedApp: string | null;
  appContexts: ContextSummary[];
  contextsLoading: boolean;
  onToggleAppExpand: (appName: string) => void;
  onAppContextMenu: (e: React.MouseEvent, appName: string) => void;
  onContextRowMenu: (
    e: React.MouseEvent,
    appName: string,
    context: string
  ) => void;
  onCreateRuleFromContext: (context: ContextSummary, appName: string) => void;
  provenanceTitleHash: string | null;
  provenance: LabelProvenance | null;
  provenanceLoading: boolean;
  onShowProvenance: (titleHash: string) => void;
  onClearProvenance: () => void;
}

export function OverviewTab(props: OverviewTabProps) {
  const {
    appBreakdown,
    categoryBreakdown,
    showActiveOnly,
    selectedCategories,
    onCategoryClick,
    onClearSelection,
    selectedBreakdown,
    selectedBreakdownLoading,
    timelineSegments,
    dayRange,
    expandedApp,
    appContexts,
    contextsLoading,
    onToggleAppExpand,
    onAppContextMenu,
    onContextRowMenu,
    onCreateRuleFromContext,
    provenanceTitleHash,
    provenance,
    provenanceLoading,
    onShowProvenance,
    onClearProvenance,
  } = props;

  const [selectedSegment, setSelectedSegment] = useState<TimelineSegment | null>(null);

  // Filter apps by displayMs threshold
  const filteredApps = useMemo(() => {
    return appBreakdown
      .map((app) => {
        const displayMs = showActiveOnly
          ? app.total_duration_ms - app.idle_duration_ms
          : app.total_duration_ms;
        return { ...app, displayMs };
      })
      .filter((app) => app.displayMs >= 5000);
  }, [appBreakdown, showActiveOnly]);

  // Active category breakdown (respects showActiveOnly)
  const activeCategoryBreakdown = useMemo<[string, number][]>(() => {
    if (!showActiveOnly) {
      return categoryBreakdown
        .map((entry) => [entry.category, entry.total_ms] as [string, number])
        .filter(([, ms]) => ms >= 5000)
        .sort((a, b) => b[1] - a[1]);
    }
    return categoryBreakdown
      .map((entry) => [entry.category, entry.total_ms - entry.idle_ms] as [string, number])
      .filter(([, ms]) => ms >= 5000)
      .sort((a, b) => b[1] - a[1]);
  }, [showActiveOnly, categoryBreakdown]);

  const totalIdleMs = useMemo(
    () => categoryBreakdown.reduce((sum, entry) => sum + entry.idle_ms, 0),
    [categoryBreakdown]
  );
  const totalTrackedMs = activeCategoryBreakdown.reduce((sum, [, ms]) => sum + ms, 0);
  const unconditionalTotalMs = useMemo(
    () => categoryBreakdown.reduce((sum, entry) => sum + entry.total_ms, 0),
    [categoryBreakdown]
  );
  const unconditionalActiveMs = unconditionalTotalMs - totalIdleMs;

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
    return { visibleRows, otherMs };
  }, [selectedBreakdown, showActiveOnly]);

  const handleSelectSegment = (segment: TimelineSegment | null) => {
    setSelectedSegment(segment);
    onClearProvenance();
  };

  return (
    <>
      <CategoryChips
        breakdown={activeCategoryBreakdown}
        totalTrackedMs={totalTrackedMs}
        selectedCategories={selectedCategories}
        onCategoryClick={onCategoryClick}
        unconditionalTotalMs={unconditionalTotalMs}
        unconditionalActiveMs={unconditionalActiveMs}
        totalIdleMs={totalIdleMs}
        selectedTotalMs={selectedTotalMs}
        onClearSelection={onClearSelection}
      />

      {dayRange && timelineSegments.length > 0 && (
        <Timeline
          segments={timelineSegments}
          dayRange={dayRange}
          selectedSegment={selectedSegment}
          onSelectSegment={handleSelectSegment}
          provenanceTitleHash={provenanceTitleHash}
          provenance={provenance}
          provenanceLoading={provenanceLoading}
          onShowProvenance={onShowProvenance}
        />
      )}

      <section className="app-list">
        <h2>
          {selectedCategories.size > 0 ? "Selected Breakdown" : "Application Usage"}
        </h2>
        {selectedCategories.size > 0 && (
          <p className="setting-description">
            Click category chips above to filter what's shown here (browsers are broken down by site).
          </p>
        )}
        {selectedCategories.size > 0 ? (
          <SelectedBreakdownList
            visibleRows={selectedBreakdownView.visibleRows}
            otherMs={selectedBreakdownView.otherMs}
            loading={selectedBreakdownLoading}
            onAppContextMenu={onAppContextMenu}
            onContextRowMenu={onContextRowMenu}
          />
        ) : (
          <AppList
            apps={filteredApps}
            showActiveOnly={showActiveOnly}
            expandedApp={expandedApp}
            contexts={appContexts}
            contextsLoading={contextsLoading}
            onToggleExpand={onToggleAppExpand}
            onAppContextMenu={onAppContextMenu}
            onContextRowMenu={onContextRowMenu}
            onCreateRuleFromContext={onCreateRuleFromContext}
          />
        )}
      </section>
    </>
  );
}
