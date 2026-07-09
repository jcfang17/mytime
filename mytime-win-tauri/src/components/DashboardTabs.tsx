export type DashboardTab = "overview" | "history" | "cleanup" | "digest";

interface DashboardTabsProps {
  active: DashboardTab;
  onChange: (tab: DashboardTab) => void;
  cleanupBadgeCount: number;
}

export function DashboardTabs({
  active,
  onChange,
  cleanupBadgeCount,
}: DashboardTabsProps) {
  return (
    <nav className="dashboard-tabs">
      <button
        className={`dashboard-tab ${active === "overview" ? "active" : ""}`}
        onClick={() => onChange("overview")}
      >
        Overview
      </button>
      <button
        className={`dashboard-tab ${active === "history" ? "active" : ""}`}
        onClick={() => onChange("history")}
      >
        History
      </button>
      <button
        className={`dashboard-tab ${active === "cleanup" ? "active" : ""}`}
        onClick={() => onChange("cleanup")}
      >
        Cleanup
        {cleanupBadgeCount > 0 && (
          <span className="tab-badge">{cleanupBadgeCount}</span>
        )}
      </button>
      <button
        className={`dashboard-tab ${active === "digest" ? "active" : ""}`}
        onClick={() => onChange("digest")}
      >
        Digest
      </button>
    </nav>
  );
}
