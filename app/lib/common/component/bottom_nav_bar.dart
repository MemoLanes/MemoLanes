import 'dart:ui';

import 'package:badges/badges.dart' as badges;
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/constants/index.dart';

class BottomNavBar extends StatelessWidget {
  final int selectedIndex;
  final ValueChanged<int> onIndexChanged;
  final bool Function() hasUpdateNotification;
  static const int _itemCount = 5;
  static const Duration _selectionSlideDuration = Duration(milliseconds: 260);

  const BottomNavBar({
    super.key,
    required this.selectedIndex,
    required this.onIndexChanged,
    required this.hasUpdateNotification,
  }) : assert(
          selectedIndex >= 0 && selectedIndex < _itemCount,
          'selectedIndex must match a BottomNavBar item index.',
        );

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 2, sigmaY: 2),
        child: Container(
          height: StyleConstants.navBarHeight,
          decoration: BoxDecoration(
            color: Colors.white.withValues(alpha: 0.8),
            borderRadius: BorderRadius.circular(16),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.1),
                blurRadius: 8,
                offset: const Offset(0, 2),
              ),
            ],
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final itemWidth = constraints.maxWidth / BottomNavBar._itemCount;
              return TweenAnimationBuilder<double>(
                tween: Tween<double>(end: selectedIndex.toDouble()),
                duration: BottomNavBar._selectionSlideDuration,
                curve: Curves.easeOutCubic,
                builder: (context, selectionPosition, child) {
                  return Stack(
                    children: [
                      Positioned(
                        left: itemWidth * selectionPosition + 8,
                        top: 8,
                        bottom: 8,
                        width: itemWidth - 16,
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            color: Colors.white.withValues(alpha: 0.5),
                            borderRadius: BorderRadius.circular(8),
                          ),
                        ),
                      ),
                      Row(
                        children: [
                          _buildNavItem(
                            Icons.map_outlined,
                            Icons.map,
                            0,
                            selectionPosition,
                          ),
                          _buildNavItem(
                            Icons.update_outlined,
                            Icons.update,
                            1,
                            selectionPosition,
                          ),
                          _buildNavItem(
                            Icons.route_outlined,
                            Icons.route,
                            2,
                            selectionPosition,
                          ),
                          _buildNavItem(
                            Icons.workspace_premium_outlined,
                            Icons.workspace_premium,
                            3,
                            selectionPosition,
                          ),
                          _buildNavItem(
                            Icons.settings_outlined,
                            Icons.settings,
                            4,
                            selectionPosition,
                          ),
                        ],
                      ),
                    ],
                  );
                },
              );
            },
          ),
        ),
      ),
    );
  }

  Widget _buildNavItem(
    IconData icon,
    IconData activeIcon,
    int index,
    double selectionPosition,
  ) {
    final selectedAmount =
        (1 - (selectionPosition - index).abs()).clamp(0.0, 1.0);

    return Expanded(
      child: GestureDetector(
        onTap: () {
          AppHaptics.selection();
          onIndexChanged(index);
        },
        child: Container(
          color: Colors.transparent,
          padding: const EdgeInsets.all(8),
          child: Padding(
            padding: const EdgeInsets.all(8),
            child: index == 4 && hasUpdateNotification()
                ? badges.Badge(
                    badgeStyle: badges.BadgeStyle(
                      shape: badges.BadgeShape.square,
                      borderRadius: BorderRadius.circular(5),
                      padding: const EdgeInsets.all(2),
                      badgeGradient: const badges.BadgeGradient.linear(
                        colors: [
                          Color(0xFFB7CC1F),
                          Color(0xFFB6E13D),
                          Color(0xFFB7CC1F),
                        ],
                      ),
                    ),
                    badgeContent: const Text(
                      'NEW',
                      style: TextStyle(
                        color: Colors.white,
                        fontSize: 8,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    child: Center(
                      child: _buildNavIcon(icon, activeIcon, selectedAmount),
                    ),
                  )
                : _buildNavIcon(icon, activeIcon, selectedAmount),
          ),
        ),
      ),
    );
  }

  Widget _buildNavIcon(
    IconData icon,
    IconData activeIcon,
    double selectedAmount,
  ) {
    final color = Color.lerp(Colors.grey, Colors.black, selectedAmount)!;

    return SizedBox(
      width: 28,
      height: 28,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Opacity(
            opacity: 1 - selectedAmount,
            child: Icon(icon, color: color, size: 28),
          ),
          Opacity(
            opacity: selectedAmount,
            child: Icon(activeIcon, color: color, size: 28),
          ),
        ],
      ),
    );
  }
}
