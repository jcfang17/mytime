interface DateNavProps {
  dayLabel: string;
  dayOffset: number;
  showActiveOnly: boolean;
  onPrev: () => void;
  onNext: () => void;
  onToggleActiveOnly: (enabled: boolean) => void;
}

export function DateNav({
  dayLabel,
  dayOffset,
  showActiveOnly,
  onPrev,
  onNext,
  onToggleActiveOnly,
}: DateNavProps) {
  return (
    <section className="date-nav">
      <button className="btn btn-icon" onClick={onPrev}>
        ◀
      </button>
      <span className="date-label">{dayLabel}</span>
      <button
        className="btn btn-icon"
        onClick={onNext}
        disabled={dayOffset >= 0}
      >
        ▶
      </button>
      <label className="checkbox-label">
        <input
          type="checkbox"
          checked={showActiveOnly}
          onChange={(e) => onToggleActiveOnly(e.target.checked)}
        />
        Active only
      </label>
    </section>
  );
}
