type Page = "dashboard" | "settings";

interface SidebarProps {
  currentPage: Page;
  onPageChange: (page: Page) => void;
  isTracking: boolean;
}

export function Sidebar({ currentPage, onPageChange, isTracking }: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h1>⏱ MyTime</h1>
      </div>
      <nav className="sidebar-nav">
        <button
          className={`nav-item ${currentPage === "dashboard" ? "active" : ""}`}
          onClick={() => onPageChange("dashboard")}
        >
          📊 Dashboard
        </button>
        <button
          className={`nav-item ${currentPage === "settings" ? "active" : ""}`}
          onClick={() => onPageChange("settings")}
        >
          ⚙️ Settings
        </button>
      </nav>
      <div className="sidebar-footer">
        <div className={`tracking-status ${isTracking ? "active" : ""}`}>
          {isTracking ? "● Tracking" : "○ Stopped"}
        </div>
      </div>
    </aside>
  );
}
