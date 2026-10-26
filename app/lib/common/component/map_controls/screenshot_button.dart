import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

/// A single tap opens the preview; the pending capture owns the busy state.
class MapScreenshotButton extends StatefulWidget {
  const MapScreenshotButton({super.key, required this.onPressed});

  final Future<void> Function()? onPressed;

  @override
  State<MapScreenshotButton> createState() => _MapScreenshotButtonState();
}

class _MapScreenshotButtonState extends State<MapScreenshotButton> {
  bool _busy = false;

  Future<void> _capture() async {
    final capture = widget.onPressed;
    if (_busy || capture == null) return;
    // Guard immediately, including another tap before the next frame.
    setState(() => _busy = true);
    try {
      await capture();
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return Semantics(
      liveRegion: _busy,
      child: LiquidGlassSurface(
        circular: true,
        backgroundAlpha: 0.62,
        reflectionAlpha: 0.1,
        child: SizedBox.square(
          dimension: StyleConstants.mapControlSize,
          child: IconButton(
            tooltip: context.tr(
              _busy ? 'screenshot.generating' : 'screenshot.button_label',
            ),
            onPressed: _busy || widget.onPressed == null ? null : _capture,
            color: colors.deepGreen,
            disabledColor: colors.mutedInkColor,
            padding: EdgeInsets.zero,
            icon: _busy
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
