# MyTime Windows Implementation Details

## Current Implementation: native-windows-gui

### Why We Switched from egui/eframe

The original implementation used **egui/eframe** but was abandoned due to a fundamental architectural limitation:

**Problem**: egui/eframe's event loop stops processing when the window is hidden/minimized. This made system tray functionality impossible:
- `request_repaint()` is ignored when window is not visible
- Tray menu clicks don't work (ghost tray icon)
- App can only be killed via Task Manager
- No amount of workarounds (Win32 ShowWindow, PostMessage, etc.) could fix this

**Root Cause**: This is a known limitation documented in:
- https://github.com/emilk/egui/discussions/5127
- https://github.com/emilk/egui/discussions/737
- https://github.com/emilk/egui/issues/1223

The egui maintainers acknowledge: *"egui doesn't update once the window is not interactable e.g. invisible/minimized"*

**Conclusion**: egui/eframe is not suitable for tray-based applications. We switched to **native-windows-gui** which provides proper Win32 integration.

---

## Technology Stack

- **GUI Framework**: native-windows-gui (nwg) 1.0
- **Language**: Rust (edition 2021)
- **Windows API**: windows-rs 0.58 for tracking functionality
- **Build**: winres for Windows manifest embedding

## Key Dependencies

```toml
[dependencies]
native-windows-gui = "1.0"
native-windows-derive = "1.0"
windows = { version = "0.58", features = [...] }
chrono = { version = "0.4", features = ["serde"] }
csv = "1.3"
serde = { version = "1.0", features = ["derive"] }

[build-dependencies]
winres = "0.1"
```

## Architecture

### Single-File Design
All code is in `src/main.rs` - approximately 650 lines:
- UI definition using NwgUi derive macro
- Event handlers for window and tray
- Tracker module for foreground window monitoring
- Storage module for CSV persistence

### UI Components

```rust
#[derive(Default, NwgUi)]
pub struct MyTimeApp {
    // Main window with minimize-to-tray behavior
    window: nwg::Window,

    // Controls: labels, buttons, list view
    status_label: nwg::Label,
    time_label: nwg::Label,
    start_btn: nwg::Button,
    stop_btn: nwg::Button,
    app_list: nwg::ListView,

    // Timer for 1-second UI updates
    timer: nwg::AnimationTimer,

    // System tray
    tray_icon: nwg::Icon,
    tray: nwg::TrayNotification,
    tray_menu: nwg::Menu,
    // ... menu items

    // State (RefCell for interior mutability)
    is_tracking: RefCell<bool>,
    // ...
}
```

### Why native-windows-gui Works

1. **Proper Win32 Message Loop**: `nwg::dispatch_thread_events()` is a real Win32 message pump
2. **Window Visibility**: `window.set_visible(false)` truly hides the window while keeping the message loop running
3. **Timer Events**: `AnimationTimer` fires regardless of window visibility
4. **Tray Updates**: All tray operations happen synchronously on the UI thread

## Critical Implementation Details

### Visual Styles (Modern Look)

**Must call before `nwg::init()`**:
```rust
nwg::enable_visual_styles();
nwg::init().expect("Failed to init Native Windows GUI");
```

Without this, the app looks like Windows 95.

### Windows Manifest (Required)

The app requires a manifest for Common Controls v6. Without it, you get `STATUS_ENTRYPOINT_NOT_FOUND` error.

**build.rs**:
```rust
fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest(r#"
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#);
        res.compile().unwrap();
    }
}
```

### Tray Icon Requirement

`TrayNotification` **requires an icon at creation**. Use a system icon as fallback:
```rust
#[nwg_resource(source_system: Some(nwg::OemIcon::Information))]
tray_icon: nwg::Icon,

#[nwg_control(icon: Some(&data.tray_icon), tip: Some("MyTime - Stopped"))]
tray: nwg::TrayNotification,
```

### Minimize to Tray

```rust
fn on_close(&self) {
    // Hide instead of close
    self.window.set_visible(false);
}

fn on_show(&self) {
    self.window.set_visible(true);
    self.window.set_focus();
}
```

### Timer for Live Updates

The timer ensures tray tooltip and UI update every second, even when hidden:
```rust
#[nwg_control(interval: Duration::from_millis(1000))]
#[nwg_events(OnTimerTick: [MyTimeApp::on_timer])]
timer: nwg::AnimationTimer,

// In main():
ui.timer.start();
```

## Features

### Time Tracking
- Monitors foreground window using Win32 `GetForegroundWindow()`
- Records app name, window title, duration, idle time, keystrokes, mouse clicks
- Saves to CSV in same directory as executable

### System Tray
- Left-click: Show window
- Right-click: Context menu (Show, Start, Stop, Auto-start, Exit)
- Tooltip: Shows current status and tracked time

### Auto-Start with Windows
- Uses Registry key: `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`
- Toggle via tray menu with checkbox indicator

### Single Instance
- Uses Windows Mutex to prevent multiple instances:
```rust
let mutex = CreateMutexW(None, true, w!("MyTimeAppMutex"));
if GetLastError() == ERROR_ALREADY_EXISTS {
    return; // Exit if already running
}
```

## Build & Run

```powershell
cd mytime-windows
cargo build --release
.\target\release\mytime-win.exe
```

## Data Format

CSV file: `mytime_data.csv` (same directory as executable)

```csv
app_name,window_title,start_time,end_time,duration_seconds,idle_seconds,keystrokes,mouse_clicks
Code.exe,main.rs - mytime-windows,2025-01-15T10:30:00-05:00,2025-01-15T10:35:00-05:00,300,10,150,25
```

## Troubleshooting

### STATUS_ENTRYPOINT_NOT_FOUND
- Missing Windows manifest in build.rs
- Add winres dependency and manifest configuration

### Win95 Look
- Missing `nwg::enable_visual_styles()` call before `nwg::init()`

### Tray Icon Not Showing
- Icon resource not provided at TrayNotification creation
- Use system icon as fallback

### App Not Responding When Hidden
- This was the egui problem - native-windows-gui handles this correctly
- Timer keeps the message loop active
