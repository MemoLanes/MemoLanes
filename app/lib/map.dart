import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart' as geolocator;
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:memolanes/token.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:json_annotation/json_annotation.dart';

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
  static const String overlayLayerId = "overlay-layer";
  static const String overlayImageSourceId = "overlay-image-source";
  static const String mainMapStatePrefsKey = "MainMap.mapState";

  MapUiBodyState() {
    // TODO: Kinda want the default implementation is maplibre instead of mapbox.
    // However maplibre is very buggy + lack of global view +
    // cannot handle antimeridian well.
    MapboxOptions.setAccessToken(token["MAPBOX-ACCESS-TOKEN"]);
  }

  MapboxMap? mapboxMap;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();
  Timer? refreshTimer;
  Timer? trackTimer;
  TrackingMode trackingMode = TrackingMode.displayAndTracking;

  CameraOptions? _initialCameraOptions;

  Future<void> _doActualRefresh() async {
    var mapboxMap = this.mapboxMap;
    if (mapboxMap == null) return;

    final cameraState = await mapboxMap.getCameraState();
    final zoom = cameraState.zoom;
    final coordinateBounds = await mapboxMap.coordinateBoundsForCamera(
        CameraOptions(
            center: cameraState.center, zoom: zoom, pitch: cameraState.pitch));
    final northeast = coordinateBounds.northeast.coordinates;
    final southwest = coordinateBounds.southwest.coordinates;

    final left = southwest[0];
    final top = northeast[1];
    final right = northeast[0];
    final bottom = southwest[1];

    final renderResult = await renderMapOverlay(
      zoom: zoom,
      left: left!.toDouble(),
      top: top!.toDouble(),
      right: right!.toDouble(),
      bottom: bottom!.toDouble(),
    );

    if (renderResult != null) {
      final coordinates = [
        [renderResult.left, renderResult.top],
        [renderResult.right, renderResult.top],
        [renderResult.right, renderResult.bottom],
        [renderResult.left, renderResult.bottom]
      ];
      final image = MbxImage(
          width: renderResult.width,
          height: renderResult.height,
          data: renderResult.data);

      if (!mounted) return;
      // TODO: we kinda need transaction to avoid flickering
      if (layerAdded) {
        await Future.wait([
          mapboxMap.style
              .updateStyleImageSourceImage(overlayImageSourceId, image),
          mapboxMap.style.setStyleSourceProperty(
              overlayImageSourceId, "coordinates", coordinates)
        ]);
      } else {
        layerAdded = true;
        await mapboxMap.style.addSource(
            ImageSource(id: overlayImageSourceId, coordinates: coordinates));
        await mapboxMap.style.addLayer(RasterLayer(
          id: overlayLayerId,
          sourceId: overlayImageSourceId,
        ));
        await mapboxMap.style
            .updateStyleImageSourceImage(overlayImageSourceId, image);
      }
    }
  }

  void _refreshLoop() async {
    _initRefershTimerIfNecessary();
    await resetMapRenderer();
    while (true) {
      await requireRefresh?.future;
      if (requireRefresh == null) return;
      // make it ready for the next request
      requireRefresh = Completer();

      await _doActualRefresh();
    }
  }

  void _triggerRefresh() async {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
  }

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    CameraState? cameraState = await mapboxMap?.getCameraState();
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
      geolocator.Position? lastKnownPosition =
          await geolocator.Geolocator.getLastKnownPosition();
      if (lastKnownPosition != null) {
        cameraOptions.zoom = 16;
        cameraOptions.center = Point(
            coordinates: Position(
                lastKnownPosition.longitude, lastKnownPosition.latitude));
      } else {
        // nothing we can use, just look at the whole earth
        cameraOptions.zoom = 2;
      }
    }

    setState(() {
      _initialCameraOptions = cameraOptions;
    });
  }

  void _initRefershTimerIfNecessary() {
    refreshTimer ??= Timer.periodic(const Duration(seconds: 1), (Timer _) {
      _triggerRefresh();
    });
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _loadMapState();
    _refreshLoop();
  }

  @override
  void dispose() {
    _saveMapState();
    WidgetsBinding.instance.removeObserver(this);
    refreshTimer?.cancel();
    trackTimer?.cancel();
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
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
      refreshTimer?.cancel();
      refreshTimer = null;
      trackTimer?.cancel();
      trackTimer = null;
    }
  }

  _onMapCreated(MapboxMap mapboxMap) async {
    await mapboxMap.gestures
        .updateSettings(GesturesSettings(pitchEnabled: false));
    this.mapboxMap = mapboxMap;
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
  }

  _onMapScrollListener(MapContentGestureContext context) {
    if (trackingMode == TrackingMode.displayAndTracking) {
      _triggerRefresh();
      setState(() {
        trackingMode = TrackingMode.displayOnly;
      });
      setupTrackingMode();
    }
  }

  _onMapLoadedListener(MapLoadedEventData data) {
    setupTrackingMode();
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
    switch (trackingMode) {
      case TrackingMode.displayAndTracking:
        trackTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
          try {
            double? zoom;
            final position = await mapboxMap?.style.getPuckPosition();
            CameraState? cameraState = await mapboxMap?.getCameraState();
            if (cameraState != null) {
              if (cameraState.zoom < 10.5) {
                zoom = 16.0;
              }
            }
            await mapboxMap?.flyTo(
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
    await mapboxMap?.location.updateSettings(locationSettings);
  }

  @override
  Widget build(BuildContext context) {
    if (_initialCameraOptions == null) {
      return const CircularProgressIndicator();
    } else {
      return Scaffold(
        body: (MapWidget(
          key: const ValueKey("mapWidget"),
          onMapCreated: _onMapCreated,
          onCameraChangeListener: _onCameraChangeListener,
          onScrollListener: _onMapScrollListener,
          onMapLoadedListener: _onMapLoadedListener,
          styleUri: MapboxStyles.OUTDOORS,
          cameraOptions: _initialCameraOptions,
        )),
        floatingActionButton: FloatingActionButton(
          backgroundColor: trackingMode == TrackingMode.displayAndTracking
              ? Colors.blue
              : Colors.grey,
          onPressed: _trackingModeButton,
          child: Icon(
            trackingMode == TrackingMode.off
                ? Icons.near_me_disabled
                : Icons.near_me,
            color: Colors.black,
          ),
        ),
      );
    }
  }
}
