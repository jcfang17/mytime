import { useCallback, useEffect, useState } from "react";
import { getUnknownQueue } from "../api";
import type { UnknownQueueItem } from "../types";

const POLL_MS = 5000;

export function useUnknownQueue(dayOffset: number) {
  const [queue, setQueue] = useState<UnknownQueueItem[]>([]);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    try {
      setLoading(true);
      const data = await getUnknownQueue(dayOffset);
      setQueue(data);
    } catch (err) {
      console.error("Failed to load unknown queue:", err);
    } finally {
      setLoading(false);
    }
  }, [dayOffset]);

  useEffect(() => {
    reload();
    const interval = setInterval(reload, POLL_MS);
    return () => clearInterval(interval);
  }, [reload]);

  return { queue, loading, reload };
}
