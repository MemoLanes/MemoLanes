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
  final TrackingMode trackingMode;
  final void Function()? onMapMoved;
  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      this.trackingMode = TrackingMode.off,
      this.onMapMoved});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  late WebViewController _webViewController;
  late GpsManager _gpsManager;

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trackingMode != widget.trackingMode) _updateLocationMarker();
    // TODO: the below is for compatibility for android or ios, double check later.
    // Only update URL if the mapRendererProxy actually changed
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _updateMapUrl();
    }
  }

  Future<void> _updateMapUrl() async {
    final url = widget.mapRendererProxy.getUrl();

    // TODO: currently when trackingMode updates, the upper layer will trigger a
    // rebuid of this widget? we should not reload the page if url is unchanged
    // this may be an iOS bug to be investigated further
    final currentUrl = await _webViewController.currentUrl();

    if (currentUrl != url) {
      await _webViewController.loadRequest(Uri.parse(url));
    }
  }

  @override
  void initState() {
    super.initState();
    _webViewController = WebViewController();
    _gpsManager = Provider.of<GpsManager>(context, listen: false);
    _gpsManager.addListener(_updateLocationMarker);
    _initWebView();
  }

  @override
  void dispose() {
    _gpsManager.removeListener(_updateLocationMarker);
    super.dispose();
  }

  void _updateLocationMarker() {
    if (widget.trackingMode == TrackingMode.off) {
      _webViewController.runJavaScript('''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(0, 0, false);
        }
      ''');
    } else {
      final position = _gpsManager.latestPosition;
      if (position != null) {
        _webViewController.runJavaScript('''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(
            ${position.longitude}, 
            ${position.latitude}, 
            true, 
            ${widget.trackingMode == TrackingMode.displayAndTracking}
          );
        }
      ''');
      }
    }
  }

// TODO: solve the following known issues:
// 1. ios tap-and-hold triggers a magnifier
//     ref: https://stackoverflow.com/questions/75628788/disable-double-tap-magnifying-glass-in-safari-ios
//     but the settings seems not be exposed by current webview_flutter
//     ref (another WKPreference setting): https://github.com/flutter/flutter/issues/112276
// 2. ios double-tap zoom not working (triple tap needed, maybe related to tap event capture)
  Future<void> _initWebView() async {
    _webViewController
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
          onWebResourceError: (WebResourceError error) {
            // the mapbox error is common (maybe blocked by some firewall )
            if (error.url?.contains('events.mapbox.com') != true) {
              api.writeLog(message: '''Map WebView Error: 
                  Description: ${error.description}
                  Error Type: ${error.errorType} 
                  Error Code: ${error.errorCode}
                  Failed URL: ${error.url}''', level: api.LogLevel.error);
            }

            if ((error.errorCode == -1004 || // iOS error code
                    (error.errorType == WebResourceErrorType.connect &&
                        error.errorCode == -6)) && // Android error code
                error.url?.contains('localhost') == true) {
              api.restartMapServer();
              final url = widget.mapRendererProxy.getUrl();
              _webViewController.loadRequest(Uri.parse(url));
              return;
            }

            if (error.errorType ==
                WebResourceErrorType.webContentProcessTerminated) {
              _webViewController.reload();
            }
          },
        ),
      )
      ..addJavaScriptChannel(
        'onMapMoved',
        onMessageReceived: (JavaScriptMessage message) {
          widget.onMapMoved?.call();
        },
      );

    final url = widget.mapRendererProxy.getUrl();
    await _webViewController.loadRequest(Uri.parse(url));
  }

  @override
  Widget build(BuildContext context) {
    return WebViewWidget(
        key: const ValueKey('map_webview'), controller: _webViewController);
  }
}
