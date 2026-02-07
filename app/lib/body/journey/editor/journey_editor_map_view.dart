import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

typedef JourneyEditorMapViewCamera = ({double lng, double lat, double zoom});
typedef JourneyEditorDrawPoint = ({double lat, double lng});

class JourneyEditorMapView extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final JourneyEditorMapViewCamera? initialMapView;
  final void Function(
          double startLat, double startLng, double endLat, double endLng)?
      onSelectionBox;
  final void Function(List<JourneyEditorDrawPoint> points)? onDrawPath;
  final void Function()? onMapMoved;
  final void Function(int)? onMapZoomChanged;

  const JourneyEditorMapView({
    super.key,
    required this.mapRendererProxy,
    this.initialMapView,
    this.onSelectionBox,
    this.onDrawPath,
    this.onMapMoved,
    this.onMapZoomChanged,
  });

  @override
  State<JourneyEditorMapView> createState() => JourneyEditorMapViewState();
}

class JourneyEditorMapViewState extends State<JourneyEditorMapView> {
  final GlobalKey<_JourneyEditorMapWebviewState> _innerKey = GlobalKey();

  Future<void> manualRefresh() async {
    await _innerKey.currentState?.manualRefresh();
  }

  void setDeleteMode(bool enabled) {
    _innerKey.currentState?.setDeleteMode(enabled);
  }

  void setDrawMode(bool enabled) {
    _innerKey.currentState?.setDrawMode(enabled);
  }

  @override
  Widget build(BuildContext context) {
    // Use a RenderObject widget as the root so callers can measure the map
    // bounds via this widget's BuildContext.
    return SizedBox.expand(
      child: _JourneyEditorMapWebview(
        key: _innerKey,
        mapRendererProxy: widget.mapRendererProxy,
        onSelectionBox: widget.onSelectionBox,
        onDrawPath: widget.onDrawPath,
        onMapMoved: widget.onMapMoved,
        initialMapView: widget.initialMapView,
        onMapZoomChanged: widget.onMapZoomChanged,
      ),
    );
  }
}

class _JourneyEditorMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final JourneyEditorMapViewCamera? initialMapView;
  final void Function(
          double startLat, double startLng, double endLat, double endLng)?
      onSelectionBox;
  final void Function(List<JourneyEditorDrawPoint> points)? onDrawPath;
  final void Function()? onMapMoved;
  final void Function(int)? onMapZoomChanged;

  const _JourneyEditorMapWebview({
    super.key,
    required this.mapRendererProxy,
    this.initialMapView,
    this.onSelectionBox,
    this.onDrawPath,
    this.onMapMoved,
    this.onMapZoomChanged,
  });

  @override
  State<_JourneyEditorMapWebview> createState() =>
      _JourneyEditorMapWebviewState();
}

class _JourneyEditorMapWebviewState extends State<_JourneyEditorMapWebview> {
  final GlobalKey<BaseMapWebviewState> _baseKey = GlobalKey();

  void setDeleteMode(bool enabled) {
    _baseKey.currentState?.runJavaScript('''
      if (typeof setDeleteMode === 'function') {
        setDeleteMode($enabled);
      }
    ''');
  }

  Future<void> manualRefresh() async {
    await _baseKey.currentState?.runJavaScript('''
      if (typeof refreshMapData === 'function') {
        refreshMapData();
      }
    ''');
  }

  void setDrawMode(bool enabled) {
    _baseKey.currentState?.runJavaScript('''
      if (typeof setDrawMode === 'function') {
        setDrawMode($enabled);
      }
    ''');
  }

  @override
  Widget build(BuildContext context) {
    final baseInitialMapView = widget.initialMapView == null
        ? null
        : (
            lng: widget.initialMapView!.lng,
            lat: widget.initialMapView!.lat,
            zoom: widget.initialMapView!.zoom,
          );

    return BaseMapWebview(
      key: _baseKey,
      mapRendererProxy: widget.mapRendererProxy,
      initialMapView: baseInitialMapView,
      trackingMode: TrackingMode.off,
      isEditor: true,
      onMapMoved: widget.onMapMoved,
      onMapZoomChanged: widget.onMapZoomChanged,
      extraJavaScriptChannels: [
        BaseMapJavaScriptChannel(
          name: 'onSelectionBox',
          onMessageReceived: (raw) {
            final handler = widget.onSelectionBox;
            if (handler == null) return;
            try {
              final data = jsonDecode(raw) as Map<String, dynamic>;
              final startLat = (data['startLat'] as num).toDouble();
              final startLng = (data['startLng'] as num).toDouble();
              final endLat = (data['endLat'] as num).toDouble();
              final endLng = (data['endLng'] as num).toDouble();
              handler(startLat, startLng, endLat, endLng);
            } catch (e) {
              log.error('Error parsing onSelectionBox message: $e');
            }
          },
        ),
        BaseMapJavaScriptChannel(
          name: 'onDrawPath',
          onMessageReceived: (raw) {
            final handler = widget.onDrawPath;
            if (handler == null) return;
            try {
              final data = jsonDecode(raw) as Map<String, dynamic>;
              final rawPoints = data['points'] as List<dynamic>;
              final points = rawPoints
                  .map((p) => (
                        lat: (p['lat'] as num).toDouble(),
                        lng: (p['lng'] as num).toDouble(),
                      ))
                  .toList(growable: false);
              handler(points);
            } catch (e) {
              log.error('Error parsing onDrawPath message: $e');
            }
          },
        ),
      ],
    );
  }
}
