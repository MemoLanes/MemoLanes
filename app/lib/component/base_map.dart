import 'dart:async';

import 'package:flutter/material.dart';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/token.dart';

class MapController {
  final MapboxMap mapboxMap;
  final void Function() triggerRefresh;

  MapController(this.mapboxMap, this.triggerRefresh);
}

class BaseMap extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final CameraOptions initialCameraOptions;
  final void Function(MapController mapController)? onMapCreated;
  final OnMapScrollListener? onScrollListener;
  final OnCameraChangeListener? onCameraChangeListener;
  const BaseMap(
      {super.key,
      required this.mapRendererProxy,
      required this.initialCameraOptions,
      this.onMapCreated,
      this.onScrollListener,
      this.onCameraChangeListener});

  @override
  State<StatefulWidget> createState() => BaseMapState();
}

class BaseMapState extends State<BaseMap> {
  static const String overlayLayerId = "overlay-layer";
  static const String overlayImageSourceId = "overlay-image-source";

  BaseMapState() {
    // TODO: Kinda want the default implementation is maplibre instead of mapbox.
    // However maplibre is very buggy + lack of global view +
    // cannot handle antimeridian well.
    MapboxOptions.setAccessToken(token["MAPBOX-ACCESS-TOKEN"]);
  }

  MapController? _mapController;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();

  Future<void> _doActualRefresh() async {
    var mapboxMap = _mapController?.mapboxMap;
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

    final renderResult = await widget.mapRendererProxy.renderMapOverlay(
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

  @override
  void didUpdateWidget(BaseMap oldWidget) {
    super.didUpdateWidget(oldWidget);
    _triggerRefresh();
  }

  void _refreshLoop() async {
    await widget.mapRendererProxy.resetMapRenderer();
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

  @override
  void initState() {
    super.initState();
    _refreshLoop();
  }

  @override
  void dispose() {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
    super.dispose();
  }

  _onMapCreated(MapboxMap mapboxMap) async {
    await mapboxMap.gestures
        .updateSettings(GesturesSettings(pitchEnabled: false));
    final mapController = MapController(mapboxMap, _triggerRefresh);
    _mapController = mapController;
    final onMapCreated = widget.onMapCreated;
    if (onMapCreated != null) {
      onMapCreated(mapController);
    }
  }

  _onCameraChangeListener(CameraChangedEventData event) {
    _triggerRefresh();
    final onCameraChange = widget.onCameraChangeListener;
    if (onCameraChange != null) {
      onCameraChange(event);
    }
  }

  @override
  Widget build(BuildContext context) {
    return MapWidget(
      key: widget.key,
      onMapCreated: _onMapCreated,
      onCameraChangeListener: _onCameraChangeListener,
      styleUri: MapboxStyles.OUTDOORS,
      cameraOptions: widget.initialCameraOptions,
      onScrollListener: widget.onScrollListener,
    );
  }
}
