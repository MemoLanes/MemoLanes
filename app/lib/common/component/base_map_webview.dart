import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_flutter/webview_flutter.dart';

typedef MapView = ({double lng, double lat, double zoom});

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final MapView? initialMapView;
  final TrackingMode trackingMode;
  final void Function()? onMapMoved;
  final void Function(MapView)? onRoughMapViewUpdate;

  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      this.initialMapView,
      this.trackingMode = TrackingMode.off,
      this.onMapMoved,
      this.onRoughMapViewUpdate});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  late WebViewController _webViewController;
  late GpsManager _gpsManager;
  late Timer _roughMapViewUpdaeTimer;
  bool _readyForDisplay = false;

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  // For bug workaround
  bool _isiOS18 = false;

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
    _currentRoughMapView = widget.initialMapView;
    _roughMapViewUpdaeTimer =
        Timer.periodic(Duration(seconds: 2), (Timer t) async {
      final newMapView = await _getCurrentMapView();
      if (newMapView != _currentRoughMapView) {
        _currentRoughMapView = newMapView;
        widget.onRoughMapViewUpdate?.call(newMapView);
      }
    });
    _initWebView();

    () async {
      if (Platform.isIOS) {
        DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
        var iosInfo = await deviceInfo.iosInfo;
        if (iosInfo.systemVersion.startsWith('18.')) {
          setState(() {
            _isiOS18 = true;
          });
        }
      }
    }();
  }

  @override
  void dispose() {
    _gpsManager.removeListener(_updateLocationMarker);
    _roughMapViewUpdaeTimer.cancel();
    super.dispose();
  }

  Future<({double lng, double lat, double zoom})> _getCurrentMapView() async {
    // TODO: `runJavaScriptReturningResult` is very buggy. I only made it work
    // by forcing the js side only return string with the platform hack below.
    // See more: https://github.com/flutter/flutter/issues/80328
    String jsonString =
        await _webViewController.runJavaScriptReturningResult('''
        if (typeof getCurrentMapView === 'function') {
          getCurrentMapView();
        }
      ''') as String;
    if (Platform.isAndroid) {
      jsonString = jsonDecode(jsonString) as String;
    }

    // NOTE: when js is returning a double, we may get an int.
    final map = jsonDecode(jsonString);
    return (
      lng: map['lng'].toDouble() as double,
      lat: map['lat'].toDouble() as double,
      zoom: map['zoom'].toDouble() as double,
    );
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
              log.error('''Map WebView Error: 
                  Description: ${error.description}
                  Error Type: ${error.errorType} 
                  Error Code: ${error.errorCode}
                  Failed URL: ${error.url}''');
            }

            // TODO: The whole thing is a workaround. We should try to find a way
            // to make the map server work properly or just avoid using a real
            // Http server.
            if ((error.errorCode == -1004 || // iOS error code
                    (error.errorType == WebResourceErrorType.connect &&
                        error.errorCode == -6)) && // Android error code
                error.url?.contains('localhost') == true) {
              await api.restartMapServer();
              final url = getUrl();
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
      )
      ..addJavaScriptChannel(
        'readyForDisplay',
        onMessageReceived: (JavaScriptMessage message) {
          setState(() {
            _readyForDisplay = true;
          });
        },
      );

    final url = getUrl();
    await _webViewController.loadRequest(url);
  }

  Uri getUrl() {
    Uri uri = Uri.parse(widget.mapRendererProxy.getUrl());
    final mapView = _currentRoughMapView;
    if (mapView != null) {
      String fragment = uri.fragment;
      Map<String, String> fragmentParams = Uri.splitQueryString(fragment);
      fragmentParams['lng'] = mapView.lng.toString();
      fragmentParams['lat'] = mapView.lat.toString();
      fragmentParams['zoom'] = mapView.zoom.toString();
      String newFragment = Uri(queryParameters: fragmentParams).query;
      uri = uri.replace(fragment: newFragment);
    }
    return uri;
  }

  @override
  Widget build(BuildContext context) {
    // TODO: The `IgnorePointer` is a workaround for a bug in the webview on iOS.
    // https://github.com/flutter/flutter/issues/165305
    // But unfortunately, it only works for iOS 18, so we still have this weird
    // double tap behavior on older iOS versions.
    return Stack(
      children: [
        IgnorePointer(
            ignoring: _isiOS18,
            child: WebViewWidget(
                key: const ValueKey('map_webview'),
                controller: _webViewController)),
        IgnorePointer(
          ignoring: true,
          child: AnimatedOpacity(
            opacity: !_readyForDisplay ? 1 : 0.0,
            duration: const Duration(milliseconds: 200),
            child: Container(
              color: Color.fromARGB(255, 118, 116, 114),
              width: double.infinity,
              height: double.infinity,
            ),
          ),
        ),
      ],
    );
  }
}
