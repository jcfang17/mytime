import { useState } from "react";
import { previewRuleMatches, formatDurationLocal } from "../api";
import { CATEGORY_INFO } from "../types";
import type { Category, ClassificationRule, MatchType, RulePreview } from "../types";

export interface RuleFormState {
  appPattern: string;
  titlePattern: string;
  matchType: MatchType;
  category: string;
}

interface RuleFormModalProps {
  editingRule: ClassificationRule | null;
  initialForm: RuleFormState;
  onSave: (form: RuleFormState) => void | Promise<void>;
  onCancel: () => void;
}

export function RuleFormModal({
  editingRule,
  initialForm,
  onSave,
  onCancel,
}: RuleFormModalProps) {
  const [form, setForm] = useState<RuleFormState>(initialForm);
  const [preview, setPreview] = useState<RulePreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const handlePreview = async () => {
    const appPattern = form.appPattern.trim() || null;
    const titlePattern = form.titlePattern.trim() || null;

    if (!appPattern && !titlePattern) {
      setPreview(null);
      return;
    }

    try {
      setPreviewLoading(true);
      const data = await previewRuleMatches(
        appPattern,
        titlePattern,
        form.matchType,
        7
      );
      setPreview(data);
    } catch (err) {
      console.error("Failed to preview rule:", err);
    } finally {
      setPreviewLoading(false);
    }
  };

  const handleSave = () => {
    const appPattern = form.appPattern.trim();
    const titlePattern = form.titlePattern.trim();
    if (!appPattern && !titlePattern) {
      alert("Please enter at least an app pattern or title pattern");
      return;
    }
    onSave(form);
  };

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{editingRule ? "Edit Rule" : "Add Rule"}</h3>

        <div className="form-group">
          <label>App Pattern</label>
          <input
            type="text"
            value={form.appPattern}
            onChange={(e) => setForm({ ...form, appPattern: e.target.value })}
            placeholder="e.g., msedge, chrome, code"
          />
          <span className="form-help">Match app name (exe filename)</span>
        </div>

        <div className="form-group">
          <label>Title Pattern</label>
          <input
            type="text"
            value={form.titlePattern}
            onChange={(e) => setForm({ ...form, titlePattern: e.target.value })}
            placeholder="e.g., YouTube, GitHub, Slack"
          />
          <span className="form-help">Match window title text</span>
        </div>

        <div className="form-group">
          <label>Match Type</label>
          <select
            value={form.matchType}
            onChange={(e) =>
              setForm({ ...form, matchType: e.target.value as MatchType })
            }
          >
            <option value="contains">Contains</option>
            <option value="prefix">Starts with</option>
            <option value="exact">Exact match</option>
            <option value="regex">Regex</option>
          </select>
        </div>

        <div className="form-group">
          <label>Category</label>
          <select
            value={form.category}
            onChange={(e) => setForm({ ...form, category: e.target.value })}
          >
            {(Object.keys(CATEGORY_INFO) as Category[])
              .filter((cat) => cat !== "unknown")
              .map((cat) => {
                const info = CATEGORY_INFO[cat];
                return (
                  <option key={cat} value={cat}>
                    {info.emoji} {info.label}
                  </option>
                );
              })}
          </select>
        </div>

        <div className="rule-preview">
          <button
            className="btn btn-secondary"
            onClick={handlePreview}
            disabled={previewLoading}
          >
            {previewLoading ? "Loading..." : "Preview Matches"}
          </button>
          {preview && (
            <div className="preview-result">
              <p>
                <strong>{preview.match_count}</strong> matches (
                {formatDurationLocal(preview.total_duration_ms)} total)
              </p>
              {preview.sample_titles.length > 0 && (
                <ul className="sample-titles">
                  {preview.sample_titles.slice(0, 3).map((title, i) => (
                    <li key={i}>{title}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>

        <div className="modal-actions">
          <button className="btn btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleSave}>
            {editingRule ? "Update" : "Create"}
          </button>
        </div>
      </div>
    </div>
  );
}
