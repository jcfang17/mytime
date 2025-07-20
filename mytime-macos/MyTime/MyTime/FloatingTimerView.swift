import SwiftUI
import AppKit

struct FloatingTimerView: View {
    @EnvironmentObject var viewModel: TimeTrackerViewModel
    @State private var showingMenu = false
    
    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: viewModel.isTracking ? "clock.fill" : "clock")
                .foregroundColor(viewModel.isTracking ? .green : .secondary)
                .font(.system(size: 14))
            
            if viewModel.isTracking {
                Text(viewModel.formattedDuration)
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
            }
            
            Button(action: {
                if viewModel.isTracking {
                    viewModel.stopTracking()
                } else {
                    viewModel.startTracking()
                }
            }) {
                Image(systemName: viewModel.isTracking ? "stop.circle" : "play.circle")
                    .foregroundColor(viewModel.isTracking ? .red : .green)
                    .font(.system(size: 14))
            }
            .buttonStyle(.plain)
            
            Menu {
                Button(action: {
                    viewModel.showMainWindow()
                }) {
                    Label("Show Main Window", systemImage: "macwindow")
                }
                
                Divider()
                
                Button(action: {
                    viewModel.openCSVLocation()
                }) {
                    Label("Open Data Location", systemImage: "folder")
                }
                
                Divider()
                
                Button(action: {
                    viewModel.quit()
                }) {
                    Label("Quit MyTime", systemImage: "xmark.circle")
                }
                .keyboardShortcut("q")
            } label: {
                Image(systemName: "gear")
                    .font(.system(size: 14))
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color(NSColor.controlBackgroundColor))
                .shadow(radius: 2)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color(NSColor.separatorColor), lineWidth: 0.5)
        )
    }
}

class FloatingWindow: NSPanel {
    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: [.borderless, .nonactivatingPanel, .utilityWindow], backing: backingStoreType, defer: flag)
        
        self.isOpaque = false
        self.backgroundColor = .clear
        self.level = .floating
        self.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary]
        self.isMovableByWindowBackground = true
        self.hasShadow = true
        self.becomesKeyOnlyIfNeeded = true
        self.isFloatingPanel = true
        self.acceptsMouseMovedEvents = true
    }
    
    override var canBecomeKey: Bool {
        return true
    }
    
    override var canBecomeMain: Bool {
        return false
    }
}