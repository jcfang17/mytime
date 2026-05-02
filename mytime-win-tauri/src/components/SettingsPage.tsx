import { useState } from "react";
import { exportCsv } from "../api";
import { RulesList } from "./RulesList";
import { SuggestionsList } from "./SuggestionsList";
import type { AiSuggestion, ClassificationRule } from "../types";

interface SettingsPageProps {
  dayLabel: string;
  dayOffset: number;
  dayStartHour: number;
  autostartEnabled: boolean;
  rules: ClassificationRule[];
  suggestions: AiSuggestion[];
  onDayStartHourChange: (hour: number) => void;
  onAutostartToggle: (enabled: boolean) => void;
  onAddRule: () => void;
  onEditRule: (rule: ClassificationRule) => void;
  onDeleteRule: (ruleId: string) => void;
  onToggleRule: (rule: ClassificationRule) => void;
  onApproveSuggestion: (suggestionId: string) => void;
  onRejectSuggestion: (suggestionId: string) => void;
}

export function SettingsPage({
  dayLabel,
  dayOffset,
  dayStartHour,
  autostartEnabled,
  rules,
  suggestions,
  onDayStartHourChange,
  onAutostartToggle,
  onAddRule,
  onEditRule,
  onDeleteRule,
  onToggleRule,
  onApproveSuggestion,
  onRejectSuggestion,
}: SettingsPageProps) {
  const [exportStatus, setExportStatus] = useState<string | null>(null);

  const handleExport = async () => {
    try {
      setExportStatus("Exporting...");
      const count = await exportCsv(dayOffset);
      setExportStatus(count === 0 ? "Export cancelled" : `Exported ${count} records`);
      setTimeout(() => setExportStatus(null), 3000);
    } catch (err) {
      console.error("Failed to export:", err);
      setExportStatus("Export failed");
      setTimeout(() => setExportStatus(null), 3000);
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
          Export time tracking data for {dayLabel} as a CSV file.
        </p>
        <div className="setting-row">
          <button className="btn btn-primary" onClick={handleExport}>
            Export to CSV
          </button>
          {exportStatus && <span className="export-status">{exportStatus}</span>}
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
          MyTime v0.1.0 - Personal Time Tracking
        </p>
      </section>
    </div>
  );
}
