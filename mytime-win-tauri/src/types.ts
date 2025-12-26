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

// === Context (for browser site/domain breakdown) ===

export interface ContextSummary {
  context: string;           // Site/domain extracted from title (e.g., "youtube", "github")
  category: string | null;   // Category for this context
  total_duration_ms: number;
  idle_duration_ms: number;
  segment_count: number;
  sample_titles: string[];   // Up to 3 example window titles
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
