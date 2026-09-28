import Flutter
import UIKit
import share_handler_ios

class SceneDelegate: FlutterSceneDelegate {}

// TODO: Remove this bridge when share_handler supports UIScene upstream.
extension SwiftShareHandlerIosPlatform: @retroactive FlutterSceneLifeCycleDelegate {
  public func scene(
    _ scene: UIScene,
    willConnectTo session: UISceneSession,
    options connectionOptions: UIScene.ConnectionOptions?
  ) -> Bool {
    guard let connectionOptions else {
      return false
    }

    var handled = false
    for urlContext in connectionOptions.urlContexts
    where hasMatchingSchemePrefix(url: urlContext.url) {
      handled = application(
        UIApplication.shared,
        didFinishLaunchingWithOptions: [UIApplication.LaunchOptionsKey.url: urlContext.url]
      ) || handled
    }
    for userActivity in connectionOptions.userActivities
    where hasMatchingSchemePrefix(url: userActivity.webpageURL) {
      handled = application(
        UIApplication.shared,
        continue: userActivity,
        restorationHandler: { _ in }
      ) || handled
    }
    return handled
  }

  public func scene(_ scene: UIScene, openURLContexts URLContexts: Set<UIOpenURLContext>) -> Bool {
    var handled = false
    for urlContext in URLContexts where hasMatchingSchemePrefix(url: urlContext.url) {
      handled = application(UIApplication.shared, open: urlContext.url, options: [:]) || handled
    }
    return handled
  }

  public func scene(_ scene: UIScene, continue userActivity: NSUserActivity) -> Bool {
    guard hasMatchingSchemePrefix(url: userActivity.webpageURL) else {
      return false
    }
    return application(
      UIApplication.shared,
      continue: userActivity,
      restorationHandler: { _ in }
    )
  }
}
