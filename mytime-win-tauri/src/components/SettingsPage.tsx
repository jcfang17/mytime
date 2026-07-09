import { useState } from "react";
import { deleteDataRange, exportCsv, openDataFolder } from "../api";
import { RulesList } from "./RulesList";
import { SuggestionsList } from "./SuggestionsList";
import type { AiSuggestion, ClassificationRule } from "../types";

// Day-offset ranges for export/delete. "" encodes all time (null offset).
const RANGE_OPTIONS: { value: string; label: string; start: number | null }[] = [
  { value: "0", label: "Today", start: 0 },
  { value: "-6", label: "Last 7 days", start: -6 },
  { value: "-29", label: "Last 30 days", start: -29 },
  { value: "", label: "All time", start: null },
];

interface SettingsPageProps {
  dayStartHour: number;
  autostartEnabled: boolean;
  autoTrackEnabled: boolean;
  rules: ClassificationRule[];
  suggestions: AiSuggestion[];
  onDayStartHourChange: (hour: number) => void;
  onAutostartToggle: (enabled: boolean) => void;
  onAutoTrackToggle: (enabled: boolean) => void;
  onAddRule: () => void;
  onEditRule: (rule: ClassificationRule) => void;
  onDeleteRule: (ruleId: string) => void;
  onToggleRule: (rule: ClassificationRule) => void;
  onApproveSuggestion: (suggestionId: string) => void;
  onRejectSuggestion: (suggestionId: string) => void;
  onGenerateSuggestions: () => Promise<number>;
  onDataChanged: () => Promise<void>;
}

