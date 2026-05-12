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
    startedMs: Int? = Int((Date().addingTimeInterval(-90 * 60).timeIntervalSince1970 * 1000.0).rounded()),
    accuracy: Double? = 4.5,
    hasGpsFix: Bool = true
  ) -> LiveActivitiesAppAttributes {
    let attrs = LiveActivitiesAppAttributes()
    guard let defaults = UserDefaults(suiteName: kRecordingLiveActivityAppGroup) else {
      return attrs
    }
    defaults.set(recordingStatus, forKey: attrs.prefixedKey("recordingStatus"))
    if let ms = startedMs {
      defaults.set(ms, forKey: attrs.prefixedKey("startedAtEpochMs"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("startedAtEpochMs"))
    }
    if let acc = accuracy {
      defaults.set(acc, forKey: attrs.prefixedKey("accuracyM"))
    } else {
      defaults.removeObject(forKey: attrs.prefixedKey("accuracyM"))
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
  startedMs: Int((Date().addingTimeInterval(-45 * 60).timeIntervalSince1970 * 1000.0).rounded()),
  accuracy: 12,
  hasGpsFix: true
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
