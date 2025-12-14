// API functions for communicating with the Tauri backend

import { invoke } from "@tauri-apps/api/core";
import type { TrackingState, AppSummary } from "./types";

export async function startTracking(): Promise<TrackingState> {
  return await invoke("start_tracking");
}

export async function stopTracking(): Promise<TrackingState> {
  return await invoke("stop_tracking");
}

export async function getTrackingState(): Promise<TrackingState> {
  return await invoke("get_tracking_state");
}

export async function getAppBreakdown(dayOffset: number): Promise<AppSummary[]> {
  return await invoke("get_app_breakdown", { dayOffset });
}

export async function getCategoryBreakdown(
  dayOffset: number
): Promise<[string, number][]> {
  return await invoke("get_category_breakdown", { dayOffset });
}

export async function setAppCategory(
  appName: string,
  category: string,
  dayOffset: number
): Promise<void> {
  return await invoke("set_app_category", { appName, category, dayOffset });
}

export async function getDayLabel(dayOffset: number): Promise<string> {
  return await invoke("get_day_label", { dayOffset });
}

export async function getDayStartHour(): Promise<number> {
  return await invoke("get_day_start_hour");
}

export async function setDayStartHour(hour: number): Promise<void> {
  return await invoke("set_day_start_hour", { hour });
}

export async function exportCsv(dayOffset: number): Promise<number> {
  return await invoke("export_csv", { dayOffset });
}

export async function formatDuration(ms: number): Promise<string> {
  return await invoke("format_duration", { ms });
}

export async function getAutostartEnabled(): Promise<boolean> {
  return await invoke("get_autostart_enabled");
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  return await invoke("set_autostart_enabled", { enabled });
}

// Utility function for formatting duration on the frontend
export function formatDurationLocal(ms: number): string {
  const totalSecs = Math.floor(ms / 1000);
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;

  if (hours > 0) {
    return `${hours.toString().padStart(2, "0")}:${minutes
      .toString()
      .padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
  }
  return `${minutes.toString().padStart(2, "0")}:${secs
    .toString()
    .padStart(2, "0")}`;
}
