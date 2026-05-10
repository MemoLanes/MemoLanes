import 'package:flutter/material.dart';

class FrostedBarItem extends StatelessWidget {
  const FrostedBarItem({
    super.key,
    required this.icon,
    required this.label,
    this.isSelected = false,
    this.isEnabled = true,
    this.onTap,
    this.selectedColor = Colors.black,
    this.unselectedColor,
    this.disabledColor,
    this.horizontalPadding = 14,
    this.horizontalMargin = 2,
    this.verticalMargin = 6,
  });

  final IconData icon;
  final String label;
  final bool isSelected;
  final bool isEnabled;
  final VoidCallback? onTap;
  final Color selectedColor;
  final Color? unselectedColor;
  final Color? disabledColor;
  final double horizontalPadding;
  final double horizontalMargin;
  final double verticalMargin;

  @override
  Widget build(BuildContext context) {
    final themeColor = selectedColor;
    final baseUnselectedColor = unselectedColor ?? Colors.grey.shade800;
    final baseDisabledColor = disabledColor ?? Colors.grey.shade400;

    final Color bgColor = isSelected
        ? (isEnabled
            ? themeColor.withValues(alpha: 0.12)
            : Colors.black.withValues(alpha: 0.05))
        : Colors.transparent;

    final Color contentColor = !isEnabled
        ? baseDisabledColor
        : isSelected
            ? themeColor
            : baseUnselectedColor;

    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 250),
        margin: EdgeInsets.symmetric(
          vertical: verticalMargin,
          horizontal: horizontalMargin,
        ),
        padding: EdgeInsets.symmetric(horizontal: horizontalPadding),
        decoration: BoxDecoration(
          color: bgColor,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            SizedBox(
              width: 24,
              height: 24,
              child: Align(
                alignment: Alignment.center,
                child: Icon(icon, color: contentColor, size: 22),
              ),
            ),
            const SizedBox(height: 2),
            Text(
              label,
              maxLines: 1,
              softWrap: false,
              overflow: TextOverflow.fade,
              style: TextStyle(
                fontSize: 10,
                fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
                color: contentColor,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
