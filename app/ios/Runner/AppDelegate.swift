import UIKit
import Flutter
import notification_when_app_is_killed

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  private var recordingLiveActivityBridge: AnyObject?

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    if #available(iOS 10.0, *) {
      UNUserNotificationCenter.current().delegate = self as? UNUserNotificationCenterDelegate
    }

    // TODO: This is a compatibility workaround for the current Flutter version; revisit on upgrade.
    // AppIntents can launch without a scene. Early registration uses Flutter's
    // launch-engine compatibility path; Main.storyboard later adopts that engine.
    registerPlugins(with: self)
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    registerPlugins(with: engineBridge.pluginRegistry)
  }

  private func registerPlugins(with registry: FlutterPluginRegistry) {
    // The storyboard may reuse the launch engine or create a new one. Check
    // its registry rather than keeping a process-wide registration flag.
    guard !registry.hasPlugin("ShareHandlerIosPlatform") else { return }

    GeneratedPluginRegistrant.register(with: registry)
    if let registrar = registry.registrar(forPlugin: "RecordingLiveActivityBridge") {
      if #available(iOS 16.1, *) {
        recordingLiveActivityBridge = RecordingLiveActivityBridge(messenger: registrar.messenger())
      } else {
        FlutterMethodChannel(name: "com.memolanes/recording_live_activity", binaryMessenger: registrar.messenger())
          .setMethodCallHandler { _, result in result(nil) }
      }
    }
  }

  override func applicationWillTerminate(_ application: UIApplication) {
    let notificationWhenAppIsKilledInstance = NotificationWhenAppIsKilledPlugin.instance
    notificationWhenAppIsKilledInstance.applicationWillTerminate();
  }
}
