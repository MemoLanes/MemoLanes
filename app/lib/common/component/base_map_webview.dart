import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_flutter/webview_flutter.dart';

import 'map_controls/map_copyright_button.dart';

typedef MapView = ({double lng, double lat, double zoom});

typedef BaseMapJavaScriptMessageHandler = void Function(String message);

class BaseMapJavaScriptChannel {
  final String name;
  final BaseMapJavaScriptMessageHandler onMessageReceived;

  const BaseMapJavaScriptChannel({
    required this.name,
    required this.onMessageReceived,
  });
}

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final MapView? initialMapView;
  final TrackingMode trackingMode;
  final bool isEditor;
  final void Function()? onMapMoved;
  final void Function(MapView)? onRoughMapViewUpdate;
  final void Function(int)? onMapZoomChanged;
  final List<BaseMapJavaScriptChannel> extraJavaScriptChannels;

  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      this.initialMapView,
      this.trackingMode = TrackingMode.off,
      this.isEditor = false,
      this.onMapMoved,
      this.onRoughMapViewUpdate,
      this.onMapZoomChanged,
      this.extraJavaScriptChannels = const []});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  late WebViewController _webViewController;
  late GpsManager _gpsManager;
  bool _readyForDisplay = false;

  // TODO: define a proper type to make it more type-safe
  // TODO: we may let user to choose base map providers.
  final _mapStyle = "https://tiles.openfreemap.org/styles/liberty";
  // final _mapStyle = "mapbox://styles/mapbox/streets-v12";

  // Dev server URL for loading map webview from a local dev server.
  // Usage: flutter run --dart-define=DEV_SERVER=http://ip:port
  static const _devServer = String.fromEnvironment('DEV_SERVER');

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  // For bug workaround
  bool _isiOS18 = false;

  Future<void> runJavaScript(String javaScript) {
    return _webViewController.runJavaScript(javaScript);
  }

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trackingMode != widget.trackingMode) _updateLocationMarker();

    // Refresh map data when the renderer proxy changes
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _refreshMapData();
    }
  }

  /// Request the WebView to refresh map data from the backend
  Future<void> _refreshMapData() async {
    log.info('[base_map_webview] Refreshing map data');
    await _webViewController.runJavaScript('''
      if (typeof refreshMapData === 'function') {
        console.log('Refreshing map data');
        refreshMapData();
      } else {
        console.warn('refreshMapData function not available yet');
      }
    ''');
  }

  @override
  void initState() {
    super.initState();
    _webViewController = WebViewController();
    _gpsManager = Provider.of<GpsManager>(context, listen: false);
    _gpsManager.addListener(_updateLocationMarker);
    _currentRoughMapView = widget.initialMapView;
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
            // Allow dev server URLs during development
            if (_devServer.isNotEmpty && request.url.startsWith(_devServer)) {
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
        onMessageReceived: (JavaScriptMessage message) async {
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
      )
      ..addJavaScriptChannel(
        'onMapViewChanged',
        onMessageReceived: (JavaScriptMessage message) async {
          _handleMapViewPush(message.message);
        },
      )
      ..addJavaScriptChannel(
        'onMapZoomChanged',
        onMessageReceived: (JavaScriptMessage message) async {
          _handleMapZoomPush(message.message);
        },
      );

    for (final channel in widget.extraJavaScriptChannels) {
      _webViewController.addJavaScriptChannel(
        channel.name,
        onMessageReceived: (JavaScriptMessage message) {
          channel.onMessageReceived(message.message);
        },
      );
    }
    final assetPath = 'assets/map_webview/index.html';
    log.info('[base_map_webview] Initial loading asset: $assetPath');
    await _webViewController.loadFlutterAsset(assetPath);
    if (_devServer.isNotEmpty) {
      // Load from dev server for hot-reload during development
      final devUrl = _devServer.endsWith('/')
          ? '${_devServer}index.html'
          : '$_devServer/index.html';
      log.info('[base_map_webview] Loading from dev server: $devUrl');
      await _webViewController.loadRequest(Uri.parse(devUrl));
    } else {
      // Load from bundled assets
      final assetPath = 'assets/map_webview/index.html';
      log.info('[base_map_webview] Loading asset: $assetPath');
      await _webViewController.loadFlutterAsset(assetPath);
    }
  }

  Future<void> _injectApiEndpoint() async {
    final accessKey = api.getMapboxAccessToken();

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
        render: "canvas",
        map_style: "$_mapStyle",
        access_key: ${accessKey != null ? "\"$accessKey\"" : "null"},
        lng: $lngParam,
        lat: $latParam,
        zoom: $zoomParam,
        editor: ${widget.isEditor ? "true" : "false"},
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

  void _handleMapViewPush(String message) {
    try {
      final map = jsonDecode(message);

      double readNum(dynamic v, String key) {
        if (v is num) return v.toDouble();
        throw StateError('Invalid $key: $v');
      }

      final mapView = (
        lng: readNum(map['lng'], 'lng'),
        lat: readNum(map['lat'], 'lat'),
        zoom: readNum(map['zoom'], 'zoom'),
      );

      if (mapView != _currentRoughMapView) {
        _currentRoughMapView = mapView;
        widget.onRoughMapViewUpdate?.call(mapView);
      }
    } catch (e) {
      log.error('[base_map_webview] invalid mapView push: $message, error=$e');
    }
  }

  void _handleMapZoomPush(String message) {
    try {
      final int? zoom = int.tryParse(message);

      if (zoom == null) {
        log.error('[base_map_webview] zoom is not a valid integer: $message');
        return;
      }

      widget.onMapZoomChanged?.call(zoom);
    } catch (e) {
      log.error('[base_map_webview] error parsing zoom: $message, error=$e');
    }
  }

  void _handleTileProviderRequest(String message) async {
    try {
      // debugPrint('Tile Provider IPC Request: $message');

      // Forward the JSON request transparently to Rust and get raw JSON response
      final responseJson = await widget.mapRendererProxy.handleWebviewRequests(
        request: message,
      );

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
    var mapCopyrightTextMarkdown = 'UNKNOWN';
    if (_mapStyle.contains('openfreemap.org')) {
      mapCopyrightTextMarkdown =
          "[OpenFreeMap](https://openfreemap.org) [© OpenMapTiles](https://www.openmaptiles.org/) Data from [OpenStreetMap](https://www.openstreetmap.org/copyright)";
    } else if (_mapStyle.contains('mapbox')) {
      mapCopyrightTextMarkdown =
          '[© Mapbox](https://www.mapbox.com/about/maps) [© OpenStreetMap](https://www.openstreetmap.org/copyright/) [Improve this map](https://www.mapbox.com/contribute/)';
    }

    return Stack(
      children: [
        IgnorePointer(
            ignoring: _isiOS18,
            child: WebViewWidget(
                key: const ValueKey('map_webview'),
                controller: _webViewController)),
        GestureDetector(
            child: MapCopyrightButton(
          textMarkdown: mapCopyrightTextMarkdown,
        )),
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
        // This is to prevent actions to iOS home indicator affects the
        // underlying webview. (e.g. back to home gesture moves the map)
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          height: MediaQuery.of(context).padding.bottom,
          child: PointerInterceptor(
            intercepting: Platform.isIOS,
            child: const ColoredBox(color: Colors.transparent),
          ),
        ),
      ],
    );
  }
}
