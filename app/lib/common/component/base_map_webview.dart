import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/map_fog_style.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/common/map_webview_resources.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/app_layout.dart';
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

enum TrackingMode { displayAndTracking, displayOnly, off }

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final MapView? initialMapView;
  final MapBounds? initialMapBounds;
  final EdgeInsets? boundsPadding;

  /// Reuse the existing panel's current footprint when fitting map bounds.
  final GlobalKey<MapOverlayPanelState>? overlayPanel;
  final MapBounds? flyToBounds;
  final MapView? flyToView;
  final TrackingMode trackingMode;
  final bool isEditor;
  final void Function()? onMapMoved;
  final void Function(MapView)? onRoughMapViewUpdate;
  final void Function(int)? onMapZoomChanged;
  final List<BaseMapJavaScriptChannel> extraJavaScriptChannels;

  const BaseMapWebview({
    super.key,
    required this.mapRendererProxy,
    this.initialMapView,
    this.initialMapBounds,
    this.boundsPadding,
    this.overlayPanel,
    this.flyToBounds,
    this.flyToView,
    this.trackingMode = TrackingMode.off,
    this.isEditor = false,
    this.onMapMoved,
    this.onRoughMapViewUpdate,
    this.onMapZoomChanged,
    this.extraJavaScriptChannels = const [],
  });

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  static const _webGlRecoveryWindow = Duration(minutes: 1);
  static const _maxWebGlRecoveryReloads = 2;

  InAppWebViewController? _webViewController;
  late GpsManager _gpsManager;
  bool _readyForDisplay = false;
  bool _webGlRecoveryReloadInProgress = false;
  bool _pendingMapDataSync = false;
  bool _mapSyncScheduled = false;
  AppLayout? _mapLayout;

  EdgeInsets get _boundsPadding =>
      widget.overlayPanel?.currentState?.mapFitPadding ??
      widget.boundsPadding ??
      const EdgeInsets.all(24);

  MapBounds? get _cameraBounds =>
      widget.flyToBounds ??
      (widget.overlayPanel != null ? widget.initialMapBounds : null);

  // Bounds and panel measurements must belong to the same completed layout.
  // Coalesce a target change and a physical viewport change into one move.
  void _scheduleMapSync() {
    if (_mapSyncScheduled) return;
    _mapSyncScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _mapSyncScheduled = false;
      if (!mounted) return;
      unawaited(_syncMapDataAndCamera());
    });
  }

  final List<DateTime> _webGlRecoveryReloads = [];

  late MapStyle _selectedMapStyle;
  int _basemapRequestId = 0;
  Map<String, Object> get _basemapConfig => {
    'id': _selectedMapStyle.id,
    'style': _selectedMapStyle.mapLibreStyle,
    'requestId': _basemapRequestId,
  };
  late MapFogStyle _selectedMapFogStyle;

  // Dev server URL for loading map webview from a local dev server.
  // Usage: flutter run --dart-define=DEV_SERVER=http://ip:port
  static const _devServer = String.fromEnvironment('DEV_SERVER');

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  Future<void> runJavaScript(String javaScript) async {
    await _webViewController?.evaluateJavascript(source: javaScript);
  }

  /// Read the live camera before opening a journey, including recent gestures
  /// that have not reached the throttled map-view callback yet.
  Future<MapView?> getCurrentMapView() async {
    final controller = _webViewController;
    if (controller == null || !_readyForDisplay) return _currentRoughMapView;
    try {
      final result = await controller.evaluateJavascript(
        source: 'getCurrentMapView()',
      );
      final view = jsonDecode(result as String);
      return (
        lng: (view['lng'] as num).toDouble(),
        lat: (view['lat'] as num).toDouble(),
        zoom: (view['zoom'] as num).toDouble(),
      );
    } catch (error, stackTrace) {
      log.error(
        '[base_map_webview] Reading map view failed: $error',
        stackTrace,
      );
      return _currentRoughMapView;
    }
  }

  void _setStateIfMounted(VoidCallback fn) {
    if (!mounted) return;
    setState(fn);
  }

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.trackingMode != widget.trackingMode) {
      if (widget.trackingMode == TrackingMode.off) {
        _hideLocationMarker();
      } else {
        _updateLocationMarker();
      }
    }

    if (oldWidget.mapRendererProxy != widget.mapRendererProxy ||
        oldWidget.flyToBounds != widget.flyToBounds ||
        (widget.overlayPanel != null &&
            oldWidget.initialMapBounds != widget.initialMapBounds) ||
        oldWidget.flyToView != widget.flyToView ||
        oldWidget.boundsPadding != widget.boundsPadding ||
        oldWidget.overlayPanel != widget.overlayPanel) {
      _scheduleMapSync();
    }
  }

  /// Switch the renderer data and move the existing MapLibre map to a journey.
  /// The WebView and its WebGL context stay mounted throughout the transition.
  Future<void> _syncMapDataAndCamera() async {
    final controller = _webViewController;
    if (!mounted || controller == null || !_readyForDisplay) {
      _pendingMapDataSync = true;
      return;
    }
    _pendingMapDataSync = false;
    final bounds = _cameraBounds;
    final view = widget.flyToView;
    final padding = _boundsPadding;
    final boundsJson = bounds == null
        ? 'null'
        : jsonEncode({
            'west': bounds.west,
            'south': bounds.south,
            'east': bounds.east,
            'north': bounds.north,
          });
    final paddingJson = jsonEncode({
      'top': padding.top,
      'right': padding.right,
      'bottom': padding.bottom,
      'left': padding.left,
    });
    final viewJson = view == null
        ? 'null'
        : jsonEncode({'lng': view.lng, 'lat': view.lat, 'zoom': view.zoom});
    try {
      await controller.evaluateJavascript(
        source:
            '''
        if (typeof refreshMapData === 'function') refreshMapData();
        if ($boundsJson !== null && typeof flyToBounds === 'function') {
          flyToBounds($boundsJson, $paddingJson);
        } else if ($viewJson !== null && typeof flyToView === 'function') {
          flyToView($viewJson);
        }
      ''',
      );
    } catch (error, stackTrace) {
      log.error('[base_map_webview] Map data sync failed: $error', stackTrace);
    }
  }

  /// Request the WebView to refresh map data from the backend
  Future<void> _refreshMapData() async {
    if (!mounted) return;
    log.info('[base_map_webview] Refreshing map data');
    await _webViewController?.evaluateJavascript(
      source: '''
      if (typeof refreshMapData === 'function') {
        console.log('Refreshing map data');
        refreshMapData();
      } else {
        console.warn('refreshMapData function not available yet');
      }
    ''',
    );
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
    MapStyleStore.instance.addListener(_onMapStyleChanged);
    _selectedMapFogStyle = _loadMapFogStyleFromStorage();
  }

  @override
  void dispose() {
    MapStyleStore.instance.removeListener(_onMapStyleChanged);
    _gpsManager.removeListener(_updateLocationMarker);
    _webViewController = null;
    super.dispose();
  }

  void _onMapStyleChanged() {
    final style = MapStyleStore.instance.selected;
    final changed = !_selectedMapStyle.hasSameRenderer(style);
    _selectedMapStyle = style;
    if (!changed) return;
    _basemapRequestId++;
    final controller = _webViewController;
    if (controller == null || _webGlRecoveryReloadInProgress) return;
    unawaited(
      controller.evaluateJavascript(
        source:
            '''
      (() => {
        const config = ${jsonEncode(_basemapConfig)};
        window.EXTERNAL_PARAMS = {...window.EXTERNAL_PARAMS, basemap: config};
        if (typeof window.updateBasemap === 'function') {
          window.updateBasemap(config);
        }
      })();
    ''',
      ),
    );
  }

  void _updateLocationMarker() {
    if (!mounted || widget.trackingMode == TrackingMode.off) return;

    // Prefer the live position; fall back to the OS-cached last known
    // location so the marker shows up immediately on cold start while the
    // GPS stream is still acquiring its first fix.
    final position =
        _gpsManager.latestPosition ?? _gpsManager.lastKnownPosition;
    final controller = _webViewController;
    if (position != null && controller != null) {
      unawaited(
        controller.evaluateJavascript(
          source:
              '''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(
            ${position.longitude}, 
            ${position.latitude}, 
            true, 
            ${widget.trackingMode == TrackingMode.displayAndTracking}
          );
        }
      ''',
        ),
      );
    } else if (position == null) {
      _hideLocationMarker();
    }
  }

  void _hideLocationMarker() {
    final controller = _webViewController;
    if (controller == null) return;
    unawaited(
      controller.evaluateJavascript(
        source: '''
        if (typeof updateLocationMarker === 'function') {
          updateLocationMarker(0, 0, false);
        }
      ''',
      ),
    );
  }

  Future<void> _onWebViewCreated(InAppWebViewController controller) async {
    if (!mounted) return;
    _webViewController = controller;

    // Add web message listeners (JS channels) before loading the page.
    // These create window.channelName.postMessage(str) objects on the JS side,
    // matching the calling convention of webview_flutter's addJavaScriptChannel.
    await Future.wait([
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onBasemapLoadFailed',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || !isMainFrame || !_selectedMapStyle.isCustom) return;
            final data = message?.data;
            if (data is! String) return;
            try {
              final failure = jsonDecode(data);
              if (failure is! Map ||
                  failure['requestId'] != _basemapRequestId) {
                return;
              }
              // Saving notifies every mounted map. They update their existing
              // MapLibre instance, including this map, without a page reload.
              if (!MapStyleStore.instance.recordFallback(_selectedMapStyle)) {
                return;
              }
              Fluttertoast.showToast(
                msg: context.tr(
                  'general.map_settings.basemaps.fallback_message',
                ),
              );
            } on FormatException {
              // Ignore stale or malformed messages.
            } on StateError {
              Fluttertoast.showToast(
                msg: context.tr(
                  'general.map_settings.basemaps.fallback_save_failed',
                ),
              );
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapAttributionChanged',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || !isMainFrame) return;
            final data = message?.data;
            if (data is! String) return;
            try {
              final metadata = jsonDecode(data);
              if (metadata is! Map ||
                  metadata['requestId'] != _basemapRequestId) {
                return;
              }
              final entries = metadata['entries'];
              if (entries is List &&
                  entries.every((entry) => entry is String)) {
                MapStyleStore.instance.recordSourceAttributions(
                  _selectedMapStyle,
                  entries.cast<String>(),
                );
              }
            } on FormatException {
              // Ignore malformed metadata without affecting the map.
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapMoved',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted) return;
            widget.onMapMoved?.call();
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'readyForDisplay',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            final recoveredAfterReload = _webGlRecoveryReloadInProgress;
            _webGlRecoveryReloadInProgress = false;
            _updateLocationMarker();
            _setStateIfMounted(() {
              _readyForDisplay = true;
            });
            if (_pendingMapDataSync) unawaited(_syncMapDataAndCamera());
            if (recoveredAfterReload) {
              log.info(
                '[base_map_webview] WebGL context recovered after WebView reload',
              );
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onWebGLContextEvent',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted) return;
            final data = message?.data;
            _handleWebGlContextEvent(data is String ? data : data.toString());
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapViewChanged',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted) return;
            final data = message?.data;
            if (data is String) {
              _handleMapViewPush(data);
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapZoomChanged',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted) return;
            final data = message?.data;
            if (data is String) {
              _handleMapZoomPush(data);
            }
          },
        ),
      ),
      for (final channel in widget.extraJavaScriptChannels)
        controller.addWebMessageListener(
          WebMessageListener(
            jsObjectName: channel.name,
            allowedOriginRules: {'*'},
            onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
              if (!mounted) return;
              final data = message?.data;
              channel.onMessageReceived(
                data is String ? data : data.toString(),
              );
            },
          ),
        ),
    ]);

    if (!mounted) return;

    // Load the page after listeners are registered. Platform-specific asset
    // mechanics are isolated in MapWebViewResources.
    if (_devServer.isNotEmpty) {
      final pageUrl = _devServer.endsWith('/')
          ? '${_devServer}index.html'
          : '$_devServer/index.html';
      log.info('[base_map_webview] Loading map page: $pageUrl');
      await controller.loadUrl(urlRequest: URLRequest(url: WebUri(pageUrl)));
    } else {
      log.info('[base_map_webview] Loading bundled map page');
      await MapWebViewResources.load(controller, 'index.html');
    }
  }

  Future<void> _injectApiEndpoint() async {
    if (!mounted) return;
    final controller = _webViewController;
    if (controller == null) return;

    final accessKey = api.getMapboxAccessToken();

    final mapView = _currentRoughMapView;
    final bounds = widget.flyToBounds ?? widget.initialMapBounds;
    final boundsPadding = _boundsPadding;

    final style = _selectedMapStyle;
    _basemapRequestId++;
    final fogStyle = _selectedMapFogStyle;
    final params = <String, Object?>{
      'cgi_endpoint': MapWebViewResources.apiEndpoint,
      'render': 'canvas',
      'basemap': _basemapConfig,
      'fog_style': fogStyle.id,
      'fog_density': style.fogOpacityByStyle[fogStyle] ?? 0.50,
      'access_key': accessKey,
      'lng': mapView?.lng,
      'lat': mapView?.lat,
      'zoom': mapView?.zoom,
      'west': bounds?.west,
      'south': bounds?.south,
      'east': bounds?.east,
      'north': bounds?.north,
      'fit_padding_top': boundsPadding.top,
      'fit_padding_right': boundsPadding.right,
      'fit_padding_bottom': boundsPadding.bottom,
      'fit_padding_left': boundsPadding.left,
      'editor': widget.isEditor,
    };
    await controller.evaluateJavascript(
      source:
          '''
      window.EXTERNAL_PARAMS = ${jsonEncode(params)};
      
      // Check if JS is ready and trigger initialization if so
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

    debugPrint('Initialization completed');
  }

  MapStyle _loadMapStyleFromStorage() => MapStyleStore.instance.selected;

  MapFogStyle _loadMapFogStyleFromStorage() {
    final id = MMKVUtil.getStringOpt(MMKVKey.mapFogMode);
    return MapFogStyle.findById(id);
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

  void _handleWebGlContextEvent(String message) {
    try {
      final decoded = jsonDecode(message);
      if (decoded is! Map<String, dynamic>) {
        throw const FormatException('expected a JSON object');
      }

      final state = decoded['state'];
      if (state == 'lost') {
        log.warning('[base_map_webview] WebGL context lost: $message');

        final view = decoded['view'];
        if (view is Map<String, dynamic>) {
          // Preserve the camera at the point of failure so a fallback reload
          // comes back at the same location.
          _handleMapViewPush(jsonEncode(view));
        }

        unawaited(_reloadAfterWebGlContextLoss());
        return;
      }

      if (state == 'restored') {
        log.info('[base_map_webview] WebGL context restored: $message');
        return;
      }

      log.warning('[base_map_webview] Unknown WebGL context event: $message');
    } catch (e) {
      log.error(
        '[base_map_webview] Invalid WebGL context event: $message, error=$e',
      );
    }
  }

  Future<void> _reloadAfterWebGlContextLoss() async {
    if (!mounted || _webGlRecoveryReloadInProgress) return;

    final now = DateTime.now();
    _webGlRecoveryReloads.removeWhere(
      (attempt) => now.difference(attempt) >= _webGlRecoveryWindow,
    );

    if (_webGlRecoveryReloads.length >= _maxWebGlRecoveryReloads) {
      log.error(
        '[base_map_webview] WebGL context recovery reload suppressed: '
        'already attempted $_maxWebGlRecoveryReloads times within '
        '${_webGlRecoveryWindow.inSeconds}s',
      );
      return;
    }

    final controller = _webViewController;
    if (controller == null) {
      log.error(
        '[base_map_webview] Cannot recover WebGL context: WebView unavailable',
      );
      return;
    }

    _webGlRecoveryReloads.add(now);
    _webGlRecoveryReloadInProgress = true;
    _setStateIfMounted(() {
      _readyForDisplay = false;
    });
    log.warning(
      '[base_map_webview] Reloading WebView after WebGL context loss '
      '(attempt ${_webGlRecoveryReloads.length}/$_maxWebGlRecoveryReloads)',
    );

    try {
      await controller.reload();
    } catch (e) {
      _webGlRecoveryReloadInProgress = false;
      log.error('[base_map_webview] WebGL recovery reload failed: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.overlayPanel != null) {
      final layout = AppLayoutScope.of(context);
      if (_mapLayout case final previous?) {
        if (_cameraBounds != null && !layout.hasSameMapViewport(previous)) {
          _scheduleMapSync();
        }
      }
      _mapLayout = layout;
    }
    return Stack(
      children: [
        InAppWebView(
          key: const ValueKey('map_webview'),
          initialSettings: InAppWebViewSettings(
            topScrollEdgeEffectHidden: true,
            javaScriptEnabled: true,
            useShouldOverrideUrlLoading: true,
            allowFileAccessFromFileURLs: true,
            allowUniversalAccessFromFileURLs: true,
            resourceCustomSchemes: ['memolanes'],
            // Skip the Android → Dart round trip for ordinary map resources.
            shouldInterceptRequestUrlPrefixes: [
              MapWebViewResources.androidApiRequestPrefix,
            ],
            webViewAssetLoader: MapWebViewResources.createAssetLoader(),
          ),
          onWebViewCreated: (controller) {
            _onWebViewCreated(controller);
          },
          onContentSizeChanged: (controller, oldSize, newSize) {
            // Native WebView resizing can finish after Flutter's layout frame.
            // Fit again once the map can use the new content dimensions.
            if (oldSize != newSize &&
                _readyForDisplay &&
                _cameraBounds != null) {
              _scheduleMapSync();
              WidgetsBinding.instance.ensureVisualUpdate();
            }
          },
          // iOS: intercept custom scheme requests (memolanes://)
          onLoadResourceWithCustomScheme: (controller, request) async {
            if (!mounted) return null;
            return MapWebViewResources.loadApiResource(
              widget.mapRendererProxy,
              request.url,
            );
          },
          // Android: intercept URL pattern requests (https://memolanes.local/api/)
          shouldInterceptRequest: (controller, request) async {
            if (!mounted) return null;
            return MapWebViewResources.interceptApiRequest(
              widget.mapRendererProxy,
              request.url,
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
            if (scheme == 'about') {
              return NavigationActionPolicy.ALLOW;
            }
            if (MapWebViewResources.owns(url)) {
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
            _webGlRecoveryReloadInProgress = true;
            _setStateIfMounted(() {
              _readyForDisplay = false;
            });
            log.warning(
              '[base_map_webview] Web content process terminated; reloading WebView',
            );
            await controller.reload();
          },
        ),
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
