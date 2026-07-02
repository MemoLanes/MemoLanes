import ActivityKit
import SwiftUI
import WidgetKit

// Must match `live_activities` plugin — see LiveActivitiesPlugin.swift in the package.
struct LiveActivitiesAppAttributes: ActivityAttributes, Identifiable {
  public typealias LiveDeliveryData = ContentState

  public struct ContentState: Codable, Hashable {
    var appGroupId: String
  }

  var id = UUID()
}

extension LiveActivitiesAppAttributes {
  func prefixedKey(_ key: String) -> String {
    "\(id)_\(key)"
  }
}

// Align with Flutter app theme (dark + lime accent).
enum MemoLiveTheme {
  static let lime = Color(red: 0.71, green: 0.93, blue: 0.32)
  static let limeDark = Color(red: 0.55, green: 0.76, blue: 0.18)
  static let blue = Color(red: 0.38, green: 0.72, blue: 1.0)
  static let surface = Color(red: 0.08, green: 0.08, blue: 0.09)
  static let surfaceElevated = Color(red: 0.12, green: 0.12, blue: 0.13)
  static let border = Color.white.opacity(0.08)
  static let textPrimary = Color.white.opacity(0.95)
  static let textSecondary = Color.white.opacity(0.55)
  static let textMuted = Color.white.opacity(0.38)
  static let recordingDot = Color(red: 1.0, green: 0.32, blue: 0.28)
  static let pausedDot = Color(red: 1.0, green: 0.84, blue: 0.2)
}

@available(iOS 16.1, *)
private func readInt(
  _ defaults: UserDefaults, attributes: LiveActivitiesAppAttributes, key: String, defaultValue: Int = 0
) -> Int {
  let fullKey = attributes.prefixedKey(key)
  let v = defaults.object(forKey: fullKey)
  if let i = v as? Int { return i }
  if let n = v as? NSNumber { return n.intValue }
  return defaultValue
}

@available(iOS 16.1, *)
private func readOptionalInt(_ defaults: UserDefaults, attributes: LiveActivitiesAppAttributes, key: String) -> Int? {
  let fullKey = attributes.prefixedKey(key)
  guard defaults.object(forKey: fullKey) != nil else { return nil }
  let value = readInt(defaults, attributes: attributes, key: key, defaultValue: -1)
  return value >= 0 ? value : nil
}

@available(iOS 16.1, *)
private func readOptionalDouble(_ defaults: UserDefaults, attributes: LiveActivitiesAppAttributes, key: String) -> Double? {
  let fullKey = attributes.prefixedKey(key)
  let v = defaults.object(forKey: fullKey)
  if let d = v as? Double { return d }
  if let n = v as? NSNumber { return n.doubleValue }
  return nil
}

@available(iOS 16.1, *)
private func readBool(_ defaults: UserDefaults, attributes: LiveActivitiesAppAttributes, key: String) -> Bool {
  defaults.bool(forKey: attributes.prefixedKey(key))
}

@available(iOS 16.1, *)
private func appDisplayName() -> String {
  if let name = Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String, !name.isEmpty {
    return name
  }
  if let name = Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String, !name.isEmpty {
    return name
  }
  return "MemoLanes"
}

@available(iOS 16.1, *)
private func statusTitle(recordingStatus: Int) -> String {
  switch recordingStatus {
  case 2:
    return String(localized: "live_activity.status_paused", bundle: .main)
  case 1:
    return String(localized: "live_activity.status_recording", bundle: .main)
  default:
    return String(localized: "live_activity.status_recording", bundle: .main)
  }
}

@available(iOS 16.1, *)
private func islandCompactStatusTitle(recordingStatus: Int) -> String {
  statusTitle(recordingStatus: recordingStatus)
}

@available(iOS 16.1, *)
private func statusAccent(recordingStatus: Int) -> Color {
  recordingStatus == 2 ? MemoLiveTheme.pausedDot : MemoLiveTheme.lime
}

@available(iOS 16.1, *)
private func locationStateText(recordingStatus: Int, hasFix: Bool) -> String {
  if recordingStatus == 2 {
    return String(localized: "live_activity.location_paused", bundle: .main)
  }
  if hasFix {
    return String(localized: "live_activity.location_title", bundle: .main)
  }
  return String(localized: "live_activity.no_gps_fix", bundle: .main)
}

@available(iOS 16.1, *)
private func locationStateIcon(recordingStatus: Int, hasFix: Bool) -> String {
  if recordingStatus == 2 {
    return "pause.circle"
  }
  return hasFix ? "location.viewfinder" : "location.slash"
}

@available(iOS 16.1, *)
private func locationStateColor(recordingStatus: Int, hasFix: Bool) -> Color {
  if recordingStatus == 2 {
    return MemoLiveTheme.pausedDot
  }
  return hasFix ? MemoLiveTheme.blue : MemoLiveTheme.textMuted
}

@available(iOS 16.1, *)
private func gpsSignalTimeText(epochMs: Int) -> String {
  let date = Date(timeIntervalSince1970: Double(epochMs) / 1000.0)
  let time = DateFormatter.localizedString(from: date, dateStyle: .none, timeStyle: .medium)
  return String(format: String(localized: "live_activity.gps_signal_time_format", bundle: .main), time)
}

