import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/map_controls/accuracy_display.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/tracking_button.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/component/recording_buttons.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

part 'map.g.dart';

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
  const MapBody({super.key});

  @override
  State<StatefulWidget> createState() => MapBodyState();
}

class MapBodyState extends State<MapBody> with WidgetsBindingObserver {
  final _mapRendererProxy = api.getMapRendererProxyForMainMap();
  MapView? _roughMapView;
  DateTime? _lastSavedTime;
  Timer? _intervalTimer;

  TrackingMode _currentTrackingMode = TrackingMode.off;
  api.LayerKind _currentLayer = api.getCurrentMapLayerKind();

  late GpsManager _gpsManager;
  bool _hasForcedSavedOnce = false;

  void _syncTrackingModeWithGpsManager() {
    Provider.of<GpsManager>(context, listen: false)
        .toggleMapTracking(_currentTrackingMode != TrackingMode.off);
  }

  void _trackingModeButton() async {
    final newMode = _currentTrackingMode == TrackingMode.off
        ? TrackingMode.displayAndTracking
        : TrackingMode.off;
    setState(() {
      _currentTrackingMode = newMode;
    });
    _syncTrackingModeWithGpsManager();
  }

  void _layerButton() async {
    final newLayerKind = api.LayerKind
        .values[(_currentLayer.index + 1) % api.LayerKind.values.length];
    setState(() {
      _currentLayer = newLayerKind;
    });
    await api.setMainMapLayerKind(layerKind: _currentLayer);
  }

  @override
  void initState() {
    super.initState();
    _loadMapState();
    WidgetsBinding.instance.addObserver(this);
    _gpsManager = Provider.of<GpsManager>(context, listen: false);
    _gpsManager.addListener(_onGpsUpdated);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _gpsManager.removeListener(_onGpsUpdated);
    _saveMapState(force: true);
    super.dispose();
  }

  @override
  void deactivate() {
    super.deactivate();
    _gpsManager.toggleMapTracking(false);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _syncTrackingModeWithGpsManager();
    } else if (state == AppLifecycleState.paused) {
      _gpsManager.toggleMapTracking(false);
    }
  }

  void _onGpsUpdated() {
    final latest = _gpsManager.latestPosition;
    if (latest == null) return;

    if (!_hasForcedSavedOnce &&
        MMKVUtil.getString(MMKVKey.mainMapState).isEmpty) {
      _saveMapState(force: true);
      _hasForcedSavedOnce = true;
      return;
    }

    if (_currentTrackingMode == TrackingMode.displayAndTracking) {
      _saveMapState();
    }
  }

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState({bool force = false}) {
    final now = DateTime.now();
    if (_roughMapView == null) return;

    final lastSaved = _lastSavedTime;

    if (force ||
        lastSaved == null ||
        now.difference(lastSaved) >= const Duration(seconds: 10)) {
      _lastSavedTime = now;
      _writeMapStateSafely();
      return;
    }
    _intervalTimer ??=
        Timer(const Duration(seconds: 10) - now.difference(lastSaved), () {
      _lastSavedTime = DateTime.now();
      _writeMapStateSafely();
      _intervalTimer = null;
    });
  }

  void _writeMapStateSafely() {
    try {
      final mapView = _roughMapView;
      if (mapView == null) {
        debugPrint('⚠️ MapView is null, skipping save');
        return;
      }

      final mapState = MapState(
        _currentTrackingMode,
        mapView.zoom,
        mapView.lng,
        mapView.lat,
        0,
      );

      MMKVUtil.putString(MMKVKey.mainMapState, jsonEncode(mapState.toJson()));
      debugPrint('✅ Map state saved successfully');
    } catch (e, st) {
      debugPrint('❌ Failed to save map state: $e\n$st');
    }
  }

  void _loadMapState() async {
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

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;
    var gpsManager = context.watch<GpsManager>();

    // TODO: Add profile button top right
    if (_roughMapView == null) {
      // TODO: This should be a loading spinner and it should be cover the whole
      // screen until the map is fully loaded.
      return SizedBox.shrink();
    } else {
      return Stack(
        children: [
          BaseMapWebview(
            key: const ValueKey("mainMap"),
            mapRendererProxy: _mapRendererProxy,
            initialMapView: _roughMapView,
            trackingMode: _currentTrackingMode,
            onRoughMapViewUpdate: (roughMapView) {
              _roughMapView = roughMapView;
            },
            onMapMoved: () {
              if (_currentTrackingMode == TrackingMode.displayAndTracking) {
                setState(() {
                  _currentTrackingMode = TrackingMode.displayOnly;
                });
                _syncTrackingModeWithGpsManager();
              }
              _saveMapState();
            },
          ),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  const Spacer(),
                  Padding(
                    padding: EdgeInsets.only(
                      right: 8,
                      bottom: isLandscape ? 16 : screenSize.height * 0.08,
                    ),
                    child: PointerInterceptor(
                        child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        TrackingButton(
                          trackingMode: _currentTrackingMode,
                          onPressed: _trackingModeButton,
                        ),
                        const AccuracyDisplay(),
                        LayerButton(
                          layerKind: _currentLayer,
                          onPressed: _layerButton,
                        )
                      ],
                    )),
                  ),
                  const RecordingButtons(),
                  const SizedBox(height: 116),
                ],
              ),
            ),
          ),
          RecIndicator(
            isRecording:
                gpsManager.recordingStatus == GpsRecordingStatus.recording,
            blinkDurationMs: 1000,
          )
        ],
      );
    }
  }
}
