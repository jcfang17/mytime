# MyTime - macOS Time Tracker

A macOS time tracking application that monitors active windows and tracks time spent in different applications.

## Features

- Floating timer window that avoids the notch on newer MacBooks
- Automatic window tracking using NSWorkspace
- CSV export of time entries to `~/Documents/mytime_data.csv`
- App usage statistics and charts
- Menu bar app that runs in background
- Single instance enforcement
- Auto-save on quit

## Building the App

### Requirements
- macOS 14.0+ (for Charts framework)
- Xcode 15.0+
- Swift 5.9+

### Build Instructions

1. Open Terminal and navigate to the project directory:
   ```bash
   cd mytime-macos/MyTime
   ```

2. Open the project in Xcode:
   ```bash
   open MyTime.xcodeproj
   ```

3. Select your development team in the project settings (if needed)

4. Build and run the project (⌘+R)

### Permissions

The app requires accessibility permissions to track window titles. macOS will prompt you to grant these permissions on first run.

## Usage

1. A floating timer window appears in the top-right corner
2. Click the play button to start tracking
3. Click the gear menu for options:
   - Show Main Window - Opens detailed statistics
   - Open Data Location - Opens the folder containing CSV
   - Quit MyTime - Exits the app
4. Time data is automatically saved to `~/Documents/mytime_data.csv`

## Technical Implementation

### Key Components

- **FloatingTimerView.swift** - Floating window UI with timer controls
- **WindowTracker.swift** - Uses NSWorkspace and CGWindow APIs to track active windows
- **TimeTrackerViewModel.swift** - ObservableObject managing app state
- **Storage.swift** - CSV file operations for persistent storage
- **WindowAccessor.swift** - Helper to capture NSWindow references

### Solutions to Common Issues

1. **Notch Compatibility** - Used a floating window instead of menu bar to avoid notch
2. **Window Persistence** - Set `isReleasedWhenClosed = false` to prevent window destruction
3. **LSUIElement Issues** - Temporarily switch activation policy when showing windows
4. **Single Instance** - Check for existing instances on launch
5. **Actor Isolation** - Wrapped async calls in `Task { @MainActor in }`

## File Structure

```
mytime-macos/
└── MyTime/
    ├── MyTime.xcodeproj/
    └── MyTime/
        ├── MyTimeApp.swift            # Main app entry point
        ├── ContentView.swift          # Main window UI
        ├── FloatingTimerView.swift    # Floating timer window
        ├── WindowAccessor.swift       # Window reference helper
        ├── TimeEntry.swift            # Data models
        ├── WindowTracker.swift        # Window tracking logic
        ├── Storage.swift              # CSV storage
        ├── TimeTrackerViewModel.swift # App state management
        ├── Assets.xcassets/
        ├── MyTime.entitlements
        └── Info.plist
```