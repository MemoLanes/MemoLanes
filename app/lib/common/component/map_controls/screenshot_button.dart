import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/map_controls/map_floating_control.dart';

class MapScreenshotButton extends StatelessWidget {
  const MapScreenshotButton({
    super.key,
    required this.onPressed,
    required this.busy,
  });

  final VoidCallback onPressed;
  final bool busy;

  @override
  Widget build(BuildContext context) => MapFloatingControl.button(
    icon: Icons.photo_camera_outlined,
    onPressed: onPressed,
    busy: busy,
    tooltip: context.tr(
      busy ? 'screenshot.generating' : 'screenshot.button_label',
    ),
  );
}
