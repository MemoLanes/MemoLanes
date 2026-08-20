import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/constants/style_constants.dart';

class TrackingButton extends StatelessWidget {
  final TrackingMode trackingMode;
  final VoidCallback onPressed;

  const TrackingButton({
    super.key,
    required this.trackingMode,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      width: 48,
      height: 48,
      decoration: const BoxDecoration(
        color: StyleConstants.glassControlSurfaceColor,
        shape: BoxShape.circle,
        border: Border.fromBorderSide(
          BorderSide(color: StyleConstants.glassControlBorderColor),
        ),
      ),
      child: IconButton(
        onPressed: () {
          AppHaptics.selection();
          onPressed();
        },
        icon: Icon(
          trackingMode == TrackingMode.off
              ? Icons.near_me_disabled
              : Icons.near_me,
          color: trackingMode == TrackingMode.displayAndTracking
              ? StyleConstants.defaultColor
              : StyleConstants.glassControlMutedContentColor,
        ),
        tooltip: trackingMode == TrackingMode.off
            ? 'Enable location tracking'
            : 'Disable location tracking',
      ),
    );
  }
}
