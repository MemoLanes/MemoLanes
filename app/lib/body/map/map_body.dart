import 'dart:convert';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/body/map/overlay/journey_detail_overlay.dart';
import 'package:memolanes/body/map/overlay/normal_map_overlay.dart';
import 'package:memolanes/body/map/overlay/time_machine_overlay.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:provider/provider.dart';

part 'map.g.dart';

enum MapMode {
  /// NormalMapOverlay
  normal,

  /// TimeMachineOverlay
  timeMachine,

  /// JourneyOverlay
  journeys,
}

// TODO: `dart run build_runner build` is needed for generating `map.g.dart`,
// we should automate this.
@JsonSerializable()
class MapState {
  MapState(this.trackingMode, this.zoom, this.lng, this.lat, this.bearing);

  TrackingMode trackingMode;
  double zoom;
  double lng;
  double lat;
  double bearing;

  factory MapState.fromJson(Map<String, dynamic> json) =>
      _$MapStateFromJson(json);

  Map<String, dynamic> toJson() => _$MapStateToJson(this);
}

class MapBody extends StatefulWidget {
  const MapBody({
    super.key,
    this.mode = MapMode.normal,
    this.onAppChromeVisibilityChanged,
  });

  final MapMode mode;

  /// Whether the app controls drawn above the map should remain visible.
  final ValueChanged<bool>? onAppChromeVisibilityChanged;

  @override
  State<StatefulWidget> createState() => MapBodyState();
}

class MapBodyState extends State<MapBody> with WidgetsBindingObserver {
  /// Main map proxy; initialized only here (main holds MapBody instance via
  /// GlobalKey).
  final _mapRendererProxy = api.getMapRendererProxyForMainMap();
  MapView? _roughMapView;
  api.MapRendererProxy? _journeyMapRendererProxy;
  api.MapRendererProxy? _selectedJourneyRendererProxy;
  JourneyHeader? _selectedJourney;
  MapBounds? _selectedJourneyBounds;
  MapView? _viewBeforeJourney;
  MapView? _returnToMapView;
  bool _isLoadingJourney = false;
  int _journeyLoadGeneration = 0;
  int _pickerRevision = 0;

  TrackingMode _currentTrackingMode = TrackingMode.off;

  /// Map tracking controls only belong to Record mode. Timeline and Journeys
  /// keep the current viewport but do not follow the user's live location.
  TrackingMode get _effectiveTrackingMode =>
      widget.mode == MapMode.normal ? _currentTrackingMode : TrackingMode.off;

  /// GlobalKey pins the main map WebView's State so tab 0↔1↔2 switches do not
  /// cause a mistaken rebuild and reload.
  final GlobalKey<BaseMapWebviewState> _mainMapKey =
      GlobalKey<BaseMapWebviewState>();

  void setJourneyMapRendererProxy(api.MapRendererProxy? proxy) {
    setState(() => _journeyMapRendererProxy = proxy);
  }

  Future<void> _openJourneyDetails(JourneyHeader journey) async {
    if (_isLoadingJourney || _selectedJourney != null) return;
    final generation = ++_journeyLoadGeneration;
    setState(() => _isLoadingJourney = true);
    try {
      final previousView =
          await _mainMapKey.currentState?.getCurrentMapView() ?? _roughMapView;
      final rendererAndBounds = await api.getMapRendererProxyForJourney(
        journeyId: journey.id,
      );
      if (!mounted ||
          widget.mode != MapMode.journeys ||
          generation != _journeyLoadGeneration) {
        return;
      }
      setState(() {
        _selectedJourney = journey;
        _selectedJourneyRendererProxy = rendererAndBounds.$1;
        _selectedJourneyBounds = rendererAndBounds.$2;
        _viewBeforeJourney = previousView;
        _returnToMapView = null;
      });
      widget.onAppChromeVisibilityChanged?.call(false);
    } catch (error, stackTrace) {
      log.error('Loading journey map failed: $error', stackTrace);
      if (!mounted || generation != _journeyLoadGeneration) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    } finally {
      if (mounted && generation == _journeyLoadGeneration) {
        setState(() => _isLoadingJourney = false);
      }
    }
  }

  void _updateSelectedJourneyMap(
    api.MapRendererProxy proxy,
    MapBounds? bounds,
  ) {
    if (!mounted || _selectedJourney == null) return;
    setState(() {
      _selectedJourneyRendererProxy = proxy;
      _selectedJourneyBounds = bounds;
    });
  }

  void closeJourneyDetail() {
    if (_selectedJourney == null) return;
    setState(() {
      _returnToMapView = _viewBeforeJourney;
      _viewBeforeJourney = null;
      _selectedJourney = null;
      _selectedJourneyRendererProxy = null;
      _selectedJourneyBounds = null;
      _pickerRevision++;
    });
    widget.onAppChromeVisibilityChanged?.call(true);
  }

  Future<void> _syncTrackingModeWithGpsManager() async {
    final enable = _effectiveTrackingMode != TrackingMode.off;
    final applied = await Provider.of<GpsManager>(
      context,
      listen: false,
    ).toggleMapTracking(enable);

    // When we requested tracking but GpsManager could not enable (e.g. no
    // permission), set UI and MMKV to off so we do not persist invalid state.
    if (enable && !applied && mounted) {
      setState(() => _currentTrackingMode = TrackingMode.off);
      _saveMapState();
    }
  }

