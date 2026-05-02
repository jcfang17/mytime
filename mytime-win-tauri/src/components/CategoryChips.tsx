import { formatDurationLocal } from "../api";
import { getCategoryInfo } from "../types";

interface CategoryChipsProps {
  breakdown: [string, number][];
  totalTrackedMs: number;
  selectedCategories: Set<string>;
  onCategoryClick: (category: string) => void;
  unconditionalTotalMs: number;
  unconditionalActiveMs: number;
  totalIdleMs: number;
  selectedTotalMs: number;
  onClearSelection: () => void;
}

export function CategoryChips({
  breakdown,
  totalTrackedMs,
  selectedCategories,
  onCategoryClick,
  unconditionalTotalMs,
  unconditionalActiveMs,
  totalIdleMs,
  selectedTotalMs,
  onClearSelection,
}: CategoryChipsProps) {
  if (breakdown.length === 0) return null;

  return (
    <section className="category-section">
      <div className="category-summary">
        <span className="summary-total">
          Total: <strong>{formatDurationLocal(unconditionalTotalMs)}</strong>
        </span>
        <span className="summary-active">
          Active: <strong>{formatDurationLocal(unconditionalActiveMs)}</strong>
        </span>
        {totalIdleMs > 0 && (
          <span className="summary-idle">
            💤 Idle: {formatDurationLocal(totalIdleMs)}
          </span>
        )}
        {selectedCategories.size > 0 && (
          <span className="summary-selected">
            Selected: <strong>{formatDurationLocal(selectedTotalMs)}</strong>
            <button
              className="btn btn-xs"
              onClick={onClearSelection}
              title="Clear selection"
            >
              ✕
            </button>
          </span>
        )}
      </div>
      <div className="category-chips">
        {breakdown.map(([cat, ms]) => {
          const info = getCategoryInfo(cat);
          const pct = totalTrackedMs > 0 ? Math.round((ms / totalTrackedMs) * 100) : 0;
          const isSelected = selectedCategories.has(cat);
          return (
            <div
              key={cat}
              className={`category-chip ${isSelected ? "selected" : ""}`}
              style={{
                borderColor: info.color,
                backgroundColor: isSelected ? info.color + "20" : undefined,
              }}
              onClick={() => onCategoryClick(cat)}
            >
              <span className="category-emoji">{info.emoji}</span>
              <span className="category-name">{info.label}</span>
              <span className="category-time">{formatDurationLocal(ms)}</span>
              <span className="category-pct">{pct}%</span>
            </div>
          );
        })}
      </div>
    </section>
  );
}
