import 'dart:convert';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:webview_flutter/webview_flutter.dart';

class RenderDiagnosticsPage extends StatefulWidget {
  const RenderDiagnosticsPage({super.key});

  @override
  State<RenderDiagnosticsPage> createState() => _RenderDiagnosticsPageState();
}

class _RenderDiagnosticsPageState extends State<RenderDiagnosticsPage> {
  late final WebViewController _controller;

  @override
  void initState() {
    super.initState();

    // Initialize the WebView controller
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      // Add JavaScript channel for IPC communication
      ..addJavaScriptChannel(
        'RenderDiagnosticsChannel',
        onMessageReceived: (JavaScriptMessage message) {
          _handleIpcRequest(message.message);
        },
      )
      ..setNavigationDelegate(
        NavigationDelegate(
          onPageStarted: (String url) {
            debugPrint('Page started loading: $url');
          },
          onPageFinished: (String url) {
            debugPrint('Page finished loading: $url');
            // Inject the API endpoint after page loads
            _injectApiEndpoint();
          },
          onWebResourceError: (WebResourceError error) {
            debugPrint('WebView error: ${error.description}');
          },
        ),
      )
      ..loadFlutterAsset('assets/map_webview/render_diagnostics.html');
  }

  void _handleIpcRequest(String message) async {
    final startTime = DateTime.now().microsecondsSinceEpoch;

    try {
      final request = jsonDecode(message);
      final size = request['size'] ?? 1048576;
      final requestId = request['requestId'];

      debugPrint(
          'Render Diagnostics Request: size=$size, requestId=$requestId');

      // Use the Rust API instead of Dart random generation
      // Ensure 'size' is passed as BigInt if required by the API
      final data = await api.generateIpcTestData(
          size: size is BigInt ? size : BigInt.from(size));
      final base64Data = base64Encode(data);

      final endTime = DateTime.now().microsecondsSinceEpoch;
      final processingTime = endTime - startTime;

      // Use minimal JavaScript injection for response
      await _controller.runJavaScript(
          'window.handleIpcResponse($requestId,"$base64Data",${data.length},$processingTime)');
    } catch (e) {
      debugPrint('Error handling render diagnostics request: $e');
      await _controller.runJavaScript(
          'window.handleIpcError && window.handleIpcError("${e.toString()}")');
    }
  }

  Future<void> _injectApiEndpoint() async {
    final endpoint = api.getServerIpcTestUrl();

    debugPrint('Injecting API endpoint: $endpoint');

    await _controller.runJavaScript('''
      // Set the params
      window.EXTERNAL_PARAMS = {
        api_endpoint: "$endpoint"
      };
      
      // Check if JS is ready and trigger initialization if so
      if (typeof window.JS_READY !== 'undefined' && window.JS_READY) {
        console.log("JS already ready, triggering initialization");
        if (typeof initializeTest === 'function') {
          initializeTest();
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
      appBar: AppBar(
        title: Text(context.tr("general.advance_settings.render_diagnostics")),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
      ),
      body: WebViewWidget(controller: _controller),
    );
  }
}
