import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_checkbox.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum AppOptionTileTrailing { chevron, selection }

class AppOptionTile extends StatelessWidget {
  const AppOptionTile({
    super.key,
    required this.title,
    required this.onTap,
    this.icon,
    this.iconWidget,
    this.subtitle,
    this.selected = false,
    this.trailing = AppOptionTileTrailing.chevron,
    this.backgroundAlpha = 0.76,
  }) : assert(icon == null || iconWidget == null),
       assert(backgroundAlpha >= 0 && backgroundAlpha <= 1),
       assert(!selected || trailing == AppOptionTileTrailing.selection);

  final String title;
  final VoidCallback onTap;
  final IconData? icon;
  final Widget? iconWidget;
  final String? subtitle;
  final bool selected;
  final AppOptionTileTrailing trailing;
  final double backgroundAlpha;

  Widget _buildTrailing() {
    return switch (trailing) {
      AppOptionTileTrailing.chevron => Icon(
        Icons.chevron_right_rounded,
        color: StyleConstants.mutedInkColor,
        size: 22,
      ),
      AppOptionTileTrailing.selection => AppCheckbox.indicator(
        value: selected,
        shape: AppCheckboxShape.circle,
      ),
    };
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      selected: trailing == AppOptionTileTrailing.selection ? selected : null,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(14),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 170),
            curve: Curves.easeOutCubic,
            constraints: const BoxConstraints(minHeight: 58),
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            decoration: BoxDecoration(
              color: selected
                  ? StyleConstants.softGreen.withValues(alpha: 0.88)
                  : StyleConstants.surfaceColor.withValues(
                      alpha: backgroundAlpha,
                    ),
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: selected
                    ? StyleConstants.primaryGreen
                    : StyleConstants.lineColor,
                width: selected ? 1.4 : 1,
              ),
            ),
            child: Row(
              children: [
                if (icon != null || iconWidget != null) ...[
                  AnimatedContainer(
                    duration: const Duration(milliseconds: 170),
                    width: 36,
                    height: 36,
                    decoration: BoxDecoration(
                      color: selected
                          ? StyleConstants.primaryGreen.withValues(alpha: 0.34)
                          : StyleConstants.softGreen,
                      borderRadius: BorderRadius.circular(11),
                    ),
                    alignment: Alignment.center,
                    child:
                        iconWidget ??
                        Icon(icon, color: StyleConstants.deepGreen, size: 20),
                  ),
                  const SizedBox(width: 12),
                ],
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: AppTypography.itemTitle.copyWith(
                          color: StyleConstants.inkColor,
                        ),
                      ),
                      if (subtitle != null) ...[
                        const SizedBox(height: 3),
                        Text(
                          subtitle!,
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          style: AppTypography.caption.copyWith(
                            color: StyleConstants.mutedInkColor,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
                const SizedBox(width: 10),
                _buildTrailing(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
