import ActivityKit
import Foundation

@available(iOS 16.1, *)
struct RecordingActivityAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var status: String
        var accuracy: Double?
        var gpsTimestamp: Date?
    }
}
