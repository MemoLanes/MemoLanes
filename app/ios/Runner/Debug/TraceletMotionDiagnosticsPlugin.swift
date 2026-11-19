import CoreLocation
import Flutter
import Foundation
import TraceletSDK

final class TraceletMotionDiagnosticsPlugin: NSObject, FlutterPlugin {
  private static let pluginKey = "MemoLanesTraceletMotionDiagnostics"

  static func register(with registry: FlutterPluginRegistry) {
    guard !registry.hasPlugin(pluginKey) else { return }
    guard let registrar = registry.registrar(forPlugin: pluginKey) else {
      NSLog("[TraceletMotionDiagnostics] Registration failed: no registrar for %@", pluginKey)
      return
    }

    register(with: registrar)
  }

  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.memolanes/tracelet_motion_diagnostics",
      binaryMessenger: registrar.messenger()
    )
    registrar.addMethodCallDelegate(TraceletMotionDiagnosticsPlugin(), channel: channel)
  }

  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == "snapshot" else {
      result(FlutterMethodNotImplemented)
      return
    }
    let sdk = TraceletSdk.shared
    guard sdk.isReadyState, let coordinator = sdk.smartMotionCoordinator else {
      result(nil)
      return
    }
    var details: [String: Any] = [
      "accelerometerMoving": coordinator.isAccelMoving,
      "speedMoving": coordinator.isSpeedMoving,
    ]
    if let motion = sdk.locationEngine?.getMotionSpeedDiagnostics() {
      details.merge(motion) { _, new in new }
    }
    if let state = sdk.speedMotionManager?.state {
      details["speedState"] = String(describing: state)
    }
    if let fixAtMs = (call.arguments as? [String: Any])?["fixAtMs"] as? NSNumber {
      let fix = sdk.locationEngine?.getLastLocation()
      // Tracelet serializes the timestamp with millisecond precision.
      let matched = fix.map {
        abs($0.timestamp.timeIntervalSince1970 * 1000 - fixAtMs.doubleValue) < 1
      } ?? false
      details["rawFixMatched"] = matched
      if matched, let fix, fix.speed.isFinite, fix.speed >= 0 {
        details["rawSpeedMps"] = fix.speed
      } else {
        details["rawSpeedMps"] = NSNull()
      }
    }
    result(details)
  }
}
