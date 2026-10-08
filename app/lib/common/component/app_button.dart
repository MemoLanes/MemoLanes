import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';

enum AppButtonVariant { primary, secondary, tonal, danger }

enum AppButtonSize { compact, regular, large }

({Color background, Color foreground}) _buttonColors(
  AppButtonVariant variant,
  AppColors colors,
) => switch (variant) {
  AppButtonVariant.primary => (
    background: colors.primaryActionColor,
    foreground: colors.onPrimaryActionColor,
  ),
  AppButtonVariant.secondary => (
    background: colors.surfaceColor,
    foreground: colors.secondaryButtonForeground,
  ),
  AppButtonVariant.tonal => (
    background: colors.selectedSurfaceColor,
    foreground: colors.deepGreen,
  ),
  AppButtonVariant.danger => (
    background: colors.dangerColor,
    foreground: colors.onDangerColor,
  ),
};

class AppButton extends StatelessWidget {
  const AppButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
    this.variant = AppButtonVariant.primary,
    this.size = AppButtonSize.regular,
    this.expand = false,
    this.loading = false,
    this.backgroundAlpha = 1,
    this.borderRadius,
    this.fontSize,
    this.labelMaxLines = 1,
  }) : assert(backgroundAlpha >= 0 && backgroundAlpha <= 1),
       assert(borderRadius == null || borderRadius >= 0),
       assert(fontSize == null || fontSize > 0),
       assert(labelMaxLines > 0);

  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;
  final AppButtonVariant variant;
  final AppButtonSize size;
  final bool expand;
  final bool loading;
  final double backgroundAlpha;
  final double? borderRadius;
  final double? fontSize;
  final int labelMaxLines;

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final buttonColors = _buttonColors(variant, colors);
    final side = variant == AppButtonVariant.secondary
        ? BorderSide(color: colors.lineColor)
        : null;
    final height = switch (size) {
      AppButtonSize.compact => 38.0,
      AppButtonSize.regular => 44.0,
      AppButtonSize.large => 52.0,
    };
    final radius =
        borderRadius ??
        switch (size) {
          AppButtonSize.compact => 13.0,
          AppButtonSize.regular => 14.0,
          AppButtonSize.large => 24.0,
        };
    final typeStyle = switch (size) {
      AppButtonSize.compact => AppTypography.compactButton,
      AppButtonSize.regular => AppTypography.button,
      AppButtonSize.large => AppTypography.largeButton,
    };
    final horizontalPadding = switch (size) {
      AppButtonSize.compact => 10.0,
      AppButtonSize.regular => 16.0,
      AppButtonSize.large => 18.0,
    };
    final iconSize = switch (size) {
      AppButtonSize.compact => 16.0,
      AppButtonSize.regular => 18.0,
      AppButtonSize.large => 20.0,
    };
    final style = FilledButton.styleFrom(
      elevation: 0,
      backgroundColor: buttonColors.background.withValues(
        alpha: backgroundAlpha,
      ),
      foregroundColor: buttonColors.foreground,
      disabledBackgroundColor: colors.lineColor,
      disabledForegroundColor: colors.subtleInkColor,
      side: side,
      padding: EdgeInsets.symmetric(horizontal: horizontalPadding),
      textStyle: typeStyle.copyWith(fontSize: fontSize),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(radius),
      ),
    );

    final iconWidget = loading
        ? SizedBox.square(
            dimension: size == AppButtonSize.compact ? 15 : 17,
            child: CircularProgressIndicator(
              strokeWidth: 2,
              color: buttonColors.foreground,
            ),
          )
        : icon == null
        ? null
        : Icon(icon, size: iconSize);
    final effectiveOnPressed = loading ? null : onPressed;
    final button = iconWidget == null
        ? FilledButton(
            onPressed: effectiveOnPressed,
            style: style,
            child: Text(
              label,
              maxLines: labelMaxLines,
              overflow: TextOverflow.ellipsis,
            ),
          )
        : FilledButton.icon(
            onPressed: effectiveOnPressed,
            style: style,
            icon: iconWidget,
            label: Text(
              label,
              maxLines: labelMaxLines,
              overflow: TextOverflow.ellipsis,
            ),
          );

    return ConstrainedBox(
      constraints: BoxConstraints(
        minWidth: expand ? double.infinity : 0,
        minHeight: height,
      ),
      child: button,
    );
  }
}

class AppIconButton extends StatelessWidget {
  const AppIconButton({
    super.key,
    required this.icon,
    required this.onPressed,
    this.variant = AppButtonVariant.tonal,
    this.size = 42,
    this.backgroundAlpha = 1,
  }) : assert(size > 0),
       assert(backgroundAlpha >= 0 && backgroundAlpha <= 1);

  final IconData icon;
  final VoidCallback? onPressed;
  final AppButtonVariant variant;
  final double size;
  final double backgroundAlpha;

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    final buttonColors = _buttonColors(variant, colors);
    return IconButton.filled(
      onPressed: onPressed,
      style: IconButton.styleFrom(
        backgroundColor: buttonColors.background.withValues(
          alpha: backgroundAlpha,
        ),
        foregroundColor: buttonColors.foreground,
        disabledBackgroundColor: colors.lineColor,
        disabledForegroundColor: colors.subtleInkColor,
        fixedSize: Size.square(size),
        side: variant == AppButtonVariant.secondary
            ? BorderSide(color: colors.lineColor)
            : null,
      ),
      icon: Icon(icon, size: size * 0.48),
    );
  }
}
