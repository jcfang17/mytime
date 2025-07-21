use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, CheckMenuItem},
    TrayIcon, TrayIconBuilder,
};
use std::sync::mpsc;
use eframe::egui;
use std::time::Duration;
use windows::Win32::System::Registry::*;

pub enum TrayCommand {
    Show,
    Start,
    Stop,
    Exit,
    ToggleAutoStart,
}

pub struct TrayManager {
    tray_icon: TrayIcon,
    start_item: MenuItem,
    stop_item: MenuItem,
    auto_start_item: CheckMenuItem,
}

impl TrayManager {
    pub fn new(ctx: egui::Context) -> Result<(Self, mpsc::Receiver<TrayCommand>), Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();

        let menu = Menu::new();
        let show_item = MenuItem::new("Show", true, None);
        let start_item = MenuItem::new("▶ Start Tracking", false, None); // Initially disabled
        let stop_item = MenuItem::new("⏸ Stop Tracking", false, None);   // Initially disabled
        let auto_start_item = CheckMenuItem::new("Start with Windows", true, Self::is_auto_start_enabled(), None);
        let exit_item = MenuItem::new("Exit", true, None);

        // Store IDs before moving into thread
        let show_id = show_item.id().clone();
        let start_id = start_item.id().clone();
        let stop_id = stop_item.id().clone();
        let auto_start_id = auto_start_item.id().clone();
        let exit_id = exit_item.id().clone();

        menu.append(&show_item)?;
        menu.append(&start_item)?;
        menu.append(&stop_item)?;
        menu.append(&auto_start_item)?;
        menu.append(&exit_item)?;

        // Create icon for stopped state (blue)
        let icon = Self::create_stopped_icon()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("MyTime - Stopped")
            .with_icon(icon)
            .build()?;

        let tray_manager = TrayManager {
            tray_icon: tray,
            start_item,
            stop_item,
            auto_start_item,
        };

        // Spawn a thread to handle menu events
        std::thread::spawn(move || {
            loop {
                if let Ok(event) = MenuEvent::receiver().recv() {
                    let command_sent = if event.id == show_id {
                        tx.send(TrayCommand::Show).is_ok()
                    } else if event.id == start_id {
                        tx.send(TrayCommand::Start).is_ok()
                    } else if event.id == stop_id {
                        tx.send(TrayCommand::Stop).is_ok()
                    } else if event.id == auto_start_id {
                        tx.send(TrayCommand::ToggleAutoStart).is_ok()
                    } else if event.id == exit_id {
                        let sent = tx.send(TrayCommand::Exit).is_ok();
                        if sent {
                            ctx.request_repaint();
                        }
                        break;
                    } else {
                        false
                    };

                    // Force the main app to repaint so it processes the command immediately
                    if command_sent {
                        ctx.request_repaint();
                    }
                }
            }
        });

        Ok((tray_manager, rx))
    }

    pub fn update_status(&mut self, is_tracking: bool, total_time: Duration) -> Result<(), Box<dyn std::error::Error>> {
        // Update menu items based on tracking state
        self.start_item.set_enabled(!is_tracking);
        self.stop_item.set_enabled(is_tracking);

        // Update tooltip with status and time
        let hours = total_time.as_secs() / 3600;
        let minutes = (total_time.as_secs() % 3600) / 60;
        let seconds = total_time.as_secs() % 60;

        let status_text = if is_tracking {
            format!("MyTime - Tracking ({}h {}m {}s)", hours, minutes, seconds)
        } else {
            format!("MyTime - Stopped ({}h {}m {}s)", hours, minutes, seconds)
        };

        self.tray_icon.set_tooltip(Some(&status_text))?;

        // Update icon based on tracking state
        let icon = if is_tracking {
            Self::create_tracking_icon()?
        } else {
            Self::create_stopped_icon()?
        };

        self.tray_icon.set_icon(Some(icon))?;

        Ok(())
    }

    fn create_stopped_icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
        // Blue icon for stopped state
        let mut icon_data = vec![0u8; 16 * 16 * 4];
        for chunk in icon_data.chunks_mut(4) {
            chunk[0] = 66;  // R
            chunk[1] = 135; // G
            chunk[2] = 245; // B
            chunk[3] = 255; // A
        }
        Ok(tray_icon::Icon::from_rgba(icon_data, 16, 16)?)
    }

    fn create_tracking_icon() -> Result<tray_icon::Icon, Box<dyn std::error::Error>> {
        // Green icon for tracking state
        let mut icon_data = vec![0u8; 16 * 16 * 4];
        for chunk in icon_data.chunks_mut(4) {
            chunk[0] = 76;  // R
            chunk[1] =175; // G
            chunk[2] = 80;  // B
            chunk[3] = 255; // A
        }
        Ok(tray_icon::Icon::from_rgba(icon_data, 16, 16)?)
    }
    
    fn is_auto_start_enabled() -> bool {
        unsafe {
            let key_path = windows::core::w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut key: HKEY = HKEY::default();
            
            if RegOpenKeyExW(HKEY_CURRENT_USER, key_path, 0, KEY_READ, &mut key).is_err() {
                return false;
            }
            
            let value_name = windows::core::w!("MyTime");
            let result = RegQueryValueExW(key, value_name, None, None, None, None);
            let _ = RegCloseKey(key);
            
            result.is_ok()
        }
    }
    
    pub fn toggle_auto_start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let current_state = Self::is_auto_start_enabled();
        
        unsafe {
            let key_path = windows::core::w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
            let mut key: HKEY = HKEY::default();
            
            if RegOpenKeyExW(HKEY_CURRENT_USER, key_path, 0, KEY_WRITE, &mut key).is_err() {
                return Err("Failed to open registry key".into());
            }
            
            let value_name = windows::core::w!("MyTime");
            
            if current_state {
                // Disable auto-start
                let _ = RegDeleteValueW(key, value_name);
                self.auto_start_item.set_checked(false);
            } else {
                // Enable auto-start
                let exe_path = std::env::current_exe()?
                    .to_str()
                    .ok_or("Invalid exe path")?
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<u16>>();
                
                let exe_bytes = exe_path.as_slice();
                let byte_ptr = exe_bytes.as_ptr() as *const u8;
                let byte_len = exe_bytes.len() * 2; // u16 is 2 bytes
                
                let result = RegSetValueExW(
                    key,
                    value_name,
                    0,
                    REG_SZ,
                    Some(std::slice::from_raw_parts(byte_ptr, byte_len)),
                );
                
                if result.is_err() {
                    let _ = RegCloseKey(key);
                    return Err("Failed to set registry value".into());
                }
                self.auto_start_item.set_checked(true);
            }
            
            let _ = RegCloseKey(key);
        }
        
        Ok(())
    }
}

// Legacy function for backward compatibility
pub fn create_tray_icon(ctx: egui::Context) -> Result<(TrayManager, mpsc::Receiver<TrayCommand>), Box<dyn std::error::Error>> {
    TrayManager::new(ctx)
}