  Future<void> _trackingModeButton() async {
    final newMode = _currentTrackingMode == TrackingMode.off
        ? TrackingMode.displayAndTracking
        : TrackingMode.off;
    if (newMode != TrackingMode.off) {
      if (!await checkAndRequestPermission()) {
        return;
      }
    }
    if (!mounted) return;
    setState(() {
      _currentTrackingMode = newMode;
    });
    await _syncTrackingModeWithGpsManager();
    _saveMapState();
  }

  @override
  void initState() {
    super.initState();
    _loadMapState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _saveMapState();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant MapBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.mode == widget.mode) return;
    if (oldWidget.mode == MapMode.journeys) {
      _journeyLoadGeneration++;
      _isLoadingJourney = false;
      _viewBeforeJourney = null;
      _returnToMapView = null;
    }
    if (widget.mode != MapMode.journeys && _selectedJourney != null) {
      _selectedJourney = null;
      _selectedJourneyRendererProxy = null;
      _selectedJourneyBounds = null;
      _pickerRevision++;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) widget.onAppChromeVisibilityChanged?.call(true);
      });
    }
    final gpsManager = Provider.of<GpsManager>(context, listen: false);
    if (widget.mode != MapMode.normal) {
      gpsManager.toggleMapTracking(false);
    } else {
      _syncTrackingModeWithGpsManager();
    }
  }

  @override
  void deactivate() {
    Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
    super.deactivate();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final gpsManager = Provider.of<GpsManager>(context, listen: false);
    switch (state) {
      case AppLifecycleState.resumed:
        _syncTrackingModeWithGpsManager();
        _mainMapKey.currentState?.manualRefresh();
        break;

      case AppLifecycleState.paused:
        gpsManager.toggleMapTracking(false);
        break;

      default:
        break;
    }
  }

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState() {
    final roughMapView = _roughMapView;
    if (roughMapView == null) return;

    final mapState = MapState(
      _currentTrackingMode,
      roughMapView.zoom,
      roughMapView.lng,
      roughMapView.lat,
      0,
    );

    MMKVUtil.putString(MMKVKey.mainMapState, jsonEncode(mapState.toJson()));
  }

  void _loadMapState() {
    MapView mapView = (lat: 0, lng: 0, zoom: 2);
    TrackingMode trackingMode = _currentTrackingMode;
    final mapStateString = MMKVUtil.getString(MMKVKey.mainMapState);
    if (mapStateString.isNotEmpty) {
      try {
        final mapState = MapState.fromJson(jsonDecode(mapStateString));
        trackingMode = mapState.trackingMode;
        mapView = (lat: mapState.lat, lng: mapState.lng, zoom: mapState.zoom);
      } catch (_) {
        // best effort
      }
    }
    setState(() {
      _roughMapView = mapView;
      _currentTrackingMode = trackingMode;
    });
    _syncTrackingModeWithGpsManager();
  }

  Widget _buildMapLayer() {
    // After Time Machine date selection: reuse same WebView, only swap proxy;
    // didUpdateWidget triggers refreshMapData(), no full page reload.
    final proxy =
        widget.mode == MapMode.journeys && _selectedJourneyRendererProxy != null
        ? _selectedJourneyRendererProxy!
        : (widget.mode == MapMode.timeMachine &&
              _journeyMapRendererProxy != null)
        ? _journeyMapRendererProxy!
        : _mapRendererProxy;
    final detailCardPadding =
        MediaQuery.orientationOf(context) == Orientation.landscape
        ? 190.0
        : 330.0;
    final journeyBounds = _selectedJourneyBounds;
    final mainMap = BaseMapWebview(
      key: _mainMapKey,
      mapRendererProxy: proxy,
      initialMapView: _roughMapView,
      flyToBounds: widget.mode == MapMode.journeys ? journeyBounds : null,
      flyToView: widget.mode == MapMode.journeys ? _returnToMapView : null,
      flyToBoundsPadding: journeyBounds == null
          ? null
          : CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
              context,
              edgePadding: 28,
              bottomOverlayHeight:
                  detailCardPadding + MediaQuery.viewPaddingOf(context).bottom,
            ),
      trackingMode: _effectiveTrackingMode,
      onRoughMapViewUpdate: (roughMapView) {
        _roughMapView = roughMapView;
        _saveMapState();
      },
      onMapMoved: () {
        if (widget.mode == MapMode.normal &&
            _currentTrackingMode == TrackingMode.displayAndTracking) {
          setState(() {
            _currentTrackingMode = TrackingMode.displayOnly;
          });
          _syncTrackingModeWithGpsManager();
          _saveMapState();
        }
      },
    );

    return mainMap;
  }

  /// Returns the overlay for the given mode; each overlay lives in its own
  /// file.
  Widget _buildOverlay(BuildContext context, MapMode mode) {
    switch (mode) {
      case MapMode.normal:
        return NormalMapOverlay(
          trackingMode: _effectiveTrackingMode,
          onTrackingPressed: _trackingModeButton,
        );
      case MapMode.timeMachine:
        return TimeMachineOverlay(
          onJourneyRangeLoaded: setJourneyMapRendererProxy,
        );
      case MapMode.journeys:
        final selected = _selectedJourney;
        return JourneyOverlay(
          onJourneySelected: _openJourneyDetails,
          isLoading: _isLoadingJourney,
          refreshRevision: _pickerRevision,
          detail: selected == null
              ? null
              : JourneyDetailOverlay(
                  key: ValueKey(selected.id),
                  journey: selected,
                  onClose: closeJourneyDetail,
                  onMapChanged: _updateSelectedJourneyMap,
                ),
        );
    }
  }

  @override
  Widget build(BuildContext context) {
    final mode = widget.mode;

    final children = <Widget>[_buildMapLayer(), _buildOverlay(context, mode)];

    return Stack(children: children);
  }
}
