import 'package:badges/badges.dart' as badges;
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/frosted_bar_container.dart';
import 'package:memolanes/common/component/frosted_bar_item.dart';
import 'package:memolanes/common/component/frosted_bar_selection_group.dart';

class BottomNavBar extends StatelessWidget {
  final int selectedIndex;
  final Function(int) onIndexChanged;
  final Function hasUpdateNotification;

  // Visual height of the floating navigation capsule.
  static const double height = 64;

  // Original side margin used to derive the unscaled design width.
  // The actual side inset is handled by the surrounding safe-area layout.
  static const double designHorizontalMargin = 24;

  const BottomNavBar({
    super.key,
    required this.selectedIndex,
    required this.onIndexChanged,
    required this.hasUpdateNotification,
  });

  @override
  Widget build(BuildContext context) {
    return FrostedBarContainer(
      extent: height,
      mainAxisPadding: 8,
      child: FrostedBarSelectionGroup(
        selectedIndex: selectedIndex,
        children: [
          _buildNavItem(
            icon: Icons.map_outlined,
            activeIcon: Icons.map,
            label: context.tr('home.navigation.map'),
            index: 0,
          ),
          _buildNavItem(
            icon: Icons.update_outlined,
            activeIcon: Icons.update,
            label: context.tr('home.navigation.time_machine'),
            index: 1,
          ),
          _buildNavItem(
            icon: Icons.route_outlined,
            activeIcon: Icons.route,
            label: context.tr('home.navigation.journey'),
            index: 2,
          ),
          _buildNavItem(
            icon: Icons.workspace_premium_outlined,
            activeIcon: Icons.workspace_premium,
            label: context.tr('home.navigation.achievement'),
            index: 3,
          ),
          _buildNavItem(
            icon: Icons.settings_outlined,
            activeIcon: Icons.settings,
            label: context.tr('home.navigation.settings'),
            index: 4,
          ),
        ],
      ),
    );
  }

  Widget _buildNavItem({
    required IconData icon,
    required IconData activeIcon,
    required String label,
    required int index,
  }) {
    final isSelected = selectedIndex == index;
    final hasUpdateBadge = index == 4 && hasUpdateNotification();

    return FrostedBarItem(
      icon: isSelected ? activeIcon : icon,
      label: label,
      isSelected: isSelected,
      horizontalPadding: 4,
      horizontalMargin: 0,
      onTap: () {
        AppHaptics.selection();
        onIndexChanged(index);
      },
      iconBuilder: hasUpdateBadge
          ? (contentColor) => badges.Badge(
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
                child: Icon(
                  isSelected ? activeIcon : icon,
                  color: contentColor,
                  size: 22,
                ),
              )
          : null,
    );
  }
}
