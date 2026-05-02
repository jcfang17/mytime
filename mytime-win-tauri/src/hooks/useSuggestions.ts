import { useCallback, useEffect, useState } from "react";
import { getSuggestions } from "../api";
import type { AiSuggestion } from "../types";

export function useSuggestions() {
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);

  const reload = useCallback(async () => {
    try {
      const data = await getSuggestions();
      setSuggestions(data);
    } catch (err) {
      console.error("Failed to load suggestions:", err);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return { suggestions, reload };
}
