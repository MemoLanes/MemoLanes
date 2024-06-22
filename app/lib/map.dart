import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart' as geolocator;
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:project_dv/token.dart';
import 'package:shared_preferences/shared_preferences.dart';

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
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

class MapUiBodyState extends State<MapUiBody> {
  static const String overlayLayerId = "overlay-layer";
  static const String overlayImageSourceId = "overlay-image-source";
  static const String trackCacheKey = "mapWidget.track";
  static const String lngCacheKey = "mapWidget.camera.lng";
  static const String latCacheKey = "mapWidget.camera.lat";
  static const String zoomCacheKey = "mapWidget.camera.zoom";

  MapUiBodyState() {
    // TODO: Kinda want the default implementation is maplibre instead of mapbox.
    // However maplibre is very buggy + lack of global view +
    // cannot handle antimeridian well.
    MapboxOptions.setAccessToken(token["MAPBOX-ACCESS-TOKEN"]);
  }

  MapboxMap? mapboxMap;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();
  Timer? timer;
  Timer? trackTimer;
  TrackingMode trackingMode = TrackingMode.displayAndTracking;

  CameraOptions? _defaultCameraOptions;

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
    await resetMapRenderer();
    while (true) {
      await requireRefresh?.future;
      if (requireRefresh == null) return;
      // make it ready for the next request
      requireRefresh = Completer();

      await _doActualRefresh();
    }
  }

  @override
  void initState() {
    super.initState();
    timer = Timer.periodic(
        const Duration(seconds: 1),
        (Timer _) =>
            // TODO: constantly calling `_triggerRefresh` isn't too bad, becuase
            // it doesn't do much if nothing is changed. However, this doesn't
            // mean we couldn't do something better.
            _triggerRefresh());
    _refreshLoop();
    _initCameraOptions();
  }

  void _triggerRefresh() async {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
  }

  void _initCameraOptions() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    Point? point;

    geolocator.Position? lastKnownPosition =
        await geolocator.Geolocator.getLastKnownPosition();

    if (lastKnownPosition != null) {
      point = Point(
          coordinates: Position(
              lastKnownPosition.longitude, lastKnownPosition.latitude));
    } else {
      double? lng = prefs.getDouble(lngCacheKey);
      double? lat = prefs.getDouble(latCacheKey);
      if (lng != null && lat != null) {
        point = Point(coordinates: Position(lng, lat));
      }
    }

    setState(() {
      double zoom = prefs.getDouble(zoomCacheKey) ?? 14;
      _defaultCameraOptions = point != null
          ? CameraOptions(center: point, zoom: zoom)
          : CameraOptions();
    });
  }

  void _setCameraCache() async {
    CameraState? cameraState = await mapboxMap?.getCameraState();
    SharedPreferences prefs = await SharedPreferences.getInstance();
    Position? position = await mapboxMap?.style.getPuckPosition();
    if (cameraState != null) {
      prefs.setDouble(zoomCacheKey, cameraState.zoom);
    }
    if (position != null) {
      prefs.setDouble(lngCacheKey, position.lng.toDouble());
      prefs.setDouble(latCacheKey, position.lat.toDouble());
    }
  }

  _setTrackingMode() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    prefs.setString(trackCacheKey, trackingMode.toString());
  }

  _getTrackingMode() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    String? recordState = prefs.getString(trackCacheKey);
    if (recordState != null) {
      setState(() {
        trackingMode = TrackingMode.values.firstWhere(
            (e) => e.toString() == recordState,
            orElse: () => TrackingMode.displayAndTracking);
      });
    }
  }

  @override
  void dispose() {
    timer?.cancel();
    trackTimer?.cancel();
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
    super.dispose();
  }

  _onMapCreated(MapboxMap mapboxMap) async {
    await mapboxMap.gestures
        .updateSettings(GesturesSettings(pitchEnabled: false));
    this.mapboxMap = mapboxMap;
    await _getTrackingMode();
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
    _setCameraCache();
  }

  _onMapScrollListener(MapContentGestureContext context) {
    if (trackingMode == TrackingMode.displayAndTracking) {
      _triggerRefresh();
      setState(() {
        trackingMode = TrackingMode.displayOnly;
      });
      updateCamera();
    }
    _setTrackingMode();
  }

  _onMapLoadedListener(MapLoadedEventData data) {
    _refreshTrackLocation();
    updateCamera();
  }

  _trackingModeButton() async {
    setState(() {
      if (trackingMode == TrackingMode.off) {
        trackingMode = TrackingMode.displayAndTracking;
      } else {
        trackingMode = TrackingMode.off;
      }
    });
    _setTrackingMode();
    await updateCamera();
  }

  _refreshTrackLocation() async {
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
          CameraOptions(center: Point(coordinates: position!), zoom: zoom),
          null);
    } catch (e) {
      // just best effort
    }
  }

  updateCamera() async {
    trackTimer?.cancel();
    LocationComponentSettings locationSettings;
    switch (trackingMode) {
      case TrackingMode.displayAndTracking:
        trackTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
          _refreshTrackLocation();
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
    if (_defaultCameraOptions == null) {
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
          cameraOptions: _defaultCameraOptions,
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
