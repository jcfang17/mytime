import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getTrackingState, startTracking, stopTracking } from "../api";
import type { TrackingState } from "../types";

const POLL_MS = 1000;

export function useTrackingState() {
  const [trackingState, setTrackingState] = useState<TrackingState>({
    is_tracking: false,
    session_start_ms: null,
    total_time_ms: 0,
    baseline_ms: null,
  });

  const reload = useCallback(async () => {
    try {
      const state = await getTrackingState();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to load tracking state:", err);
    }
  }, []);

  const start = useCallback(async () => {
    try {
      const state = await startTracking();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to start:", err);
    }
  }, []);

  const stop = useCallback(async () => {
    try {
      const state = await stopTracking();
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to stop:", err);
    }
  }, []);

  useEffect(() => {
    reload();
    const interval = setInterval(reload, POLL_MS);
    return () => clearInterval(interval);
  }, [reload]);

  useEffect(() => {
    const unlistenStart = listen("tray-start", () => start());
    const unlistenStop = listen("tray-stop", () => stop());
    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, [start, stop]);

  return { trackingState, start, stop, reload };
}
