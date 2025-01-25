import 'dart:async';
import 'package:flutter/material.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:memolanes/src/rust/api/api.dart' as api;

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final TrackingMode initialTrackingMode;
  final void Function(TrackingMode)? onTrackingModeChanged;
  const BaseMapWebview({
    super.key,
    required this.mapRendererProxy,
    this.initialTrackingMode = TrackingMode.off,
    this.onTrackingModeChanged,
  });

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  WebViewController? _webViewController;
  late TrackingMode _trackingMode;

  // Getter for external access
  TrackingMode get trackingMode => _trackingMode;

  // Method to update tracking mode from outside
  void updateTrackingMode(TrackingMode newMode) {
    if (_trackingMode == newMode) return;

    _trackingMode = newMode;
    Provider.of<GpsManager>(context, listen: false)
        .toggleMapTracking(trackingMode != TrackingMode.off);
    widget.onTrackingModeChanged?.call(_trackingMode);

    if (mounted) {
      _updateLocationMarker(context);
    }
  }

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    // TODO: the below is for compatibility for android or ios, double check later.
    // Only update URL if the mapRendererProxy actually changed
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _updateMapUrl();
    }
  }

  Future<void> _updateMapUrl() async {
    if (_webViewController == null) return;

    final url = widget.mapRendererProxy.getUrl();

    // TODO: currently when trackingMode updates, the upper layer will trigger a
    // rebuid of this widget? we should not reload the page if url is unchanged
    // this may be an iOS bug to be investigated further
    final currentUrl = await _webViewController?.currentUrl();

    if (currentUrl != url) {
      await _webViewController?.loadRequest(Uri.parse(url));
    }
  }

  @override
  void initState() {
    super.initState();
    _trackingMode = widget.initialTrackingMode;
    Provider.of<GpsManager>(context, listen: false)
        .toggleMapTracking(trackingMode != TrackingMode.off);
    _initWebView();
  }

// TODO: solve the following known issues:
// 1. ios tap-and-hold triggers a magnifier
//     ref: https://stackoverflow.com/questions/75628788/disable-double-tap-magnifying-glass-in-safari-ios
//     but the settings seems not be exposed by current webview_flutter
//     ref (another WKPreference setting): https://github.com/flutter/flutter/issues/112276
// 2. ios double-tap zoom not working (triple tap needed, maybe related to tap event capture)
  Future<void> _initWebView() async {
    _webViewController = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setNavigationDelegate(
        NavigationDelegate(
          onNavigationRequest: (NavigationRequest request) {
            // TODO: Block localhost URLs except for our map
            if (request.url.contains('localhost') ||
                request.url.contains('127.0.0.1')) {
              return NavigationDecision.navigate;
            }
            // Allow all other URLs to open in system browser
            launchUrl(
              Uri.parse(request.url),
              mode: LaunchMode.externalApplication,
            );
            return NavigationDecision.prevent;
          },
        ),
      )
      ..addJavaScriptChannel(
        'onMapMoved',
        onMessageReceived: (JavaScriptMessage message) {
          if (_trackingMode == TrackingMode.displayAndTracking) {
            updateTrackingMode(TrackingMode.displayOnly);
          }
        },
      );

    final url = widget.mapRendererProxy.getUrl();
    await _webViewController?.loadRequest(Uri.parse(url));
  }

  @override
  void dispose() {
    super.dispose();
  }

  void _updateLocationMarker(BuildContext context) {
    if (_webViewController == null) return;

    final gpsState = context.read<GpsManager>();
    final position = gpsState.latestPosition;

    if (_trackingMode == TrackingMode.off) {
      _webViewController?.runJavaScript('''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(0, 0, false);
        }
      ''');
    } else if (position != null) {
      _webViewController?.runJavaScript('''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(
            ${position.longitude}, 
            ${position.latitude}, 
            true, 
            ${_trackingMode == TrackingMode.displayAndTracking}
          );
        }
      ''');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_trackingMode != TrackingMode.off) {
      _updateLocationMarker(context);
    }

    var webViewController = _webViewController;
    if (webViewController == null) {
      return Container();
    }
    return WebViewWidget(
        key: const ValueKey('map_webview'), controller: webViewController);
  }
}
