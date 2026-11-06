import ActivityKit
import Flutter
import UIKit

/// Dart serializes commands. Native observers only preserve dismissal evidence;
/// they never request an activity or change recording business state.
@available(iOS 16.1, *)
@MainActor
final class RecordingLiveActivityBridge {
    private let defaults = UserDefaults.standard
    private let key = "recordingLiveActivity.session"
    private var observers: [String: Task<Void, Never>] = [:]
    private var authorizationObserver: Task<Void, Never>?
    private var session: [String: Any]
    private var lastState: RecordingActivityAttributes.ContentState?
    private var lastUpdate: Date?

    init(messenger: FlutterBinaryMessenger) {
        session = defaults.dictionary(forKey: key) ?? [:]
        let channel = FlutterMethodChannel(
            name: "com.memolanes/recording_live_activity", binaryMessenger: messenger)
        channel.setMethodCallHandler { [weak self] call, result in
            guard call.method == "synchronize", let payload = call.arguments as? [String: Any] else {
                result(FlutterMethodNotImplemented)
                return
            }
            guard let self else {
                result(FlutterError(code: "live_activity_unavailable", message: "Activity bridge released", details: nil))
                return
            }
            Task { @MainActor in
                do {
                    try await self.synchronize(payload)
                    result(nil)
                } catch {
                    result(FlutterError(code: "live_activity_sync", message: String(describing: error), details: nil))
                }
            }
        }
        for activity in Activity<RecordingActivityAttributes>.activities { observe(activity) }
        authorizationObserver = Task { @MainActor [weak self] in
            for await enabled in ActivityAuthorizationInfo().activityEnablementUpdates {
                guard let self else { return }
                // Revoking authorization isn't dismissing one recording card.
                if !enabled { self.session["authorizationRevoked"] = true }
                self.persist()
            }
        }
    }

    private func persist() { defaults.set(session, forKey: key) }

    private func observe(_ activity: Activity<RecordingActivityAttributes>) {
        guard observers[activity.id] == nil else { return }
        observers[activity.id] = Task { @MainActor [weak self] in
            for await state in activity.activityStateUpdates {
                guard let self else { return }
                self.recordEnd(activity, state: state)
                if state == .dismissed { break }
            }
            self?.observers.removeValue(forKey: activity.id)
        }
    }

    private func recordEnd(_ activity: Activity<RecordingActivityAttributes>, state: ActivityState) {
        guard session["activityID"] as? String == activity.id,
              state == .ended || state == .dismissed else { return }
        if !ActivityAuthorizationInfo().areActivitiesEnabled {
            session["authorizationRevoked"] = true
        }
        if session["authorizationRevoked"] as? Bool != true {
            // ActivityKit doesn't identify whether person, app or system ended
            // it. Honor a possible dismissal until recording explicitly stops.
            session["suppressed"] = true
        }
        persist()
    }

    private func end(_ activity: Activity<RecordingActivityAttributes>) async {
        if #available(iOS 16.2, *) {
            await activity.end(nil, dismissalPolicy: .immediate)
        } else {
            await activity.end(using: nil, dismissalPolicy: .immediate)
        }
        observers.removeValue(forKey: activity.id)?.cancel()
    }

    private func synchronize(_ payload: [String: Any]) async throws {
        guard let status = payload["status"] as? String,
              ["none", "recording", "paused"].contains(status) else {
            throw BridgeError.invalidPayload
        }
        if status == "none" {
            // Clear first: observers can't mistake our own end for dismissal.
            session = [:]
            persist()
            lastState = nil
            lastUpdate = nil
            for activity in Activity<RecordingActivityAttributes>.activities { await end(activity) }
            return
        }
        var existing: Activity<RecordingActivityAttributes>?
        for activity in Activity<RecordingActivityAttributes>.activities {
            if activity.id == session["activityID"] as? String {
                recordEnd(activity, state: activity.activityState)
                var isLive = activity.activityState == .active
                if #available(iOS 16.2, *) { isLive = isLive || activity.activityState == .stale }
                if isLive {
                    existing = activity
                    observe(activity)
                    continue
                }
            }
            await end(activity) // orphan, duplicate or ended lock screen card
        }
        let enabled = ActivityAuthorizationInfo().areActivitiesEnabled
        if !enabled { session["authorizationRevoked"] = true }
        if existing == nil, session["activityID"] != nil,
           enabled, session["authorizationRevoked"] as? Bool != true {
            session["suppressed"] = true // disappearance alone doesn't reveal why
        }
        persist()
        guard enabled, session["suppressed"] as? Bool != true else { return }

        let now = Date()
        var precision = payload["accuracy"] as? Double
        var timestamp = (payload["gpsTimestampMs"] as? Double).map { Date(timeIntervalSince1970: $0 / 1000) }
        // A valid fix can expire while waiting in Dart's command queue.
        if status != "recording" || precision == nil || !precision!.isFinite || precision! < 0 ||
            timestamp == nil || now.timeIntervalSince(timestamp!) < 0 || now.timeIntervalSince(timestamp!) >= 12 {
            precision = nil
            timestamp = nil
        }
        let state = RecordingActivityAttributes.ContentState(status: status, accuracy: precision, gpsTimestamp: timestamp)
        let staleDate: Date? = status == "paused" ? nil : timestamp?.addingTimeInterval(12) ?? now.addingTimeInterval(60)
        if let activity = existing {
            session.removeValue(forKey: "authorizationRevoked")
            persist()
            // Reconcile even identical payloads; only recording needs renewal.
            if state != lastState || (status == "recording" &&
                (lastUpdate == nil || now.timeIntervalSince(lastUpdate!) >= 30)) {
                if #available(iOS 16.2, *) {
                    await activity.update(ActivityContent(state: state, staleDate: staleDate))
                } else {
                    await activity.update(using: state)
                }
                lastState = state
                lastUpdate = now
            }
        } else if status == "recording", UIApplication.shared.applicationState == .active {
            let activity: Activity<RecordingActivityAttributes>
            if #available(iOS 16.2, *) {
                activity = try Activity.request(attributes: RecordingActivityAttributes(),
                    content: ActivityContent(state: state, staleDate: staleDate), pushType: nil)
            } else {
                activity = try Activity.request(attributes: RecordingActivityAttributes(), contentState: state, pushType: nil)
            }
            session["activityID"] = activity.id
            session.removeValue(forKey: "authorizationRevoked")
            persist()
            lastState = state
            lastUpdate = now
            observe(activity)
        }
    }

    private enum BridgeError: Error { case invalidPayload }
}
