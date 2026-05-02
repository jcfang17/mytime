import { formatDurationLocal } from "../api";

interface DashboardHeaderProps {
  isTracking: boolean;
  displayTimeMs: number;
  onStart: () => void;
  onStop: () => void;
}

export function DashboardHeader({
  isTracking,
  displayTimeMs,
  onStart,
  onStop,
}: DashboardHeaderProps) {
  return (
    <header className="dashboard-header">
      <div className="time-display">
        <span className="time-value">{formatDurationLocal(displayTimeMs)}</span>
        <span className="time-label">{isTracking ? "Tracking" : "Stopped"}</span>
      </div>
      <div className="controls">
        <button
          className="btn btn-primary"
          onClick={onStart}
          disabled={isTracking}
        >
          ▶ Start
        </button>
        <button
          className="btn btn-secondary"
          onClick={onStop}
          disabled={!isTracking}
        >
          ⏹ Stop
        </button>
      </div>
    </header>
  );
}
