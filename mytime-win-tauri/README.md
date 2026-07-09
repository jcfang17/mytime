# MyTime for Windows

A private, automatic Windows activity journal. MyTime records which application
window is in the foreground, splits the day into segments, categorizes them,
and shows you where your time went — all stored locally in SQLite.

## Features

- **Automatic tracking** — event-driven foreground-window detection with
  stable-title segmentation, idle detection, keystroke/click activity counts,
  focus-session grouping, and workstation-lock / sleep handling
- **Dashboard** — live timer, per-app and per-category totals, an interactive
  day timeline with idle shading, and per-site drill-down for browsers
- **History** — 7/30-day stacked charts, comparison against the previous
  period, daily averages, and range-wide top apps
- **Categorization** — pattern-based classification rules with live preview,
  manual overrides, a cleanup queue for uncategorized time, and "why this
  category?" provenance
- **AI (optional)** — Claude-generated rule suggestions from your
  uncategorized activity and natural-language period insights. Requires
  `ANTHROPIC_API_KEY` in the environment; off otherwise. Suggestions send
  window-title samples of uncategorized activity to the Anthropic API;
  insights send only aggregates (categories, app names, durations).
- **System tray** — close-to-tray, start/stop from the tray menu, autostart
  at login
- **Local-first** — all data lives in a local SQLite database
  (`%APPDATA%\MyTime`), with CSV export

## Development

Prerequisites: [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/), and
[pnpm](https://pnpm.io/) (this project uses pnpm, not npm).

```bash
pnpm install        # install frontend dependencies
pnpm tauri dev      # run in development mode (hot reload)
pnpm tauri build    # build release exe + installers
```

Release output lands in `src-tauri/target/release/` (standalone
`mytime-win-tauri.exe`, plus MSI/NSIS installers under `bundle/`).

Rust-only checks, from `src-tauri/`:

```bash
cargo check
cargo test
cargo fmt
cargo clippy
```

CI (`.github/workflows/ci.yml`) runs formatting, Clippy, Rust tests, and the
frontend build.

## Architecture

- `src-tauri/src/tracker.rs` — Win32 window tracking (SetWinEventHook,
  GetLastInputInfo, keyboard/mouse hooks, lock/suspend detection)
- `src-tauri/src/storage/sqlite.rs` — SQLite storage, migrations,
  overlap-aware aggregate queries
- `src-tauri/src/commands/` — Tauri commands by domain (tracking, breakdown,
  history, digest, rules, suggestions, insights, settings)
- `src-tauri/src/ai.rs` — minimal Anthropic Messages API client (optional
  features)
- `src/` — React + TypeScript frontend (Vite)

See `../plan.md` for the roadmap and design notes.
