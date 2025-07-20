import Foundation

struct TimeEntry: Codable {
    let appName: String
    let windowTitle: String
    let startTime: Date
    let durationSeconds: TimeInterval
    
    var csvRow: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return "\"\(appName)\",\"\(windowTitle)\",\"\(formatter.string(from: startTime))\",\(Int(durationSeconds))"
    }
}

struct AppUsage {
    var appName: String
    var totalDuration: TimeInterval
}