@available(iOS 16.1, *)
private struct BrandLabel: View {
  let appName: String
  var compact: Bool = false

  var body: some View {
    Text(appName)
      .font(compact ? .caption.weight(.semibold) : .subheadline.weight(.semibold))
      .foregroundStyle(MemoLiveTheme.textPrimary)
      .lineLimit(1)
  }
}

@available(iOS 16.1, *)
private struct StatusBadge: View {
  let recordingStatus: Int

  var body: some View {
    HStack(spacing: 6) {
      Circle()
        .fill(statusAccent(recordingStatus: recordingStatus))
        .frame(width: 8, height: 8)
      Text(statusTitle(recordingStatus: recordingStatus))
        .font(.subheadline.weight(.semibold))
        .foregroundStyle(MemoLiveTheme.textPrimary)
    }
    .padding(.horizontal, 10)
    .padding(.vertical, 5)
    .background(MemoLiveTheme.surfaceElevated)
    .clipShape(Capsule())
    .overlay(
      Capsule()
        .stroke(MemoLiveTheme.border, lineWidth: 1)
    )
  }
}

@available(iOS 16.1, *)
private struct IslandStatusGlyph: View {
  let recordingStatus: Int

  var body: some View {
    let accent = statusAccent(recordingStatus: recordingStatus)
    ZStack {
      if recordingStatus == 2 {
        Image(systemName: locationStateIcon(recordingStatus: recordingStatus, hasFix: false))
          .font(.system(size: 17, weight: .semibold))
          .symbolRenderingMode(.hierarchical)
          .foregroundStyle(accent)
      } else {
        Circle()
          .stroke(accent.opacity(0.95), lineWidth: 1.5)
          .frame(width: 18, height: 18)
        Circle()
          .fill(accent)
          .frame(width: 6, height: 6)
        Circle()
          .stroke(accent.opacity(0.24), lineWidth: 3)
          .frame(width: 11, height: 11)
      }
    }
    .frame(width: 20, height: 20)
    .accessibilityHidden(true)
  }
}

@available(iOS 16.1, *)
private struct IslandStatusCapsule: View {
  let recordingStatus: Int

  var body: some View {
    let accent = statusAccent(recordingStatus: recordingStatus)
    HStack(spacing: 4) {
      Circle()
        .fill(accent)
        .frame(width: 5, height: 5)
      Text(islandCompactStatusTitle(recordingStatus: recordingStatus))
        .font(.system(size: 11, weight: .semibold))
        .foregroundStyle(accent)
        .lineLimit(1)
        .minimumScaleFactor(0.72)
    }
    .padding(.horizontal, 7)
    .padding(.vertical, 4)
    .background(accent.opacity(0.14))
    .clipShape(Capsule())
  }
}

@available(iOS 16.1, *)
private struct CoordinateRow: View {
  let label: String
  let value: Double
  var compact: Bool = false

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 10) {
      Text(label)
        .font(compact ? .caption2.weight(.medium) : .caption.weight(.medium))
        .foregroundStyle(MemoLiveTheme.textSecondary)
        .frame(width: compact ? 28 : 36, alignment: .leading)
      Text(String(format: "%.6f", value))
        .font((compact ? Font.callout : Font.title3).weight(.semibold).monospacedDigit())
        .foregroundStyle(MemoLiveTheme.textPrimary)
        .lineLimit(1)
        .minimumScaleFactor(0.82)
    }
  }
}

@available(iOS 16.1, *)
private struct MetadataItem: View {
  let systemImage: String
  let text: String
  var compact: Bool = false

  var body: some View {
    HStack(spacing: 6) {
      Image(systemName: systemImage)
        .font(.system(size: compact ? 11 : 12, weight: .semibold))
      Text(text)
        .font(compact ? .caption2.weight(.medium) : .caption.weight(.medium))
        .lineLimit(1)
        .minimumScaleFactor(0.82)
    }
    .foregroundStyle(MemoLiveTheme.textSecondary)
  }
}

@available(iOS 16.1, *)
private struct LocationMetadataRow: View {
  let accuracy: Double?
  let gpsTimestampMs: Int?
  var compact: Bool = false

  var body: some View {
    ViewThatFits(in: .horizontal) {
      HStack(spacing: compact ? 10 : 14) {
        metadataItems
      }
      VStack(alignment: .leading, spacing: 4) {
        metadataItems
      }
    }
  }

  @ViewBuilder
  private var metadataItems: some View {
    if let acc = accuracy {
      MetadataItem(
        systemImage: "scope",
        text: String(format: String(localized: "live_activity.accuracy_format", bundle: .main), acc),
        compact: compact
      )
    }
    if let epochMs = gpsTimestampMs {
      MetadataItem(
        systemImage: "clock",
        text: gpsSignalTimeText(epochMs: epochMs),
        compact: compact
      )
    }
  }
}

