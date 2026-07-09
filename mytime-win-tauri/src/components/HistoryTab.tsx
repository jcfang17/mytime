import { useMemo, useState } from "react";
import { formatDurationLocal } from "../api";
import { CATEGORY_ORDER, getCategoryInfo } from "../types";
import type { CategoryBreakdownEntry, DayHistory } from "../types";
import { PERIOD_DAYS, useHistory } from "../hooks/useHistory";
import type { HistoryPeriod } from "../hooks/useHistory";

interface HistoryTabProps {
  showActiveOnly: boolean;
  onToggleActiveOnly: (enabled: boolean) => void;
  onSelectDay: (dayOffset: number) => void;
}

const CHART_HEIGHT = 180;
const NICE_HOURS = [1, 2, 3, 4, 6, 8, 10, 12, 16, 24];
const HOUR_MS = 3_600_000;

interface BarTooltip {
  day: DayHistory;
  x: number;
  y: number;
}

function niceCeilMs(maxMs: number): number {
  const hours = maxMs / HOUR_MS;
  const nice = NICE_HOURS.find((h) => h >= hours) ?? 24;
  return nice * HOUR_MS;
}

export function HistoryTab({
  showActiveOnly,
  onToggleActiveOnly,
  onSelectDay,
}: HistoryTabProps) {
  const [period, setPeriod] = useState<HistoryPeriod>("week");
  const [endOffset, setEndOffset] = useState(0);
  const [tooltip, setTooltip] = useState<BarTooltip | null>(null);

  const periodDays = PERIOD_DAYS[period];
  const { days, prevDays, topApps, loading } = useHistory(period, endOffset);

  const catMs = (c: CategoryBreakdownEntry) =>
    showActiveOnly ? c.total_ms - c.idle_ms : c.total_ms;
  const dayMs = (d: DayHistory) => (showActiveOnly ? d.active_ms : d.total_ms);

  const stats = useMemo(() => {
    const total = days.reduce((sum, d) => sum + dayMs(d), 0);
    const prevTotal = prevDays.reduce((sum, d) => sum + dayMs(d), 0);
    const trackedDays = days.filter((d) => d.total_ms > 0).length;

    const catTotals = new Map<string, number>();
    for (const d of days) {
      for (const c of d.categories) {
        catTotals.set(c.category, (catTotals.get(c.category) ?? 0) + catMs(c));
      }
    }
    let topCategory: { category: string; ms: number } | null = null;
    for (const [category, ms] of catTotals) {
      if (category !== "unknown" && (!topCategory || ms > topCategory.ms)) {
        topCategory = { category, ms };
      }
    }

    return { total, prevTotal, trackedDays, catTotals, topCategory };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [days, prevDays, showActiveOnly]);

  const maxMs = niceCeilMs(Math.max(...days.map(dayMs), 1));
  const presentCategories = CATEGORY_ORDER.filter(
    (cat) => (stats.catTotals.get(cat) ?? 0) > 0
  );
  const hasData = stats.trackedDays > 0;

  const rangeLabel =
    days.length > 0
      ? `${days[0].date_label} – ${days[days.length - 1].date_label}`
      : "";

  const deltaLabel = (() => {
    if (stats.prevTotal <= 0) return null;
    const delta = (stats.total - stats.prevTotal) / stats.prevTotal;
    const pct = Math.abs(Math.round(delta * 100));
    return `${delta >= 0 ? "▲" : "▼"} ${pct}% vs previous ${periodDays} days`;
  })();

  const changePeriod = (next: HistoryPeriod) => {
    setPeriod(next);
    setEndOffset(0);
  };

  const showBarLabel = (index: number) =>
    period === "week" || index % 5 === 0 || index === days.length - 1;

  return (
    <div className="tab-content">
      <div className="history-controls">
        <div className="period-toggle">
          <button
            className={period === "week" ? "active" : ""}
            onClick={() => changePeriod("week")}
          >
            7 days
          </button>
          <button
            className={period === "month" ? "active" : ""}
            onClick={() => changePeriod("month")}
          >
            30 days
          </button>
        </div>
        <button
          className="btn btn-icon"
          onClick={() => setEndOffset((prev) => prev - periodDays)}
        >
          ◀
        </button>
        <span className="date-label history-range-label">{rangeLabel}</span>
        <button
          className="btn btn-icon"
          onClick={() => setEndOffset((prev) => Math.min(prev + periodDays, 0))}
          disabled={endOffset >= 0}
        >
          ▶
        </button>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={showActiveOnly}
            onChange={(e) => onToggleActiveOnly(e.target.checked)}
          />
          Active only
        </label>
      </div>

      {loading && days.length === 0 ? (
        <p className="no-data">Loading history...</p>
      ) : !hasData ? (
        <p className="no-data">No data in this period</p>
      ) : (
        <>
          <div className="digest-grid history-stats">
            <div className="digest-card">
              <div className="digest-card-title">
                {showActiveOnly ? "Active Time" : "Total Time"}
              </div>
              <div className="digest-card-value">
                {formatDurationLocal(stats.total)}
              </div>
              {deltaLabel && (
                <div className="digest-card-detail">{deltaLabel}</div>
              )}
            </div>

            <div className="digest-card">
              <div className="digest-card-title">Daily Average</div>
              <div className="digest-card-value">
                {formatDurationLocal(
                  stats.trackedDays > 0 ? stats.total / stats.trackedDays : 0
                )}
              </div>
              <div className="digest-card-detail">
                across {stats.trackedDays} tracked{" "}
                {stats.trackedDays === 1 ? "day" : "days"}
              </div>
            </div>

            {stats.topCategory && (
              <div className="digest-card">
                <div className="digest-card-title">Top Category</div>
                <div className="digest-card-value">
                  {getCategoryInfo(stats.topCategory.category).emoji}{" "}
                  {getCategoryInfo(stats.topCategory.category).label}
                </div>
                <div className="digest-card-detail">
                  {formatDurationLocal(stats.topCategory.ms)}
                </div>
              </div>
            )}
          </div>

          <div className="history-chart-card">
            <div className="history-chart-header">
              <h2>Daily activity</h2>
              <div className="history-legend">
                {presentCategories.map((cat) => {
                  const info = getCategoryInfo(cat);
                  return (
                    <span key={cat} className="history-legend-item">
                      <span
                        className="history-swatch"
                        style={{ backgroundColor: info.color }}
                      />
                      {info.label}
                    </span>
                  );
                })}
              </div>
            </div>

            <div className="history-chart" style={{ height: CHART_HEIGHT }}>
              <div className="history-gridline" style={{ bottom: "100%" }}>
                <span>{Math.round(maxMs / HOUR_MS)}h</span>
              </div>
              <div className="history-gridline" style={{ bottom: "50%" }}>
                <span>{maxMs / 2 / HOUR_MS}h</span>
              </div>
              <div className="history-bars">
                {days.map((day) => {
                  const segments = CATEGORY_ORDER.map((cat) => {
                    const entry = day.categories.find(
                      (c) => c.category === cat
                    );
                    const ms = entry ? catMs(entry) : 0;
                    return { cat, ms };
                  }).filter((s) => s.ms > 0);

                  return (
                    <div
                      key={day.day_offset}
                      className="history-bar-col"
                      onMouseMove={(e) =>
                        setTooltip({ day, x: e.clientX, y: e.clientY })
                      }
                      onMouseLeave={() => setTooltip(null)}
                      onClick={() => onSelectDay(day.day_offset)}
                    >
                      <div className="history-bar-stack">
                        {segments.map((seg, i) => (
                          <div
                            key={seg.cat}
                            className="history-bar-segment"
                            style={{
                              height: Math.max(
                                Math.round((seg.ms / maxMs) * CHART_HEIGHT),
                                2
                              ),
                              backgroundColor: getCategoryInfo(seg.cat).color,
                              borderRadius:
                                i === segments.length - 1
                                  ? "3px 3px 0 0"
                                  : undefined,
                            }}
                          />
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="history-bar-labels">
              {days.map((day, i) => (
                <div
                  key={day.day_offset}
                  className={`history-bar-label ${
                    day.day_offset === 0 ? "today" : ""
                  }`}
                >
                  {showBarLabel(i)
                    ? period === "week"
                      ? day.weekday
                      : day.date_label
                    : ""}
                </div>
              ))}
            </div>
          </div>

          {topApps.length > 0 && (
            <div className="digest-card history-top-apps">
              <div className="digest-card-title">Top Apps</div>
              {topApps.slice(0, 8).map((app) => {
                const info = getCategoryInfo(app.primary_category);
                const ms = showActiveOnly
                  ? app.total_duration_ms - app.idle_duration_ms
                  : app.total_duration_ms;
                return (
                  <div key={app.app_name} className="digest-row">
                    <span>
                      {info.emoji} {app.friendly_name}
                    </span>
                    <span>{formatDurationLocal(ms)}</span>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}

      {tooltip && (
        <div
          className="timeline-tooltip"
          style={{ left: tooltip.x + 14, top: tooltip.y + 14 }}
        >
          <strong>
            {tooltip.day.weekday}, {tooltip.day.date_label}
          </strong>
          <div className="tooltip-duration">
            {showActiveOnly ? "Active" : "Total"}:{" "}
            {formatDurationLocal(dayMs(tooltip.day))}
          </div>
          {CATEGORY_ORDER.map((cat) => {
            const entry = tooltip.day.categories.find(
              (c) => c.category === cat
            );
            const ms = entry ? catMs(entry) : 0;
            if (ms <= 0) return null;
            const info = getCategoryInfo(cat);
            return (
              <div key={cat} className="history-tooltip-row">
                <span
                  className="history-swatch"
                  style={{ backgroundColor: info.color }}
                />
                <span className="history-tooltip-label">{info.label}</span>
                <span className="history-tooltip-value">
                  {formatDurationLocal(ms)}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
