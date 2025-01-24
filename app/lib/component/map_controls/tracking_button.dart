import 'package:flutter/material.dart';
import 'package:memolanes/component/base_map_webview.dart' show TrackingMode;

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
          onPressed: onPressed,
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
    );
  }
}
