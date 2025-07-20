import Foundation
import AppKit
import Accessibility

class WindowTracker {
    private var timer: Timer?
    private var lastWindowInfo: (appName: String, windowTitle: String)?
    private var lastChangeTime = Date()
    private let onWindowChange: (TimeEntry) -> Void
    
    init(onWindowChange: @escaping (TimeEntry) -> Void) {
        self.onWindowChange = onWindowChange
    }
    
    func startTracking() {
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            self?.checkForegroundWindow()
        }
    }
    
    func stopTracking() {
        timer?.invalidate()
        timer = nil
        
        if let lastInfo = lastWindowInfo {
            let duration = Date().timeIntervalSince(lastChangeTime)
            if duration > 0 {
                let entry = TimeEntry(
                    appName: lastInfo.appName,
                    windowTitle: lastInfo.windowTitle,
                    startTime: lastChangeTime,
                    durationSeconds: duration
                )
                onWindowChange(entry)
            }
        }
        
        lastWindowInfo = nil
    }
    
    private func checkForegroundWindow() {
        guard let frontApp = NSWorkspace.shared.frontmostApplication else { return }
        
        let appName = frontApp.localizedName ?? "Unknown"
        var windowTitle = ""
        
        if let windowInfo = getActiveWindowInfo() {
            windowTitle = windowInfo
        }
        
        let currentInfo = (appName: appName, windowTitle: windowTitle)
        
        if lastWindowInfo?.appName != currentInfo.appName || 
           lastWindowInfo?.windowTitle != currentInfo.windowTitle {
            
            if let lastInfo = lastWindowInfo {
                let duration = Date().timeIntervalSince(lastChangeTime)
                if duration > 0 {
                    let entry = TimeEntry(
                        appName: lastInfo.appName,
                        windowTitle: lastInfo.windowTitle,
                        startTime: lastChangeTime,
                        durationSeconds: duration
                    )
                    onWindowChange(entry)
                }
            }
            
            lastWindowInfo = currentInfo
            lastChangeTime = Date()
        }
    }
    
    private func getActiveWindowInfo() -> String? {
        guard let frontApp = NSWorkspace.shared.frontmostApplication else { return nil }
        
        let options = CGWindowListOption([.optionOnScreenOnly, .excludeDesktopElements])
        let windowList = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
        
        for window in windowList {
            guard let windowOwnerPID = window[kCGWindowOwnerPID as String] as? Int32,
                  windowOwnerPID == frontApp.processIdentifier,
                  let windowTitle = window[kCGWindowName as String] as? String,
                  !windowTitle.isEmpty else { continue }
            
            return windowTitle
        }
        
        return nil
    }
}