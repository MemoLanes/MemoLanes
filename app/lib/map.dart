import 'dart:io';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'dart:async';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:project_dv/token.dart';

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

enum TrackingMode {
  Display_and_tracking,
  Display_only,
  Off,
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
  TrackingMode trackingMode = TrackingMode.Display_and_tracking;

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
    this.mapboxMap = mapboxMap;
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
  }

  _onMapScrollListener(ScreenCoordinate coordinate) {
    if (trackingMode == TrackingMode.Display_and_tracking) {
      _triggerRefresh();
      setState(() {
        trackingMode = TrackingMode.Display_only;
      });
      updateCamera();
    }
  }

  _onStyleLoadedCallback(StyleLoadedEventData data) {
    updateCamera();
  }

  _gpsButton() {
    setState(() {
      if (trackingMode == TrackingMode.Off) {
        trackingMode = TrackingMode.Display_and_tracking;
      } else {
        trackingMode = TrackingMode.Off;
      }
      updateCamera();
    });
  }

  // We need to implement our own location tracking. Basically we need 3 kinds of state and 1 button.
  // State: 1.Display_location_and_camera_tracking / 2.Display_location_only / 3.Off.
  // The button will toggle between 1/2 -> 3 or 3 -> 1.
  // When user touched the map, then the state will stay as 3 or change from 1 to 2.

  _refreshTrackLocation() async {
    try {
      final position = await mapboxMap?.style.getPuckPosition();
      mapboxMap?.flyTo(
          CameraOptions(
              center: Point(coordinates: position!).toJson(),
              zoom: 12.0
          ),
          null);
    }catch (e) {
      trackTimer?.cancel();
      setState(() {
        trackingMode = TrackingMode.Off;
      });
      Fluttertoast.showToast(msg: "No final orientation");
    }
  }

  updateCamera() async {
    if (trackingMode == TrackingMode.Display_and_tracking) {
      await mapboxMap?.location
          .updateSettings(LocationComponentSettings(enabled: true));
      trackTimer?.cancel();
      if (trackingMode == TrackingMode.Display_and_tracking) {
        trackTimer  = Timer.periodic(const Duration(seconds: 1), (timer) async {
          _refreshTrackLocation();
        });
        return;
      }
    }

    if (trackingMode == TrackingMode.Display_only) {
      await mapboxMap?.location
          .updateSettings(LocationComponentSettings(enabled: true));
    }

    if (trackingMode == TrackingMode.Off) {
      await mapboxMap?.location
          .updateSettings(LocationComponentSettings(enabled: false));
    }
    trackTimer?.cancel();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: (MapWidget(
        key: const ValueKey("mapWidget"),
        onMapCreated: _onMapCreated,
        onCameraChangeListener: _onCameraChangeListener,
        onScrollListener: _onMapScrollListener,
        onStyleLoadedListener: _onStyleLoadedCallback,
        styleUri: MapboxStyles.OUTDOORS,
        cameraOptions: CameraOptions(zoom: 12.0),
      )),
      floatingActionButton: FloatingActionButton(
        child: Icon(
          trackingMode == TrackingMode.Off
              ? Icons.near_me_disabled
              : Icons.near_me,
          color: Colors.black,
        ),
        backgroundColor:
            trackingMode == TrackingMode.Off ? Colors.grey : Colors.blue,
        onPressed: () {
          _gpsButton();
        },
      ),
    );
  }
}
