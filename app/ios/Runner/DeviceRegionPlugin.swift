import Foundation
import Flutter

final class DeviceRegionPlugin: NSObject, FlutterPlugin {
  private static let pluginKey = "MemoLanesDeviceRegion"

  static func register(with registry: FlutterPluginRegistry) {
    // Each engine needs its own channel, including scene-free launches.
    guard !registry.hasPlugin(pluginKey),
      let registrar = registry.registrar(forPlugin: pluginKey)
    else { return }

    register(with: registrar)
  }

  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.memolanes/device_region",
      binaryMessenger: registrar.messenger()
    )
    registrar.addMethodCallDelegate(DeviceRegionPlugin(), channel: channel)
  }

  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == "getRegion" else {
      result(FlutterMethodNotImplemented)
      return
    }
    result(Locale.current.region?.identifier)
  }
}
