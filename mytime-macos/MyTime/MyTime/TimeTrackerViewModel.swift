import Foundation
import SwiftUI

@MainActor
class TimeTrackerViewModel: ObservableObject {
    @Published var isTracking = false
    @Published var currentSessionStart: Date?
    @Published var totalTrackedTime: TimeInterval = 0
    @Published var timeEntries: [TimeEntry] = []
    @Published var appUsage: [String: TimeInterval] = [:]
    @Published var showWindow = false
    
    weak var mainWindow: NSWindow?
    
    private var windowTracker: WindowTracker?
    private var timer: Timer?
    private let storage = Storage()
    
    init() {
        windowTracker = WindowTracker { [weak self] entry in
            Task { @MainActor in
                self?.addTimeEntry(entry)
            }
        }
    }
    
    func startTracking() {
        guard !isTracking else { return }
        
        isTracking = true
        currentSessionStart = Date()
        windowTracker?.startTracking()
        
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.objectWillChange.send()
            }
        }
    }
    
    func stopTracking() {
        guard isTracking else { return }
        
        isTracking = false
        if let start = currentSessionStart {
            totalTrackedTime += Date().timeIntervalSince(start)
        }
        currentSessionStart = nil
        
        windowTracker?.stopTracking()
        timer?.invalidate()
        timer = nil
        
        Task {
            await storage.saveToCSV(entries: timeEntries)
        }
    }
    
    func showMainWindow() {
        showWindow = true
        
        DispatchQueue.main.async {
            // Temporarily switch to regular app to show in dock
            NSApp.setActivationPolicy(.regular)
            NSApp.activate(ignoringOtherApps: true)
            
            if let window = self.mainWindow {
                // Show the hidden window
                window.makeKeyAndOrderFront(nil)
                window.center()
            }
            
            // Switch back to accessory after a delay
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                NSApp.setActivationPolicy(.accessory)
            }
        }
    }
    
    func openCSVLocation() {
        let documentsPath = FileManager.default.urls(for: .documentDirectory, 
                                                    in: .userDomainMask).first!
        NSWorkspace.shared.open(documentsPath)
    }
    
    func quit() {
        stopTracking()
        NSApplication.shared.terminate(nil)
    }
    
    var currentDuration: TimeInterval {
        if let start = currentSessionStart {
            return totalTrackedTime + Date().timeIntervalSince(start)
        }
        return totalTrackedTime
    }
    
    var formattedDuration: String {
        let hours = Int(currentDuration) / 3600
        let minutes = (Int(currentDuration) % 3600) / 60
        let seconds = Int(currentDuration) % 60
        return String(format: "%02d:%02d:%02d", hours, minutes, seconds)
    }
    
    var statusText: String {
        if isTracking {
            return "MyTime - Tracking (\(formattedDuration))"
        } else {
            return "MyTime - Stopped (\(formattedDuration))"
        }
    }
    
    private func addTimeEntry(_ entry: TimeEntry) {
        timeEntries.append(entry)
        appUsage[entry.appName, default: 0] += entry.durationSeconds
    }
}