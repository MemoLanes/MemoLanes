import AppIntents
import flutter_app_intents

@available(iOS 16.0, *)
struct StartRecordingIntentSpec: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_start_recording_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_start_recording_desc", table: "Localizable")
    )

    @MainActor
    func perform() async throws -> some IntentResult {
        let plugin = FlutterAppIntentsPlugin.shared
        let result = await plugin.handleIntentInvocation(
            identifier: "com.memolanes.StartRecordingIntent",
            parameters: [:]
        )
        try ensureIntentSuccess(result)
        return .result()
    }
}

@available(iOS 16.0, *)
struct EndJourneyIntentSpec: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_end_journey_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_end_journey_desc", table: "Localizable")
    )

    @MainActor
    func perform() async throws -> some IntentResult {
        let plugin = FlutterAppIntentsPlugin.shared
        let result = await plugin.handleIntentInvocation(
            identifier: "com.memolanes.EndJourneyIntent",
            parameters: [:]
        )
        try ensureIntentSuccess(result)
        return .result()
    }
}

@available(iOS 16.0, *)
struct PauseRecordingIntentSpec: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_pause_recording_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_pause_recording_desc", table: "Localizable")
    )

    @MainActor
    func perform() async throws -> some IntentResult {
        let plugin = FlutterAppIntentsPlugin.shared
        let result = await plugin.handleIntentInvocation(
            identifier: "com.memolanes.PauseRecordingIntent",
            parameters: [:]
        )
        try ensureIntentSuccess(result)
        return .result()
    }
}

@available(iOS 16.0, *)
private func ensureIntentSuccess(_ result: [String: Any]) throws {
    if let success = result["success"] as? Bool, success {
        return
    }
    let message = result["error"] as? String ?? "Intent execution failed"
    throw AppIntentExecutionError.executionFailed(message)
}

enum AppIntentExecutionError: Error {
    case executionFailed(String)
}

@available(iOS 16.0, *)
struct AppShortcuts: AppShortcutsProvider {
    @AppShortcutsBuilder
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: StartRecordingIntentSpec(),
            phrases: [
                "Start recording in \(.applicationName)",
                "Resume recording in \(.applicationName)",
                "在 \(.applicationName) 中开始记录",
                "在 \(.applicationName) 中恢复记录",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_start_recording_title", table: "Localizable"),
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: EndJourneyIntentSpec(),
            phrases: [
                "End journey in \(.applicationName)",
                "在 \(.applicationName) 中结束旅程",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_end_journey_title", table: "Localizable"),
            systemImageName: "stop.circle"
        )
        AppShortcut(
            intent: PauseRecordingIntentSpec(),
            phrases: [
                "Pause recording in \(.applicationName)",
                "在 \(.applicationName) 中暂停记录",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_pause_recording_title", table: "Localizable"),
            systemImageName: "pause.circle"
        )
    }
}
