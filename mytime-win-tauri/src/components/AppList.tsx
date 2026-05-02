import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";
import type { AppSummary, ContextSummary } from "../types";

const BROWSER_APP_RE =
  /^(msedge|chrome|firefox|brave|opera|vivaldi|arc|safari)/i;

interface AppListProps {
  apps: (AppSummary & { displayMs: number })[];
  showActiveOnly: boolean;
  expandedApp: string | null;
  contexts: ContextSummary[];
  contextsLoading: boolean;
  onToggleExpand: (appName: string) => void;
  onAppContextMenu: (e: React.MouseEvent, appName: string) => void;
  onContextRowMenu: (
    e: React.MouseEvent,
    appName: string,
    context: string
  ) => void;
  onCreateRuleFromContext: (context: ContextSummary, appName: string) => void;
}

export function AppList({
  apps,
  showActiveOnly,
  expandedApp,
  contexts,
  contextsLoading,
  onToggleExpand,
  onAppContextMenu,
  onContextRowMenu,
  onCreateRuleFromContext,
}: AppListProps) {
  if (apps.length === 0) {
    return <p className="no-data">No activity tracked yet</p>;
  }

  return (
    <div className="app-table">
      <div className="app-row header">
        <span className="app-name">Application</span>
        <span className="app-time">Time</span>
        <span className="app-idle">Idle</span>
      </div>
      {apps.map((app) => {
        const catInfo = getCategoryInfo(app.primary_category);
        const isBrowser = BROWSER_APP_RE.test(app.app_name);
        const isExpanded = expandedApp === app.app_name;
        return (
          <div key={app.app_name} className="app-row-container">
            <div
              className={`app-row ${isExpanded ? "expanded" : ""}`}
              onContextMenu={(e) => onAppContextMenu(e, app.app_name)}
              onClick={isBrowser ? () => onToggleExpand(app.app_name) : undefined}
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
            {isExpanded && (
              <div className="context-list">
                {contextsLoading ? (
                  <div className="context-loading">Loading...</div>
                ) : contexts.length === 0 ? (
                  <div className="context-empty">No site data</div>
                ) : (
                  contexts
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
                            onContextRowMenu(e, app.app_name, ctx.context)
                          }
                        >
                          <span className="context-name">
                            <span className="context-icon">{ctxCatInfo.emoji}</span>
                            {ctx.context}
                          </span>
                          <span className="context-time">
                            {formatDurationLocal(ctxDisplayMs)}
                          </span>
                          {ctx.context !== "other" && (
                            <button
                              className="btn btn-sm"
                              onClick={() => onCreateRuleFromContext(ctx, app.app_name)}
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
  );
}
