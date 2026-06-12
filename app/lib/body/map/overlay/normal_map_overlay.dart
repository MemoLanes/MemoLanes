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

  Widget _buildPortraitLayout(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            const Spacer(),
            Padding(
              padding: EdgeInsets.only(
                right: 8,
                bottom: screenSize.height * 0.08,
              ),
              child: _buildMapControls(),
            ),
            const RecordingButtons(),
            const SizedBox(height: 116),
          ],
        ),
      ),
    );
  }

  Widget _buildLandscapeLayout(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final bottomInset = padding.bottom + StyleConstants.navBarSafeArea + 16.0;

    return Stack(
      children: [
        Positioned(
          right: padding.right + 32.0,
          bottom: bottomInset,
          child: _buildMapControls(),
        ),
        Positioned(
          left: padding.left + 24.0,
          right: padding.right + 24.0,
          bottom: bottomInset,
          child: const Align(
            heightFactor: 1,
            alignment: Alignment.center,
            child: RecordingButtons(),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;
    final gpsManager = context.watch<GpsManager>();

    return Stack(
      children: [
        if (isLandscape)
          _buildLandscapeLayout(context)
        else
          _buildPortraitLayout(context),
        RecIndicator(
          isRecording:
              gpsManager.recordingStatus == GpsRecordingStatus.recording,
          blinkDurationMs: 1000,
        ),
      ],
    );
  }
}
