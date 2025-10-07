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

  TrackingMode _currentTrackingMode = TrackingMode.off;
  api.LayerKind _currentLayer = api.getCurrentMapLayerKind();

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
    _saveMapState();
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
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _saveMapState();
    super.dispose();
  }

  @override
  void deactivate() {
    Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
    super.deactivate();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _syncTrackingModeWithGpsManager();
    } else if (state == AppLifecycleState.paused) {
      Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
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
              _saveMapState();
            },
            onMapMoved: () {
              if (_currentTrackingMode == TrackingMode.displayAndTracking) {
                setState(() {
                  _currentTrackingMode = TrackingMode.displayOnly;
                });
                _syncTrackingModeWithGpsManager();
                _saveMapState();
              }
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
