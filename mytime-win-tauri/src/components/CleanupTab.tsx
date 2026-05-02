import { formatDurationLocal } from "../api";
import type { UnknownQueueItem } from "../types";

interface CleanupTabProps {
  queue: UnknownQueueItem[];
  loading: boolean;
  onCreateRule: (item: UnknownQueueItem) => void;
}

export function CleanupTab({ queue, loading, onCreateRule }: CleanupTabProps) {
  return (
    <div className="tab-content">
      <section className="cleanup-queue">
        <h2>Unknown Activity</h2>
        <p className="setting-description">
          These activities have no category. Click "+ Rule" to create a classification rule.
        </p>

        {loading && queue.length === 0 ? (
          <p className="no-data">Loading...</p>
        ) : queue.length === 0 ? (
          <p className="no-data">All caught up! No unknown activity for this day.</p>
        ) : (
          <div className="app-table">
            <div className="app-row header">
              <span className="app-name">Activity</span>
              <span className="app-time">Time</span>
              <span className="app-idle">Action</span>
            </div>
            {queue.map((item) => {
              const label = item.context
                ? `${item.friendly_name} · ${item.context}`
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
                      onClick={() => onCreateRule(item)}
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
  );
}
