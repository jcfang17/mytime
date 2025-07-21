# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

MyTime is a cross-platform time tracking application with separate native implementations for each platform:

- **mytime-macos/** - macOS version using Swift/SwiftUI
- **mytime-windows/** - Windows version using Rust/egui  
- **Future:** iOS and Android versions will follow the same pattern

Each platform has its own directory with independent codebases tailored to platform-specific capabilities.

**Important Files:**
- **plan.md** - Enhancement plans and feature requirements for schema improvements
- **mytime-windows/implementation-detail.md** - Critical Windows implementation patterns

## Platform-Specific Development

### Windows Version (mytime-windows)

**Technology Stack:**
- Language: Rust (edition 2021)
- GUI Framework: egui/eframe 0.29
- System Integration: windows-rs, tray-icon
- Data Storage: CSV files (mytime_data.csv)

**Build & Run Commands:**
```bash
cd mytime-windows
cargo build --release     # Build release version
cargo run --release       # Run release version
cargo test               # Run tests
cargo clippy             # Run linter
cargo fmt                # Format code
```

**Critical Implementation Detail:**
When implementing features that involve cross-thread communication (e.g., system tray commands), you MUST use `ctx.request_repaint()` to force egui to process events immediately. See `implementation-detail.md` for the specific pattern.

**Key Files:**
- `src/main.rs` - Main application entry, egui setup, window management
- `src/tray.rs` - System tray integration
- `Cargo.toml` - Dependencies and build configuration

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

### Core Functionality (Both Platforms)
1. **Time Tracking:** Records time spent on active application windows
2. **Window Detection:** Monitors currently focused application
3. **Data Persistence:** Stores tracking data in CSV format
4. **System Tray/Menu Bar:** Provides quick access controls
5. **Floating Timer:** Shows current session time (visual differences per platform)

### Data Format
Both platforms use compatible CSV format:
```csv
start_time,end_time,duration,window_title,application
2024-01-20T10:00:00,2024-01-20T10:30:00,1800,Document.txt,TextEdit
```

### Platform Differences
- **Windows:** Single window with system tray, uses Win32 API for window detection
- **macOS:** Menu bar app with floating timer, uses Accessibility API for window detection
- **Windows:** Built as single .exe file
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
- **Windows:** Check `mytime_data.csv` in the working directory
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