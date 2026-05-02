import { useCallback, useState } from "react";
import { getLabelProvenance } from "../api";
import type { LabelProvenance } from "../types";

export function useProvenance() {
  const [titleHash, setTitleHash] = useState<string | null>(null);
  const [provenance, setProvenance] = useState<LabelProvenance | null>(null);
  const [loading, setLoading] = useState(false);

  const show = useCallback(async (hash: string) => {
    if (titleHash === hash) {
      setTitleHash(null);
      setProvenance(null);
      return;
    }
    setTitleHash(hash);
    setLoading(true);
    try {
      const data = await getLabelProvenance(hash);
      setProvenance(data);
    } catch (err) {
      console.error("Failed to load provenance:", err);
    } finally {
      setLoading(false);
    }
  }, [titleHash]);

  const clear = useCallback(() => {
    setTitleHash(null);
    setProvenance(null);
  }, []);

  return { titleHash, provenance, loading, show, clear };
}
