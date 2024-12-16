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
      borderRadius: BorderRadius.circular(16),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 2, sigmaY: 2),
        child: Container(
          height: 64,
          decoration: BoxDecoration(
            color: Colors.white.withOpacity(0.8),
            borderRadius: BorderRadius.circular(16),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withOpacity(0.1),
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
              _buildNavItem(Icons.settings_outlined, Icons.settings, 3),
              _buildNavItem(Icons.data_array_outlined, Icons.data_array, 4),
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
          height: double.infinity,
          margin: const EdgeInsets.all(8),
          decoration: isSelected
              ? BoxDecoration(
                  color: Colors.white.withOpacity(0.5),
                  borderRadius: BorderRadius.circular(8),
                )
              : null,
          child: Icon(
            isSelected ? activeIcon : icon,
            color: isSelected ? Colors.black : Colors.grey,
            size: 28,
          ),
        ),
      ),
    );
  }
}