// Live Activity / Widget extension targets may only use WidgetKit-style previews
// (see Xcode: "The widget extension can only host widget previews").
// https://developer.apple.com/documentation/widgetkit/previewing-widgets-and-live-activities-in-xcode
#if DEBUG
import SwiftUI
import WidgetKit

extension LiveActivitiesAppAttributes {
  /// Seeds App Group `UserDefaults` keys (`{uuid}_recordingStatus`, etc.) for the given attributes id,
  /// matching what Flutter + `live_activities` write at runtime.
  static func previewSeeded(
    recordingStatus: Int = 1,
    latitude: Double? = 31.230416,
    longitude: Double? = 121.473701,
    accuracy: Double? = 4.5,
    gpsTimestampMs: Int? = Int((Date().addingTimeInterval(-4).timeIntervalSince1970 * 1000.0).rounded()),
    hasGpsFix: Bool = true
  ) -> LiveActivitiesAppAttributes {
    let attrs = LiveActivitiesAppAttributes()
    guard let defaults = UserDefaults(suiteName: kRecordingLiveActivityAppGroup) else {
      return attrs
    }
    defaults.set(recordingStatus, forKey: attrs.prefixedKey("recordingStatus"))
    if let lat = latitude {
      defaults.set(lat, forKey: attrs.prefixedKey("latitude"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("latitude"))
    }
    if let lng = longitude {
      defaults.set(lng, forKey: attrs.prefixedKey("longitude"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("longitude"))
    }
    if let acc = accuracy {
      defaults.set(acc, forKey: attrs.prefixedKey("accuracyM"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("accuracyM"))
    }
    if let ts = gpsTimestampMs {
      defaults.set(ts, forKey: attrs.prefixedKey("gpsTimestampMs"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("gpsTimestampMs"))
    }
    defaults.set(hasGpsFix, forKey: attrs.prefixedKey("hasGpsFix"))
    defaults.synchronize()
    return attrs
  }
}

// #Preview for Live Activities requires Swift 5.9+ (Xcode 15+).
// `contentStates:` uses APIs that require iOS / extension 17+ even when the extension
// deployment target is 16.1; mark previews accordingly so the target still ships for 16.1 devices.
#if swift(>=5.9)
@available(iOSApplicationExtension 17.0, *)
#Preview("Live Activity — lock screen", as: .content, using: LiveActivitiesAppAttributes.previewSeeded()) {
  RecordingLiveActivityWidget()
} contentStates: {
  LiveActivitiesAppAttributes.ContentState(appGroupId: kRecordingLiveActivityAppGroup)
}

@available(iOSApplicationExtension 17.0, *)
#Preview("Live Activity — paused", as: .content, using: LiveActivitiesAppAttributes.previewSeeded(
  recordingStatus: 2,
  latitude: nil,
  longitude: nil,
  accuracy: nil,
  gpsTimestampMs: nil,
  hasGpsFix: false
)) {
  RecordingLiveActivityWidget()
} contentStates: {
  LiveActivitiesAppAttributes.ContentState(appGroupId: kRecordingLiveActivityAppGroup)
}

@available(iOSApplicationExtension 17.0, *)
#Preview("Live Activity — island expanded", as: .dynamicIsland(.expanded), using: LiveActivitiesAppAttributes.previewSeeded()) {
  RecordingLiveActivityWidget()
} contentStates: {
  LiveActivitiesAppAttributes.ContentState(appGroupId: kRecordingLiveActivityAppGroup)
}
#endif
#endif
