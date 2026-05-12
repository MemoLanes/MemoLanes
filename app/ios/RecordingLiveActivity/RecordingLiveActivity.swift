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

let kRecordingLiveActivityAppGroup = "group.com.xingxifeng.oss.dev"

// Align with Flutter app theme (dark + lime accent). Internal so preview-only files can reuse colors.
enum MemoLiveTheme {
  static let lime = Color(red: 0.71, green: 0.93, blue: 0.32)
  static let limeDark = Color(red: 0.55, green: 0.76, blue: 0.18)
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
  let i = readInt(defaults, attributes: attributes, key: key, defaultValue: -1)
  return i >= 0 ? i : nil
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
private struct StatusBadge: View {
  let recordingStatus: Int

  var body: some View {
    HStack(spacing: 6) {
      Circle()
        .fill(recordingStatus == 2 ? MemoLiveTheme.pausedDot : MemoLiveTheme.recordingDot)
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

/// Shared UI for Live Activity + Xcode SwiftUI previews (no `ActivityViewContext` required).
@available(iOS 16.1, *)
struct RecordingLiveActivityPanel: View {
  let recordingStatus: Int
  let startedMs: Int?
  let accuracy: Double?
  let hasFix: Bool
  var compact: Bool = false
  /// When `true`, skip `TimelineView` (static elapsed). Use `true` in Xcode Previews to avoid Canvas infinite refresh.
  var freezeElapsedForPreview: Bool = false

  private let mapUrl = URL(string: "memolaneslive://action?op=openMap")!
  private let pauseUrl = URL(string: "memolaneslive://action?op=pause")!
  private let resumeUrl = URL(string: "memolaneslive://action?op=resume")!
  private let endUrl = URL(string: "memolaneslive://action?op=end")!

  var body: some View {
    let hPad: CGFloat = compact ? 10 : 14
    let vPad: CGFloat = compact ? 10 : 14

    VStack(alignment: .leading, spacing: compact ? 8 : 12) {
      Link(destination: mapUrl) {
        VStack(alignment: .leading, spacing: 6) {
          HStack(alignment: .center, spacing: 10) {
            StatusBadge(recordingStatus: recordingStatus)
            Spacer(minLength: 8)
            if let ms = startedMs {
              // TimelineView + Previews often causes Xcode Canvas to load forever; freeze for explicit preview or Xcode env.
              if freezeElapsedForPreview
                || ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] == "1"
              {
                elapsedView(epochMs: ms)
              } else {
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                  elapsedView(epochMs: ms)
                }
              }
            }
          }

          if hasFix, let acc = accuracy {
            Text(String(format: String(localized: "live_activity.accuracy_format", bundle: .main), acc))
              .font(compact ? .caption2 : .caption)
              .foregroundStyle(MemoLiveTheme.textSecondary)
          } else {
            Text(String(localized: "live_activity.no_gps_fix", bundle: .main))
              .font(compact ? .caption2 : .caption)
              .foregroundStyle(MemoLiveTheme.textMuted)
          }
        }
      }

      HStack(spacing: 10) {
        if recordingStatus == 1 {
          Link(destination: pauseUrl) {
            Label(String(localized: "live_activity.pause", bundle: .main), systemImage: "pause.fill")
              .font(.subheadline.weight(.semibold))
              .labelStyle(.titleAndIcon)
              .foregroundStyle(MemoLiveTheme.lime)
              .frame(maxWidth: .infinity)
              .padding(.vertical, 11)
              .background(MemoLiveTheme.surfaceElevated)
              .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
              .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                  .stroke(MemoLiveTheme.border, lineWidth: 1)
              )
          }
        } else if recordingStatus == 2 {
          Link(destination: resumeUrl) {
            Label(String(localized: "live_activity.resume", bundle: .main), systemImage: "play.fill")
              .font(.subheadline.weight(.semibold))
              .labelStyle(.titleAndIcon)
              .foregroundStyle(Color.black.opacity(0.88))
              .frame(maxWidth: .infinity)
              .padding(.vertical, 11)
              .background(
                LinearGradient(
                  colors: [MemoLiveTheme.lime, MemoLiveTheme.limeDark],
                  startPoint: .topLeading,
                  endPoint: .bottomTrailing
                )
              )
              .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
          }
        }

        Link(destination: endUrl) {
          Image(systemName: "stop.circle.fill")
            .font(.system(size: compact ? 34 : 38))
            .symbolRenderingMode(.palette)
            .foregroundStyle(MemoLiveTheme.textPrimary, MemoLiveTheme.recordingDot.opacity(0.92))
            .frame(width: compact ? 48 : 52, height: compact ? 44 : 48)
            .background(MemoLiveTheme.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(
              RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(MemoLiveTheme.border, lineWidth: 1)
            )
        }
        .accessibilityLabel(Text(String(localized: "live_activity.end_a11y", bundle: .main)))
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

  @ViewBuilder
  private func elapsedView(epochMs: Int) -> some View {
    let start = Date(timeIntervalSince1970: Double(epochMs) / 1000.0)
    let sec = max(0, Int(Date().timeIntervalSince(start)))
    Text(
      String(
        format: String(localized: "live_activity.elapsed_format", bundle: .main),
        sec / 60, sec % 60))
      .font(.title3.weight(.medium).monospacedDigit())
      .foregroundStyle(MemoLiveTheme.lime)
  }
}

@available(iOS 16.1, *)
private struct RecordingLiveActivityView: View {
  let context: ActivityViewContext<LiveActivitiesAppAttributes>
  /// Slightly tighter layout when embedded in Dynamic Island expanded region.
  var compact: Bool = false

  var body: some View {
    let defaults = UserDefaults(suiteName: kRecordingLiveActivityAppGroup)!
    let attrs = context.attributes
    return RecordingLiveActivityPanel(
      recordingStatus: readInt(defaults, attributes: attrs, key: "recordingStatus"),
      startedMs: readOptionalInt(defaults, attributes: attrs, key: "startedAtEpochMs"),
      accuracy: readOptionalDouble(defaults, attributes: attrs, key: "accuracyM"),
      hasFix: readBool(defaults, attributes: attrs, key: "hasGpsFix"),
      compact: compact,
      freezeElapsedForPreview: false
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
      let defaults = UserDefaults(suiteName: kRecordingLiveActivityAppGroup)!
      let attrs = context.attributes
      let recordingStatus = readInt(defaults, attributes: attrs, key: "recordingStatus")
      let mapUrl = URL(string: "memolaneslive://action?op=openMap")!
      let pauseUrl = URL(string: "memolaneslive://action?op=pause")!
      let resumeUrl = URL(string: "memolaneslive://action?op=resume")!

      return DynamicIsland {
        DynamicIslandExpandedRegion(.bottom) {
          RecordingLiveActivityView(context: context, compact: true)
            .padding(.horizontal, 4)
            .padding(.bottom, 4)
        }
      } compactLeading: {
        Link(destination: mapUrl) {
          ZStack {
            Circle()
              .fill(MemoLiveTheme.surfaceElevated)
              .frame(width: 26, height: 26)
            Image(systemName: "map.fill")
              .font(.system(size: 13, weight: .semibold))
              .foregroundStyle(MemoLiveTheme.textSecondary)
          }
        }
      } compactTrailing: {
        if recordingStatus == 1 {
          Link(destination: pauseUrl) {
            ZStack {
              Circle()
                .fill(MemoLiveTheme.recordingDot.opacity(0.22))
                .frame(width: 26, height: 26)
              Image(systemName: "pause.fill")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(MemoLiveTheme.recordingDot)
            }
          }
        } else if recordingStatus == 2 {
          Link(destination: resumeUrl) {
            ZStack {
              Circle()
                .fill(MemoLiveTheme.lime.opacity(0.22))
                .frame(width: 26, height: 26)
              Image(systemName: "play.fill")
                .font(.system(size: 11, weight: .bold))
                .foregroundStyle(MemoLiveTheme.lime)
            }
          }
        } else {
          Link(destination: mapUrl) {
            Image(systemName: "ellipsis.circle.fill")
              .font(.system(size: 22))
              .foregroundStyle(MemoLiveTheme.textSecondary)
          }
        }
      } minimal: {
        Link(destination: mapUrl) {
          Circle()
            .fill(recordingStatus == 2 ? MemoLiveTheme.pausedDot : MemoLiveTheme.recordingDot)
            .frame(width: 10, height: 10)
        }
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

