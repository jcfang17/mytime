import { useCallback, useEffect, useState } from "react";
import { getDailyDigest } from "../api";
import type { DailyDigest } from "../types";

export function useDigest(dayOffset: number) {
  const [digest, setDigest] = useState<DailyDigest | null>(null);
  const [loading, setLoading] = useState(false);

  const reload = useCallback(async () => {
    try {
      setLoading(true);
      const data = await getDailyDigest(dayOffset);
      setDigest(data);
    } catch (err) {
      console.error("Failed to load digest:", err);
    } finally {
      setLoading(false);
    }
  }, [dayOffset]);

  useEffect(() => {
    reload();
  }, [reload]);

  return { digest, loading, reload };
}
