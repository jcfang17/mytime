import { useCallback, useEffect, useState } from "react";
import {
  getAutostartEnabled,
  getAutoTrack,
  getDayStartHour,
  setAutostartEnabled,
  setAutoTrack,
  setDayStartHour,
} from "../api";

export function useSettings() {
  const [dayStartHour, setDayStartHourState] = useState(6);
  const [autostartEnabled, setAutostartEnabledState] = useState(false);
  const [autoTrackEnabled, setAutoTrackEnabledState] = useState(true);

  const reload = useCallback(async () => {
    try {
      const [hour, autostart, autoTrack] = await Promise.all([
        getDayStartHour(),
        getAutostartEnabled(),
        getAutoTrack(),
      ]);
      setDayStartHourState(hour);
      setAutostartEnabledState(autostart);
      setAutoTrackEnabledState(autoTrack);
    } catch (err) {
      console.error("Failed to load settings:", err);
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const updateDayStartHour = useCallback(async (hour: number) => {
    try {
      await setDayStartHour(hour);
      setDayStartHourState(hour);
    } catch (err) {
      console.error("Failed to set day start hour:", err);
    }
  }, []);

  const updateAutostart = useCallback(async (enabled: boolean) => {
    try {
      await setAutostartEnabled(enabled);
      setAutostartEnabledState(enabled);
    } catch (err) {
      console.error("Failed to set autostart:", err);
    }
  }, []);

  const updateAutoTrack = useCallback(async (enabled: boolean) => {
    try {
      await setAutoTrack(enabled);
      setAutoTrackEnabledState(enabled);
    } catch (err) {
      console.error("Failed to set auto-track:", err);
    }
  }, []);

  return {
    dayStartHour,
    autostartEnabled,
    autoTrackEnabled,
    updateDayStartHour,
    updateAutostart,
    updateAutoTrack,
    reload,
  };
}
