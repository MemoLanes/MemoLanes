import 'dart:io';

import 'package:flutter_inappwebview/flutter_inappwebview.dart';

/// Loads bundled map assets without opening a localhost listening socket.
///
/// Android needs WebViewAssetLoader's virtual HTTPS origin for MapLibre module
/// workers, while WKWebView can load the same bundle directly from Flutter
/// assets. The platform difference stays isolated in this adapter.
class MapWebViewAssets {
  MapWebViewAssets._();

  static const _assetRoot = 'assets/map_webview';
  static const _androidAssetDomain = 'appassets.androidplatform.net';
  static const _androidAssetBaseUrl =
      'https://$_androidAssetDomain/assets/flutter_assets/$_assetRoot/';

  static WebViewAssetLoader? createAssetLoader() {
    if (!Platform.isAndroid) return null;
    return WebViewAssetLoader(
      domain: _androidAssetDomain,
      pathHandlers: [AssetsPathHandler(path: '/assets/')],
    );
  }

  static Future<void> load(
    InAppWebViewController controller,
    String fileName,
  ) async {
    if (Platform.isAndroid) {
      await controller.loadUrl(
        urlRequest: URLRequest(url: WebUri('$_androidAssetBaseUrl$fileName')),
      );
      return;
    }

    await controller.loadFile(assetFilePath: '$_assetRoot/$fileName');
  }

  static bool owns(WebUri url) => Platform.isAndroid
      ? url.toString().startsWith(_androidAssetBaseUrl)
      : url.scheme == 'file';
}
