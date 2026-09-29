import UIKit
import Flutter
import notification_when_app_is_killed
import share_handler_ios

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
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
    let sceneBridgeKey = "ShareHandlerIosSceneBridge"
    // The storyboard may reuse the launch engine or create a new one. Check
    // its registry rather than keeping a process-wide registration flag.
    guard !registry.hasPlugin(sceneBridgeKey) else { return }

    GeneratedPluginRegistrant.register(with: registry)
    if let registrar = registry.registrar(forPlugin: sceneBridgeKey) {
      registrar.addSceneDelegate(SwiftShareHandlerIosPlatform.instance)
    }
  }

  override func applicationWillTerminate(_ application: UIApplication) {
    let notificationWhenAppIsKilledInstance = NotificationWhenAppIsKilledPlugin.instance
    notificationWhenAppIsKilledInstance.applicationWillTerminate();
  }
}
