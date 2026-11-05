import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

class MapScreenshotButton extends StatelessWidget {
  const MapScreenshotButton({
    super.key,
    required this.loading,
    required this.onPressed,
  });

  final bool loading;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return Semantics(
      liveRegion: loading,
      child: LiquidGlassSurface(
        circular: true,
        child: SizedBox.square(
          dimension: StyleConstants.mapControlSize,
          child: IconButton(
            tooltip: context.tr(
              loading ? 'screenshot.generating' : 'screenshot.button_label',
            ),
            onPressed: loading ? null : onPressed,
            color: colors.deepGreen,
            disabledColor: colors.mutedInkColor,
            padding: EdgeInsets.zero,
            icon: loading
                ? SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: colors.deepGreen,
                    ),
                  )
                : const Icon(Icons.photo_camera_outlined, size: 21),
          ),
        ),
      ),
    );
  }
}
