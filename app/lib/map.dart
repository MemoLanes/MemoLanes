import 'dart:io';
import 'package:flutter/material.dart';
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
    this.mapboxMap = mapboxMap;
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
  }

  _onMapScrollListener(ScreenCoordinate coordinate) {
    if (trackingMode == TrackingMode.displayAndTracking) {
      _triggerRefresh();
      setState(() {
        trackingMode = TrackingMode.displayOnly;
      });
      updateCamera();
    }
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
    await updateCamera();
  }

  _refreshTrackLocation() async {
    try {
      final position = await mapboxMap?.style.getPuckPosition();
      await mapboxMap?.flyTo(
          CameraOptions(
              center: Point(coordinates: position!).toJson(), zoom: 14.0),
          null);
    } catch (e) {
      // just best effort
    }
  }

  updateCamera() async {
    trackTimer?.cancel();
    if (trackingMode == TrackingMode.displayAndTracking) {
      trackTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
        _refreshTrackLocation();
      });
      await mapboxMap?.location.updateSettings(
          LocationComponentSettings(enabled: true, pulsingEnabled: true));
    } else if (trackingMode == TrackingMode.displayOnly) {
      // nothing to do here, we always get here from `displayAndTracking`.
    } else if (trackingMode == TrackingMode.off) {
      await mapboxMap?.location
          .updateSettings(LocationComponentSettings(enabled: false));
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: (MapWidget(
        key: const ValueKey("mapWidget"),
        onMapCreated: _onMapCreated,
        onCameraChangeListener: _onCameraChangeListener,
        onScrollListener: _onMapScrollListener,
        onMapLoadedListener: _onMapLoadedListener,
        styleUri: MapboxStyles.OUTDOORS,
        cameraOptions: CameraOptions(zoom: 12.0),
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
