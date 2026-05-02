import { useCallback, useEffect, useState } from "react";
import { getAppBreakdown, getCategoryBreakdown, getDayLabel } from "../api";
import type { AppSummary, CategoryBreakdownEntry } from "../types";

const POLL_MS = 5000;

export function useDayBreakdown(dayOffset: number) {
  const [appBreakdown, setAppBreakdown] = useState<AppSummary[]>([]);
  const [categoryBreakdown, setCategoryBreakdown] = useState<CategoryBreakdownEntry[]>([]);
  const [dayLabel, setDayLabel] = useState("Today");

  const reload = useCallback(async () => {
    try {
      const [apps, categories, label] = await Promise.all([
        getAppBreakdown(dayOffset),
        getCategoryBreakdown(dayOffset),
        getDayLabel(dayOffset),
      ]);
      setAppBreakdown(apps);
      setCategoryBreakdown(categories);
      setDayLabel(label);
    } catch (err) {
      console.error("Failed to load breakdown:", err);
    }
  }, [dayOffset]);

  useEffect(() => {
    reload();
    const interval = setInterval(reload, POLL_MS);
    return () => clearInterval(interval);
  }, [reload]);

  return { appBreakdown, categoryBreakdown, dayLabel, reload };
}
