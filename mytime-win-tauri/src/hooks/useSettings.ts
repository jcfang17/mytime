import { useCallback, useEffect, useState } from "react";
import {
  getAutostartEnabled,
  getDayStartHour,
  setAutostartEnabled,
  setDayStartHour,
} from "../api";

export function useSettings() {
  const [dayStartHour, setDayStartHourState] = useState(6);
  const [autostartEnabled, setAutostartEnabledState] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [hour, autostart] = await Promise.all([
        getDayStartHour(),
        getAutostartEnabled(),
      ]);
      setDayStartHourState(hour);
      setAutostartEnabledState(autostart);
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

  return {
    dayStartHour,
    autostartEnabled,
    updateDayStartHour,
    updateAutostart,
    reload,
  };
}
