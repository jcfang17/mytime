import { useCallback, useEffect, useState } from "react";
import { getSelectedBreakdown } from "../api";
import type { SelectedBreakdownRow } from "../types";

const POLL_MS = 5000;

export function useSelectedBreakdown(
  dayOffset: number,
  selectedCategories: Set<string>
) {
  const [rows, setRows] = useState<SelectedBreakdownRow[]>([]);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    if (selectedCategories.size === 0) {
      setRows([]);
      return;
    }
    try {
      setLoading(true);
      const data = await getSelectedBreakdown(dayOffset, Array.from(selectedCategories));
      setRows(data);
    } catch (err) {
      console.error("Failed to load selected breakdown:", err);
    } finally {
      setLoading(false);
    }
  }, [dayOffset, selectedCategories]);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    if (selectedCategories.size === 0) return;
    const interval = setInterval(reload, POLL_MS);
    return () => clearInterval(interval);
  }, [reload, selectedCategories.size]);

  return { rows, loading, reload };
}
