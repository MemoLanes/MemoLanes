import 'package:flutter/material.dart';
import 'dart:ui';

class BottomNavBar extends StatelessWidget {
  final int selectedIndex;
  final Function(int) onIndexChanged;

  const BottomNavBar({
    super.key,
    required this.selectedIndex,
    required this.onIndexChanged,
  });

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(20),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
        child: Container(
          decoration: BoxDecoration(
            color: Colors.white.withOpacity(0.8),
            borderRadius: BorderRadius.circular(20),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withOpacity(0.1),
                blurRadius: 8,
                offset: const Offset(0, 2),
              ),
            ],
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                _buildNavItem(0, Icons.map_outlined, Icons.map),
                _buildNavItem(1, Icons.update_outlined, Icons.update),
                _buildNavItem(2, Icons.route_outlined, Icons.route),
                _buildNavItem(3, Icons.settings_outlined, Icons.settings),
                _buildNavItem(4, Icons.data_array_outlined, Icons.data_array),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildNavItem(int index, IconData icon, IconData activeIcon) {
    final isSelected = selectedIndex == index;

    return Expanded(
      child: AspectRatio(
        aspectRatio: 1.0,
        child: GestureDetector(
          onTap: () => onIndexChanged(index),
          behavior: HitTestBehavior.opaque,
          child: Container(
            margin: const EdgeInsets.all(4),
            decoration: isSelected
                ? BoxDecoration(
                    color: Colors.white.withOpacity(0.5),
                    borderRadius: BorderRadius.circular(12),
                  )
                : null,
            child: Icon(
              isSelected ? activeIcon : icon,
              color: isSelected ? Colors.black : Colors.grey,
              size: 28,
            ),
          ),
        ),
      ),
    );
  }
}
