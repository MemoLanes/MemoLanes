import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/component/base_map.dart';
import 'package:memolanes/component/map_controls/accuracy_display.dart';
import 'package:memolanes/component/map_controls/tracking_button.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/gps_page.dart';

part 'map.g.dart';

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
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

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

extension PuckPosition on StyleManager {
  Future<Position> getPuckPosition() async {
    Layer? layer;
    if (Platform.isAndroid) {
      layer = await getLayer("mapbox-location-indicator-layer");
    } else {
      layer = await getLayer("puck");
    }
    final location = (layer as LocationIndicatorLayer).location;
    return Position(location![1]!, location[0]!);
  }
}

class MapUiBodyState extends State<MapUiBody> with WidgetsBindingObserver {
  static const String mainMapStatePrefsKey = "MainMap.mapState";
  MapController? mapController;
  Timer? refreshTimer;
  Timer? trackTimer;
  TrackingMode trackingMode = TrackingMode.off;
  CameraOptions? _initialCameraOptions;

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    CameraState? cameraState = await mapController?.mapboxMap.getCameraState();
    if (cameraState == null) return;
    final mapState = MapState(
      trackingMode,
      cameraState.zoom,
      cameraState.center.coordinates.lng.toDouble(),
      cameraState.center.coordinates.lat.toDouble(),
      cameraState.bearing,
    );
    prefs.setString(mainMapStatePrefsKey, jsonEncode(mapState.toJson()));
  }

  void _loadMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    MapState? mapState;
    final mapStateString = prefs.getString(mainMapStatePrefsKey);
    if (mapStateString != null) {
      try {
        mapState = MapState.fromJson(jsonDecode(mapStateString));
      } catch (_) {
        // best effort
      }
    }

    var cameraOptions = CameraOptions();

    if (mapState != null) {
      trackingMode = mapState.trackingMode;
      cameraOptions.bearing = mapState.bearing;
      cameraOptions.zoom = mapState.zoom;
      cameraOptions.center =
          Point(coordinates: Position(mapState.lng, mapState.lat));
    } else {
      // nothing we can use, just look at the whole earth
      cameraOptions.zoom = 2;
    }

    setState(() {
      _initialCameraOptions = cameraOptions;
    });
  }

  void _initRefershTimerIfNecessary() {
    refreshTimer ??= Timer.periodic(const Duration(seconds: 1), (Timer _) {
      mapController?.triggerRefresh();
    });
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _loadMapState();
  }

  @override
  void dispose() {
    _saveMapState();
    WidgetsBinding.instance.removeObserver(this);
    refreshTimer?.cancel();
    trackTimer?.cancel();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // TODO: we could consider clean up more resources, especially when
    // recording. We take the partical wake lock for that.
    if (state == AppLifecycleState.resumed) {
      _initRefershTimerIfNecessary();
      setupTrackingMode();
    } else if (state == AppLifecycleState.paused) {
      _saveMapState();
      Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
      refreshTimer?.cancel();
      refreshTimer = null;
      trackTimer?.cancel();
      trackTimer = null;
    }
  }

  _onMapCreated(MapController mapController) async {
    this.mapController = mapController;
    setupTrackingMode();
  }

  _onMapScrollListener(MapContentGestureContext context) {
    if (trackingMode == TrackingMode.displayAndTracking) {
      setState(() {
        trackingMode = TrackingMode.displayOnly;
      });
      setupTrackingMode();
    }
  }

  _trackingModeButton() async {
    setState(() {
      if (trackingMode == TrackingMode.off) {
        trackingMode = TrackingMode.displayAndTracking;
      } else {
        trackingMode = TrackingMode.off;
      }
    });
    await setupTrackingMode();
  }

  setupTrackingMode() async {
    trackTimer?.cancel();
    LocationComponentSettings locationSettings;
    Provider.of<GpsManager>(context, listen: false)
        .toggleMapTracking(trackingMode != TrackingMode.off);
    switch (trackingMode) {
      case TrackingMode.displayAndTracking:
        trackTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
          try {
            double? zoom;
            final position =
                await mapController?.mapboxMap.style.getPuckPosition();
            CameraState? cameraState =
                await mapController?.mapboxMap.getCameraState();
            if (cameraState != null) {
              if (cameraState.zoom < 10.5) {
                zoom = 16.0;
              }
            }
            await mapController?.mapboxMap.flyTo(
                CameraOptions(
                    center: Point(coordinates: position!), zoom: zoom),
                null);
          } catch (e) {
            // just best effort
          }
        });
        locationSettings =
            LocationComponentSettings(enabled: true, pulsingEnabled: true);
        break;
      case TrackingMode.displayOnly:
        locationSettings =
            LocationComponentSettings(enabled: true, pulsingEnabled: true);
        break;
      case TrackingMode.off:
        locationSettings = LocationComponentSettings(enabled: false);
        break;
    }
    await mapController?.mapboxMap.location.updateSettings(locationSettings);
  }

  @override
  Widget build(BuildContext context) {
    final initialCameraOptions = _initialCameraOptions;
    final mapRendererProxy = api.getMapRendererProxyForMainMap();
    if (initialCameraOptions == null) {
      return const CircularProgressIndicator();
    }

    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    // TODO: Add profile button top right
    return Stack(
      children: [
        BaseMap(
          key: const ValueKey("mapWidget"),
          mapRendererProxy: mapRendererProxy,
          initialCameraOptions: initialCameraOptions,
          onMapCreated: _onMapCreated,
          onScrollListener: _onMapScrollListener,
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
                    bottom: isLandscape ? 16 : screenSize.height * 0.08,
                  ),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: [
                      TrackingButton(
                        trackingMode: trackingMode,
                        onPressed: _trackingModeButton,
                      ),
                      const AccuracyDisplay(),
                      // TODO: Implement layer picker functionality
                      // LayerButton(
                      //   onPressed: () {};
                      // )
                    ],
                  ),
                ),
                const GPSPage(),
                const SizedBox(height: 116),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
