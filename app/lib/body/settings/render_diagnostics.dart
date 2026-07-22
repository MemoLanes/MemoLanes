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

  Future<
      ({
        int status,
        Uint8List body,
        String contentType,
        Map<String, String> headers
      })> _handleInterceptedRequest(WebUri url) async {
    final path = url.path.replaceFirst(RegExp(r'^/?(api/)?'), '');
    final result = await _mapRendererProxy.handleRequest(
      path: path,
      queryParams: url.queryParameters,
    );
    return (
      status: result.status,
      body: result.body,
      contentType: result.contentType,
      headers: result.headers,
    );
  }

  Future<void> _injectApiEndpoint() async {
    final cgiEndpoint =
        Platform.isIOS ? 'memolanes://api' : 'https://memolanes.local/api';

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
          final result = await _handleInterceptedRequest(request.url);
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
          final result = await _handleInterceptedRequest(request.url);
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
