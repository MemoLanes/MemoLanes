import 'package:memolanes/theme/app_colors.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';

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
    return LiquidGlassSurface(
      circular: true,
      child: SizedBox(
        width: 44,
        height: 44,
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
                ? context.appColors.deepGreen
                : context.appColors.mutedInkColor,
          ),
        ),
      ),
    );
  }
}
