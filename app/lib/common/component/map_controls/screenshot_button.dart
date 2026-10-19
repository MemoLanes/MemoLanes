import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/theme/app_colors.dart';

/// Visible screenshot entry shared by the three map views.
class MapScreenshotButton extends StatelessWidget {
  const MapScreenshotButton({super.key, required this.onPressed});

  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    const radius = BorderRadius.all(Radius.circular(24));
    final color = onPressed == null
        ? context.appColors.mutedInkColor
        : context.appColors.deepGreen;

    return LiquidGlassSurface(
      borderRadius: radius,
      blurSigma: 28,
      reflectionAlpha: 0.1,
      child: Tooltip(
        message: context.tr('screenshot.button_label'),
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onPressed,
            borderRadius: radius,
            child: SizedBox.square(
              dimension: 48,
              child: Icon(Icons.crop_free_rounded, size: 20, color: color),
            ),
          ),
        ),
      ),
    );
  }
}
