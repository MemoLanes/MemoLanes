import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/component/map_controls/accuracy_display.dart';
import 'package:memolanes/component/map_controls/tracking_button.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/gps_page.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

part 'map.g.dart';

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

class MapUiBodyState extends State<MapUiBody> with WidgetsBindingObserver {
  static const String mainMapStatePrefsKey = "MainMap.mapState";
  final _mapRendererProxy = api.getMapRendererProxyForMainMap();
  MapState? _mapState;

  TrackingMode _currentTrackingMode = TrackingMode.off;

  void _syncTrackingModeWithGpsManager() {
    Provider.of<GpsManager>(context, listen: false)
        .toggleMapTracking(_currentTrackingMode != TrackingMode.off);
  }

  void _trackingModeButton() async {
    final newMode = _currentTrackingMode == TrackingMode.off
        ? TrackingMode.displayAndTracking
        : TrackingMode.off;
    setState(() {
      _currentTrackingMode = newMode;
    });
    _syncTrackingModeWithGpsManager();
  }

  @override
  void initState() {
    super.initState();
    _loadMapState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void deactivate() {
    super.deactivate();
    Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _syncTrackingModeWithGpsManager();
    } else if (state == AppLifecycleState.paused) {
      Provider.of<GpsManager>(context, listen: false).toggleMapTracking(false);
    }
  }

  // TODO: We don't enough time to save if the app got killed. Losing data here
  // is fine but we could consider saving every minute or so.
  void _saveMapState(double lng, double lat, double zoom) async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    final mapState = MapState(
      _currentTrackingMode,
      zoom,
      lng,
      lat,
      0,
    );
    prefs.setString(mainMapStatePrefsKey, jsonEncode(mapState.toJson()));
  }

  void _loadMapState() async {
    SharedPreferences prefs = await SharedPreferences.getInstance();
    MapState? mapState;
    TrackingMode trackingMode = _currentTrackingMode;
    final mapStateString = prefs.getString(mainMapStatePrefsKey);
    if (mapStateString != null) {
      try {
        mapState = MapState.fromJson(jsonDecode(mapStateString));
        trackingMode = mapState.trackingMode;
      } catch (_) {
        // best effort
      }
    } else {
      mapState = MapState(trackingMode, 0, 0, 2, 0);
    }
    setState(() {
      _mapState = mapState;
      _currentTrackingMode = trackingMode;
    });
    _syncTrackingModeWithGpsManager();
  }

  @override
  Widget build(BuildContext context) {
    // TODO: I'm not sure if we need to keep the circular progress indicator
    // here. but the initial camera options things has been removed.
    // if (initialCameraOptions == null) {
    //   return const CircularProgressIndicator();
    // }

    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    // TODO: Add profile button top right
    if (_mapState == null) {
      return const CircularProgressIndicator();
    } else {
      return Stack(
        children: [
          BaseMapWebview(
            key: const ValueKey("mainMap"),
            mapRendererProxy: _mapRendererProxy,
            mapState: _mapState,
            trackingMode: _currentTrackingMode,
            onMapStatus: (lng, lat, zoom) {
              _saveMapState(lng, lat, zoom);
            },
            onMapMoved: () {
              if (_currentTrackingMode == TrackingMode.displayAndTracking) {
                setState(() {
                  _currentTrackingMode = TrackingMode.displayOnly;
                });
                _syncTrackingModeWithGpsManager();
              }
            },
          ),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  const Spacer(),
                  Padding(
                    padding: EdgeInsets.only(
                      bottom: isLandscape ? 16 : screenSize.height * 0.08,
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        TrackingButton(
                          trackingMode: _currentTrackingMode,
                          onPressed: _trackingModeButton,
                        ),
                        const AccuracyDisplay(),
                        // TODO: Implement layer picker functionality
                        // LayerButton(
                        //   onPressed: () {};
                        // )
                      ],
                    ),
                  ),
                  const GPSPage(),
                  const SizedBox(height: 116),
                ],
              ),
            ),
          ),
        ],
      );
    }
  }
}
