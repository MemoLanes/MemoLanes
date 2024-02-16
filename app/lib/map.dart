import 'package:flutter/material.dart';
import 'package:maplibre_gl/maplibre_gl.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'dart:async';

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

class MapUiBodyState extends State<MapUiBody> {
  MapUiBodyState();

  static const CameraPosition _kInitialPosition = CameraPosition(
    target: LatLng(-33.852, 151.211),
    zoom: 11.0,
  );

  bool ready = false;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();
  MaplibreMapController? mapController;
  Timer? timer;

  Future<void> _doActualRefresh() async {
    // TODO: this is buggy when view is at the meridian, or when the map is
    // zoom out.
    if (!ready) return;
    var controller = mapController;
    if (controller == null) return;

    final zoom = controller.cameraPosition?.zoom;
    if (zoom == null) return;
    if (!zoom.isFinite) return;
    final visiableRegion = await controller.getVisibleRegion();
    final left = visiableRegion.southwest.longitude;
    final top = visiableRegion.northeast.latitude;
    final right = visiableRegion.northeast.longitude;
    final bottom = visiableRegion.southwest.latitude;

    final renderResult = await renderMapOverlay(
      zoom: zoom,
      left: left,
      top: top,
      right: right,
      bottom: bottom,
    );

    if (renderResult != null) {
      final coordinates = LatLngQuad(
        topLeft: LatLng(renderResult.top, renderResult.left),
        topRight: LatLng(renderResult.top, renderResult.right),
        bottomRight: LatLng(renderResult.bottom, renderResult.right),
        bottomLeft: LatLng(renderResult.bottom, renderResult.left),
      );
      if (layerAdded) {
        await mapController?.updateImageSource(
            "main-image-source", renderResult.data, coordinates);
      } else {
        layerAdded = true;
        await controller.addImageSource(
            "main-image-source", renderResult.data, coordinates);
        await controller.addImageLayer("main-image-layer", "main-image-source");
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

  void _onMapCreated(MaplibreMapController controller) async {
    controller.addListener(_onMapChanged);
    mapController = controller;
  }

  void _triggerRefresh() async {
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
  }

  _onStyleLoadedCallback() async {
    ready = true;
    _triggerRefresh();
  }

  void _onMapChanged() async {
    _triggerRefresh();
  }

  @override
  void dispose() {
    mapController?.removeListener(_onMapChanged);
    timer?.cancel();
    if (requireRefresh?.isCompleted == false) {
      requireRefresh?.complete();
    }
    requireRefresh = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final MaplibreMap maplibreMap = MaplibreMap(
      onMapCreated: _onMapCreated,
      onStyleLoadedCallback: _onStyleLoadedCallback,
      initialCameraPosition: _kInitialPosition,
      trackCameraPosition: true,
      myLocationEnabled: true,
      myLocationTrackingMode: MyLocationTrackingMode.Tracking,
      myLocationRenderMode: MyLocationRenderMode.NORMAL,
    );

    return maplibreMap;
  }
}
