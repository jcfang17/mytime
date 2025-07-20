import SwiftUI
import AppKit

@main
struct MyTimeApp: App {
    @StateObject private var viewModel = TimeTrackerViewModel()
    @State private var floatingWindow: FloatingWindow?
    
    init() {
        // Check for existing instance
        let runningApps = NSWorkspace.shared.runningApplications
        let currentApp = NSRunningApplication.current
        
        for app in runningApps {
            if app.bundleIdentifier == currentApp.bundleIdentifier && 
               app.processIdentifier != currentApp.processIdentifier {
                // Another instance is already running
                NSWorkspace.shared.open(app.bundleURL!)
                app.activate()
                exit(0)
            }
        }
    }
    
    var body: some Scene {
        WindowGroup("MyTime") {
            ContentView()
                .environmentObject(viewModel)
                .background(WindowAccessor(onWindow: { window in
                    viewModel.mainWindow = window
                    window.titlebarAppearsTransparent = false
                    window.title = "MyTime"
                    window.standardWindowButton(.miniaturizeButton)?.isHidden = true
                    window.standardWindowButton(.zoomButton)?.isHidden = true
                    window.isReleasedWhenClosed = false // Important: prevent window destruction
                }))
                .onAppear {
                    // Create floating timer window
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                        createFloatingTimer()
                    }
                    
                    // Set up termination notification
                    NotificationCenter.default.addObserver(
                        forName: NSApplication.willTerminateNotification,
                        object: nil,
                        queue: .main
                    ) { _ in
                        Task { @MainActor in
                            viewModel.stopTracking()
                        }
                    }
                }
        }
        .windowResizability(.contentSize)
        .defaultPosition(.center)
        .commands {
            CommandGroup(replacing: .newItem) { }
        }
    }
    
    func createFloatingTimer() {
        let floatingView = FloatingTimerView()
            .environmentObject(viewModel)
        
        let controller = NSHostingController(rootView: floatingView)
        let window = FloatingWindow(
            contentRect: NSRect(x: 100, y: 100, width: 200, height: 40),
            styleMask: [],
            backing: .buffered,
            defer: false
        )
        
        window.contentView = controller.view
        window.center()
        window.orderFront(nil)
        window.isReleasedWhenClosed = false // Keep floating window alive
        
        // Position it in the top-right corner, avoiding the notch
        if let screen = NSScreen.main {
            let screenFrame = screen.visibleFrame
            let windowFrame = window.frame
            let x = screenFrame.maxX - windowFrame.width - 20
            let y = screenFrame.maxY - windowFrame.height - 20
            window.setFrameOrigin(NSPoint(x: x, y: y))
        }
    }
}