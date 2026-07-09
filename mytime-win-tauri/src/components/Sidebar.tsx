import type { TrackingState } from "../types";

type Page = "dashboard" | "settings";

interface SidebarProps {
  currentPage: Page;
  onPageChange: (page: Page) => void;
  trackingState: TrackingState;
}

function captureAge(lastCaptureMs: number): string {
  const secs = Math.max(0, Math.floor((Date.now() - lastCaptureMs) / 1000));
  if (secs < 90) return `${secs}s ago`;
  return `${Math.floor(secs / 60)}m ago`;
}

function resumeLabel(pausedUntilMs: number): string {
  return new Date(pausedUntilMs).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function Sidebar({ currentPage, onPageChange, trackingState }: SidebarProps) {
  const { is_tracking, last_capture_ms, last_error, paused_until_ms } =
    trackingState;

  let statusClass = "";
  let statusText = "○ Stopped";
  if (last_error) {
    statusClass = "error";
    statusText = "⚠ Capture error";
  } else if (is_tracking) {
    statusClass = "active";
    statusText = last_capture_ms
      ? `● Tracking · captured ${captureAge(last_capture_ms)}`
      : "● Tracking";
  } else if (paused_until_ms) {
    statusClass = "paused";
    statusText = `⏸ Paused · resumes ${resumeLabel(paused_until_ms)}`;
  }

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
        <div
          className={`tracking-status ${statusClass}`}
          title={last_error ?? undefined}
        >
          {statusText}
        </div>
      </div>
    </aside>
  );
}
