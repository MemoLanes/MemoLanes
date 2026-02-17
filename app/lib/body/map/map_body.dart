import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/body/map/overlay/normal_map_overlay.dart';
import 'package:memolanes/body/map/overlay/time_machine_overlay.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/service/permission_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:provider/provider.dart';

part 'map.g.dart';

/// 页面模式：同一地图上叠加不同层即不同页面。
enum MapMode {
  /// 首页：轨迹记录叠加层 [NormalMapOverlay]
  normal,
  /// 时光机：日期范围 + 历程地图叠加层 [TimeMachineOverlay]
  timeMachine,
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
  const MapBody({super.key, this.mode = MapMode.normal});

  final MapMode mode;

  @override
  State<StatefulWidget> createState() => MapBodyState();
}

class MapBodyState extends State<MapBody> with WidgetsBindingObserver {
  /// 主地图 proxy，仅此一处初始化，无重复（main 层用 GlobalKey 固定 MapBody 实例）。
  final _mapRendererProxy = api.getMapRendererProxyForMainMap();
  MapView? _roughMapView;
  api.MapRendererProxy? _journeyMapRendererProxy;

  TrackingMode _currentTrackingMode = TrackingMode.off;

  /// 用 GlobalKey 固定主地图 WebView 的 State，避免 0↔1 切换时被误判重建导致重载
  final GlobalKey<BaseMapWebviewState> _mainMapKey =
      GlobalKey<BaseMapWebviewState>();

  void setJourneyMapRendererProxy(api.MapRendererProxy? proxy) {
    setState(() => _journeyMapRendererProxy = proxy);
  }

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
  Future<void> didChangeAppLifecycleState(AppLifecycleState state) async {
    final gpsManager = Provider.of<GpsManager>(context, listen: false);
    // On certain Android ROMs,
    // when the user disables location permission from the system settings,
    // permission_handler returns denied instead of permanentlyDenied.
    // Requesting the permission triggers an AppLifecycleState change,
    // which in turn initiates another permission request.
    // This recursive interaction results in a loop of repeated permission requests.
    if (Platform.isAndroid && _currentTrackingMode != TrackingMode.off) {
      final hasPermission = await PermissionService().checkLocationPermission();
      if (!hasPermission) {
        setState(() => _currentTrackingMode = TrackingMode.off);
        gpsManager.toggleMapTracking(false);
        return;
      }
    }

    switch (state) {
      case AppLifecycleState.resumed:
        _syncTrackingModeWithGpsManager();
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
    // 时光机选日期后：复用同一 WebView，只换 proxy，由 didUpdateWidget 触发 refreshMapData()，不整页重载
    final proxy = (widget.mode == MapMode.timeMachine &&
            _journeyMapRendererProxy != null)
        ? _journeyMapRendererProxy!
        : _mapRendererProxy;
    return BaseMapWebview(
      key: _mainMapKey,
      mapRendererProxy: proxy,
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
    );
  }

  /// 按 mode 返回对应叠加层，叠加层均在独立文件中。
  Widget _buildOverlay(BuildContext context, MapMode mode) {
    switch (mode) {
      case MapMode.normal:
        return NormalMapOverlay(
          trackingMode: _currentTrackingMode,
          onTrackingPressed: _trackingModeButton,
        );
      case MapMode.timeMachine:
        return TimeMachineOverlay(
          onJourneyRangeLoaded: setJourneyMapRendererProxy,
        );
    }
  }

  @override
  Widget build(BuildContext context) {
    final mode = widget.mode;

    // TODO: Add profile button top right
    if (_roughMapView == null) {
      // TODO: This should be a loading spinner and it should be cover the whole
      // screen until the map is fully loaded.
      return const SizedBox.shrink();
    }

    final children = <Widget>[
      _buildMapLayer(),
      _buildOverlay(context, mode),
    ];

    return Stack(children: children);
  }
}
