import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";
import type { DailyDigest } from "../types";

interface DigestTabProps {
  digest: DailyDigest | null;
  loading: boolean;
  showActiveOnly: boolean;
}

export function DigestTab({ digest, loading, showActiveOnly }: DigestTabProps) {
  if (loading && !digest) {
    return (
      <div className="tab-content">
        <p className="no-data">Loading digest...</p>
      </div>
    );
  }

  if (!digest || digest.total_tracked_ms === 0) {
    return (
      <div className="tab-content">
        <p className="no-data">No data for this day</p>
      </div>
    );
  }

  return (
    <div className="tab-content">
      <div className="digest-grid">
        <div className="digest-card">
          <div className="digest-card-title">
            {showActiveOnly ? "Active Time" : "Total Time"}
          </div>
          <div className="digest-card-value">
            {formatDurationLocal(
              showActiveOnly ? digest.total_active_ms : digest.total_tracked_ms
            )}
          </div>
          <div className="digest-card-detail">
            {showActiveOnly
              ? `Total: ${formatDurationLocal(digest.total_tracked_ms)}`
              : `Active: ${formatDurationLocal(digest.total_active_ms)}`}
          </div>
        </div>

        <div className="digest-card">
          <div className="digest-card-title">Top Categories</div>
          {digest.top_categories.map((cat) => {
            const info = getCategoryInfo(cat.category);
            const displayMs = showActiveOnly
              ? cat.duration_ms - cat.idle_ms
              : cat.duration_ms;
            const displayTotal = showActiveOnly
              ? digest.total_active_ms
              : digest.total_tracked_ms;
            const pct = displayTotal > 0
              ? Math.round((displayMs / displayTotal) * 100)
              : 0;
            return (
              <div key={cat.category} className="digest-row">
                <span>{info.emoji} {info.label}</span>
                <span>
                  {formatDurationLocal(displayMs)} ({pct}%)
                </span>
              </div>
            );
          })}
        </div>

        <div className="digest-card">
          <div className="digest-card-title">Top Apps</div>
          {digest.top_apps.map((app) => {
            const info = getCategoryInfo(app.category);
            const displayMs = showActiveOnly
              ? app.duration_ms - app.idle_ms
              : app.duration_ms;
            return (
              <div key={app.app_name} className="digest-row">
                <span>{info.emoji} {app.friendly_name}</span>
                <span>{formatDurationLocal(displayMs)}</span>
              </div>
            );
          })}
        </div>

        {digest.longest_focus && (
          <div className="digest-card">
            <div className="digest-card-title">Longest Focus Block</div>
            <div className="digest-card-value">
              {formatDurationLocal(digest.longest_focus.duration_ms)}
            </div>
            <div className="digest-card-detail">
              {digest.longest_focus.friendly_name}
            </div>
          </div>
        )}

        {digest.most_idle && (
          <div className="digest-card">
            <div className="digest-card-title">Most Idle Window</div>
            <div className="digest-card-value">
              {formatDurationLocal(digest.most_idle.idle_seconds * 1000)}
            </div>
            <div className="digest-card-detail">
              {digest.most_idle.friendly_name}
              {digest.most_idle.window_title && (
                <span className="digest-card-subtitle">
                  {" "}&mdash; {digest.most_idle.window_title}
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
