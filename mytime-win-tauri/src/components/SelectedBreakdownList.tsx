import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";
import type { SelectedBreakdownRow } from "../types";

interface SelectedBreakdownListProps {
  visibleRows: (SelectedBreakdownRow & { displayMs: number })[];
  otherMs: number;
  loading: boolean;
  onAppContextMenu: (e: React.MouseEvent, appName: string) => void;
  onContextRowMenu: (
    e: React.MouseEvent,
    appName: string,
    context: string
  ) => void;
}

export function SelectedBreakdownList({
  visibleRows,
  otherMs,
  loading,
  onAppContextMenu,
  onContextRowMenu,
}: SelectedBreakdownListProps) {
  return (
    <div className="app-table">
      <div className="app-row header">
        <span className="app-name">Activity</span>
        <span className="app-time">Time</span>
        <span className="app-idle">Idle</span>
      </div>
      {visibleRows.length === 0 ? (
        <div className="app-row">
          <span className="app-name" style={{ color: "var(--text-muted)" }}>
            {loading ? "Loading breakdown..." : "No matching activity"}
          </span>
          <span className="app-time">-</span>
          <span className="app-idle">-</span>
        </div>
      ) : (
        <>
          {visibleRows.map((row) => {
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
                    ? onContextRowMenu(e, row.app_name, row.context)
                    : onAppContextMenu(e, row.app_name)
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
          {otherMs >= 1000 && (
            <div key="other-small" className="app-row">
              <span className="app-name" style={{ color: "var(--text-muted)" }}>
                Other (small items)
              </span>
              <span className="app-time">
                {formatDurationLocal(otherMs)}
              </span>
              <span className="app-idle">-</span>
            </div>
          )}
        </>
      )}
    </div>
  );
}
