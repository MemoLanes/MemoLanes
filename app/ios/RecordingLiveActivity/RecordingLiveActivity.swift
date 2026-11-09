import ActivityKit
import SwiftUI
import WidgetKit

@available(iOSApplicationExtension 16.1, *)
private struct RecordingPresentation {
    let state: RecordingActivityAttributes.ContentState
    let stale: Bool

    var status: String {
        if stale { return "unknown" }
        return ["recording", "paused"].contains(state.status) ? state.status : "unknown"
    }
    var title: String {
        switch status {
        case "recording": return "status_recording"
        case "paused": return "status_paused"
        default: return "status_unknown"
        }
    }
    var symbol: String {
        switch status {
        case "recording": return "record.circle"
        case "paused": return "pause.circle.fill"
        default: return "questionmark.circle"
        }
    }
    var color: Color {
        switch status {
        case "recording": return Color(red: 0.71, green: 0.93, blue: 0.32)
        case "paused": return .yellow
        default: return .secondary
        }
    }
    var hasFix: Bool {
        guard status == "recording", let timestamp = state.gpsTimestamp,
              let accuracy = state.accuracy, accuracy.isFinite, accuracy >= 0 else { return false }
        let age = Date().timeIntervalSince(timestamp)
        return age >= 0 && age < 12
    }
}

@available(iOSApplicationExtension 16.1, *)
private func presentation(_ context: ActivityViewContext<RecordingActivityAttributes>) -> RecordingPresentation {
    if #available(iOSApplicationExtension 16.2, *) {
        return RecordingPresentation(state: context.state, stale: context.isStale)
    }
    return RecordingPresentation(state: context.state, stale: false)
}

@available(iOSApplicationExtension 16.1, *)
private struct StatusIcon: View {
    let display: RecordingPresentation
    var body: some View {
        Image(systemName: display.symbol)
            .foregroundStyle(display.color)
            .accessibilityLabel(Text(LocalizedStringKey(display.status == "unknown" ? "accessibility_unknown" : display.title)))
    }
}

@available(iOSApplicationExtension 16.1, *)
private struct StatusBadge: View {
    let display: RecordingPresentation

    var body: some View {
        HStack(spacing: 4) {
            Circle().fill(display.color).frame(width: 4, height: 4)
            Text(LocalizedStringKey(display.title))
                .font(.caption2.weight(.semibold))
                .lineLimit(1)
        }
        .foregroundStyle(display.color)
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(display.color.opacity(0.13), in: Capsule())
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(LocalizedStringKey(display.status == "unknown" ? "accessibility_unknown" : display.title)))
    }
}

