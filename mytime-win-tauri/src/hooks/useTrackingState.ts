import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  getTrackingState,
  pauseTracking,
  startTracking,
  stopTracking,
} from "../api";
import type { TrackingState } from "../types";

const POLL_MS = 1000;

export function useTrackingState() {
  const [trackingState, setTrackingState] = useState<TrackingState>({
    is_tracking: false,
    total_time_ms: 0,
    last_capture_ms: null,
    live_edge_ms: 0,
    last_error: null,
    paused_until_ms: null,
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

  const pause = useCallback(async (minutes: number | null) => {
    try {
      const state = await pauseTracking(minutes);
      setTrackingState(state);
    } catch (err) {
      console.error("Failed to pause:", err);
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
    // Payload is minutes; 0 means "until tomorrow".
    const unlistenPause = listen<number>("tray-pause", (event) =>
      pause(event.payload === 0 ? null : event.payload)
    );
    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
      unlistenPause.then((fn) => fn());
    };
  }, [start, stop, pause]);

  return { trackingState, start, stop, pause, reload };
}
