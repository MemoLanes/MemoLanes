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
  late Timer _roughMapViewUpdateTimer;
  bool _readyForDisplay = false;

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  // Track current journey ID to detect changes
  String? _currentJourneyId;

  // For bug workaround
  bool _isiOS18 = false;

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trackingMode != widget.trackingMode) _updateLocationMarker();

    // Check if journey ID has changed and update via JavaScript API
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _updateJourneyIdIfChanged();
    }
  }

  Future<void> _updateJourneyIdIfChanged() async {
    final newJourneyId = widget.mapRendererProxy.getJourneyId();

    // Check if journey ID has actually changed
    if (_currentJourneyId != newJourneyId) {
      log.info(
          '[base_map_webview] Journey ID changed from $_currentJourneyId to $newJourneyId');
      _currentJourneyId = newJourneyId;

      // Update journey ID via JavaScript API instead of reloading the page
      await _webViewController.runJavaScript('''
        if (typeof updateJourneyId === 'function') {
          console.log('Updating journey ID to: $newJourneyId');
          updateJourneyId('$newJourneyId');
        } else {
          console.warn('updateJourneyId function not available yet');
        }
      ''');
    }
  }

  @override
  void initState() {
    super.initState();
    _webViewController = WebViewController();
    _gpsManager = Provider.of<GpsManager>(context, listen: false);
    _gpsManager.addListener(_updateLocationMarker);
    _currentRoughMapView = widget.initialMapView;
    _currentJourneyId = widget.mapRendererProxy.getJourneyId();
    _roughMapViewUpdateTimer =
        Timer.periodic(Duration(seconds: 5), (Timer t) async {
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
    _roughMapViewUpdateTimer.cancel();
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
            // only allow navigating to our map
            var uri = Uri.parse(request.url);
            if (uri.scheme == 'file') {
              return NavigationDecision.navigate;
            }
            // all other URLs will be opened in system browser
            launchUrl(
              Uri.parse(request.url),
              mode: LaunchMode.externalApplication,
            );
            return NavigationDecision.prevent;
          },
          onPageFinished: (String url) {
            debugPrint('Page finished loading: $url');
            // Inject the API endpoint after page loads
            _injectApiEndpoint();
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
      )
      ..addJavaScriptChannel(
        'TileProviderChannel',
        onMessageReceived: (JavaScriptMessage message) {
          _handleTileProviderRequest(message.message);
        },
      );
    final assetPath = 'assets/map_webview/index.html';
    log.info('[base_map_webview] Initial loading asset: $assetPath');
    await _webViewController.loadFlutterAsset(assetPath);
  }

  Future<void> _injectApiEndpoint() async {
    final accessKey = api.getMapboxAccessToken();

    // TODO: we may let user to choose base map providers.
    final mapStyle = "https://tiles.openfreemap.org/styles/liberty";
    // final mapStyle = "mapbox://styles/mapbox/streets-v12";

    final journeyId = widget.mapRendererProxy.getJourneyId();

    // Get map view coordinates
    final mapView = _currentRoughMapView;
    final lngParam = mapView?.lng.toString() ?? 'null';
    final latParam = mapView?.lat.toString() ?? 'null';
    final zoomParam = mapView?.zoom.toString() ?? 'null';

    debugPrint('Injecting lng: $lngParam');
    debugPrint('Injecting lat: $latParam');
    debugPrint('Injecting zoom: $zoomParam');

    await _webViewController.runJavaScript('''
      // Set the params
      window.EXTERNAL_PARAMS = {
        cgi_endpoint: "flutter://TileProviderChannel",
        journey_id: "$journeyId",
        render: "canvas",
        map_style: "$mapStyle",
        access_key: ${accessKey != null ? "\"$accessKey\"" : "null"},
        lng: $lngParam,
        lat: $latParam,
        zoom: $zoomParam,
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

    debugPrint('Initialization completed');
  }

  void _handleTileProviderRequest(String message) async {
    try {
      // debugPrint('Tile Provider IPC Request: $message');

      // Forward the JSON request transparently to Rust and get raw JSON response
      final responseJson = await api.handleWebviewRequests(request: message);

      // final truncatedResponse = responseJson.length > 100
      //     ? '${responseJson.substring(0, 100)}...'
      //     : responseJson;

      // debugPrint('Tile Provider IPC Response: $truncatedResponse');

      // Send the JSON response as a JavaScript object (no escaping needed)
      await _webViewController.runJavaScript('''
        if (typeof window.handle_TileProviderChannel_JsonResponse === 'function') {
          const responseData = $responseJson;
          window.handle_TileProviderChannel_JsonResponse(responseData);
        } else {
          console.error('No TileProvider JSON response handler found');
        }
      ''');
    } catch (e) {
      debugPrint('Error processing Tile Provider IPC request: $e');

      // Create error response in same format as Rust would
      final errorResponse = jsonEncode({
        'requestId': 'unknown',
        'success': false,
        'data': null,
        'error': 'IPC processing error: $e'
      });

      await _webViewController.runJavaScript('''
        if (typeof window.handle_TileProviderChannel_JsonResponse === 'function') {
          const errorData = $errorResponse;
          window.handle_TileProviderChannel_JsonResponse(errorData);
        } else {
          console.error('Error handling failed - no handler found');
        }
      ''');
    }
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
