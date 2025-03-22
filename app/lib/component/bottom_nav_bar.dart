import 'dart:ui';

import 'package:badges/badges.dart' as badges;
import 'package:flutter/material.dart';

class BottomNavBar extends StatelessWidget {
  final int selectedIndex;
  final Function(int) onIndexChanged;
  final Function hasUpdateNotification;

  const BottomNavBar({
    super.key,
    required this.selectedIndex,
    required this.onIndexChanged,
    required this.hasUpdateNotification,
  });

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 2, sigmaY: 2),
        child: Container(
          height: 64,
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
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [
              _buildNavItem(Icons.map_outlined, Icons.map, 0),
              _buildNavItem(Icons.update_outlined, Icons.update, 1),
              _buildNavItem(Icons.route_outlined, Icons.route, 2),
              _buildNavItem(
                  Icons.workspace_premium_outlined, Icons.workspace_premium, 3),
              _buildNavItem(Icons.settings_outlined, Icons.settings, 4),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildNavItem(IconData icon, IconData activeIcon, int index) {
    final isSelected = selectedIndex == index;

    return Expanded(
      child: GestureDetector(
        onTap: () => onIndexChanged(index),
        child: Container(
          color: Colors.transparent,
          padding: const EdgeInsets.all(8),
          child: Container(
            decoration: isSelected
                ? BoxDecoration(
                    color: Colors.white.withValues(alpha: 0.5),
                    borderRadius: BorderRadius.circular(8),
                  )
                : null,
            child: Padding(
              padding: const EdgeInsets.all(8),
              child: index == 3 && hasUpdateNotification()
                  ? badges.Badge(
                      badgeStyle: badges.BadgeStyle(
                        shape: badges.BadgeShape.square,
                        borderRadius: BorderRadius.circular(5),
                        padding: const EdgeInsets.all(2),
                        badgeGradient: const badges.BadgeGradient.linear(
                          colors: [
                            Color.fromARGB(255, 129, 225, 19),
                            Color.fromARGB(255, 9, 177, 17),
                            Color.fromARGB(255, 129, 225, 19),
                          ],
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                        ),
                      ),
                      position: badges.BadgePosition.topEnd(top: 5, end: -6),
                      badgeContent: const Text(
                        'NEW',
                        style: TextStyle(
                            color: Colors.white,
                            fontSize: 8,
                            fontWeight: FontWeight.bold),
                      ),
                      child: Icon(
                        isSelected ? activeIcon : icon,
                        color: isSelected ? Colors.black : Colors.grey,
                        size: 28,
                      ),
                    )
                  : Icon(
                      isSelected ? activeIcon : icon,
                      color: isSelected ? Colors.black : Colors.grey,
                      size: 28,
                    ),
            ),
          ),
        ),
      ),
    );
  }
}
