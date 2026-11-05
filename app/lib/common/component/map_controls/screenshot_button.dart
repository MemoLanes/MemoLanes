import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class MapScreenshotButton extends StatelessWidget {
  const MapScreenshotButton({
    super.key,
    required this.onPressed,
    required this.busy,
    required this.backButtonVisible,
  });

  final VoidCallback onPressed;
  final bool busy;
  final bool backButtonVisible;

  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    if (!layout.chromeReady) return const SizedBox.shrink();
    final bounds = layout.screenshotControlBounds(
      backButtonVisible: backButtonVisible,
    );
    if (bounds.isEmpty) return const SizedBox.shrink();
    final colors = context.appColors;
    final button = Semantics(
      liveRegion: busy,
      child: LiquidGlassSurface(
        circular: true,
        child: SizedBox.square(
          dimension: StyleConstants.mapControlSize,
          child: IconButton(
            tooltip: context.tr(
              busy ? 'screenshot.generating' : 'screenshot.button_label',
            ),
            onPressed: busy ? null : onPressed,
            color: colors.deepGreen,
            disabledColor: colors.mutedInkColor,
            padding: EdgeInsets.zero,
            icon: busy
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
    return Positioned.fromRect(
      rect: bounds,
      child: PointerInterceptor(child: button),
    );
  }
}
