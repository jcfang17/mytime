// TypeScript types for MyTime

export interface TrackingState {
  is_tracking: boolean;
  session_start_ms: number | null;
  total_time_ms: number;
  baseline_ms: number | null; // Total at session start (to avoid double-counting)
}

export interface AppSummary {
  app_name: string;
  friendly_name: string;
  total_duration_ms: number;
  idle_duration_ms: number;
  segment_count: number;
  keystrokes: number;
  mouse_clicks: number;
  primary_category: string | null;
}

export type Category =
  | "entertainment"
  | "development"
  | "productivity"
  | "communication"
  | "unknown";

export const CATEGORY_INFO: Record<
  Category,
  { emoji: string; label: string; color: string }
> = {
  entertainment: { emoji: "🎬", label: "Entertainment", color: "#ef4444" },
  development: { emoji: "💻", label: "Development", color: "#3b82f6" },
  productivity: { emoji: "📝", label: "Productivity", color: "#22c55e" },
  communication: { emoji: "💬", label: "Communication", color: "#a855f7" },
  unknown: { emoji: "📁", label: "Other", color: "#6b7280" },
};

export function getCategoryInfo(category: string | null) {
  return CATEGORY_INFO[(category as Category) || "unknown"] || CATEGORY_INFO.unknown;
}
