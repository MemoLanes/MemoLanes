import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:mapbox_maps_flutter/mapbox_maps_flutter.dart';
import 'package:memolanes/component/base_map.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:json_annotation/json_annotation.dart';
import 'package:provider/provider.dart';
import 'gps_recording_state.dart';

part 'map.g.dart';

enum TrackingMode {
  displayAndTracking,
  displayOnly,
  off,
}

// TODO: `dart run build_runner build` is needed for generating `map.g.dart`,
// we should automate this.
@JsonSerializable()
class MapState {
  MapState(this.trackingMode, this.zoom, this.lng, this.lat, this.bearing);

  TrackingMode trackingMode;
  double zoom;
  double lng;
  double lat;
  double bearing;

  factory MapState.fromJson(Map<String, dynamic> json) =>
      _$MapStateFromJson(json);
  Map<String, dynamic> toJson() => _$MapStateToJson(this);
}

class MapUiBody extends StatefulWidget {
  const MapUiBody({super.key});

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
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

class MapUiBodyState extends State<MapUiBody> with WidgetsBindingObserver {
  static const String mainMapStatePrefsKey = "MainMap.mapState";

  MapUiBodyState();

  MapController? mapController;
  Timer? refreshTimer;
  Timer? trackTimer;
  TrackingMode trackingMode = TrackingMode.displayAndTracking;
  bool _showDebugInfo = false;

  CameraOptions? _initialCameraOptions;

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    CameraState? cameraState = await mapController?.mapboxMap.getCameraState();
    if (cameraState == null) return;
    final mapState = MapState(
      trackingMode,
      cameraState.zoom,
      cameraState.center.coordinates.lng.toDouble(),
      cameraState.center.coordinates.lat.toDouble(),
      cameraState.bearing,
    );
    prefs.setString(mainMapStatePrefsKey, jsonEncode(mapState.toJson()));
  }

  void _loadMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    MapState? mapState;
    final mapStateString = prefs.getString(mainMapStatePrefsKey);
    if (mapStateString != null) {
      try {
        mapState = MapState.fromJson(jsonDecode(mapStateString));
      } catch (_) {
        // best effort
      }
    }

    var cameraOptions = CameraOptions();

    if (mapState != null) {
      trackingMode = mapState.trackingMode;
      cameraOptions.bearing = mapState.bearing;
      cameraOptions.zoom = mapState.zoom;
      cameraOptions.center =
          Point(coordinates: Position(mapState.lng, mapState.lat));
    } else {
      // nothing we can use, just look at the whole earth
      cameraOptions.zoom = 2;
    }

