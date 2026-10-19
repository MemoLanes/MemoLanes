import Flutter
import Photos

final class ScreenshotPlugin: NSObject, FlutterPlugin {
  private static let pluginKey = "MemoLanesScreenshots"

  static func register(with registry: FlutterPluginRegistry) {
    guard !registry.hasPlugin(pluginKey),
      let registrar = registry.registrar(forPlugin: pluginKey)
    else { return }
    register(with: registrar)
  }

  static func register(with registrar: FlutterPluginRegistrar) {
    let channel = FlutterMethodChannel(
      name: "com.memolanes/screenshots",
      binaryMessenger: registrar.messenger()
    )
    registrar.addMethodCallDelegate(ScreenshotPlugin(), channel: channel)
  }

  func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == "saveToPhotos" else {
      result(FlutterMethodNotImplemented)
      return
    }
    guard let arguments = call.arguments as? [String: Any],
      let png = arguments["png"] as? FlutterStandardTypedData,
      !png.data.isEmpty
    else {
      result(FlutterError(code: "invalid_image", message: "Missing PNG data", details: nil))
      return
    }

    // Add-only permission never grants this feature access to existing photos.
    PHPhotoLibrary.requestAuthorization(for: .addOnly) { status in
      guard status == .authorized || status == .limited else {
        DispatchQueue.main.async {
          result(FlutterError(code: "permission_denied", message: "Photo permission denied", details: nil))
        }
        return
      }
      PHPhotoLibrary.shared().performChanges({
        let request = PHAssetCreationRequest.forAsset()
        let options = PHAssetResourceCreationOptions()
        options.originalFilename = "MemoLanes_\(UUID().uuidString).png"
        request.addResource(with: .photo, data: png.data, options: options)
      }) { success, error in
        DispatchQueue.main.async {
          if success {
            result(nil)
          } else {
            result(FlutterError(code: "save_failed", message: error?.localizedDescription, details: nil))
          }
        }
      }
    }
  }
}
