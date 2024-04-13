import 'package:flutter/material.dart';
import 'package:flutter_dotenv/flutter_dotenv.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'dart:async';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'main.dart';

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

class MapUiBodyState extends State<MapUiBody> {
  static const String overlayLayerId = "overlay-layer";
  static const String overlayImageSourceId = "overlay-image-source";

  MapUiBodyState() {
    MapboxOptions.setAccessToken(dotenv.env['MAPBOX-ACCESS-TOKEN']!);
   }

  MapboxMap? mapboxMap;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();
  Timer? timer;

  Future<void> _doActualRefresh() async {
    var mapboxMap = this.mapboxMap;
    if (mapboxMap == null) return;

    final cameraState = await mapboxMap.getCameraState();
    final zoom = cameraState.zoom;
    final coordinateBounds = await mapboxMap.coordinateBoundsForCamera(
        CameraOptions(
            center: cameraState.center, zoom: zoom, pitch: cameraState.pitch));
    final northeast = coordinateBounds.northeast['coordinates'] as List;
    final southwest = coordinateBounds.southwest['coordinates'] as List;

    final left = southwest[0];
    final top = northeast[1];
    final right = northeast[0];
    final bottom = southwest[1];

    final renderResult = await renderMapOverlay(
      zoom: zoom,
      left: left,
      top: top,
      right: right,
      bottom: bottom,
    );

    if (renderResult != null) {
      final coordinates = [
        [renderResult.left, renderResult.top],
        [renderResult.right, renderResult.top],
        [renderResult.right, renderResult.bottom],
        [renderResult.left, renderResult.bottom]
      ];

      await Future.wait([
        mapboxMap.style.updateStyleImageSourceImage(
            overlayImageSourceId,
            MbxImage(
                width: renderResult.width,
                height: renderResult.height,
                data: renderResult.data)),
        mapboxMap.style.setStyleSourceProperty(
            overlayImageSourceId, "coordinates", coordinates)
      ]);
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
  }

  void _triggerRefresh() async {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
  }

  @override
  void dispose() {
    timer?.cancel();
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
    super.dispose();
  }

  _onMapCreated(MapboxMap mapboxMap) async {
    await mapboxMap.gestures
        .updateSettings(GesturesSettings(pitchEnabled: false));
    await mapboxMap.location.updateSettings(LocationComponentSettings(
      enabled: true,
      pulsingEnabled: true,
    ));
    await mapboxMap.style
        .addSource(ImageSource(id: overlayImageSourceId, coordinates: [
      [0, 0],
      [0, 0],
      [0, 0],
      [0, 0]
    ]));
    await mapboxMap.style.addLayer(RasterLayer(
      id: overlayLayerId,
      sourceId: overlayImageSourceId,
    ));
    this.mapboxMap = mapboxMap;
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: (MapWidget(
      key: const ValueKey("mapWidget"),
      onMapCreated: _onMapCreated,
      onCameraChangeListener: _onCameraChangeListener,
      styleUri: MapboxStyles.OUTDOORS,
      cameraOptions: CameraOptions(
          // TODO: According to this: https://github.com/mapbox/mapbox-maps-flutter/issues/248
          // We need to implement our own location tracking. Basically we need 3 kinds of state and 1 button.
          // State: 1.Display_location_and_camera_tracking / 2.Display_location_only / 3.Off.
          // The button will toggle between 1/2 -> 3 or 3 -> 1.
          // When user touched the map, then the state will stay as 3 or change from 3 to 2.
          center: Point(coordinates: Position(-80.1263, 25.7845)).toJson(),
          zoom: 12.0),
    )));
  }
}
