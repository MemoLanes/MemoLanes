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
      debugPrint('Render Diagnostics Request: $message');

      // Forward the JSON request transparently to Rust and get raw JSON response
      final responseJson = await api.handleWebviewRequests(request: message);

      final endTime = DateTime.now().microsecondsSinceEpoch;
      final processingTimeMs = (endTime - startTime) / 1000;
      final truncatedResponse = responseJson.length > 100
          ? '${responseJson.substring(0, 100)}...'
          : responseJson;
      debugPrint(
          'Render Diagnostics Response (${processingTimeMs.toStringAsFixed(1)}ms): $truncatedResponse');

      // Send the raw JSON response directly to JavaScript
      // Escape the JSON string for JavaScript injection
      final escapeStartTime = DateTime.now().microsecondsSinceEpoch;
      final escapedResponse = responseJson
          .replaceAll('\\', '\\\\') // Escape backslashes first
          .replaceAll("'", "\\'") // Escape single quotes
          .replaceAll('\n', '\\n') // Escape newlines
          .replaceAll('\r', '\\r'); // Escape carriage returns
      final escapeEndTime = DateTime.now().microsecondsSinceEpoch;
      final escapeTimeMs = (escapeEndTime - escapeStartTime) / 1000;

      final jsStartTime = DateTime.now().microsecondsSinceEpoch;
      await _controller.runJavaScript('''
        if (typeof window.handle_RenderDiagnosticsChannel_JsonResponse === 'function') {
          window.handle_RenderDiagnosticsChannel_JsonResponse('$escapedResponse');
        } else {
          console.error('No RenderDiagnostics JSON response handler found');
          console.log('Raw response:', '$escapedResponse');
        }
      ''');
      final jsEndTime = DateTime.now().microsecondsSinceEpoch;
      final jsTimeMs = (jsEndTime - jsStartTime) / 1000;

      debugPrint(
          'Render Diagnostics Timing - Escape: ${escapeTimeMs.toStringAsFixed(1)}ms, JS execution: ${jsTimeMs.toStringAsFixed(1)}ms');
    } catch (e) {
      debugPrint('Error processing Render Diagnostics IPC request: $e');

      // Create error response in same format as Rust would
      final errorResponse = jsonEncode({
        'requestId': 'unknown',
        'success': false,
        'data': null,
        'error': 'IPC processing error: $e'
      });

      final errorEscapeStartTime = DateTime.now().microsecondsSinceEpoch;
      final escapedError = errorResponse
          .replaceAll('\\', '\\\\')
          .replaceAll("'", "\\'")
          .replaceAll('\n', '\\n')
          .replaceAll('\r', '\\r');
      final errorEscapeEndTime = DateTime.now().microsecondsSinceEpoch;
      final errorEscapeTimeMs =
          (errorEscapeEndTime - errorEscapeStartTime) / 1000;

      final errorJsStartTime = DateTime.now().microsecondsSinceEpoch;
      await _controller.runJavaScript('''
        if (typeof window.handle_RenderDiagnosticsChannel_JsonResponse === 'function') {
          window.handle_RenderDiagnosticsChannel_JsonResponse('$escapedError');
        } else {
          console.error('Error handling failed - no handler found');
        }
      ''');
      final errorJsEndTime = DateTime.now().microsecondsSinceEpoch;
      final errorJsTimeMs = (errorJsEndTime - errorJsStartTime) / 1000;

      debugPrint(
          'Render Diagnostics Error Timing - Escape: ${errorEscapeTimeMs.toStringAsFixed(1)}ms, JS execution: ${errorJsTimeMs.toStringAsFixed(1)}ms');
    }
  }

  Future<void> _injectApiEndpoint() async {
    final endpoint = api.getServerIpcTestUrl();

    debugPrint('Injecting API endpoint: $endpoint');

    await _controller.runJavaScript('''
      // Set the params using the new unified API structure
      window.EXTERNAL_PARAMS = {
        http_endpoint: "$endpoint",
        flutter_channel: "RenderDiagnosticsChannel"
      };
      
      // Check if JS is ready and trigger initialization if so
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
      appBar: AppBar(
        title: Text(context.tr("general.advance_settings.render_diagnostics")),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
      ),
      body: WebViewWidget(controller: _controller),
    );
  }
}
