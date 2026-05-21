import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/pressable_button.dart';

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
      child: Tooltip(
        message: trackingMode == TrackingMode.off
            ? 'Enable location tracking'
            : 'Disable location tracking',
        child: PressableButton.circle(
          backgroundColor: Colors.black,
          overlayColor: Colors.white.withValues(alpha: 0.18),
          onPressed: () {
            AppHaptics.selection();
            onPressed();
          },
          child: Icon(
            trackingMode == TrackingMode.off
                ? Icons.near_me_disabled
                : Icons.near_me,
            color: trackingMode == TrackingMode.displayAndTracking
                ? const Color(0xFFB4EC51)
                : const Color(0xFFB4EC51).withValues(alpha: 0.5),
          ),
        ),
      ),
    );
  }
}
