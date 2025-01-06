import 'package:flutter/material.dart';
import 'package:memolanes/component/base_map_webview.dart';
import 'package:memolanes/component/map_controls/accuracy_display.dart';
import 'package:memolanes/component/map_controls/tracking_button.dart';
// TODO: maybe we still need to store some states..
// import 'package:shared_preferences/shared_preferences.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:json_annotation/json_annotation.dart';
import 'package:memolanes/gps_page.dart';

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

  final _mapKey = GlobalKey<BaseMapWebviewState>();

  TrackingMode _currentTrackingMode = TrackingMode.off;

  void _trackingModeButton() async {
    final newMode = _currentTrackingMode == TrackingMode.off
        ? TrackingMode.displayAndTracking
        : TrackingMode.off;
    _mapKey.currentState?.updateTrackingMode(newMode);
  }

  @override
  Widget build(BuildContext context) {
    final mapRendererProxy = api.getMapRendererProxyForMainMap();

    // TODO: I'm not sure if we need to keep the circular progress indicator
    // here. but the initial camera options things has been removed.
    // if (initialCameraOptions == null) {
    //   return const CircularProgressIndicator();
    // }

    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    // TODO: Add profile button top right
    return Stack(
      children: [
        BaseMapWebview(
          key: _mapKey,
          mapRendererProxy: mapRendererProxy,
          initialTrackingMode: TrackingMode.off,
          onTrackingModeChanged: (TrackingMode newMode) {
            setState(() {
              _currentTrackingMode = newMode;
            });
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
