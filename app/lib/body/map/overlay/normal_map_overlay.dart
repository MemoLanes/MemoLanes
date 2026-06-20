import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/map_controls/accuracy_display.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/tracking_button.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/component/recording_buttons.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

class NormalMapOverlay extends StatelessWidget {
  const NormalMapOverlay({
    super.key,
    required this.trackingMode,
    required this.onTrackingPressed,
  });

  final TrackingMode trackingMode;
  final VoidCallback onTrackingPressed;

  Widget _buildMapControls() {
    return PointerInterceptor(
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
    );
  }

  Widget _buildRecordingButtons() {
    return const Align(
      heightFactor: 1,
      alignment: Alignment.center,
      child: RecordingButtons(),
    );
  }

  Widget _buildControlsLayout(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final padding = mediaQuery.viewPadding;
    final horizontalSafeArea = math.max(padding.left, padding.right);
    final isLandscape = mediaQuery.orientation == Orientation.landscape;
    const recordingBottom = StyleConstants.mapPrimaryControlBottomInset;

    return Stack(
      children: isLandscape
          ? [
              Positioned(
                left: horizontalSafeArea + 24.0,
                right: horizontalSafeArea + 24.0,
                bottom: recordingBottom,
                child: _buildRecordingButtons(),
              ),
              Positioned(
                right: padding.right + 32.0,
                bottom: recordingBottom,
                child: _buildMapControls(),
              ),
            ]
          : [
              Positioned(
                left: horizontalSafeArea + 24.0,
                right: horizontalSafeArea + 24.0,
                bottom: recordingBottom,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Align(
                      alignment: Alignment.centerRight,
                      child: Padding(
                        padding: EdgeInsets.only(
                          right: 8.0,
                          bottom: mediaQuery.size.height * 0.08,
                        ),
                        child: _buildMapControls(),
                      ),
                    ),
                    _buildRecordingButtons(),
                  ],
                ),
              ),
            ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final gpsManager = context.watch<GpsManager>();

    return Stack(
      children: [
        _buildControlsLayout(context),
        RecIndicator(
          isRecording:
              gpsManager.recordingStatus == GpsRecordingStatus.recording,
          blinkDurationMs: 1000,
        ),
      ],
    );
  }
}
