import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class RenderDiagnosticsPage extends StatefulWidget {
  const RenderDiagnosticsPage({super.key});

  @override
  State<RenderDiagnosticsPage> createState() => _RenderDiagnosticsPageState();
}

class _RenderDiagnosticsPageState extends State<RenderDiagnosticsPage> {
  InAppWebViewController? _controller;
  late final api.MapRendererProxy _mapRendererProxy;

  @override
  void initState() {
    super.initState();
    _mapRendererProxy = api.getEmptyMapRendererProxy();
  }

  Future<void> _onWebViewCreated(InAppWebViewController controller) async {
    _controller = controller;
    await controller.loadFile(
        assetFilePath: 'assets/map_webview/render_diagnostics.html');
  }

  Future<({int status, Uint8List body, String contentType, Map<String, String> headers})>
      _handleInterceptedRequest(WebUri url) async {
    final queryParams = url.queryParameters;
    final path = url.path.replaceFirst(RegExp(r'^/?(api/)?'), '');

    // Use binary path for tile_range requests
    if (path == 'tile_range') {
      try {
        final result = await _mapRendererProxy.handleTileRangeBinary(
          x: int.parse(queryParams['x'] ?? '0'),
          y: int.parse(queryParams['y'] ?? '0'),
          z: int.parse(queryParams['z'] ?? '0').toInt(),
          width: int.parse(queryParams['width'] ?? '1'),
          height: int.parse(queryParams['height'] ?? '1'),
          bufferSizePower: int.parse(queryParams['buffer_size_power'] ?? '8').toInt(),
          cachedVersion: queryParams['cached_version'],
        );

        if (result.status == 304) {
          return (
            status: 200,
            body: Uint8List(0),
            contentType: 'application/octet-stream',
            headers: {'X-Not-Modified': 'true'},
          );
        }
        return (
          status: 200,
          body: Uint8List.fromList(result.body),
          contentType: 'application/octet-stream',
          headers: {
            if (result.version != null) 'X-Tile-Version': result.version!,
          },
        );
      } catch (e) {
        debugPrint('Error in tile range binary handler: $e');
        return (
          status: 500,
          body: Uint8List.fromList(utf8.encode('Error: $e')),
          contentType: 'text/plain',
          headers: <String, String>{},
        );
      }
    }

    // Fallback: JSON path for other request types
    final requestJson = jsonEncode({
      'requestId': 'intercepted_${DateTime.now().millisecondsSinceEpoch}',
      'query': path,
      'payload': {
        for (final entry in queryParams.entries)
          entry.key: num.tryParse(entry.value) ?? entry.value,
      },
    });

    try {
      final responseJson = await _mapRendererProxy.handleWebviewRequests(
        request: requestJson,
      );
      return (
        status: 200,
        body: Uint8List.fromList(utf8.encode(responseJson)),
        contentType: 'application/json',
        headers: <String, String>{},
      );
    } catch (e) {
      final errorResponse = jsonEncode({
        'requestId': 'error',
        'success': false,
        'data': null,
        'error': 'Interceptor error: $e',
      });
      return (
        status: 500,
        body: Uint8List.fromList(utf8.encode(errorResponse)),
        contentType: 'application/json',
        headers: <String, String>{},
      );
    }
  }

  Future<void> _injectApiEndpoint() async {
    final cgiEndpoint = Platform.isIOS
        ? 'memolanes://api'
        : 'https://memolanes.local/api';

    await _controller?.evaluateJavascript(source: '''
      window.EXTERNAL_PARAMS = {
        cgi_endpoint: "$cgiEndpoint"
      };
      
      if (typeof window.SETUP_PENDING !== 'undefined' && window.SETUP_PENDING) {
        console.log("JS already ready, triggering initialization");
        if (typeof trySetup === 'function') {
          trySetup();
        }
      } else {
        console.log("JS not ready yet, params stored for later");
      }
    ''');
  }

  @override
  void dispose() {
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.advanced_settings.render_diagnostics"),
      ),
      body: InAppWebView(
        initialSettings: InAppWebViewSettings(
          javaScriptEnabled: true,
          allowFileAccessFromFileURLs: true,
          allowUniversalAccessFromFileURLs: true,
          resourceCustomSchemes: ['memolanes'],
        ),
        onWebViewCreated: (controller) {
          _onWebViewCreated(controller);
        },
        onLoadResourceWithCustomScheme: (controller, request) async {
          final result =
              await _handleInterceptedRequest(request.url);
          return CustomSchemeResponse(
            data: result.body,
            contentType: result.contentType,
            contentEncoding: 'utf-8',
            statusCode: result.status,
            headers: {
              'Content-Type': result.contentType,
              ...result.headers,
            },
          );
        },
        shouldInterceptRequest: (controller, request) async {
          final url = request.url.toString();
          if (!url.startsWith('https://memolanes.local/api/')) {
            return null;
          }
          final result =
              await _handleInterceptedRequest(request.url);
          return WebResourceResponse(
            contentType: result.contentType,
            contentEncoding: 'utf-8',
            data: result.body,
            statusCode: result.status,
            reasonPhrase: result.status == 200 ? 'OK' : 'Not Modified',
            headers: {
              'Access-Control-Allow-Origin': '*',
              'Access-Control-Expose-Headers': 'X-Tile-Version, X-Not-Modified',
              'Content-Type': result.contentType,
              ...result.headers,
            },
          );
        },
        onLoadStop: (controller, url) {
          debugPrint('Page finished loading: $url');
          _injectApiEndpoint();
        },
        onConsoleMessage: (controller, consoleMessage) {
          debugPrint(
              '[${consoleMessage.messageLevel.name}] ${consoleMessage.message}');
        },
        onReceivedError: (controller, request, error) {
          debugPrint('WebView error: ${error.description}');
        },
      ),
    );
  }
}
