import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/map.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_flutter/webview_flutter.dart';

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final MapState? mapState;
  final TrackingMode trackingMode;
  final void Function()? onMapMoved;
  final void Function(double lnt, double lat, double zoom)? onMapStatus;

  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      this.trackingMode = TrackingMode.off,
      this.onMapMoved,
      this.mapState,
      this.onMapStatus});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  late WebViewController _webViewController;
  late GpsManager _gpsManager;
  late Timer _mapStateTimer;

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
    _mapStateTimer = Timer.periodic(Duration(seconds: 5), (Timer t) {
      _callMapState();
    });
    _initWebView();
  }

  @override
  void dispose() {
    _gpsManager.removeListener(_updateLocationMarker);
    _mapStateTimer.cancel();
    super.dispose();
  }

  void _callMapState() async {
    final dynamic jsResult =
        await _webViewController.runJavaScriptReturningResult('''
        if (typeof getCurrentMapView === 'function') {
          getCurrentMapView();
        }
      ''');
    if (jsResult == null || jsResult == "null") {
      return;
    }
    final String jsonString =
        jsResult is String ? jsResult : jsResult.toString();
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setString("mapViewStatus", jsonString);
    final map = jsonDecode(jsonString);
    final lng = double.parse(map['lng'].toString());
    final lat = double.parse(map['lat'].toString());
    final zoom = double.parse(map['zoom'].toString());
    widget.onMapStatus?.call(lng, lat, zoom);
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
          onWebResourceError: (WebResourceError error) async {
            // the mapbox error is common (maybe blocked by some firewall )
            if (error.url?.contains('events.mapbox.com') != true) {
              api.writeLog(message: '''Map WebView Error: 
                  Description: ${error.description}
                  Error Type: ${error.errorType} 
                  Error Code: ${error.errorCode}
                  Failed URL: ${error.url}''', level: api.LogLevel.error);
            }

            // TODO: The whole thing is a workaround. We should try to find a way
            // to make the map server work properly or just avoid using a real
            // Http server.
            if ((error.errorCode == -1004 || // iOS error code
                    (error.errorType == WebResourceErrorType.connect &&
                        error.errorCode == -6)) && // Android error code
                error.url?.contains('localhost') == true) {
              await api.restartMapServer();
              final url =
                  replaceUri(widget.mapRendererProxy.getUrl(), widget.mapState);
              await _webViewController.loadRequest(url);
              return;
            }

            if (error.errorType ==
                WebResourceErrorType.webContentProcessTerminated) {
              await _webViewController.reload();
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

    final url = replaceUri(widget.mapRendererProxy.getUrl(), widget.mapState);
    await _webViewController.loadRequest(url);
  }

  Uri replaceUri(String url, MapState? mapViewStatus) {
    Uri uri = Uri.parse(url);
    if (mapViewStatus != null) {
      String fragment = uri.fragment ?? '';
      Map<String, String> fragmentParams = Uri.splitQueryString(fragment);
      fragmentParams['lng'] = mapViewStatus.lng.toString();
      fragmentParams['lat'] = mapViewStatus.lat.toString();
      fragmentParams['zoom'] = mapViewStatus.zoom.toString();
      String newFragment = Uri(queryParameters: fragmentParams).query;
      uri = uri.replace(fragment: newFragment);
    }
    return uri;
  }

  @override
  Widget build(BuildContext context) {
    return WebViewWidget(
        key: const ValueKey('map_webview'), controller: _webViewController);
  }
}
