import Foundation

class Storage {
    private let fileName = "mytime_data.csv"
    
    private var fileURL: URL {
        let documentsPath = FileManager.default.urls(for: .documentDirectory, 
                                                    in: .userDomainMask).first!
        return documentsPath.appendingPathComponent(fileName)
    }
    
    func saveToCSV(entries: [TimeEntry]) async {
        var csvContent = ""
        
        let fileExists = FileManager.default.fileExists(atPath: fileURL.path)
        
        if !fileExists {
            csvContent = "app_name,window_title,start_time,duration_seconds\n"
        }
        
        for entry in entries {
            csvContent += entry.csvRow + "\n"
        }
        
        do {
            if fileExists {
                let fileHandle = try FileHandle(forWritingTo: fileURL)
                fileHandle.seekToEndOfFile()
                if let data = csvContent.data(using: .utf8) {
                    fileHandle.write(data)
                }
                fileHandle.closeFile()
            } else {
                try csvContent.write(to: fileURL, atomically: true, encoding: .utf8)
            }
        } catch {
            print("Error saving CSV: \(error)")
        }
    }
    
    func loadEntries() async -> [TimeEntry] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return []
        }
        
        do {
            let content = try String(contentsOf: fileURL, encoding: .utf8)
            let lines = content.components(separatedBy: .newlines)
            
            var entries: [TimeEntry] = []
            
            for (index, line) in lines.enumerated() {
                if index == 0 || line.isEmpty { continue }
                
                let components = parseCSVLine(line)
                if components.count == 4,
                   let duration = TimeInterval(components[3]) {
                    
                    let formatter = ISO8601DateFormatter()
                    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
                    
                    if let date = formatter.date(from: components[2]) {
                        let entry = TimeEntry(
                            appName: components[0],
                            windowTitle: components[1],
                            startTime: date,
                            durationSeconds: duration
                        )
                        entries.append(entry)
                    }
                }
            }
            
            return entries
        } catch {
            print("Error loading CSV: \(error)")
            return []
        }
    }
    
    private func parseCSVLine(_ line: String) -> [String] {
        var components: [String] = []
        var currentComponent = ""
        var inQuotes = false
        
        for char in line {
            if char == "\"" {
                inQuotes.toggle()
            } else if char == "," && !inQuotes {
                components.append(currentComponent)
                currentComponent = ""
            } else {
                currentComponent.append(char)
            }
        }
        
        if !currentComponent.isEmpty {
            components.append(currentComponent)
        }
        
        return components
    }
}