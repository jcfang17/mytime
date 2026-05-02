import { useCallback, useEffect, useState } from "react";
import { getDayRange, getTimelineSegments } from "../api";
import type { TimelineSegment } from "../types";

const POLL_MS = 5000;

export function useTimeline(dayOffset: number) {
  const [segments, setSegments] = useState<TimelineSegment[]>([]);
  const [dayRange, setDayRange] = useState<[number, number] | null>(null);

  const reload = useCallback(async () => {
    try {
      const [segs, range] = await Promise.all([
        getTimelineSegments(dayOffset),
        getDayRange(dayOffset),
      ]);
      setSegments(segs);
      setDayRange(range);
    } catch (err) {
      console.error("Failed to load timeline:", err);
    }
  }, [dayOffset]);

  useEffect(() => {
    reload();
    const interval = setInterval(reload, POLL_MS);
    return () => clearInterval(interval);
  }, [reload]);

  return { segments, dayRange, reload };
}
