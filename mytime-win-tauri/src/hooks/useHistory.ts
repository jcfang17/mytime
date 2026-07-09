import { useCallback, useEffect, useState } from "react";
import { getHistory, getRangeAppBreakdown } from "../api";
import type { AppSummary, DayHistory } from "../types";

export type HistoryPeriod = "week" | "month";

export const PERIOD_DAYS: Record<HistoryPeriod, number> = {
  week: 7,
  month: 30,
};

/**
 * Loads the current period, the immediately preceding period (for the
 * comparison delta), and the range-wide app breakdown.
 */
export function useHistory(period: HistoryPeriod, endOffset: number) {
  const [days, setDays] = useState<DayHistory[]>([]);
  const [prevDays, setPrevDays] = useState<DayHistory[]>([]);
  const [topApps, setTopApps] = useState<AppSummary[]>([]);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    const n = PERIOD_DAYS[period];
    try {
      setLoading(true);
      const [current, previous, apps] = await Promise.all([
        getHistory(n, endOffset),
        getHistory(n, endOffset - n),
        getRangeAppBreakdown(endOffset - n + 1, endOffset),
      ]);
      setDays(current);
      setPrevDays(previous);
      setTopApps(apps);
    } catch (err) {
      console.error("Failed to load history:", err);
    } finally {
      setLoading(false);
    }
  }, [period, endOffset]);

  useEffect(() => {
    reload();
  }, [reload]);

  return { days, prevDays, topApps, loading, reload };
}
