import 'dart:io';

import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/renderer/internal_server.dart';

/// Serves bundled map assets and Rust API resources to both WebView pages.
///
/// Android needs WebViewAssetLoader's virtual HTTPS origin for MapLibre module
/// workers, while WKWebView can load the same bundle directly from Flutter
/// assets. The platform difference stays isolated in this adapter.
class MapWebViewResources {
  MapWebViewResources._();

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

  static const androidApiEndpoint = 'https://memolanes.local/api';
  static const androidApiRequestPrefix = '$androidApiEndpoint/';
  static final _apiPathPrefix = RegExp(r'^/?(api/)?');
  static String get apiEndpoint =>
      Platform.isIOS ? 'memolanes://api' : androidApiEndpoint;

  static Future<WebviewResponse> _request(
    api.MapRendererProxy proxy,
    WebUri url,
  ) => proxy.handleRequest(
    path: url.path.replaceFirst(_apiPathPrefix, ''),
    queryParams: url.queryParameters,
  );

  static Map<String, String> _headers(WebviewResponse response) => {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Expose-Headers': 'X-Tile-Version, X-Not-Modified',
    'Content-Type': response.contentType,
    ...response.headers,
  };

  static Future<CustomSchemeResponse> loadApiResource(
    api.MapRendererProxy proxy,
    WebUri url,
  ) async {
    final response = await _request(proxy, url);
    return CustomSchemeResponse(
      data: response.body,
      contentType: response.contentType,
      contentEncoding: 'utf-8',
      statusCode: response.status,
      headers: _headers(response),
    );
  }

  static Future<WebResourceResponse?> interceptApiRequest(
    api.MapRendererProxy proxy,
    WebUri url,
  ) async {
    if (!url.toString().startsWith(androidApiRequestPrefix)) return null;
    final response = await _request(proxy, url);
    return WebResourceResponse(
      data: response.body,
      contentType: response.contentType,
      contentEncoding: 'utf-8',
      statusCode: response.status,
      reasonPhrase: response.status == 200 ? 'OK' : 'Not Modified',
      headers: _headers(response),
    );
  }
}
