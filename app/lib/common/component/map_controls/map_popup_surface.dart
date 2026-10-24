import 'package:material_ui/material_ui.dart';
import 'package:memolanes/theme/app_colors.dart';

/// The existing glass appearance shared by GPS and map-layer menus.
class MapPopupSurface extends StatelessWidget {
  const MapPopupSurface({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(16),
    decoration: BoxDecoration(
      color: context.appColors.glassColor.withValues(
        alpha: context.appColors.mapPopupBackgroundAlpha,
      ),
      borderRadius: BorderRadius.circular(24),
      border: Border.all(
        color: context.appColors.glassBorderColor.withValues(
          alpha: context.appColors.mapPopupBorderAlpha,
        ),
      ),
      boxShadow: [
        BoxShadow(
          color: context.appColors.shadowColor.withValues(
            alpha: context.appColors.mapPopupShadowAlpha,
          ),
          blurRadius: 22,
          offset: const Offset(0, 8),
        ),
      ],
    ),
    child: child,
  );
}