/// Shared UI for Live Activity + Xcode SwiftUI previews (no `ActivityViewContext` required).
@available(iOS 16.1, *)
struct RecordingLiveActivityPanel: View {
  let appName: String
  let recordingStatus: Int
  let latitude: Double?
  let longitude: Double?
  let accuracy: Double?
  let gpsTimestampMs: Int?
  let hasFix: Bool
  var compact: Bool = false

  var body: some View {
    let hPad: CGFloat = compact ? 12 : 16
    let vPad: CGFloat = compact ? 11 : 15

    VStack(alignment: .leading, spacing: compact ? 9 : 13) {
      HStack(alignment: .center, spacing: 10) {
        BrandLabel(appName: appName, compact: compact)
        StatusBadge(recordingStatus: recordingStatus)
        Spacer(minLength: 8)
        Image(systemName: locationStateIcon(recordingStatus: recordingStatus, hasFix: hasFix))
          .font(.system(size: compact ? 16 : 18, weight: .semibold))
          .symbolRenderingMode(.hierarchical)
          .foregroundStyle(locationStateColor(recordingStatus: recordingStatus, hasFix: hasFix))
      }

      VStack(alignment: .leading, spacing: compact ? 5 : 7) {
        Text(locationStateText(recordingStatus: recordingStatus, hasFix: hasFix))
          .font(compact ? .caption.weight(.semibold) : .subheadline.weight(.semibold))
          .foregroundStyle(MemoLiveTheme.textSecondary)

        if hasFix, let lat = latitude, let lng = longitude {
          CoordinateRow(
            label: String(localized: "live_activity.latitude_label", bundle: .main),
            value: lat,
            compact: compact
          )
          CoordinateRow(
            label: String(localized: "live_activity.longitude_label", bundle: .main),
            value: lng,
            compact: compact
          )

          LocationMetadataRow(
            accuracy: accuracy,
            gpsTimestampMs: gpsTimestampMs,
            compact: compact
          )
        } else if recordingStatus != 2 {
          Text(
            String(localized: "live_activity.no_gps_fix_detail", bundle: .main)
          )
            .font(compact ? .callout.weight(.medium) : .body.weight(.medium))
            .foregroundStyle(MemoLiveTheme.textMuted)
            .padding(.top, 1)
        }
      }
    }
    .padding(.horizontal, hPad)
    .padding(.vertical, vPad)
    .background(
      RoundedRectangle(cornerRadius: compact ? 14 : 18, style: .continuous)
        .fill(
          LinearGradient(
            colors: [MemoLiveTheme.surface.opacity(0.92), MemoLiveTheme.surfaceElevated.opacity(0.95)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
          )
        )
        .overlay(
          RoundedRectangle(cornerRadius: compact ? 14 : 18, style: .continuous)
            .stroke(MemoLiveTheme.border, lineWidth: 1)
        )
    )
  }
}

@available(iOS 16.1, *)
private struct RecordingLiveActivityView: View {
  let context: ActivityViewContext<LiveActivitiesAppAttributes>
  /// Slightly tighter layout when embedded in Dynamic Island expanded region.
  var compact: Bool = false

  var body: some View {
    let defaults = UserDefaults(suiteName: context.state.appGroupId)!
    let attrs = context.attributes
    return RecordingLiveActivityPanel(
      appName: appDisplayName(),
      recordingStatus: readInt(defaults, attributes: attrs, key: "recordingStatus"),
      latitude: readOptionalDouble(defaults, attributes: attrs, key: "latitude"),
      longitude: readOptionalDouble(defaults, attributes: attrs, key: "longitude"),
      accuracy: readOptionalDouble(defaults, attributes: attrs, key: "accuracyM"),
      gpsTimestampMs: readOptionalInt(defaults, attributes: attrs, key: "gpsTimestampMs"),
      hasFix: readBool(defaults, attributes: attrs, key: "hasGpsFix"),
      compact: compact
    )
  }
}

@available(iOS 16.1, *)
struct RecordingLiveActivityWidget: Widget {
  var body: some WidgetConfiguration {
    ActivityConfiguration(for: LiveActivitiesAppAttributes.self) { context in
      RecordingLiveActivityView(context: context, compact: false)
        .activityBackgroundTint(MemoLiveTheme.surface.opacity(0.35))
    } dynamicIsland: { context in
      let defaults = UserDefaults(suiteName: context.state.appGroupId)!
      let attrs = context.attributes
      let recordingStatus = readInt(defaults, attributes: attrs, key: "recordingStatus")

      return DynamicIsland {
        DynamicIslandExpandedRegion(.bottom) {
          RecordingLiveActivityView(context: context, compact: true)
            .padding(.horizontal, 4)
            .padding(.bottom, 4)
        }
      } compactLeading: {
        IslandStatusGlyph(recordingStatus: recordingStatus)
      } compactTrailing: {
        IslandStatusCapsule(recordingStatus: recordingStatus)
      } minimal: {
        IslandStatusGlyph(recordingStatus: recordingStatus)
      }
    }
  }
}

@available(iOS 16.1, *)
@main
struct RecordingLiveActivityBundle: WidgetBundle {
  var body: some Widget {
    RecordingLiveActivityWidget()
  }
}
