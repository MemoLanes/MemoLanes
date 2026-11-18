import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/map_controls/accuracy_display.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/tracking_button.dart';
import 'package:memolanes/common/component/rec_indicator.dart';
import 'package:memolanes/common/component/recording_buttons.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:provider/provider.dart';

class NormalMapOverlay extends StatelessWidget {
  const NormalMapOverlay({
    super.key,
    required this.trackingMode,
    required this.onTrackingPressed,
    this.recordingControls,
    this.mapControlsKey,
  });

  final TrackingMode trackingMode;
  final VoidCallback onTrackingPressed;

  /// Replaces the recorder controls and omits the live REC indicator.
  final Widget? recordingControls;
  final Key? mapControlsKey;

  Widget _buildMapControls(Axis axis) {
    return PointerInterceptor(
      child: Flex(
        key: mapControlsKey,
        direction: axis,
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        spacing: StyleConstants.mapControlSpacing,
        children: [
          TrackingButton(
            trackingMode: trackingMode,
            onPressed: onTrackingPressed,
          ),
          const AccuracyDisplay(),
          const LayerButton(),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    if (!layout.chromeReady) return const SizedBox.shrink();
    return Stack(
      fit: StackFit.expand,
      children: [
        Positioned.fromRect(
          rect: layout.recordingControlsBoundsFor(
            const Size(RecordingButtons.maximumWidth, RecordingButtons.height),
            minimumWidth: RecordingButtons.minimumWidth,
          ),
          child: Align(
            heightFactor: 1,
            alignment: Alignment.center,
            child: recordingControls ?? const RecordingButtons(),
          ),
        ),
        Positioned.fromRect(
          rect: layout.mapControlsBounds,
          child: _buildMapControls(layout.mapControlsAxis),
        ),
        if (recordingControls == null)
          Positioned(
            left: layout.recordingIndicatorOffset.dx,
            top: layout.recordingIndicatorOffset.dy,
            child: RecIndicator(
              isRecording: context.select<GpsManager, bool>(
                (gpsManager) =>
                    gpsManager.recordingStatus == GpsRecordingStatus.recording,
              ),
            ),
          ),
      ],
    );
  }
}
