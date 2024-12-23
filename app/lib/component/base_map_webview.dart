import 'dart:async';
import 'dart:io' show Platform;
import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:provider/provider.dart';
import 'package:memolanes/gps_recording_state.dart';

// for compatibility with base_map.dart (camaraOptions, etc.)
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

class MapController {
  final WebViewController webViewController;
  final void Function() triggerRefresh;

  MapController(this.webViewController, this.triggerRefresh);
}

class BaseMapWebview extends StatefulWidget {
  final api.MapRendererProxy mapRendererProxy;
  final CameraOptions initialCameraOptions;
  final TrackingMode initialTrackingMode;
  final void Function(TrackingMode)? onTrackingModeChanged;
  final void Function(MapController mapController)? onMapCreated;
  final OnMapScrollListener? onScrollListener;
  const BaseMapWebview(
      {super.key,
      required this.mapRendererProxy,
      required this.initialCameraOptions,
      this.initialTrackingMode = TrackingMode.off,
      this.onTrackingModeChanged,
      this.onMapCreated,
      this.onScrollListener});

  @override
  State<StatefulWidget> createState() => BaseMapWebviewState();
}

class BaseMapWebviewState extends State<BaseMapWebview> {
  WebViewController? _webViewController;
  bool layerAdded = false;
  Completer? requireRefresh = Completer();
  late TrackingMode _trackingMode;

  // Getter for external access
  TrackingMode get trackingMode => _trackingMode;

  // Method to update tracking mode from outside
  void updateTrackingMode(TrackingMode newMode) {
    if (_trackingMode == newMode) return;

    _trackingMode = newMode;
    widget.onTrackingModeChanged?.call(_trackingMode);

    if (mounted) {
      _updateLocationMarker(context);
    }
  }

  @override
  void didUpdateWidget(BaseMapWebview oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Only update URL if the mapRendererProxy actually changed
    if (oldWidget.mapRendererProxy != widget.mapRendererProxy) {
      _updateMapUrl();
    }
  }

  Future<void> _updateMapUrl() async {
    if (_webViewController == null) return;

    final url = await widget.mapRendererProxy.getUrl();

    // TODO: currently when trackingMode updates, the upper layer will trigger a
    // rebuid of this widget? we should not reload the page if url is unchanged
    // this may be an iOS bug to be investigated further
    final currentUrl = await _webViewController?.currentUrl();

    if (currentUrl != url) {
      await _webViewController?.loadRequest(Uri.parse(url));
    }
  }

  @override
  void initState() {
    super.initState();
    _trackingMode = widget.initialTrackingMode;
    _initWebView();
  }

  Future<void> _initWebView() async {
    _webViewController = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..addJavaScriptChannel(
        'onMapMoved',
        onMessageReceived: (JavaScriptMessage message) {
          if (_trackingMode == TrackingMode.displayAndTracking) {
            updateTrackingMode(TrackingMode.displayOnly);
          }
        },
      );

    final url = await widget.mapRendererProxy.getUrl();
    await _webViewController?.loadRequest(Uri.parse(url));
  }

  @override
  void dispose() {
    super.dispose();
  }

  void _updateLocationMarker(BuildContext context) {
    if (_webViewController == null) return;

    final gpsState = context.read<GpsRecordingState>();
    final position = gpsState.latestPosition;

    if (_trackingMode == TrackingMode.off) {
      _webViewController?.runJavaScript('updateLocationMarker(0, 0, false);');
    } else if (position != null) {
      _webViewController?.runJavaScript('''
        updateLocationMarker(
          ${position.longitude}, 
          ${position.latitude}, 
          true, 
          ${_trackingMode == TrackingMode.displayAndTracking}
        );
      ''');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_trackingMode != TrackingMode.off) {
      context.watch<GpsRecordingState>().latestPosition;
      _updateLocationMarker(context);
    }

    // TODO: remove the debug panel before merging
    return Stack(
      children: [
        if (_webViewController == null)
          Container()
        else
          WebViewWidget(
            key: const ValueKey('map_webview'),
            controller: _webViewController!,
          ),
        // Simplified debug overlay
        Positioned(
          left: 16,
          top: 16,
          child: Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: Colors.black.withOpacity(0.7),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Text(
              'Tracking: $_trackingMode',
              style: const TextStyle(
                color: Colors.white,
                fontSize: 12,
              ),
            ),
          ),
        ),
      ],
    );
  }
}
