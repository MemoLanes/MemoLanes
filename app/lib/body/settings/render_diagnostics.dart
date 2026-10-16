import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/map_webview_resources.dart';
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
    await MapWebViewResources.load(controller, 'render_diagnostics.html');
  }

  Future<void> _injectApiEndpoint() async {
    final cgiEndpoint = MapWebViewResources.apiEndpoint;
    final appearance = (Theme.of(context).brightness == Brightness.dark)
        ? 'dark'
        : 'light';

    await _controller?.evaluateJavascript(
      source:
          '''
      document.documentElement.dataset.theme = "$appearance";
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
    ''',
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr("general.advanced_settings.render_diagnostics"),
      ),
      body: PageContentInsets(
        child: InAppWebView(
          initialSettings: InAppWebViewSettings(
            javaScriptEnabled: true,
            allowFileAccessFromFileURLs: true,
            allowUniversalAccessFromFileURLs: true,
            resourceCustomSchemes: ['memolanes'],
            webViewAssetLoader: MapWebViewResources.createAssetLoader(),
          ),
          onWebViewCreated: (controller) {
            _onWebViewCreated(controller);
          },
          onLoadResourceWithCustomScheme: (controller, request) async {
            if (!mounted) return null;
            return MapWebViewResources.loadApiResource(
              _mapRendererProxy,
              request.url,
            );
          },
          shouldInterceptRequest: (controller, request) async {
            if (!mounted) return null;
            return MapWebViewResources.interceptApiRequest(
              _mapRendererProxy,
              request.url,
            );
          },
          onLoadStop: (controller, url) {
            debugPrint('Page finished loading: $url');
            _injectApiEndpoint();
          },
          onConsoleMessage: (controller, consoleMessage) {
            debugPrint(
              '[${consoleMessage.messageLevel.name}] ${consoleMessage.message}',
            );
          },
          onReceivedError: (controller, request, error) {
            debugPrint('WebView error: ${error.description}');
          },
        ),
      ),
    );
  }
}