export function SettingsPage({
  dayStartHour,
  autostartEnabled,
  autoTrackEnabled,
  rules,
  suggestions,
  onDayStartHourChange,
  onAutostartToggle,
  onAutoTrackToggle,
  onAddRule,
  onEditRule,
  onDeleteRule,
  onToggleRule,
  onApproveSuggestion,
  onRejectSuggestion,
  onGenerateSuggestions,
  onDataChanged,
}: SettingsPageProps) {
  const [exportStatus, setExportStatus] = useState<string | null>(null);
  const [exportRange, setExportRange] = useState("0");
  const [deleteRange, setDeleteRange] = useState("0");
  const [deleteStatus, setDeleteStatus] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [generateStatus, setGenerateStatus] = useState<string | null>(null);

  const handleGenerate = async () => {
    try {
      setGenerating(true);
      setGenerateStatus(null);
      const count = await onGenerateSuggestions();
      setGenerateStatus(
        count === 0
          ? "No new suggestions — everything confident is already covered"
          : `${count} new ${count === 1 ? "suggestion" : "suggestions"} to review below`
      );
    } catch (err) {
      console.error("Failed to generate suggestions:", err);
      setGenerateStatus(String(err));
    } finally {
      setGenerating(false);
    }
  };

  const rangeStart = (value: string): number | null =>
    RANGE_OPTIONS.find((o) => o.value === value)?.start ?? 0;

  const handleExport = async () => {
    try {
      setExportStatus("Exporting...");
      const count = await exportCsv(rangeStart(exportRange), 0);
      setExportStatus(count === 0 ? "Export cancelled" : `Exported ${count} records`);
      setTimeout(() => setExportStatus(null), 3000);
    } catch (err) {
      console.error("Failed to export:", err);
      setExportStatus("Export failed");
      setTimeout(() => setExportStatus(null), 3000);
    }
  };

  const handleDelete = async () => {
    const label = RANGE_OPTIONS.find((o) => o.value === deleteRange)?.label;
    const warning =
      deleteRange === ""
        ? "Delete ALL activity data (segments, labels, AI suggestions)? Rules and settings are kept. This cannot be undone."
        : `Delete all activity for: ${label}? This cannot be undone.`;
    if (!window.confirm(warning)) return;
    try {
      const count = await deleteDataRange(rangeStart(deleteRange), 0);
      setDeleteStatus(`Deleted ${count} segments`);
      setTimeout(() => setDeleteStatus(null), 4000);
      await onDataChanged();
    } catch (err) {
      console.error("Failed to delete data:", err);
      setDeleteStatus(String(err));
    }
  };

  return (
    <div className="settings">
      <h2>Settings</h2>

      <section className="setting-section">
        <h3>Startup</h3>
        <label className="setting-toggle">
          <input
            type="checkbox"
            checked={autostartEnabled}
            onChange={(e) => onAutostartToggle(e.target.checked)}
          />
          <span>Launch MyTime when you log in</span>
        </label>
        <label className="setting-toggle" style={{ marginTop: 10 }}>
          <input
            type="checkbox"
            checked={autoTrackEnabled}
            onChange={(e) => onAutoTrackToggle(e.target.checked)}
          />
          <span>Start tracking automatically when MyTime launches</span>
        </label>
      </section>

      <section className="setting-section">
        <h3>Day Start Hour</h3>
        <p className="setting-description">
          When does your day start? Time tracked after midnight but before this hour
          will count toward the previous day.
        </p>
        <select
          className="setting-select"
          value={dayStartHour}
          onChange={(e) => onDayStartHourChange(Number(e.target.value))}
        >
          {Array.from({ length: 24 }, (_, i) => i).map((hour) => {
            const hour12 = hour === 0 ? 12 : hour > 12 ? hour - 12 : hour;
            const ampm = hour < 12 ? "AM" : "PM";
            const label = hour === 0 ? "12:00 AM (Midnight)"
              : hour === 12 ? "12:00 PM (Noon)"
              : `${hour12}:00 ${ampm}`;
            return (
              <option key={hour} value={hour}>
                {label}
              </option>
            );
          })}
        </select>
      </section>

      <section className="setting-section">
        <h3>Export Data</h3>
        <p className="setting-description">
          Export your tracked activity as a CSV file.
        </p>
        <div className="setting-row">
          <select
            className="setting-select"
            value={exportRange}
            onChange={(e) => setExportRange(e.target.value)}
          >
            {RANGE_OPTIONS.map((o) => (
              <option key={o.label} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          <button className="btn btn-primary" onClick={handleExport}>
            Export to CSV
          </button>
          {exportStatus && <span className="export-status">{exportStatus}</span>}
        </div>
      </section>

      <section className="setting-section">
        <h3>Your Data</h3>
        <p className="setting-description">
          Everything MyTime records stays in a local SQLite database. You can
          open the folder (for backups) or permanently delete recorded
          activity.
        </p>
        <div className="setting-row">
          <button className="btn btn-secondary" onClick={() => openDataFolder()}>
            Open Data Folder
          </button>
        </div>
        <div className="setting-row" style={{ marginTop: 12 }}>
          <select
            className="setting-select"
            value={deleteRange}
            onChange={(e) => setDeleteRange(e.target.value)}
          >
            {RANGE_OPTIONS.map((o) => (
              <option key={o.label} value={o.value}>
                {o.value === "" ? "Everything" : o.label}
              </option>
            ))}
          </select>
          <button className="btn btn-danger" onClick={handleDelete}>
            Delete Activity
          </button>
          {deleteStatus && <span className="export-status">{deleteStatus}</span>}
        </div>
      </section>

      <section className="setting-section">
        <h3>AI Suggestions</h3>
        <p className="setting-description">
          Analyze uncategorized activity from the last 14 days with Claude and
          propose categorization rules for your review. Window titles of
          uncategorized activity are sent to the Anthropic API (requires
          ANTHROPIC_API_KEY).
        </p>
        <div className="setting-row">
          <button
            className="btn btn-primary"
            onClick={handleGenerate}
            disabled={generating}
          >
            {generating ? "Analyzing..." : "✨ Generate Suggestions"}
          </button>
          {generateStatus && (
            <span className="export-status">{generateStatus}</span>
          )}
        </div>
      </section>

      {suggestions.length > 0 && (
        <SuggestionsList
          suggestions={suggestions}
          onApprove={onApproveSuggestion}
          onReject={onRejectSuggestion}
        />
      )}

      <RulesList
        rules={rules}
        onAdd={onAddRule}
        onEdit={onEditRule}
        onDelete={onDeleteRule}
        onToggle={onToggleRule}
      />

      <section className="setting-section">
        <h3>About</h3>
        <p className="setting-description">
          MyTime v0.2.0 - Personal Time Tracking
        </p>
      </section>
    </div>
  );
}