    setState(() {
      _initialCameraOptions = cameraOptions;
    });
  }

  void _initRefershTimerIfNecessary() {
    refreshTimer ??= Timer.periodic(const Duration(seconds: 1), (Timer _) {
      mapController?.triggerRefresh();
    });
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _loadMapState();
  }

  @override
  void dispose() {
    _saveMapState();
    WidgetsBinding.instance.removeObserver(this);
    refreshTimer?.cancel();
    trackTimer?.cancel();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // TODO: we could consider clean up more resources, especially when
    // recording. We take the partical wake lock for that.
    if (state == AppLifecycleState.resumed) {
      _initRefershTimerIfNecessary();
      setupTrackingMode();
    } else if (state == AppLifecycleState.paused) {
      _saveMapState();
      refreshTimer?.cancel();
      refreshTimer = null;
      trackTimer?.cancel();
      trackTimer = null;
    }
  }

  _onMapCreated(MapController mapController) async {
    this.mapController = mapController;
    setupTrackingMode();
  }

  _onMapScrollListener(MapContentGestureContext context) {
    if (trackingMode == TrackingMode.displayAndTracking) {
      setState(() {
        trackingMode = TrackingMode.displayOnly;
      });
      setupTrackingMode();
    }
  }

  _trackingModeButton() async {
    setState(() {
      if (trackingMode == TrackingMode.off) {
        trackingMode = TrackingMode.displayAndTracking;
      } else {
        trackingMode = TrackingMode.off;
      }
    });
    await setupTrackingMode();
  }

  setupTrackingMode() async {
    trackTimer?.cancel();
    LocationComponentSettings locationSettings;
    switch (trackingMode) {
      case TrackingMode.displayAndTracking:
        trackTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
          try {
            double? zoom;
            final position =
                await mapController?.mapboxMap.style.getPuckPosition();
            CameraState? cameraState =
                await mapController?.mapboxMap.getCameraState();
            if (cameraState != null) {
              if (cameraState.zoom < 10.5) {
                zoom = 16.0;
              }
            }
            await mapController?.mapboxMap.flyTo(
                CameraOptions(
                    center: Point(coordinates: position!), zoom: zoom),
                null);
          } catch (e) {
            // just best effort
          }
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
    await mapController?.mapboxMap.location.updateSettings(locationSettings);
  }

  @override
  Widget build(BuildContext context) {
    final initialCameraOptions = _initialCameraOptions;
    final mapRendererProxy = api.getMapRendererProxyForMainMap();
    if (initialCameraOptions == null) {
      return const CircularProgressIndicator();
    } else {
      return Scaffold(
        body: Stack(
          children: [
            BaseMap(
              key: const ValueKey("mapWidget"),
              mapRendererProxy: mapRendererProxy,
              initialCameraOptions: initialCameraOptions,
              onMapCreated: _onMapCreated,
              onScrollListener: _onMapScrollListener,
            ),
            Positioned(
              right: 16,
              bottom: 256,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Container(
                    margin: const EdgeInsets.all(8),
                    width: 48,
                    height: 48,
                    decoration: const BoxDecoration(
                      color: Colors.black,
                      shape: BoxShape.circle,
                    ),
                    child: Material(
                      color: Colors.transparent,
                      child: IconButton(
                        onPressed: _trackingModeButton,
                        icon: Icon(
                          trackingMode == TrackingMode.off
                              ? Icons.near_me_disabled
                              : Icons.near_me,
                          color: trackingMode == TrackingMode.displayAndTracking
                              ? const Color(0xFFB4EC51)
                              : const Color(0xFFB4EC51).withOpacity(0.5),
                        ),
                        tooltip: trackingMode == TrackingMode.off
                            ? 'Enable location tracking'
                            : 'Disable location tracking',
                      ),
                    ),
                  ),
                  Container(
                    margin: const EdgeInsets.all(8),
                    width: 48,
                    height: 48,
                    child: Stack(
                      alignment: Alignment.center,
                      clipBehavior: Clip.none,
                      children: [
                        Consumer<GpsRecordingState>(
                          builder: (context, gpsState, child) {
                            final position = gpsState.latestPosition;
                            final accuracy = position?.accuracy ?? 0.0;

                            Color getAccuracyColor(double accuracy) {
                              if (accuracy <= 5) {
                                return const Color(0xFFB4EC51);
                              } else if (accuracy <= 10) {
                                return Colors.yellow;
                              } else if (accuracy <= 20) {
                                return Colors.orange;
                              } else {
                                return Colors.red;
                              }
                            }

                            int getFilledTicks(double accuracy) {
                              if (accuracy <= 5) {
                                return 4;
                              } else if (accuracy <= 10) {
                                return 3;
                              } else if (accuracy <= 20) {
                                return 2;
                              } else {
                                return 1;
                              }
                            }

                            final accuracyColor = getAccuracyColor(accuracy);
                            final filledTicks =
                                getFilledTicks(accuracy.roundToDouble());

                            return Container(
                              width: 48,
                              height: 48,
                              decoration: const BoxDecoration(
                                color: Colors.black,
                                shape: BoxShape.circle,
                              ),
                              child: Material(
                                color: Colors.transparent,
                                child: InkWell(
                                  onTap: () {
                                    setState(() {
                                      _showDebugInfo = !_showDebugInfo;
                                    });
                                  },
                                  borderRadius: BorderRadius.circular(24),
                                  child: Stack(
                                    alignment: Alignment.center,
                                    children: [
                                      Center(
                                        child: Text(
                                          '${accuracy.round()}m\nACC',
                                          textAlign: TextAlign.center,
                                          style: const TextStyle(
                                            color: Colors.white,
                                            fontSize: 10,
                                            height: 1.0,
                                          ),
                                        ),
                                      ),
                                      if (position != null)
                                        CustomPaint(
                                          size: const ui.Size(48, 48),
                                          painter: AccuracyTicksPainter(
                                            filledTicks: filledTicks,
                                            color: accuracyColor,
                                          ),
                                        ),
                                    ],
                                  ),
                                ),
                              ),
                            );
                          },
                        ),
                        if (_showDebugInfo)
                          Positioned(
                            right: 64,
                            child: Consumer<GpsRecordingState>(
                              builder: (context, gpsState, child) {
                                final position = gpsState.latestPosition;
                                if (position != null) {
                                  String getSignalStatus(double accuracy) {
                                    if (accuracy <= 5) return "Excellent";
                                    if (accuracy <= 10) return "Good";
                                    if (accuracy <= 20) return "Fair";
                                    return "Poor";
                                  }

                                  Color getStatusColor(double accuracy) {
                                    if (accuracy <= 5)
                                      return const Color(0xFFB4EC51);
                                    if (accuracy <= 10) return Colors.yellow;
                                    if (accuracy <= 15) return Colors.orange;
                                    return Colors.red;
                                  }

                                  final signalStatus =
                                      getSignalStatus(position.accuracy);
                                  final statusColor =
                                      getStatusColor(position.accuracy);

                                  return GestureDetector(
                                    onTap: () {
                                      setState(() {
                                        _showDebugInfo = false;
                                      });
                                    },
                                    child: Container(
                                      padding: const EdgeInsets.all(16),
                                      decoration: BoxDecoration(
                                        color: Colors.black,
                                        borderRadius: BorderRadius.circular(24),
                                      ),
                                      child: Column(
                                        crossAxisAlignment:
                                            CrossAxisAlignment.start,
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          Row(
                                            mainAxisAlignment:
                                                MainAxisAlignment.spaceBetween,
                                            crossAxisAlignment:
                                                CrossAxisAlignment.start,
                                            children: [
                                              Column(
                                                crossAxisAlignment:
                                                    CrossAxisAlignment.start,
                                                children: [
                                                  Padding(
                                                    padding:
                                                        const EdgeInsets.only(
                                                            right: 16.0),
                                                    child: Text(
                                                      '${position.accuracy.round()} m',
                                                      style: const TextStyle(
                                                        color: Colors.white,
                                                        fontSize: 32,
                                                        fontWeight:
                                                            FontWeight.w400,
                                                      ),
                                                    ),
                                                  ),
                                                  const Text(
                                                    'Accuracy',
                                                    style: TextStyle(
                                                      color: Colors.white70,
                                                      fontSize: 16,
                                                    ),
                                                  ),
                                                ],
                                              ),
                                              Container(
                                                padding:
                                                    const EdgeInsets.symmetric(
                                                        horizontal: 8,
                                                        vertical: 4),
                                                decoration: BoxDecoration(
                                                  color: statusColor,
                                                  borderRadius:
                                                      BorderRadius.circular(12),
                                                ),
                                                child: Text(
                                                  signalStatus,
                                                  style: const TextStyle(
                                                    color: Colors.black,
                                                    fontSize: 12,
                                                    fontWeight: FontWeight.w400,
                                                  ),
                                                ),
                                              ),
                                            ],
                                          ),
                                          const SizedBox(height: 12),
                                          Text(
                                            '${position.latitude.toStringAsFixed(4)}, ${position.longitude.toStringAsFixed(4)}',
                                            style: const TextStyle(
                                              color: Colors.white70,
                                              fontSize: 12,
                                            ),
                                          ),
                                          Text(
                                            position.timestamp
                                                .toLocal()
                                                .toString()
                                                .substring(0, 19),
                                            style: const TextStyle(
                                              color: Colors.white70,
                                              fontSize: 12,
                                            ),
                                          ),
                                        ],
                                      ),
                                    ),
                                  );
                                } else {
                                  return Container();
                                }
                              },
                            ),
                          ),
                      ],
                    ),
                  ),
                  Container(
                    margin: const EdgeInsets.all(8),
                    width: 48,
                    height: 48,
                    decoration: const BoxDecoration(
                      color: Colors.black,
                      shape: BoxShape.circle,
                    ),
                    child: Material(
                      color: Colors.transparent,
                      child: IconButton(
                        onPressed: () {
                          // TODO: Implement layer picker functionality
                        },
                        icon: const Icon(
                          Icons.layers,
                          color: Colors.white,
                        ),
                        tooltip: 'Layer picker',
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      );
    }
  }
}

class AccuracyTicksPainter extends CustomPainter {
  final int filledTicks;
  final Color color;

  AccuracyTicksPainter({required this.filledTicks, required this.color});

  @override
  void paint(Canvas canvas, ui.Size size) {
    final paint = Paint()
      ..strokeWidth = 2.0
      ..style = PaintingStyle.stroke;

    final center = Offset(size.width / 2, size.height / 2);
    final radius = size.width / 2 - 1;

    const totalArcSpan = math.pi * 0.6;
    const startAngle = math.pi / 2 - totalArcSpan / 2;
    const tickArcLength = math.pi * 0.12;
    const gapAngle = (totalArcSpan - (tickArcLength * 4)) / 3;

    for (int i = 0; i < 4; i++) {
      paint.color = i < filledTicks ? color : Colors.grey.shade700;

      final tickStartAngle = startAngle + (i * (tickArcLength + gapAngle));

      canvas.drawArc(
        Rect.fromCircle(center: center, radius: radius),
        tickStartAngle,
        tickArcLength,
        false,
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(covariant AccuracyTicksPainter oldDelegate) {
    return oldDelegate.filledTicks != filledTicks || oldDelegate.color != color;
  }
}
