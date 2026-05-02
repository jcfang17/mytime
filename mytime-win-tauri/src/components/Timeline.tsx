import { useState } from "react";
import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";
import type { LabelProvenance, TimelineSegment } from "../types";

interface TimelineProps {
  segments: TimelineSegment[];
  dayRange: [number, number];
  selectedSegment: TimelineSegment | null;
  onSelectSegment: (segment: TimelineSegment | null) => void;
  provenanceTitleHash: string | null;
  provenance: LabelProvenance | null;
  provenanceLoading: boolean;
  onShowProvenance: (titleHash: string) => void;
}

export function Timeline({
  segments,
  dayRange,
  selectedSegment,
  onSelectSegment,
  provenanceTitleHash,
  provenance,
  provenanceLoading,
  onShowProvenance,
}: TimelineProps) {
  const [hoveredSegment, setHoveredSegment] = useState<TimelineSegment | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });

  const [rangeStart, rangeEnd] = dayRange;
  const rangeDuration = rangeEnd - rangeStart;
  const startHour = new Date(rangeStart).getHours();

  const hourLabels: number[] = [];
  for (let h = startHour; h < startHour + 24; h++) {
    const hour = h % 24;
    const hourMs = rangeStart + (h - startHour) * 3600000;
    if (hourMs >= rangeStart && hourMs <= rangeEnd) {
      hourLabels.push(hour);
    }
  }

  return (
    <section className="timeline-section">
      <div
        className="timeline-bar"
        onMouseLeave={() => setHoveredSegment(null)}
      >
        {segments.map((seg) => {
          const left = ((seg.start_time - rangeStart) / rangeDuration) * 100;
          const width = ((seg.end_time - seg.start_time) / rangeDuration) * 100;
          const catInfo = getCategoryInfo(seg.category);
          const durationMs = seg.end_time - seg.start_time;
          const idleMs = seg.idle_seconds * 1000;
          const idleRatio = durationMs > 0 ? idleMs / durationMs : 0;
          const mostlyIdle = idleRatio > 0.5;
          return (
            <div
              key={seg.segment_id}
              className={`timeline-segment ${selectedSegment?.segment_id === seg.segment_id ? "selected" : ""} ${mostlyIdle ? "idle" : ""}`}
              style={{
                left: `${left}%`,
                width: `${Math.max(width, 0.15)}%`,
                backgroundColor: mostlyIdle ? "var(--bg-tertiary)" : catInfo.color,
              }}
              onMouseEnter={(e) => {
                setHoveredSegment(seg);
                setTooltipPos({ x: e.clientX, y: e.clientY });
              }}
              onMouseMove={(e) => {
                setTooltipPos({ x: e.clientX, y: e.clientY });
              }}
              onClick={() => {
                onSelectSegment(
                  selectedSegment?.segment_id === seg.segment_id ? null : seg
                );
              }}
            />
          );
        })}
      </div>
      <div className="timeline-labels">
        {hourLabels.map((hour) => {
          const hourMs = rangeStart + ((hour - startHour + 24) % 24) * 3600000;
          const leftPct = ((hourMs - rangeStart) / rangeDuration) * 100;
          return (
            <span
              key={hour}
              className="timeline-hour-label"
              style={{ left: `${leftPct}%` }}
            >
              {hour.toString().padStart(2, "0")}
            </span>
          );
        })}
      </div>
      {hoveredSegment && (
        <div
          className="timeline-tooltip"
          style={{ left: tooltipPos.x + 12, top: tooltipPos.y - 60 }}
        >
          <strong>{hoveredSegment.friendly_name}</strong>
          {hoveredSegment.window_title && (
            <div className="tooltip-title">{hoveredSegment.window_title}</div>
          )}
          <div className="tooltip-time">
            {new Date(hoveredSegment.start_time).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
            {" - "}
            {new Date(hoveredSegment.end_time).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </div>
          <div className="tooltip-duration">
            {formatDurationLocal(hoveredSegment.end_time - hoveredSegment.start_time)}
            {" "}
            <span style={{ color: getCategoryInfo(hoveredSegment.category).color }}>
              {getCategoryInfo(hoveredSegment.category).label}
            </span>
          </div>
          {hoveredSegment.idle_seconds > 0 && (
            <div className="tooltip-idle">
              💤 {formatDurationLocal(hoveredSegment.idle_seconds * 1000)} idle
            </div>
          )}
        </div>
      )}
      {selectedSegment && (
        <TimelineDetail
          segment={selectedSegment}
          onClose={() => onSelectSegment(null)}
          provenanceTitleHash={provenanceTitleHash}
          provenance={provenance}
          provenanceLoading={provenanceLoading}
          onShowProvenance={onShowProvenance}
        />
      )}
    </section>
  );
}

interface TimelineDetailProps {
  segment: TimelineSegment;
  onClose: () => void;
  provenanceTitleHash: string | null;
  provenance: LabelProvenance | null;
  provenanceLoading: boolean;
  onShowProvenance: (titleHash: string) => void;
}

function TimelineDetail({
  segment,
  onClose,
  provenanceTitleHash,
  provenance,
  provenanceLoading,
  onShowProvenance,
}: TimelineDetailProps) {
  return (
    <div className="timeline-detail">
      <div className="timeline-detail-header">
        <span className="timeline-detail-app">
          {getCategoryInfo(segment.category).emoji}{" "}
          {segment.friendly_name}
        </span>
        <button className="btn btn-xs" onClick={onClose}>
          ✕
        </button>
      </div>
      {segment.window_title && (
        <div className="timeline-detail-title">{segment.window_title}</div>
      )}
      <div className="timeline-detail-row">
        <span>Time</span>
        <span>
          {new Date(segment.start_time).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
          })}
          {" - "}
          {new Date(segment.end_time).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
          })}
        </span>
      </div>
      <div className="timeline-detail-row">
        <span>Duration</span>
        <span>
          {formatDurationLocal(segment.end_time - segment.start_time)}
        </span>
      </div>
      <div className="timeline-detail-row">
        <span>Category</span>
        <span style={{ color: getCategoryInfo(segment.category).color }}>
          {getCategoryInfo(segment.category).label}
        </span>
      </div>
      {segment.idle_seconds > 0 && (
        <div className="timeline-detail-row">
          <span>Idle</span>
          <span>{formatDurationLocal(segment.idle_seconds * 1000)}</span>
        </div>
      )}
      <button
        className="btn btn-sm btn-secondary"
        style={{ marginTop: 8 }}
        onClick={() => onShowProvenance(segment.title_hash)}
      >
        Why "{getCategoryInfo(segment.category).label}"?
      </button>
      {provenanceTitleHash === segment.title_hash && (
        <div className="provenance-panel">
          {provenanceLoading ? (
            <span>Loading...</span>
          ) : provenance?.best_label ? (
            <>
              <div className="provenance-source">
                {provenance.best_label.source === "manual" &&
                  "Manually set by you"}
                {provenance.best_label.source === "user" &&
                  "Matched by classification rule"}
                {provenance.best_label.source === "ai" &&
                  `AI classified (${Math.round(
                    (provenance.best_label.confidence || 0) * 100
                  )}% confidence)`}
                {provenance.best_label.source === "heuristic" &&
                  "Heuristic classification (app name pattern)"}
              </div>
              {provenance.matching_rule && (
                <div className="provenance-rule">
                  {provenance.matching_rule.app_pattern && (
                    <span className="pattern-badge">
                      App: {provenance.matching_rule.app_pattern}
                    </span>
                  )}
                  {provenance.matching_rule.title_pattern && (
                    <span className="pattern-badge">
                      Title: {provenance.matching_rule.title_pattern}
                    </span>
                  )}
                  <span className="match-type">
                    ({provenance.matching_rule.match_type})
                  </span>
                </div>
              )}
            </>
          ) : (
            <span className="provenance-none">
              No label assigned (defaults to "Other")
            </span>
          )}
        </div>
      )}
    </div>
  );
}
