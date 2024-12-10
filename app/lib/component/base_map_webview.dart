import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';

// for compatibility with base_map.dart (camaraOptions, etc.)
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class MapController {
  final WebViewController webViewController;
  final void Function() triggerRefresh;

  MapController(this.webViewController, this.triggerRefresh);
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final CameraOptions initialCameraOptions;
  final void Function(MapController mapController)? onMapCreated;
  final OnMapScrollListener? onScrollListener;
  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      required this.initialCameraOptions,
      this.onMapCreated,
      this.onScrollListener});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  WebViewController? _webViewController;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();

  Future<void> _doActualRefresh() async {
    var controller = _webViewController;
    // if (controller == null) return;

    return;
  }

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      // Reinitialize WebView when proxy changes
      _initWebView();
      _triggerRefresh();
    }
  }

  void _refreshLoop() async {
    await widget.mapRendererProxy.resetMapRenderer();
    while (true) {
      await requireRefresh?.future;
      if (requireRefresh == null) return;
      // make it ready for the next request
      requireRefresh = Completer();

      await _doActualRefresh();
    }
  }

  void _triggerRefresh() async {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
  }

  @override
  void initState() {
    super.initState();
    _initWebView();
    _refreshLoop();
  }

  Future<void> _initWebView() async {
    // Create a new WebViewController each time
    _webViewController = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted);

    final url = await widget.mapRendererProxy.getUrl();
    await _webViewController?.loadRequest(Uri.parse(url));
  }

  @override
  void dispose() {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _webViewController == null
        ? Container()
        : WebViewWidget(
            controller: _webViewController!,
          );
  }
}
