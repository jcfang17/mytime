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
  productivity: { emoji: "📝", label: "Productivity", color: "#16a34a" },
  communication: { emoji: "💬", label: "Communication", color: "#a855f7" },
  unknown: { emoji: "📁", label: "Other", color: "#6b7280" },
};

// Fixed stacking/legend order for charts — never re-sorted per data point,
// so a category keeps its position and color everywhere.
export const CATEGORY_ORDER: Category[] = [
  "development",
  "productivity",
  "communication",
  "entertainment",
  "unknown",
];

export function getCategoryInfo(category: string | null) {
  return CATEGORY_INFO[(category as Category) || "unknown"] || CATEGORY_INFO.unknown;
}

// === Classification Rules ===

export type MatchType = "contains" | "prefix" | "exact" | "regex";

export type RuleSource = "builtin" | "user" | "ai-approved";

export interface ClassificationRule {
  rule_id: string;
  app_pattern: string | null;
  title_pattern: string | null;
  match_type: MatchType;
  category: string;
  tags: string[] | null;
  source: RuleSource;
  priority: number;
  enabled: boolean;
  created_at: number;
}

export interface RulePreview {
  match_count: number;
  total_duration_ms: number;
  sample_titles: string[];
}

// === Category Breakdown ===

export interface CategoryBreakdownEntry {
  category: string;
  total_ms: number;
  idle_ms: number;
}

// === History (multi-day views) ===

export interface DayHistory {
  day_offset: number;
  date_label: string; // e.g. "Jul 3"
  weekday: string; // e.g. "Thu"
  total_ms: number;
  active_ms: number;
  categories: CategoryBreakdownEntry[];
}

// === AI Insights ===

export interface InsightReport {
  headline: string;
  insights: string[];
}

// === Context (for browser site/domain breakdown) ===

export interface ContextSummary {
  context: string;           // Site/domain extracted from title (e.g., "youtube", "github")
  category: string | null;   // Category for this context
  total_duration_ms: number;
  idle_duration_ms: number;
  segment_count: number;
  sample_titles: string[];   // Up to 3 example window titles
}

// === Selected Breakdown (segment-level, respects mixed-use apps) ===

export interface SelectedBreakdownRow {
  app_name: string;
  friendly_name: string;
  context: string | null; // For browsers: extracted site (or "other"); otherwise null
  category: string;
  total_duration_ms: number;
  idle_duration_ms: number;
  segment_count: number;
}

// === Daily Digest ===

export interface DailyDigest {
  total_tracked_ms: number;
  total_active_ms: number;
  top_categories: DigestCategoryEntry[];
  top_apps: DigestAppEntry[];
  longest_focus: DigestFocusBlock | null;
  most_idle: DigestIdleEntry | null;
}

export interface DigestCategoryEntry {
  category: string;
  duration_ms: number;
  idle_ms: number;
  percentage: number;
}

export interface DigestAppEntry {
  app_name: string;
  friendly_name: string;
  duration_ms: number;
  idle_ms: number;
  category: string | null;
}

export interface DigestFocusBlock {
  app_name: string;
  friendly_name: string;
  duration_ms: number;
}

export interface DigestIdleEntry {
  app_name: string;
  friendly_name: string;
  window_title: string;
  idle_seconds: number;
  duration_ms: number;
}

// === Unknown Cleanup Queue ===

export interface UnknownQueueItem {
  app_name: string;
  friendly_name: string;
  context: string | null;
  total_duration_ms: number;
  idle_duration_ms: number;
  segment_count: number;
  sample_titles: string[];
}

// === Timeline ===

export interface TimelineSegment {
  segment_id: string;
  app_name: string;
  friendly_name: string;
  window_title: string | null;
  title_hash: string;
  start_time: number;
  end_time: number;
  category: string;
  idle_seconds: number;
}

// === Label Provenance ===

export interface Label {
  title_hash: string;
  category: string;
  source: string; // "manual" | "user" | "ai" | "heuristic"
  confidence: number | null;
  updated_at: number;
}

export interface LabelProvenance {
  best_label: Label | null;
  matching_rule: ClassificationRule | null;
}

// === AI Suggestions ===

export type SuggestionStatus = "pending" | "approved" | "rejected" | "expired";

export interface AiSuggestion {
  suggestion_id: string;
  app_pattern: string | null;
  title_pattern: string | null;
  match_type: MatchType;
  suggested_category: string;
  confidence: number;        // 0.0-1.0
  reason: string;            // Why AI suggested this
  sample_titles: string[];   // Example titles that would match
  match_count: number;       // How many historical segments match
  total_duration_ms: number; // Total time of matching segments
  status: SuggestionStatus;
  created_at: number;
  reviewed_at: number | null;
}
