import { useCallback, useEffect, useState } from "react";
import { getAppContexts } from "../api";
import type { ContextSummary } from "../types";

export function useAppContexts(dayOffset: number) {
  const [expandedApp, setExpandedApp] = useState<string | null>(null);
  const [contexts, setContexts] = useState<ContextSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [cache, setCache] = useState<Record<string, ContextSummary[]>>({});

  // Reset when day changes (contexts are day-specific)
  useEffect(() => {
    setExpandedApp(null);
    setContexts([]);
    setCache({});
  }, [dayOffset]);

  const reloadExpanded = useCallback(async () => {
    if (!expandedApp) return;
    setLoading(true);
    try {
      const data = await getAppContexts(expandedApp, dayOffset);
      setContexts(data);
      setCache((prev) => ({ ...prev, [expandedApp]: data }));
    } catch (err) {
      console.error("Failed to reload app contexts:", err);
    } finally {
      setLoading(false);
    }
  }, [dayOffset, expandedApp]);

  const toggle = useCallback(
    async (appName: string) => {
      if (expandedApp === appName) {
        setExpandedApp(null);
        setContexts([]);
        return;
      }
      setExpandedApp(appName);
      const cached = cache[appName];
      if (cached) setContexts(cached);

      setLoading(true);
      try {
        const data = await getAppContexts(appName, dayOffset);
        setContexts(data);
        setCache((prev) => ({ ...prev, [appName]: data }));
      } catch (err) {
        console.error("Failed to load app contexts:", err);
        setContexts([]);
      } finally {
        setLoading(false);
      }
    },
    [cache, dayOffset, expandedApp]
  );

  const invalidate = useCallback(() => {
    setCache({});
  }, []);

  return {
    expandedApp,
    contexts,
    loading,
    toggle,
    reloadExpanded,
    invalidate,
  };
}
