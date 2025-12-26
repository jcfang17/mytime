# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

MyTime is a cross-platform time tracking application with separate native implementations for each platform:

- **mytime-macos/** - macOS version using Swift/SwiftUI
- **mytime-win-tauri/** - Windows version using Tauri + React
- **Future:** iOS and Android versions will follow the same pattern

Each platform has its own directory with independent codebases tailored to platform-specific capabilities.

**Important Files:**
- **plan.md** - Enhancement plans and feature requirements for schema improvements

## Platform-Specific Development

### Windows Version (mytime-win-tauri)

**Technology Stack:**
- Backend: Rust with Tauri 2.x
- Frontend: React + TypeScript + Vite
- Package Manager: **pnpm** (NOT npm)
- Data Storage: SQLite

**Build & Run Commands:**
```bash
cd mytime-win-tauri
pnpm install              # Install dependencies
pnpm tauri dev            # Run in development mode
pnpm tauri build          # Build release version
pnpm build                # Build frontend only (tsc + vite)

# Rust-only commands
cd src-tauri
cargo test                # Run Rust tests
cargo check               # Check compilation
```

**Key Files:**
- `src/App.tsx` - Main React application
- `src/api.ts` - Tauri command bindings
- `src/types.ts` - TypeScript type definitions
- `src-tauri/src/lib.rs` - Tauri commands
- `src-tauri/src/storage/sqlite.rs` - SQLite storage implementation
- `src-tauri/src/tracker.rs` - Window tracking logic

### macOS Version (mytime-macos)

**Technology Stack:**
- Language: Swift 5.9+
- Framework: SwiftUI with Charts
- Minimum OS: macOS 14.0+
- Project: Xcode 15.0+ required

**Build & Run Commands:**
```bash
cd mytime-macos/MyTime
# Command line build
xcodebuild -project MyTime.xcodeproj -scheme MyTime -configuration Release build
# Run built app
open build/Release/MyTime.app

# Or use Xcode GUI
open MyTime.xcodeproj  # Then press ⌘+R to run
```

**Key Files:**
- `MyTime/MyTimeApp.swift` - App entry point, menu bar setup
- `MyTime/TimeTrackerViewModel.swift` - Core time tracking logic
- `MyTime/WindowTracker.swift` - Active window monitoring
- `MyTime/FloatingTimerView.swift` - Floating timer UI
- `MyTime/Storage.swift` - CSV data persistence

**Permissions:** Requires accessibility permissions for window tracking.

## Architecture Overview

### Core Functionality
1. **Time Tracking:** Records time spent on active application windows
2. **Window Detection:** Monitors currently focused application
3. **Data Persistence:** SQLite (Windows), CSV (macOS)
4. **System Tray/Menu Bar:** Provides quick access controls
5. **Classification Rules:** Pattern-based categorization of activities

### Platform Differences
- **Windows (Tauri):** Single window with system tray, uses Win32 API for window detection, SQLite storage
- **macOS:** Menu bar app with floating timer, uses Accessibility API for window detection, CSV storage
- **Windows:** Built as single .exe file via Tauri
- **macOS:** Built as .app bundle requiring code signing

## Development Guidelines

1. **Platform Isolation:** Each platform directory is independent. Don't share code between platforms.
2. **Testing:** Currently no formal test suites. Test manually before committing.
3. **Data Compatibility:** Ensure CSV format remains consistent across platforms.
4. **UI Patterns:** Follow platform-specific design guidelines (Windows: system tray, macOS: menu bar).

## Common Tasks

### Adding a New Feature
1. Implement separately in each platform directory
2. Maintain CSV compatibility if modifying data format
3. Test on target platform
4. Update platform-specific README if needed

### Debugging Time Tracking
- **Windows:** Check SQLite database in `%APPDATA%/com.mytime.app/` or use export CSV
- **macOS:** Check CSV export functionality via menu
- Both platforms log window changes to their respective consoles

### Cross-Platform Considerations
When implementing new features, consider:
- File paths (Windows: backslash, macOS: forward slash)
- System integration APIs differ significantly
- UI paradigms (Windows: windowed, macOS: menu bar)
- Permission models (macOS requires explicit accessibility permissions)

### Requirements

Update memory often. If we see some errors, and manage to fix it, we should update docs or crucial memory with errors and fix so we avoid repeating it.

Do not be smart and use "fallback"s or alternatives without my permission. You can iterate and use tools to try to solve, but do not just "simply" or "use fallback". If not possible, just tell me.