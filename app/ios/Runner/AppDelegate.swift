import UIKit
import Flutter
import notification_when_app_is_killed
import AppIntents
import intelligence

@main
@objc class AppDelegate: FlutterAppDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    if #available(iOS 10.0, *) {
      UNUserNotificationCenter.current().delegate = self as? UNUserNotificationCenterDelegate
    }

    GeneratedPluginRegistrant.register(with: self)
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  override func applicationWillTerminate(_ application: UIApplication) {
    let notificationWhenAppIsKilledInstance = NotificationWhenAppIsKilledPlugin.instance
    notificationWhenAppIsKilledInstance.applicationWillTerminate();
  }
}

@available(iOS 16.0, *)
struct StartRecordingIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_start_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_start_desc", table: "Localizable")
    )

    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        IntelligencePlugin.notifier.push("start_recording")
        return .result()
    }
}

@available(iOS 16.0, *)
struct StopRecordingIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_stop_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_stop_desc", table: "Localizable")
    )

    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        IntelligencePlugin.notifier.push("stop_recording")
        return .result()
    }
}

@available(iOS 16.0, *)
struct PauseRecordingIntent: AppIntent {
    static let title: LocalizedStringResource = LocalizedStringResource(
        "memolanes_intent_pause_title",
        table: "Localizable"
    )

    static let description: IntentDescription = IntentDescription(
        LocalizedStringResource("memolanes_intent_pause_desc", table: "Localizable")
    )

    static var openAppWhenRun: Bool = true

    @MainActor
    func perform() async throws -> some IntentResult {
        IntelligencePlugin.notifier.push("pause_recording")
        return .result()
    }
}

@available(iOS 16.0, *)
struct AppShortcuts: AppShortcutsProvider {
    @AppShortcutsBuilder
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: StartRecordingIntent(),
            phrases: [
                "Start recording in \(.applicationName)",
                "Begin recording in \(.applicationName)",
                "在 \(.applicationName) 中开始记录",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_start_title", table: "Localizable"),
            systemImageName: "record.circle"
        )
        AppShortcut(
            intent: StopRecordingIntent(),
            phrases: [
                "Stop recording in \(.applicationName)",
                "End recording in \(.applicationName)",
                "在 \(.applicationName) 中停止记录",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_stop_title", table: "Localizable"),
            systemImageName: "stop.circle"
        )
        AppShortcut(
            intent: PauseRecordingIntent(),
            phrases: [
                "Pause recording in \(.applicationName)",
                "在 \(.applicationName) 中暂停记录",
            ],
            shortTitle: LocalizedStringResource("memolanes_intent_pause_title", table: "Localizable"),
            systemImageName: "pause.circle"
        )
    }
}
