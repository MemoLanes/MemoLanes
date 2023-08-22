import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:maplibre_gl/mapbox_gl.dart';
import 'package:mutex/mutex.dart';
import 'dart:ui';

import 'ffi.dart' if (dart.library.html) 'ffi_web.dart';

class MapUiBody extends StatefulWidget {
  const MapUiBody();

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
  final m = Mutex();
  MaplibreMapController? mapController;
  Uint8List? image;
  @override
  void initState() {
    super.initState();
  }

  void _onMapCreated(MaplibreMapController controller) async {
    controller.addListener(_onMapChanged);
    mapController = controller;
  }

  void _triggerRefresh() async {
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

    // TODO: we use mutex to make sure only one rendering is happening at the
    // same time, but what we really want is: if there are multiple request
    // queuing up, only run the final one.
    await m.protect(() async {
      final renderResult = await api.renderMapOverlay(
          zoom: zoom, left: left, top: top, right: right, bottom: bottom);

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
    });
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
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final MaplibreMap maplibreMap = MaplibreMap(
      onMapCreated: _onMapCreated,
      onStyleLoadedCallback: _onStyleLoadedCallback,
      initialCameraPosition: _kInitialPosition,
      trackCameraPosition: true,
      myLocationRenderMode: MyLocationRenderMode.NORMAL,
    );

    return maplibreMap;
  }
}
