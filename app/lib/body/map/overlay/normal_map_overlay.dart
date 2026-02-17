import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/map_controls/accuracy_display.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/tracking_button.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/component/recording_buttons.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

/// 首页（轨迹记录）叠加层：定位、图层层级、录制按钮、录制状态指示等。
class NormalMapOverlay extends StatelessWidget {
  const NormalMapOverlay({
    super.key,
    required this.trackingMode,
    required this.onTrackingPressed,
  });

  final TrackingMode trackingMode;
  final VoidCallback onTrackingPressed;

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;
    final gpsManager = context.watch<GpsManager>();

    return Stack(
      children: [
        SafeArea(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                const Spacer(),
                Padding(
                  padding: EdgeInsets.only(
                    right: 8,
                    bottom: isLandscape ? 16 : screenSize.height * 0.08,
                  ),
                  child: PointerInterceptor(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        TrackingButton(
                          trackingMode: trackingMode,
                          onPressed: onTrackingPressed,
                        ),
                        const AccuracyDisplay(),
                        LayerButton(),
                      ],
                    ),
                  ),
                ),
                const RecordingButtons(),
                const SizedBox(height: 116),
              ],
            ),
          ),
        ),
        RecIndicator(
          isRecording:
              gpsManager.recordingStatus == GpsRecordingStatus.recording,
          blinkDurationMs: 1000,
        ),
      ],
    );
  }
}
