import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:maplibre_gl/mapbox_gl.dart';
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

  MaplibreMapController? mapController;
  Uint8List? image;
  @override
  void initState() {
    super.initState();
  }

  Future<ByteData?> drawImage() async {
    final recorder = PictureRecorder();
    final canvas = Canvas(recorder,
        Rect.fromPoints(const Offset(0.0, 0.0), const Offset(200.0, 200.0)));

    final stroke = Paint()
      ..color = const Color.fromARGB(128, 0, 0, 0)
      ..style = PaintingStyle.fill;

    canvas.drawRect(const Rect.fromLTWH(0.0, 0.0, 200.0, 200.0), stroke);

    final picture = recorder.endRecording();
    final img = await picture.toImage(200, 200);
    return await img.toByteData(format: ImageByteFormat.png);
  }

  void _onMapCreated(MaplibreMapController controller) async {
    controller.addListener(_onMapChanged);
    mapController = controller;
  }

  _onStyleLoadedCallback() async {
    var controller = mapController;
    if (controller == null) return;
    final visiableRegion = await controller.getVisibleRegion();
    image = await api.renderMapOverlay();
    final topLeft = LatLng(
        visiableRegion.northeast.latitude, visiableRegion.southwest.longitude);
    final topRight = visiableRegion.northeast;
    final bottomRight = LatLng(
        visiableRegion.southwest.latitude, visiableRegion.northeast.longitude);
    final bottomLeft = visiableRegion.southwest;
    final coordinates = LatLngQuad(
        topLeft: topLeft,
        topRight: topRight,
        bottomRight: bottomRight,
        bottomLeft: bottomLeft);
    await controller.addImageSource("main-image-source", image!, coordinates);
    await controller.addImageLayer("main-image-layer", "main-image-source");
  }

  void _onMapChanged() async {
    final position = mapController?.cameraPosition;
    if (position == null) return;
    // final isMoving = mapController!.isCameraMoving;
    final visiableRegion = await mapController!.getVisibleRegion();
    // final zoom = position.zoom;
    final topLeft = LatLng(
        visiableRegion.northeast.latitude, visiableRegion.southwest.longitude);
    final topRight = visiableRegion.northeast;
    final bottomRight = LatLng(
        visiableRegion.southwest.latitude, visiableRegion.northeast.longitude);
    final bottomLeft = visiableRegion.southwest;
    final coordinates = LatLngQuad(
        topLeft: topLeft,
        topRight: topRight,
        bottomRight: bottomRight,
        bottomLeft: bottomLeft);
    await mapController?.updateImageSource(
        "main-image-source", image!, coordinates);
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
