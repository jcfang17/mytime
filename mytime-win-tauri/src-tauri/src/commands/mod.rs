//! Tauri commands organized by domain.
//!
//! Each submodule owns the commands for one product area. lib.rs imports them
//! and registers them with `tauri::generate_handler!`.

pub mod breakdown;
pub mod digest;
pub mod rules;
pub mod settings;
pub mod suggestions;
pub mod tracking;

/// Number of days back to scan when backfilling labels after a rule
/// is created or an AI suggestion is approved.
pub(crate) const BACKFILL_DAYS: u32 = 7;
