import SwiftUI
import Charts

struct ContentView: View {
    @EnvironmentObject var viewModel: TimeTrackerViewModel
    @State private var showingChart = false
    
    var body: some View {
        ZStack {
            Color(NSColor.windowBackgroundColor)
                .ignoresSafeArea()
            
            VStack(spacing: 20) {
            Text("MyTime - Time Tracker")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            Divider()
            
            HStack(spacing: 20) {
                Button(action: { viewModel.startTracking() }) {
                    Label("Start", systemImage: "play.fill")
                        .frame(width: 100)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(viewModel.isTracking)
                .tint(.green)
                
                Button(action: { viewModel.stopTracking() }) {
                    Label("Stop", systemImage: "stop.fill")
                        .frame(width: 100)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(!viewModel.isTracking)
                .tint(.red)
                
                Spacer()
                
                Text("Status: \(viewModel.isTracking ? "Tracking" : "Stopped")")
                    .font(.headline)
                    .foregroundColor(viewModel.isTracking ? .green : .secondary)
            }
            
            Divider()
            
            HStack {
                Button(action: { viewModel.quit() }) {
                    Label("Quit", systemImage: "xmark.circle.fill")
                }
                .buttonStyle(.bordered)
                .tint(.red)
                
                Spacer()
                
                Text("(Close window to minimize to menu bar)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Divider()
            
            VStack(alignment: .leading, spacing: 10) {
                Text("Total Time: \(viewModel.formattedDuration)")
                    .font(.title2)
                    .fontWeight(.medium)
                
                let hours = Int(viewModel.currentDuration) / 3600
                let minutes = (Int(viewModel.currentDuration) % 3600) / 60
                let seconds = Int(viewModel.currentDuration) % 60
                
                Text("\(hours) hours \(minutes) minutes \(seconds) seconds")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            
            Divider()
            
            HStack {
                Button(action: { showingChart.toggle() }) {
                    Label("Show Chart", systemImage: "chart.bar.fill")
                }
                .buttonStyle(.bordered)
                
                Spacer()
                
                Text("Data saved to: ~/Documents/mytime_data.csv")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            if !viewModel.appUsage.isEmpty {
                Divider()
                
                VStack(alignment: .leading, spacing: 8) {
                    Text("App Usage")
                        .font(.headline)
                    
                    ScrollView {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(viewModel.appUsage.sorted(by: { $0.value > $1.value }), id: \.key) { app, duration in
                                HStack {
                                    Text(app)
                                        .lineLimit(1)
                                    Spacer()
                                    Text("\(Int(duration) / 60) min")
                                        .foregroundColor(.secondary)
                                        .font(.system(.body, design: .monospaced))
                                }
                            }
                        }
                    }
                    .frame(maxHeight: 200)
                }
            }
            }
            .padding()
            .frame(width: 600, height: 500)
        }
        .sheet(isPresented: $showingChart) {
            ChartView()
                .environmentObject(viewModel)
        }
    }
}

struct ChartView: View {
    @EnvironmentObject var viewModel: TimeTrackerViewModel
    @Environment(\.dismiss) var dismiss
    
    var chartData: [(app: String, duration: Double)] {
        viewModel.appUsage
            .sorted(by: { $0.value > $1.value })
            .prefix(10)
            .map { (app: $0.key, duration: $0.value / 60) }
    }
    
    var body: some View {
        VStack(spacing: 20) {
            Text("App Usage Chart")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            if #available(macOS 13.0, *) {
                Chart(chartData, id: \.app) { item in
                    BarMark(
                        x: .value("Duration (minutes)", item.duration),
                        y: .value("App", item.app)
                    )
                    .foregroundStyle(.blue.gradient)
                }
                .frame(height: 400)
                .padding()
            } else {
                Text("Charts require macOS 13.0 or later")
                    .foregroundColor(.secondary)
            }
            
            Button("Close") {
                dismiss()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
        .frame(width: 800, height: 600)
    }
}

#Preview {
    ContentView()
        .environmentObject(TimeTrackerViewModel())
}