import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

/// Shared circular surface for map buttons, menu anchors and status readouts.
class MapFloatingControl extends StatelessWidget {
  const MapFloatingControl({super.key, required this.child});

  MapFloatingControl.button({
    super.key,
    required IconData icon,
    required VoidCallback? onPressed,
    String? tooltip,
    bool muted = false,
    bool busy = false,
  }) : child = Builder(
         builder: (context) {
           final colors = context.appColors;
           return Semantics(
             liveRegion: busy,
             child: IconButton(
               tooltip: tooltip,
               onPressed: busy ? null : onPressed,
               color: muted ? colors.mutedInkColor : colors.deepGreen,
               disabledColor: colors.mutedInkColor,
               iconSize: _iconSize,
               padding: EdgeInsets.zero,
               icon: busy
                   ? SizedBox.square(
                       dimension: 20,
                       child: CircularProgressIndicator(
                         strokeWidth: 2,
                         color: colors.deepGreen,
                       ),
                     )
                   : Icon(icon),
             ),
           );
         },
       );

  static const _iconSize = 21.0;
  final Widget child;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: StyleConstants.mapControlSize,
    child: LiquidGlassSurface(
      circular: true,
      child: IconTheme.merge(
        data: IconThemeData(
          size: _iconSize,
          color: context.appColors.deepGreen,
        ),
        child: child,
      ),
    ),
  );
}
