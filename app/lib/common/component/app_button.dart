import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum AppButtonVariant {
  primary,
  secondary,
  tonal,
  dangerTonal,
  danger,
}

enum AppButtonSize { compact, regular, large }

extension on AppButtonVariant {
  Color get backgroundColor => switch (this) {
        AppButtonVariant.primary => StyleConstants.primaryActionColor,
        AppButtonVariant.secondary => StyleConstants.surfaceColor,
        AppButtonVariant.tonal => StyleConstants.selectedSurfaceColor,
        AppButtonVariant.dangerTonal => StyleConstants.dangerSurfaceColor,
        AppButtonVariant.danger => StyleConstants.dangerColor,
      };

  Color get foregroundColor => switch (this) {
        AppButtonVariant.primary => StyleConstants.onPrimaryActionColor,
        AppButtonVariant.danger => StyleConstants.onDangerColor,
        AppButtonVariant.dangerTonal => StyleConstants.dangerInkColor,
        AppButtonVariant.secondary => StyleConstants.isDarkMode
            ? StyleConstants.inkColor
            : StyleConstants.onPrimaryActionColor,
        AppButtonVariant.tonal => StyleConstants.deepGreen,
      };

  BorderSide? get side => switch (this) {
        AppButtonVariant.secondary =>
          BorderSide(color: StyleConstants.lineColor),
        _ => null,
      };
}

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
  })  : assert(backgroundAlpha >= 0 && backgroundAlpha <= 1),
        assert(borderRadius == null || borderRadius >= 0),
        assert(fontSize == null || fontSize > 0);

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

  @override
  Widget build(BuildContext context) {
    final height = switch (size) {
      AppButtonSize.compact => 38.0,
      AppButtonSize.regular => 44.0,
      AppButtonSize.large => 52.0,
    };
    final radius = borderRadius ??
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
      backgroundColor:
          variant.backgroundColor.withValues(alpha: backgroundAlpha),
      foregroundColor: variant.foregroundColor,
      disabledBackgroundColor: StyleConstants.lineColor,
      disabledForegroundColor: StyleConstants.subtleInkColor,
      side: variant.side,
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
              color: variant.foregroundColor,
            ),
          )
        : icon == null
            ? null
            : Icon(
                icon,
                size: iconSize,
              );
    final effectiveOnPressed = loading ? null : onPressed;
    final button = iconWidget == null
        ? FilledButton(
            onPressed: effectiveOnPressed,
            style: style,
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          )
        : FilledButton.icon(
            onPressed: effectiveOnPressed,
            style: style,
            icon: iconWidget,
            label: Text(
              label,
              maxLines: 1,
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
    this.tooltip,
    this.variant = AppButtonVariant.tonal,
    this.size = 42,
    this.backgroundAlpha = 1,
  })  : assert(size > 0),
        assert(backgroundAlpha >= 0 && backgroundAlpha <= 1);

  final IconData icon;
  final VoidCallback? onPressed;
  final String? tooltip;
  final AppButtonVariant variant;
  final double size;
  final double backgroundAlpha;

  @override
  Widget build(BuildContext context) {
    return IconButton.filled(
      onPressed: onPressed,
      tooltip: tooltip,
      style: IconButton.styleFrom(
        backgroundColor:
            variant.backgroundColor.withValues(alpha: backgroundAlpha),
        foregroundColor: variant.foregroundColor,
        disabledBackgroundColor: StyleConstants.lineColor,
        disabledForegroundColor: StyleConstants.subtleInkColor,
        fixedSize: Size.square(size),
        side: variant.side,
      ),
      icon: Icon(icon, size: size * 0.48),
    );
  }
}
