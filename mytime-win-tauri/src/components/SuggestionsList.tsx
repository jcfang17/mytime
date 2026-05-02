import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";
import type { AiSuggestion } from "../types";

interface SuggestionsListProps {
  suggestions: AiSuggestion[];
  onApprove: (suggestionId: string) => void;
  onReject: (suggestionId: string) => void;
}

export function SuggestionsList({
  suggestions,
  onApprove,
  onReject,
}: SuggestionsListProps) {
  return (
    <section className="setting-section">
      <h3>AI Suggestions</h3>
      <p className="setting-description">
        Review AI-generated categorization suggestions. Approve to create a rule, or reject to dismiss.
      </p>

      <div className="suggestions-list">
        {suggestions.map((suggestion) => {
          const catInfo = getCategoryInfo(suggestion.suggested_category);
          return (
            <div key={suggestion.suggestion_id} className="suggestion-item">
              <div className="suggestion-info">
                <div className="suggestion-header">
                  <span className="suggestion-category">
                    {catInfo.emoji} {catInfo.label}
                  </span>
                  <span className="suggestion-confidence">
                    {Math.round(suggestion.confidence * 100)}% confident
                  </span>
                </div>
                <div className="suggestion-pattern">
                  {suggestion.app_pattern && (
                    <span className="pattern-badge">
                      App: {suggestion.app_pattern}
                    </span>
                  )}
                  {suggestion.title_pattern && (
                    <span className="pattern-badge">
                      Title: {suggestion.title_pattern}
                    </span>
                  )}
                </div>
                <p className="suggestion-reason">{suggestion.reason}</p>
                <div className="suggestion-stats">
                  <span>{suggestion.match_count} matches</span>
                  <span>{formatDurationLocal(suggestion.total_duration_ms)} total</span>
                </div>
                {suggestion.sample_titles.length > 0 && (
                  <ul className="suggestion-samples">
                    {suggestion.sample_titles.slice(0, 3).map((title, i) => (
                      <li key={i}>{title}</li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="suggestion-actions">
                <button
                  className="btn btn-sm btn-success"
                  onClick={() => onApprove(suggestion.suggestion_id)}
                  title="Approve - create rule"
                >
                  ✓ Approve
                </button>
                <button
                  className="btn btn-sm btn-danger"
                  onClick={() => onReject(suggestion.suggestion_id)}
                  title="Reject - dismiss suggestion"
                >
                  ✗ Reject
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
