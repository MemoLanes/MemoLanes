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
  static var title: LocalizedStringResource = "开始记录"
  static var openAppWhenRun: Bool = false

  @MainActor
  func perform() async throws -> some IntentResult {
    IntelligencePlugin.notifier.push("start_recording")
    return .result()
  }
}

@available(iOS 16.0, *)
struct StopRecordingIntent: AppIntent {
  static var title: LocalizedStringResource = "停止记录"
  static var openAppWhenRun: Bool = false

  @MainActor
  func perform() async throws -> some IntentResult {
    IntelligencePlugin.notifier.push("stop_recording")
    return .result()
  }
}

@available(iOS 16.0, *)
struct MemoLanesAppShortcuts: AppShortcutsProvider {
  static var appShortcuts: [AppShortcut] {
    AppShortcut(
      intent: StartRecordingIntent(),
      phrases: [
        "开始记录 \(.applicationName)",
        "开始轨迹记录 \(.applicationName)",
        "开始行程记录 \(.applicationName)",
        "在 \(.applicationName) 开始记录",
        "Start recording in \(.applicationName)",
        "Start trip recording in \(.applicationName)"
      ],
      shortTitle: "开始记录"
    )
    AppShortcut(
      intent: StopRecordingIntent(),
      phrases: [
        "停止记录 \(.applicationName)",
        "停止轨迹记录 \(.applicationName)",
        "结束行程记录 \(.applicationName)",
        "在 \(.applicationName) 停止记录",
        "Stop recording in \(.applicationName)",
        "Stop trip recording in \(.applicationName)"
      ],
      shortTitle: "停止记录"
    )
  }
}
