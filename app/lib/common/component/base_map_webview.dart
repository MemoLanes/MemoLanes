import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:battery_plus/battery_plus.dart';
import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/utils.dart' as rust_utils show MapBounds;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

typedef MapView = ({double lng, double lat, double zoom});
typedef MapBounds = rust_utils.MapBounds;

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
  final MapBounds? initialMapBounds;
  final EdgeInsets? initialMapBoundsPadding;
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
      this.initialMapBounds,
      this.initialMapBoundsPadding,
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
  InAppWebViewController? _webViewController;
  late GpsManager _gpsManager;
  bool _readyForDisplay = false;

  late MapStyle _selectedMapStyle;

  // Dev server URL for loading map webview from a local dev server.
  // Usage: flutter run --dart-define=DEV_SERVER=http://ip:port
  static const _devServer = String.fromEnvironment('DEV_SERVER');

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  // For bug workaround
  bool _isiOS18 = false;

  // Low Power Mode tracking
  final Battery _battery = Battery();
  bool _isLowPowerMode = false;
  StreamSubscription<BatteryState>? _batteryStateSubscription;

  Future<void> runJavaScript(String javaScript) async {
    await _webViewController?.evaluateJavascript(source: javaScript);
  }

  void _setStateIfMounted(VoidCallback fn) {
    if (!mounted) return;
    setState(fn);
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
    if (!mounted) return;
    log.info('[base_map_webview] Refreshing map data');
    await _webViewController?.evaluateJavascript(source: '''
      if (typeof refreshMapData === 'function') {
        console.log('Refreshing map data');
        refreshMapData();
      } else {
        console.warn('refreshMapData function not available yet');
      }
    ''');
  }

  Future<void> manualRefresh() async {
    await _refreshMapData();
  }

  @override
  void initState() {
    super.initState();
    _gpsManager = Provider.of<GpsManager>(context, listen: false);
    _gpsManager.addListener(_updateLocationMarker);
    _currentRoughMapView = widget.initialMapView;
    _selectedMapStyle = _loadMapStyleFromStorage();

    () async {
      if (Platform.isIOS) {
        DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
        var iosInfo = await deviceInfo.iosInfo;
        if (iosInfo.systemVersion.startsWith('18.')) {
          _setStateIfMounted(() {
            _isiOS18 = true;
          });
        }
      }
    }();

    _initLowPowerMode();
  }

  Future<void> _initLowPowerMode() async {
    try {
      _isLowPowerMode = await _battery.isInBatterySaveMode;
    } catch (e) {
      log.error('[base_map_webview] Failed to query battery save mode: $e');
    }

    if (!mounted) return;
    _batteryStateSubscription =
        _battery.onBatteryStateChanged.listen((_) async {
      try {
        final newLPM = await _battery.isInBatterySaveMode;
        if (!mounted) return;
        if (newLPM != _isLowPowerMode) {
          _isLowPowerMode = newLPM;
          _pushLowPowerModeToWebView();
        }
      } catch (e) {
        log.error('[base_map_webview] Failed to query battery save mode: $e');
      }
    }, onError: (Object error) {
      // Charging status is only used as a signal to refresh low-power mode.
      // Some devices cannot provide it temporarily, so keep the stream alive
      // and retain the last known low-power-mode value.
      log.warning(
        '[base_map_webview] Battery state unavailable; '
        'keeping the last known low-power mode: $error',
      );
    });
  }

  void _pushLowPowerModeToWebView() {
    if (!mounted) return;
    _webViewController?.evaluateJavascript(source: '''
      if (typeof window.setLowPowerMode === 'function') {
        window.setLowPowerMode($_isLowPowerMode);
      }
    ''');
  }

  @override
  void dispose() {
    _batteryStateSubscription?.cancel();
    _gpsManager.removeListener(_updateLocationMarker);
    _webViewController = null;
    super.dispose();
  }

  void _updateLocationMarker() {
    if (!mounted) return;
    if (widget.trackingMode == TrackingMode.off) {
      _webViewController?.evaluateJavascript(source: '''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(0, 0, false);
        }
      ''');
    } else {
      // Prefer the live position; fall back to the OS-cached last known
      // location so the marker shows up immediately on cold start while the
      // GPS stream is still acquiring its first fix.
      final position =
          _gpsManager.latestPosition ?? _gpsManager.lastKnownPosition;
      if (position != null) {
        _webViewController?.evaluateJavascript(source: '''
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

  Future<void> _onWebViewCreated(InAppWebViewController controller) async {
    if (!mounted) return;
    _webViewController = controller;

    // Add web message listeners (JS channels) before loading the page.
    // These create window.channelName.postMessage(str) objects on the JS side,
    // matching the calling convention of webview_flutter's addJavaScriptChannel.
    await Future.wait([
      controller.addWebMessageListener(WebMessageListener(
        jsObjectName: 'onMapMoved',
        allowedOriginRules: {'*'},
        onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
          if (!mounted) return;
          widget.onMapMoved?.call();
        },
      )),
      controller.addWebMessageListener(WebMessageListener(
        jsObjectName: 'readyForDisplay',
        allowedOriginRules: {'*'},
        onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
          _setStateIfMounted(() {
            _readyForDisplay = true;
          });
        },
      )),
      controller.addWebMessageListener(WebMessageListener(
        jsObjectName: 'onMapViewChanged',
        allowedOriginRules: {'*'},
        onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
          if (!mounted) return;
          final data = message?.data;
          if (data is String) {
            _handleMapViewPush(data);
          }
        },
      )),
      controller.addWebMessageListener(WebMessageListener(
        jsObjectName: 'onMapZoomChanged',
        allowedOriginRules: {'*'},
        onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
          if (!mounted) return;
          final data = message?.data;
          if (data is String) {
            _handleMapZoomPush(data);
          }
        },
      )),
      for (final channel in widget.extraJavaScriptChannels)
        controller.addWebMessageListener(WebMessageListener(
          jsObjectName: channel.name,
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted) return;
            final data = message?.data;
            channel.onMessageReceived(data is String ? data : data.toString());
          },
        )),
    ]);

    if (!mounted) return;

    // Load the page after listeners are registered
    if (_devServer.isNotEmpty) {
      final devUrl = _devServer.endsWith('/')
          ? '${_devServer}index.html'
          : '$_devServer/index.html';
      log.info('[base_map_webview] Loading from dev server: $devUrl');
      await controller.loadUrl(urlRequest: URLRequest(url: WebUri(devUrl)));
    } else {
      final assetPath = 'assets/map_webview/index.html';
      log.info('[base_map_webview] Loading asset: $assetPath');
      await controller.loadFile(assetFilePath: assetPath);
    }
  }

  Future<void> _injectApiEndpoint() async {
    if (!mounted) return;
    final controller = _webViewController;
    if (controller == null) return;

    final accessKey = api.getMapboxAccessToken();

    final mapView = _currentRoughMapView;
    final lngParam = mapView?.lng.toString() ?? 'null';
    final latParam = mapView?.lat.toString() ?? 'null';
    final zoomParam = mapView?.zoom.toString() ?? 'null';
    final bounds = widget.initialMapBounds;
    final boundsPadding =
        widget.initialMapBoundsPadding ?? const EdgeInsets.all(24);

    debugPrint('Injecting lng: $lngParam');
    debugPrint('Injecting lat: $latParam');
    debugPrint('Injecting zoom: $zoomParam');

    final cgiEndpoint =
        Platform.isIOS ? 'memolanes://api' : 'https://memolanes.local/api';

    final style = _selectedMapStyle;
    await controller.evaluateJavascript(source: '''
      // Set the params
      window.EXTERNAL_PARAMS = {
        cgi_endpoint: "$cgiEndpoint",
        render: "canvas",
        map_style: "${style.url}",
        fog_density: ${style.fogOpacity},
        access_key: ${accessKey != null ? "\"$accessKey\"" : "null"},
        lng: $lngParam,
        lat: $latParam,
        zoom: $zoomParam,
        west: ${bounds?.west ?? 'null'},
        south: ${bounds?.south ?? 'null'},
        east: ${bounds?.east ?? 'null'},
        north: ${bounds?.north ?? 'null'},
        fit_padding_top: ${boundsPadding.top},
        fit_padding_right: ${boundsPadding.right},
        fit_padding_bottom: ${boundsPadding.bottom},
        fit_padding_left: ${boundsPadding.left},
        editor: ${widget.isEditor ? "true" : "false"},
        low_power_mode: "$_isLowPowerMode",
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

  MapStyle _loadMapStyleFromStorage() {
    final id = MMKVUtil.getString(MMKVKey.mapStyle);
    return MapStyle.findById(id);
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

  /// Handle an intercepted request by forwarding path + query to the unified Rust dispatcher.
  Future<
      ({
        int status,
        Uint8List body,
        String contentType,
        Map<String, String> headers
      })> _handleInterceptedRequest(WebUri url) async {
    final path = url.path.replaceFirst(RegExp(r'^/?(api/)?'), '');
    final result = await widget.mapRendererProxy.handleRequest(
      path: path,
      queryParams: url.queryParameters,
    );
    return (
      status: result.status,
      body: result.body,
      contentType: result.contentType,
      headers: result.headers,
    );
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
            child: InAppWebView(
              key: const ValueKey('map_webview'),
              initialSettings: InAppWebViewSettings(
                javaScriptEnabled: true,
                useShouldOverrideUrlLoading: true,
                allowFileAccessFromFileURLs: true,
                allowUniversalAccessFromFileURLs: true,
                resourceCustomSchemes: ['memolanes'],
              ),
              onWebViewCreated: (controller) {
                _onWebViewCreated(controller);
              },
              // iOS: intercept custom scheme requests (memolanes://)
              onLoadResourceWithCustomScheme: (controller, request) async {
                if (!mounted) return null;
                final result = await _handleInterceptedRequest(request.url);
                return CustomSchemeResponse(
                  data: result.body,
                  contentType: result.contentType,
                  contentEncoding: 'utf-8',
                  statusCode: result.status,
                  headers: {
                    'Content-Type': result.contentType,
                    ...result.headers,
                  },
                );
              },
              // Android: intercept URL pattern requests (https://memolanes.local/api/)
              shouldInterceptRequest: (controller, request) async {
                if (!mounted) return null;
                final url = request.url.toString();
                if (!url.startsWith('https://memolanes.local/api/')) {
                  return null;
                }
                final result = await _handleInterceptedRequest(request.url);
                return WebResourceResponse(
                  contentType: result.contentType,
                  contentEncoding: 'utf-8',
                  data: result.body,
                  statusCode: result.status,
                  reasonPhrase: result.status == 200 ? 'OK' : 'Not Modified',
                  headers: {
                    'Access-Control-Allow-Origin': '*',
                    'Access-Control-Expose-Headers':
                        'X-Tile-Version, X-Not-Modified',
                    'Content-Type': result.contentType,
                    ...result.headers,
                  },
                );
              },
              shouldOverrideUrlLoading: (controller, navigationAction) async {
                if (!mounted) {
                  return NavigationActionPolicy.CANCEL;
                }
                final url = navigationAction.request.url;
                if (url == null) {
                  return NavigationActionPolicy.CANCEL;
                }
                final scheme = url.scheme;
                if (scheme == 'file' || scheme == 'about') {
                  return NavigationActionPolicy.ALLOW;
                }
                if (_devServer.isNotEmpty &&
                    url.toString().startsWith(_devServer)) {
                  return NavigationActionPolicy.ALLOW;
                }
                launchUrl(
                  Uri.parse(url.toString()),
                  mode: LaunchMode.externalApplication,
                );
                return NavigationActionPolicy.CANCEL;
              },
              onLoadStop: (controller, url) {
                if (!mounted) return;
                debugPrint('Page finished loading: $url');
                _injectApiEndpoint();
              },
              onReceivedError: (controller, request, error) async {
                if (!mounted) return;
                final failedUrl = request.url.toString();
                if (!failedUrl.contains('events.mapbox.com')) {
                  log.error('''Map WebView Error: 
                      Description: ${error.description}
                      Error Type: ${error.type} 
                      Failed URL: $failedUrl''');
                }
              },
              onWebContentProcessDidTerminate: (controller) async {
                if (!mounted) return;
                await controller.reload();
              },
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
