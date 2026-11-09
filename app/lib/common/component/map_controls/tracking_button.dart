import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/map_controls/map_floating_control.dart';

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
    return MapFloatingControl.button(
      icon: trackingMode == TrackingMode.off
          ? Icons.near_me_disabled
          : Icons.near_me,
      muted: trackingMode != TrackingMode.displayAndTracking,
      onPressed: () {
        AppHaptics.selection();
        onPressed();
      },
    );
  }
}
