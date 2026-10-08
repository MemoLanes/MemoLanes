import 'dart:ui';

import 'package:badges/badges.dart' as badges;
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/theme/app_colors.dart';

class BottomNavBar extends StatelessWidget {
  const BottomNavBar({
    super.key,
    required this.selectedIndex,
    required this.onIndexChanged,
    required this.hasUpdateNotification,
  });

  final int selectedIndex;
  final ValueChanged<int> onIndexChanged;
  final bool Function() hasUpdateNotification;

  static const double height = StyleConstants.navBarHeight;
  static const double designHorizontalMargin = 24;

  Alignment get _selectionAlignment => switch (selectedIndex) {
    0 => Alignment.centerLeft,
    1 => const Alignment(-0.5, 0),
    2 => Alignment.center,
    3 => const Alignment(0.5, 0),
    _ => Alignment.centerRight,
  };

  @override
  Widget build(BuildContext context) {
    final darkProgress = context.appColors.darkProgress;
    final glowEdge = Color.lerp(
      Colors.white,
      context.appColors.primaryGreen,
      darkProgress,
    )!.withValues(alpha: 0);
    final glowCenter = Color.lerp(
      AppColors.light.softGreen,
      context.appColors.primaryGreen,
      darkProgress,
    )!.withValues(alpha: lerpDouble(0.54, 0.22, darkProgress)!);
    final selectionColor = Color.lerp(
      AppColors.light.strongLineColor,
      context.appColors.primaryGreen,
      darkProgress,
    )!.withValues(alpha: lerpDouble(0.22, 0.16, darkProgress)!);

    return LiquidGlassSurface(
      borderRadius: BorderRadius.circular(24),
      backgroundAlpha: lerpDouble(0.56, 0.86, darkProgress)!,
      blurSigma: 28,
      reflectionAlpha: 0.2,
      child: Stack(
        fit: StackFit.expand,
        children: [
          Positioned(
            left: 42,
            right: 42,
            bottom: 1,
            height: 1,
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  colors: [glowEdge, glowCenter, glowEdge],
                ),
              ),
            ),
          ),
          AnimatedAlign(
            duration: const Duration(milliseconds: 460),
            curve: Curves.easeOutCubic,
            alignment: _selectionAlignment,
            child: TweenAnimationBuilder<double>(
              key: ValueKey(selectedIndex),
              tween: Tween(begin: 0, end: 1),
              duration: const Duration(milliseconds: 520),
              curve: Curves.linear,
              builder: (context, progress, child) {
                const expansionEnd = 0.38;
                final expansionProgress = (progress / expansionEnd).clamp(
                  0.0,
                  1.0,
                );
                final settlingProgress =
                    ((progress - expansionEnd) / (1 - expansionEnd)).clamp(
                      0.0,
                      1.0,
                    );
                final expansion = progress <= expansionEnd
                    ? Curves.easeOutCubic.transform(expansionProgress)
                    : 1 - Curves.easeOutCubic.transform(settlingProgress);

                return Transform.scale(
                  scaleX: 1 + expansion * 0.16,
                  scaleY: 1 + expansion * 0.12,
                  alignment: Alignment.center,
                  child: child,
                );
              },
              child: FractionallySizedBox(
                widthFactor: 0.2,
                heightFactor: 1,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 4,
                    vertical: 5,
                  ),
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(19),
                    child: BackdropFilter(
                      enabled: StyleConstants.enableBackdropFilter,
                      filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          color: selectionColor,
                          borderRadius: BorderRadius.circular(19),
                          boxShadow: [
                            BoxShadow(
                              color: context.appColors.deepGreen.withValues(
                                alpha: 0.14,
                              ),
                              blurRadius: 12,
                              spreadRadius: -1,
                              offset: const Offset(0, 4),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
          Material(
            color: Colors.transparent,
            child: Row(
              children: [
                _buildNavItem(
                  context,
                  Icons.explore_outlined,
                  Icons.explore_rounded,
                  0,
                ),
                _buildNavItem(
                  context,
                  Icons.access_time_rounded,
                  Icons.history_rounded,
                  1,
                ),
                _buildNavItem(context, Icons.route_outlined, Icons.route, 2),
                _buildNavItem(
                  context,
                  Icons.emoji_events_outlined,
                  Icons.emoji_events_rounded,
                  3,
                ),
                _buildNavItem(
                  context,
                  Icons.settings_outlined,
                  Icons.settings_rounded,
                  4,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildNavItem(
    BuildContext context,
    IconData icon,
    IconData activeIcon,
    int index,
  ) {
    final isSelected = selectedIndex == index;

    return Expanded(
      child: InkWell(
        onTap: () {
          if (!isSelected) AppHaptics.selection();
          onIndexChanged(index);
        },
        borderRadius: BorderRadius.circular(22),
        child: Center(
          child: badges.Badge(
            showBadge: index == 4 && hasUpdateNotification(),
            position: badges.BadgePosition.topEnd(top: -4, end: -5),
            badgeStyle: badges.BadgeStyle(
              badgeColor: context.appColors.warningColor,
              padding: EdgeInsets.all(4),
            ),
            child: TweenAnimationBuilder<double>(
              tween: Tween(begin: 0, end: isSelected ? 1 : 0),
              duration: const Duration(milliseconds: 420),
              curve: Curves.easeInOutCubic,
              builder: (context, progress, _) {
                // Both selecting and deselecting pass through the midpoint.
                // Blur peaks there, hiding the glyph swap and creating a
                // short refraction-like focus transition.
                final blurProgress = 4 * progress * (1 - progress);
                final blurSigma = blurProgress * 2.2;
                final scale = 1 + progress * 0.1 + blurProgress * 0.045;
                final color = Color.lerp(
                  context.appColors.mutedInkColor,
                  context.appColors.deepGreen,
                  progress,
                );

                return Transform.scale(
                  scale: scale,
                  child: ImageFiltered(
                    imageFilter: ImageFilter.blur(
                      sigmaX: blurSigma,
                      sigmaY: blurSigma,
                      tileMode: TileMode.decal,
                    ),
                    child: Icon(
                      isSelected ? activeIcon : icon,
                      color: color,
                      size: 27 + progress * 2,
                    ),
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}
