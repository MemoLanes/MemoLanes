import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:memolanes/common/map_webview_resources.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/app_layout.dart';
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
  final MapStyleStore? styleStore;
  final MapView? initialMapView;
  final MapBounds? initialMapBounds;
  final EdgeInsets? boundsPadding;
  final ValueChanged<bool>? onMapAttributionChanged;

  /// Reuse the existing panel's current footprint when fitting map bounds.
  final GlobalKey<MapOverlayPanelState>? overlayPanel;
  final MapBounds? flyToBounds;
  final MapView? flyToView;

  /// Captured home camera for an atomic shared-map return, not an ordinary fit.
  final MapView? returnMapView;
  final VoidCallback? onMapReturnCompleted;

  /// Use explicit camera requests for the single-region overlay. A changed
  /// revision can refit equal bounds; size and drawing updates never refit.
  final int? cameraRevision;

  /// Monotonically increasing lease across shared-map camera handoffs.
  final int cameraOwner;

  /// The selected Achievement feature, or null to clear it.
  final String? achievementRegionGeoJson;
  final TrackingMode trackingMode;
  final bool isEditor;
  final void Function()? onMapMoved;
  final VoidCallback? onMapTap;
  final void Function(MapView)? onRoughMapViewUpdate;
  final void Function(int)? onMapZoomChanged;
  final List<BaseMapJavaScriptChannel> extraJavaScriptChannels;

  const BaseMapWebview({
    super.key,
    required this.mapRendererProxy,
    this.styleStore,
    this.initialMapView,
    this.initialMapBounds,
    this.boundsPadding,
    this.onMapAttributionChanged,
    this.overlayPanel,
    this.flyToBounds,
    this.flyToView,
    this.returnMapView,
    this.onMapReturnCompleted,
    this.cameraRevision,
    this.cameraOwner = 0,
    this.achievementRegionGeoJson,
    this.trackingMode = TrackingMode.off,
    this.isEditor = false,
    this.onMapMoved,
    this.onMapTap,
    this.onRoughMapViewUpdate,
    this.onMapZoomChanged,
    this.extraJavaScriptChannels = const [],
  });

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  static const _returnAckTimeout = Duration(seconds: 2);
  static const _maxReturnAttempts = 2;
  static const _webGlRecoveryWindow = Duration(minutes: 1);
  static const _maxWebGlRecoveryReloads = 2;

  InAppWebViewController? _webViewController;
  late GpsManager _gpsManager;
  bool _readyForDisplay = false;
  bool _readyForCommands = false;
  bool _webGlRecoveryReloadInProgress = false;
  bool _pendingMapDataSync = false;
  bool _mapSyncScheduled = false;
  AppLayout? _mapLayout;
  // A document reload restores the drawing without refitting an acknowledged camera.
  int? _confirmedCameraRevision;
  ({int owner, int revision, String? feature})? _requestedAchievementTarget;
  int _mapDocument = 0;
  int _achievementCommandGeneration = 0;
  int _requestedCameraOwner = 0;
  bool _awaitingReturn = false;
  MapView? _pendingReturnView;
  ({int document, int owner})? _returnCommandInFlight;
  bool _normalCameraQueuedDuringReturn = false;
  int _returnAttempts = 0;
  int _mapTapLease = 0;

  bool get _usesExplicitCamera => widget.cameraRevision != null;

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

  MapStyleStore get _styleStore => widget.styleStore ?? MapStyleStore.instance;
  String? _requestedMapAppearance;
  bool _mapAppearanceSyncScheduled = false;

  // Dev server URL for loading map webview from a local dev server.
  // Usage: flutter run --dart-define=DEV_SERVER=http://ip:port
  static const _devServer = String.fromEnvironment('DEV_SERVER');

  // It is rough because we don't update it frequently.
  MapView? _currentRoughMapView;

  Future<void> runJavaScript(String javaScript) async {
    await _webViewController?.evaluateJavascript(source: javaScript);
  }

  /// Export only the map canvas, without Flutter controls or system chrome.
  Future<({Uint8List png, Size logicalSize})> captureScreenshot() async {
    final controller = _webViewController;
    if (controller == null || !_readyForDisplay) {
      throw StateError('Map is not ready for a screenshot');
    }
    final box = context.findRenderObject();
    if (box is! RenderBox || !box.hasSize) {
      throw StateError('Map layout is unavailable');
    }
    final logicalSize = box.size;
    final result = await controller
        .callAsyncJavaScript(
          functionBody: 'return await window.captureMapScreenshot();',
        )
        .timeout(const Duration(seconds: 20));
    final value = result?.value;
    const prefix = 'data:image/png;base64,';
    if (result?.error != null ||
        value is! String ||
        !value.startsWith(prefix)) {
      throw StateError('Map screenshot failed: ${result?.error}');
    }
    // The map can still resize during a fold or rotation.
    if (!box.attached || box.size != logicalSize) {
      throw StateError('Map resized during screenshot');
    }
    return (
      png: base64Decode(value.substring(prefix.length)),
      logicalSize: logicalSize,
    );
  }

  Future<String?> getMapAttribution() async {
    final controller = _webViewController;
    if (controller == null ||
        !(_readyForDisplay || (_usesExplicitCamera && _readyForCommands))) {
      return null;
    }
    try {
      final result = await controller.evaluateJavascript(
        source: 'window.getMapAttribution?.() ?? null',
      );
      return result as String?;
    } catch (error, stackTrace) {
      log.error(
        '[base_map_webview] Reading map attribution failed: $error',
        stackTrace,
      );
      return null;
    }
  }

  /// Read the live camera before opening a journey, including recent gestures
  /// that have not reached the throttled map-view callback yet.
  Future<MapView?> getCurrentMapView() async {
    if (_awaitingReturn) return _pendingReturnView;
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
    if (oldWidget.styleStore != widget.styleStore) {
      (oldWidget.styleStore ?? MapStyleStore.instance).removeListener(
        _scheduleMapAppearanceSync,
      );
      _styleStore.addListener(_scheduleMapAppearanceSync);
      _scheduleMapAppearanceSync();
    }
    if (oldWidget.cameraOwner != widget.cameraOwner) {
      _confirmedCameraRevision = null;
      ++_achievementCommandGeneration;
      _returnAttempts = 0;
      _returnCommandInFlight = null;
    }
    if (!_usesExplicitCamera &&
        (oldWidget.cameraRevision != null ||
            widget.returnMapView != oldWidget.returnMapView &&
                widget.returnMapView != null)) {
      _awaitingReturn = true;
      _pendingReturnView = widget.returnMapView ?? widget.flyToView;
      _returnAttempts = 0;
      _returnCommandInFlight = null;
      ++_achievementCommandGeneration;
    } else if (_usesExplicitCamera) {
      _awaitingReturn = false;
      _pendingReturnView = null;
      _returnCommandInFlight = null;
      _normalCameraQueuedDuringReturn = false;
    }
    if (_awaitingReturn &&
        oldWidget.cameraRevision == null &&
        (oldWidget.flyToBounds != widget.flyToBounds ||
            oldWidget.flyToView != widget.flyToView ||
            oldWidget.boundsPadding != widget.boundsPadding ||
            oldWidget.overlayPanel != widget.overlayPanel ||
            oldWidget.initialMapBounds != widget.initialMapBounds)) {
      _normalCameraQueuedDuringReturn = true;
    }
    if (oldWidget.cameraOwner != widget.cameraOwner ||
        (oldWidget.onMapTap != null) != (widget.onMapTap != null)) {
      _mapTapLease++;
      unawaited(_syncMapTapListener());
    }
    if (oldWidget.trackingMode != widget.trackingMode) {
      if (widget.trackingMode == TrackingMode.off) {
        _hideLocationMarker();
      } else {
        _updateLocationMarker();
      }
    }

    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _pendingMapDataSync = true;
      _scheduleMapSync();
    }
    if (_usesExplicitCamera) {
      if (oldWidget.cameraOwner != widget.cameraOwner ||
          oldWidget.cameraRevision != widget.cameraRevision ||
          oldWidget.achievementRegionGeoJson !=
              widget.achievementRegionGeoJson) {
        _scheduleMapSync();
      }
    } else if (oldWidget.cameraRevision != widget.cameraRevision ||
        oldWidget.cameraOwner != widget.cameraOwner ||
        (widget.returnMapView != null &&
            oldWidget.returnMapView != widget.returnMapView) ||
        oldWidget.mapRendererProxy != widget.mapRendererProxy ||
        oldWidget.flyToBounds != widget.flyToBounds ||
        (widget.overlayPanel != null &&
            oldWidget.initialMapBounds != widget.initialMapBounds) ||
        oldWidget.flyToView != widget.flyToView ||
        oldWidget.boundsPadding != widget.boundsPadding ||
        oldWidget.overlayPanel != widget.overlayPanel) {
      _scheduleMapSync();
    }
  }

  Future<void> _syncMapTapListener() async {
    if (!mounted || !_readyForDisplay) return;
    try {
      await runJavaScript(
        'window.setMapTapEnabled?.($_mapDocument, $_mapTapLease, ${widget.onMapTap != null});',
      );
    } catch (error, stackTrace) {
      log.error(
        '[base_map_webview] Updating map tap listener failed: $error',
        stackTrace,
      );
    }
  }

  /// Switch the renderer data and move the existing MapLibre map to a journey.
  /// The WebView and its WebGL context stay mounted throughout the transition.
  Future<void> _syncMapDataAndCamera() async {
    final controller = _webViewController;
    if (!mounted ||
        controller == null ||
        !(_readyForDisplay || _readyForCommands)) {
      if (!_usesExplicitCamera) _pendingMapDataSync = true;
      return;
    }
    final owner = widget.cameraOwner;
    final syncDocument = _mapDocument;
    final refreshData = _pendingMapDataSync;
    _pendingMapDataSync = false;
    // A queued normal fit reuses the renderer refresh issued during return.
    final queuedNormalCamera =
        !_awaitingReturn && _normalCameraQueuedDuringReturn;
    if (queuedNormalCamera) _normalCameraQueuedDuringReturn = false;
    if (refreshData ||
        !queuedNormalCamera && !_usesExplicitCamera && !_awaitingReturn) {
      unawaited(_refreshMapData());
    }
    final revision = widget.cameraRevision;
    final moveCamera = revision == null || _confirmedCameraRevision != revision;
    final bounds = moveCamera ? _cameraBounds : null;
    final view = _awaitingReturn
        ? _pendingReturnView
        : moveCamera
        ? widget.flyToView
        : null;
    final padding = _boundsPadding;
    final boundsParams = bounds == null
        ? null
        : {
            'west': bounds.west,
            'south': bounds.south,
            'east': bounds.east,
            'north': bounds.north,
          };
    final paddingParams = {
      'top': padding.top,
      'right': padding.right,
      'bottom': padding.bottom,
      'left': padding.left,
    };
    final viewParams = view == null
        ? null
        : {'lng': view.lng, 'lat': view.lat, 'zoom': view.zoom};
    if (revision == null &&
        (_awaitingReturn ||
            _requestedAchievementTarget != null ||
            _requestedCameraOwner != owner)) {
      final returnScope = (document: syncDocument, owner: owner);
      if (_returnCommandInFlight == returnScope ||
          _returnAttempts >= _maxReturnAttempts) {
        return;
      }
      _returnCommandInFlight = returnScope;
      _returnAttempts++;
      final generation = ++_achievementCommandGeneration;
      bool isLatestReturn() =>
          mounted &&
          generation == _achievementCommandGeneration &&
          syncDocument == _mapDocument &&
          owner == widget.cameraOwner &&
          !_usesExplicitCamera;
      final clear = jsonEncode({
        'document': _mapDocument,
        'owner': widget.cameraOwner,
        'generation': generation,
        'revision': _requestedAchievementTarget?.revision ?? 0,
        'featureGeoJson': null,
        'bounds': null,
        'view': viewParams,
        'restore': true,
      });
      try {
        final result = await controller
            .evaluateJavascript(
              source: 'window.applyAchievementRegionCommand($clear)',
            )
            .timeout(_returnAckTimeout);
        if (!isLatestReturn()) return;
        if (result is! Map ||
            result['document'] != syncDocument ||
            result['owner'] != owner ||
            result['generation'] != generation) {
          throw StateError('Outline clear not acknowledged');
        }
      } catch (error, stackTrace) {
        if (!isLatestReturn()) return;
        _returnCommandInFlight = null;
        log.error(
          '[base_map_webview] Outline clear failed: $error',
          stackTrace,
        );
        // After the bounded retry, keep the captured home and block GPS follow
        // until a fresh document or owner can acknowledge a safe return.
        if (_returnAttempts < _maxReturnAttempts) {
          unawaited(_syncMapDataAndCamera());
        }
        return;
      }
      _returnCommandInFlight = null;
      _requestedAchievementTarget = null;
      _requestedCameraOwner = owner;
      _confirmedCameraRevision = null;
      final home = _pendingReturnView;
      _awaitingReturn = false;
      _pendingReturnView = null;
      if (home != null) {
        _currentRoughMapView = home;
        widget.onRoughMapViewUpdate?.call(home);
      }
      widget.onMapReturnCompleted?.call();
      _updateLocationMarker();
      if (_normalCameraQueuedDuringReturn) {
        _scheduleMapSync();
        WidgetsBinding.instance.ensureVisualUpdate();
      }
      return;
    }
    if (revision != null) {
      final target = (
        owner: widget.cameraOwner,
        revision: revision,
        feature: widget.achievementRegionGeoJson,
      );
      if (target == _requestedAchievementTarget) return;
      _requestedAchievementTarget = target;
      _requestedCameraOwner = owner;
      final document = _mapDocument;
      final generation = ++_achievementCommandGeneration;
      bool isLatestCommand() =>
          mounted &&
          document == _mapDocument &&
          generation == _achievementCommandGeneration &&
          target.owner == widget.cameraOwner;

      // Do not wait for another evaluation. A clear or replacement must
      // retire the previous outline even if its camera reply is still pending.
      final command = jsonEncode({
        'document': document,
        'owner': target.owner,
        'generation': generation,
        'revision': revision,
        'featureGeoJson': target.feature,
        'bounds': boundsParams,
        'padding': paddingParams,
        'view': viewParams,
      });
      try {
        final result = await controller.evaluateJavascript(
          source: 'window.applyAchievementRegionCommand($command)',
        );
        if (!isLatestCommand()) return;
        if (result is! Map ||
            result['document'] != document ||
            result['owner'] != target.owner ||
            result['generation'] != generation) {
          throw StateError('Map command not acknowledged');
        }
        if (revision == widget.cameraRevision &&
            target.feature == widget.achievementRegionGeoJson &&
            (bounds != null || view != null)) {
          _confirmedCameraRevision = revision;
        }
      } catch (error, stackTrace) {
        if (!isLatestCommand()) return;
        // Allow the next request to resend the full target without looping.
        _requestedAchievementTarget = null;
        log.error(
          '[base_map_webview] Achievement region sync failed: $error',
          stackTrace,
        );
      }
      return;
    }

    final boundsJson = jsonEncode(boundsParams);
    final paddingJson = jsonEncode(paddingParams);
    final viewJson = jsonEncode(viewParams);
    try {
      await controller.evaluateJavascript(
        source:
            '''
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
    try {
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
    } catch (error, stackTrace) {
      log.error(
        '[base_map_webview] Map data refresh failed: $error',
        stackTrace,
      );
    }
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
    _styleStore.addListener(_scheduleMapAppearanceSync);
  }

  @override
  void dispose() {
    _styleStore.removeListener(_scheduleMapAppearanceSync);
    _gpsManager.removeListener(_updateLocationMarker);
    _webViewController = null;
    super.dispose();
  }

  void _updateLocationMarker() {
    if (!mounted ||
        _awaitingReturn ||
        widget.trackingMode == TrackingMode.off) {
      return;
    }

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
      if (widget.onMapAttributionChanged != null)
        controller.addWebMessageListener(
          WebMessageListener(
            jsObjectName: 'onMapAttributionChanged',
            allowedOriginRules: {'*'},
            onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
              if (!mounted || !isMainFrame) return;
              widget.onMapAttributionChanged?.call(message?.data == 'true');
            },
          ),
        ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapTap',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || !_readyForDisplay || !isMainFrame) return;
            final onTap = widget.onMapTap;
            final data = message?.data;
            if (onTap == null || data is! String) return;
            try {
              final payload = jsonDecode(data);
              if (payload is Map &&
                  payload['document'] == _mapDocument &&
                  payload['lease'] == _mapTapLease) {
                onTap();
              }
            } on FormatException {
              // Unscoped messages cannot belong to the active observer.
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'onMapMoved',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || !isMainFrame || _awaitingReturn) return;
            final payload = jsonDecode(message?.data as String);
            if (payload['document'] != _mapDocument ||
                payload['owner'] != widget.cameraOwner) {
              return;
            }
            widget.onMapMoved?.call();
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'readyForCommands',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || !isMainFrame || message?.data != '$_mapDocument') {
              return;
            }
            _setStateIfMounted(() => _readyForCommands = true);
            _scheduleMapAppearanceSync();
            if (_usesExplicitCamera || _awaitingReturn) {
              unawaited(_syncMapDataAndCamera());
            }
          },
        ),
      ),
      controller.addWebMessageListener(
        WebMessageListener(
          jsObjectName: 'readyForDisplay',
          allowedOriginRules: {'*'},
          onPostMessage: (message, sourceOrigin, isMainFrame, replyProxy) {
            if (!mounted || (_usesExplicitCamera && !isMainFrame)) return;
            final recoveredAfterReload = _webGlRecoveryReloadInProgress;
            _webGlRecoveryReloadInProgress = false;
            _updateLocationMarker();
            _setStateIfMounted(() {
              _readyForDisplay = true;
            });
            unawaited(_syncMapTapListener());
            if (_usesExplicitCamera || _pendingMapDataSync) {
              unawaited(_syncMapDataAndCamera());
            }
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

    final mapView = _awaitingReturn ? _pendingReturnView : _currentRoughMapView;
    final lngParam = mapView?.lng.toString() ?? 'null';
    final latParam = mapView?.lat.toString() ?? 'null';
    final zoomParam = mapView?.zoom.toString() ?? 'null';
    // Explicit targets are applied once after readiness, never as both an
    // initial fit and a subsequent fly-to (including on WebView recovery).
    final bounds = _usesExplicitCamera
        ? widget.initialMapBounds
        : widget.flyToBounds ?? widget.initialMapBounds;
    final boundsPadding = _boundsPadding;

    debugPrint('Injecting lng: $lngParam');
    debugPrint('Injecting lat: $latParam');
    debugPrint('Injecting zoom: $zoomParam');

    final appearance = _mapAppearance();
    _requestedMapAppearance = jsonEncode(appearance);
    final params = <String, Object?>{
      'cgi_endpoint': MapWebViewResources.apiEndpoint,
      'render': 'canvas',
      ...appearance,
      'achievement_map_document': _mapDocument,
      'achievement_map_owner': widget.cameraOwner,
      'attribution_label': context.tr(
        'general.map_settings.basemaps.attribution',
      ),
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
      document.documentElement.classList.toggle('hide-map-attribution', ${widget.onMapAttributionChanged != null});
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

  Map<String, Object?> _mapAppearance() {
    final style = _styleStore.selected;
    final mapStyle = style.mapLibreStyle;
    final useAppMapboxToken =
        !style.isCustom &&
        mapStyle is String &&
        mapStyle.startsWith('mapbox://');
    final fogStyle = _styleStore.fogStyle;
    return {
      'map_style': mapStyle,
      'custom_basemap': style.isCustom,
      'fog_style': fogStyle.id,
      'fog_density': style.fogOpacityByStyle[fogStyle] ?? 0.50,
      'access_key': useAppMapboxToken ? api.getMapboxAccessToken() : null,
    };
  }

  void _scheduleMapAppearanceSync() {
    if (_mapAppearanceSyncScheduled) return;
    _mapAppearanceSyncScheduled = true;
    // Saving and selecting a custom style can write two settings at once.
    scheduleMicrotask(() {
      _mapAppearanceSyncScheduled = false;
      if (!mounted || !_readyForCommands) return;
      unawaited(_syncMapAppearance());
    });
  }

  Future<void> _syncMapAppearance() async {
    final appearance = jsonEncode(_mapAppearance());
    if (appearance == _requestedMapAppearance) return;
    _requestedMapAppearance = appearance;
    try {
      await runJavaScript(
        'window.applyMapAppearance?.($_mapDocument, $appearance);',
      );
    } catch (error, stackTrace) {
      log.error(
        '[base_map_webview] Updating map appearance failed: $error',
        stackTrace,
      );
    }
  }

  void _handleMapViewPush(String message) {
    try {
      final map = jsonDecode(message);
      if (_awaitingReturn ||
          map['document'] != _mapDocument ||
          map['owner'] != widget.cameraOwner) {
        return;
      }

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
        if (!_usesExplicitCamera &&
            _cameraBounds != null &&
            !layout.hasSameMapViewport(previous)) {
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
            if (!_usesExplicitCamera &&
                oldSize != newSize &&
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
          onLoadStart: (controller, url) {
            if (!mounted) return;
            widget.onMapAttributionChanged?.call(false);
            _readyForCommands = false;
            _mapDocument++;
            _achievementCommandGeneration = 0;
            _requestedCameraOwner = widget.cameraOwner;
            _requestedAchievementTarget = null;
            _returnCommandInFlight = null;
            _returnAttempts = 0;
            _requestedMapAppearance = null;
            if (_awaitingReturn) _pendingMapDataSync = true;
            if (_readyForDisplay) setState(() => _readyForDisplay = false);
          },
          onLoadStop: (controller, url) {
            if (!mounted) return;
            debugPrint('Page finished loading: $url');
            _injectApiEndpoint();
          },
          onReceivedError: (controller, request, error) async {
            if (!mounted) return;
            final host = request.url.host;
            if (host != 'events.mapbox.com') {
              log.error(
                '[base_map_webview] Resource load failed on $host (${error.type})',
              );
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
            opacity:
                !(_readyForDisplay ||
                    (_usesExplicitCamera && _readyForCommands))
                ? 1
                : 0.0,
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
