import { useCallback, useEffect, useState } from "react";
import { getRules } from "../api";
import type { ClassificationRule } from "../types";

export function useRules() {
  const [rules, setRules] = useState<ClassificationRule[]>([]);

  const reload = useCallback(async () => {
    try {
      const data = await getRules();
      setRules(data);
    } catch (err) {
      console.error("Failed to load rules:", err);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return { rules, reload };
}
