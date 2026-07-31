import 'package:flutter/material.dart';

class FrostedBarItem extends StatelessWidget {
  const FrostedBarItem({
    super.key,
    required this.icon,
    required this.label,
    this.isSelected = false,
    this.isEnabled = true,
    this.showSelectionBackground = true,
    this.onTap,
    this.iconBuilder,
    this.selectedColor = Colors.black,
    this.unselectedColor,
    this.disabledColor,
    this.horizontalPadding = 14,
    this.horizontalMargin = 2,
    this.verticalMargin = 6,
    this.selectionAnimationDuration = const Duration(milliseconds: 250),
  });

  final IconData icon;
  final String label;
  final bool isSelected;
  final bool isEnabled;
  final bool showSelectionBackground;
  final VoidCallback? onTap;
  final Widget Function(Color contentColor)? iconBuilder;
  final Color selectedColor;
  final Color? unselectedColor;
  final Color? disabledColor;
  final double horizontalPadding;
  final double horizontalMargin;
  final double verticalMargin;
  final Duration selectionAnimationDuration;

  @override
  Widget build(BuildContext context) {
    final themeColor = selectedColor;
    final baseUnselectedColor = unselectedColor ?? Colors.grey;
    final baseDisabledColor = disabledColor ?? Colors.grey.shade500;

    final Color bgColor = showSelectionBackground && isSelected
        ? (isEnabled
            ? Colors.white.withValues(alpha: 0.5)
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
        duration: selectionAnimationDuration,
        margin: EdgeInsets.symmetric(
          vertical: verticalMargin,
          horizontal: horizontalMargin,
        ),
        decoration: BoxDecoration(
          color: bgColor,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Padding(
              padding: EdgeInsets.symmetric(horizontal: horizontalPadding),
              child: SizedBox(
                width: 24,
                height: 24,
                child: Align(
                  alignment: Alignment.center,
                  child: iconBuilder?.call(contentColor) ??
                      Icon(icon, color: contentColor, size: 22),
                ),
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