/// The island supplies its own surface; both presentations share the same content.
@available(iOSApplicationExtension 16.1, *)
private struct RecordingActivityPanel: View {
    let display: RecordingPresentation
    var island = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 7) {
                Image(systemName: "point.topleft.down.to.point.bottomright.curvepath")
                    .foregroundStyle(display.color)
                    .accessibilityHidden(true)
                Text("MemoLanes")
                    .foregroundStyle(Color.white.opacity(0.85))
            }
            .font(.caption.weight(.medium))

            HStack(spacing: 14) {
                Text(LocalizedStringKey(display.title))
                    .font(.title.weight(.semibold))
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
                    .accessibilityLabel(Text(LocalizedStringKey(display.status == "unknown" ? "accessibility_unknown" : display.title)))
                Spacer(minLength: 0)
                Image(systemName: display.status == "recording" ? (display.hasFix ? "scope" : "location.viewfinder") : display.symbol)
                    .font(.system(size: 27, weight: .regular))
                    .foregroundStyle(display.color)
                    .frame(width: 46, height: 46)
                    .background(display.color.opacity(0.09), in: RoundedRectangle(cornerRadius: 16))
                    .accessibilityHidden(true)
            }

            if display.hasFix, let accuracy = display.state.accuracy, let timestamp = display.state.gpsTimestamp {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        precision(accuracy)
                        Spacer(minLength: 0)
                        updated(timestamp)
                    }
                    VStack(alignment: .leading, spacing: 5) {
                        precision(accuracy)
                        updated(timestamp)
                    }
                }
                .font(.caption2)
                .foregroundStyle(Color.white.opacity(0.67))
            } else if display.status != "paused" {
                Text(LocalizedStringKey(display.status == "recording" ? "locating" : "open_app"))
                    .font(.caption)
                    .foregroundStyle(Color.white.opacity(0.67))
            }
        }
        // Live Activities can be truncated above 160 pt. Keep both detail rows
        // visible on narrow devices instead of scaling beyond this surface.
        .dynamicTypeSize(...DynamicTypeSize.xxLarge)
        .padding(island ? 4 : 16)
        .foregroundStyle(.white)
        .background {
            if !island { Color(red: 36 / 255, green: 40 / 255, blue: 41 / 255) }
        }
    }

    private func precision(_ accuracy: Double) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 4) {
            Text("accuracy_title")
            Text(String(format: String(localized: "accuracy_value"), accuracy.rounded()))
                .fontWeight(.medium)
                .foregroundStyle(.white)
                .monospacedDigit()
        }
        .fixedSize(horizontal: true, vertical: false)
        .accessibilityElement(children: .combine)
    }

    private func updated(_ timestamp: Date) -> some View {
        Text("gps_updated_at \(timestamp, format: .dateTime.hour().minute().second())")
            .monospacedDigit()
            .fixedSize(horizontal: true, vertical: false)
            .accessibilityLabel(Text("gps_updated") + Text(" ") + Text(timestamp, format: .dateTime.hour().minute().second()))
    }
}

@available(iOSApplicationExtension 16.1, *)
struct RecordingLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RecordingActivityAttributes.self) { context in
            RecordingActivityPanel(display: presentation(context))
                .activityBackgroundTint(Color(red: 36 / 255, green: 40 / 255, blue: 41 / 255))
                .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            let display = presentation(context)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.bottom) {
                    RecordingActivityPanel(display: display, island: true)
                }
            } compactLeading: {
                StatusIcon(display: display)
            } compactTrailing: {
                StatusBadge(display: display)
            } minimal: {
                StatusIcon(display: display)
            }
            .keylineTint(display.color)
        }
    }
}

@available(iOSApplicationExtension 16.1, *)
private struct RecordingLiveActivityPreviews: PreviewProvider {
    static var previews: some View {
        Group {
            panel(status: "recording", accuracy: 8, timestamp: Date()).previewDisplayName("Recording · GPS")
            panel(status: "recording").previewDisplayName("Recording · Locating")
            panel(status: "paused").previewDisplayName("Paused")
            panel(status: "recording", accuracy: 8, timestamp: Date(), stale: true).previewDisplayName("Stale · Check status")
            panel(status: "recording", accuracy: 8, timestamp: Date(), island: true)
                .previewDisplayName("Expanded island")
            panel(status: "recording", accuracy: 8, timestamp: Date())
                .environment(\.locale, Locale(identifier: "zh-Hans"))
                .environment(\.dynamicTypeSize, .accessibility1)
                .previewDisplayName("大字号")
        }
        .previewLayout(.sizeThatFits)
        .preferredColorScheme(.dark)
    }

    private static func panel(status: String, accuracy: Double? = nil, timestamp: Date? = nil, island: Bool = false, stale: Bool = false) -> some View {
        RecordingActivityPanel(
            display: RecordingPresentation(
                state: .init(status: status, accuracy: accuracy, gpsTimestamp: timestamp),
                stale: stale
            ),
            island: island
        )
        .frame(width: 360)
        .background(.black)
    }
}

@main
@available(iOSApplicationExtension 16.1, *)
struct RecordingLiveActivityBundle: WidgetBundle {
    var body: some Widget { RecordingLiveActivity() }
}
