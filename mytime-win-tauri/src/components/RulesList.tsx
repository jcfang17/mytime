import { getCategoryInfo } from "../types";
import type { ClassificationRule } from "../types";

interface RulesListProps {
  rules: ClassificationRule[];
  onAdd: () => void;
  onEdit: (rule: ClassificationRule) => void;
  onDelete: (ruleId: string) => void;
  onToggle: (rule: ClassificationRule) => void;
}

export function RulesList({
  rules,
  onAdd,
  onEdit,
  onDelete,
  onToggle,
}: RulesListProps) {
  return (
    <section className="setting-section">
      <h3>Classification Rules</h3>
      <p className="setting-description">
        Create rules to automatically categorize apps and websites based on patterns.
      </p>

      <div className="rules-list">
        {rules.length === 0 ? (
          <p className="no-data">No rules defined yet</p>
        ) : (
          rules.map((rule) => {
            const catInfo = getCategoryInfo(rule.category);
            return (
              <div
                key={rule.rule_id}
                className={`rule-item ${!rule.enabled ? "disabled" : ""}`}
              >
                <div className="rule-info">
                  <span className="rule-category">
                    {catInfo.emoji} {catInfo.label}
                  </span>
                  <span className="rule-pattern">
                    {rule.app_pattern && (
                      <span className="pattern-badge">
                        App: {rule.app_pattern}
                      </span>
                    )}
                    {rule.title_pattern && (
                      <span className="pattern-badge">
                        Title: {rule.title_pattern}
                      </span>
                    )}
                    <span className="match-type">({rule.match_type})</span>
                  </span>
                  <span className="rule-source">{rule.source}</span>
                </div>
                <div className="rule-actions">
                  <button
                    className="btn btn-sm"
                    onClick={() => onToggle(rule)}
                    title={rule.enabled ? "Disable" : "Enable"}
                  >
                    {rule.enabled ? "✓" : "○"}
                  </button>
                  <button
                    className="btn btn-sm"
                    onClick={() => onEdit(rule)}
                    title="Edit"
                  >
                    ✏️
                  </button>
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={() => onDelete(rule.rule_id)}
                    title="Delete"
                  >
                    🗑️
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      <button className="btn btn-primary" onClick={onAdd}>
        + Add Rule
      </button>
    </section>
  );
}
