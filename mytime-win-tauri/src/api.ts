// API functions for communicating with the Tauri backend

import { invoke } from "@tauri-apps/api/core";
import type { TrackingState, AppSummary, ClassificationRule, RulePreview, MatchType, ContextSummary, AiSuggestion, CategoryBreakdownEntry, SelectedBreakdownRow } from "./types";

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
): Promise<CategoryBreakdownEntry[]> {
  return await invoke("get_category_breakdown", { dayOffset });
}

export async function getAppContexts(
  appName: string,
  dayOffset: number
): Promise<ContextSummary[]> {
  return await invoke("get_app_contexts", { appName, dayOffset });
}

export async function getSelectedBreakdown(
  dayOffset: number,
  categories: string[]
): Promise<SelectedBreakdownRow[]> {
  return await invoke("get_selected_breakdown", { dayOffset, categories });
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

// === Classification Rules API ===

export async function getRules(): Promise<ClassificationRule[]> {
  return await invoke("get_rules");
}

export async function getRule(ruleId: string): Promise<ClassificationRule | null> {
  return await invoke("get_rule", { ruleId });
}

export async function createRule(
  appPattern: string | null,
  titlePattern: string | null,
  matchType: MatchType,
  category: string,
  tags: string[] | null
): Promise<ClassificationRule> {
  return await invoke("create_rule", {
    appPattern,
    titlePattern,
    matchType,
    category,
    tags,
  });
}

export async function updateRule(
  ruleId: string,
  appPattern: string | null,
  titlePattern: string | null,
  matchType: MatchType,
  category: string,
  tags: string[] | null,
  enabled: boolean,
  priority: number
): Promise<void> {
  return await invoke("update_rule", {
    ruleId,
    appPattern,
    titlePattern,
    matchType,
    category,
    tags,
    enabled,
    priority,
  });
}

export async function deleteRule(ruleId: string): Promise<void> {
  return await invoke("delete_rule", { ruleId });
}

export async function previewRuleMatches(
  appPattern: string | null,
  titlePattern: string | null,
  matchType: MatchType,
  daysBack: number
): Promise<RulePreview> {
  return await invoke("preview_rule_matches", {
    appPattern,
    titlePattern,
    matchType,
    daysBack,
  });
}

// === AI Suggestions API ===

export async function getSuggestions(): Promise<AiSuggestion[]> {
  return await invoke("get_suggestions");
}

export async function approveSuggestion(
  suggestionId: string
): Promise<ClassificationRule> {
  return await invoke("approve_suggestion", { suggestionId });
}

export async function rejectSuggestion(suggestionId: string): Promise<void> {
  return await invoke("reject_suggestion", { suggestionId });
}

export async function createSuggestion(
  appPattern: string | null,
  titlePattern: string | null,
  matchType: MatchType,
  suggestedCategory: string,
  confidence: number,
  reason: string,
  sampleTitles: string[],
  matchCount: number,
  totalDurationMs: number
): Promise<AiSuggestion> {
  return await invoke("create_suggestion", {
    appPattern,
    titlePattern,
    matchType,
    suggestedCategory,
    confidence,
    reason,
    sampleTitles,
    matchCount,
    totalDurationMs,
  });
}
