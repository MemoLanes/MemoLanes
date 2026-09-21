import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/constants/index.dart';

import 'app_colors.dart';

abstract final class AppTheme {
  static const _lightSwitchInactiveTrackColor = Color(0xFFDDE2DC);

  static final light = _build(Brightness.light, AppColors.light);
  static final dark = _build(Brightness.dark, AppColors.dark);

  /// The map can be dark regardless of the interface theme.
  static SystemUiOverlayStyle mapSystemOverlayStyle(ThemeData theme) =>
      theme.appBarTheme.systemOverlayStyle!.copyWith(
        statusBarIconBrightness: Brightness.light,
        statusBarBrightness: Brightness.dark,
      );

  static ThemeData _build(Brightness brightness, AppColors colors) {
    final systemOverlayStyle = SystemUiOverlayStyle(
      statusBarColor: Colors.transparent,
      statusBarIconBrightness: brightness == Brightness.dark
          ? Brightness.light
          : Brightness.dark,
      statusBarBrightness: brightness,
      systemNavigationBarColor: colors.canvasColor,
      systemNavigationBarIconBrightness: brightness == Brightness.dark
          ? Brightness.light
          : Brightness.dark,
      systemNavigationBarDividerColor: colors.lineColor,
    );

    final switchInactiveTrackColor = brightness == Brightness.dark
        ? colors.elevatedSurfaceColor
        : _lightSwitchInactiveTrackColor;
    final dialogShape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(24),
      side: BorderSide(color: colors.lineColor),
    );
    final buttonShape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(14),
    );
    final pickerCancelStyle = OutlinedButton.styleFrom(
      foregroundColor: colors.deepGreen,
      side: BorderSide(color: colors.lineColor),
      shape: buttonShape,
    );
    final pickerConfirmStyle = FilledButton.styleFrom(
      backgroundColor: colors.primaryActionColor,
      foregroundColor: colors.onPrimaryActionColor,
      shape: buttonShape,
    );

    return ThemeData(
      useMaterial3: true,
      extensions: [colors],
      fontFamilyFallback: Platform.isIOS
          ? ['.AppleSystemUIFont', 'PingFang SC']
          : null,
      brightness: brightness,
      scaffoldBackgroundColor: colors.canvasColor,
      canvasColor: colors.canvasColor,
      colorScheme:
          ColorScheme.fromSeed(
            seedColor: colors.primaryGreen,
            brightness: brightness,
            primary: colors.primaryActionColor,
            onPrimary: colors.onPrimaryActionColor,
            primaryContainer: colors.selectedSurfaceColor,
            onPrimaryContainer: colors.deepGreen,
            secondary: colors.warningColor,
            onSecondary: colors.strongBadgeBackground,
            secondaryContainer: colors.warningSurfaceColor,
            onSecondaryContainer: colors.warningInkColor,
            error: colors.dangerColor,
            onError: colors.onDangerColor,
            errorContainer: colors.dangerSurfaceColor,
            onErrorContainer: colors.dangerInkColor,
            outline: colors.mutedInkColor,
            outlineVariant: colors.lineColor,
            surface: colors.surfaceColor,
            onSurface: colors.inkColor,
            onSurfaceVariant: colors.mutedInkColor,
            shadow: colors.shadowColor,
            scrim: colors.shadowColor,
            surfaceTint: Colors.transparent,
          ).copyWith(
            surfaceContainerLowest: colors.canvasColor,
            surfaceContainerLow: colors.surfaceColor,
            surfaceContainer: colors.surfaceColor,
            surfaceContainerHigh: colors.elevatedSurfaceColor,
            surfaceContainerHighest: colors.elevatedSurfaceColor,
          ),
      textTheme:
          (brightness == Brightness.dark ? ThemeData.dark() : ThemeData.light())
              .textTheme
              .merge(AppTypography.textTheme)
              .apply(bodyColor: colors.inkColor, displayColor: colors.inkColor),
      iconTheme: IconThemeData(color: colors.inkColor),
      dividerColor: colors.lineColor,
      cardTheme: CardThemeData(
        color: colors.surfaceColor,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        margin: EdgeInsets.zero,
      ),
      appBarTheme: AppBarTheme(
        elevation: 0,
        scrolledUnderElevation: 0,
        backgroundColor: colors.canvasColor,
        foregroundColor: colors.inkColor,
        surfaceTintColor: Colors.transparent,
        systemOverlayStyle: systemOverlayStyle,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          elevation: 0,
          backgroundColor: colors.primaryActionColor,
          foregroundColor: colors.onPrimaryActionColor,
          minimumSize: const Size(0, 44),
          textStyle: AppTypography.button,
          shape: buttonShape,
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          elevation: 0,
          backgroundColor: colors.surfaceColor,
          foregroundColor: colors.secondaryButtonForeground,
          minimumSize: const Size(0, 44),
          textStyle: AppTypography.button,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(14),
            side: BorderSide(color: colors.lineColor),
          ),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: colors.deepGreen,
          minimumSize: const Size(0, 44),
          side: BorderSide(color: colors.lineColor),
          textStyle: AppTypography.button,
          shape: buttonShape,
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: colors.deepGreen,
          textStyle: AppTypography.button,
        ),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: colors.dialogSurface,
        surfaceTintColor: Colors.transparent,
        titleTextStyle: AppTypography.surfaceTitle.copyWith(
          color: colors.deepGreen,
        ),
        contentTextStyle: AppTypography.body.copyWith(color: colors.inkColor),
        shape: dialogShape,
      ),
      datePickerTheme: DatePickerThemeData(
        backgroundColor: colors.dialogSurface,
        surfaceTintColor: Colors.transparent,
        headerBackgroundColor: colors.softGreen,
        headerForegroundColor: colors.deepGreen,
        weekdayStyle: AppTypography.label.copyWith(color: colors.mutedInkColor),
        todayBorder: BorderSide(color: colors.deepGreen),
        shape: dialogShape,
        cancelButtonStyle: pickerCancelStyle,
        confirmButtonStyle: pickerConfirmStyle,
      ),
      timePickerTheme: TimePickerThemeData(
        backgroundColor: colors.canvasColor,
        dialBackgroundColor: colors.surfaceColor,
        dialHandColor: colors.deepGreen,
        hourMinuteColor: colors.softGreen,
        hourMinuteTextColor: colors.inkColor,
        dialTextColor: colors.inkColor,
        dayPeriodColor: colors.surfaceColor,
        dayPeriodTextColor: colors.inkColor,
        entryModeIconColor: colors.deepGreen,
        shape: dialogShape,
        cancelButtonStyle: pickerCancelStyle,
        confirmButtonStyle: pickerConfirmStyle,
      ),
      checkboxTheme: CheckboxThemeData(
        fillColor: WidgetStatePropertyAll(colors.surfaceColor),
        checkColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? colors.mutedInkColor
              : colors.deepGreen,
        ),
        side: WidgetStateBorderSide.resolveWith(
          (states) => BorderSide(
            color: states.contains(WidgetState.disabled)
                ? colors.lineColor
                : colors.deepGreen,
            width: 1.4,
          ),
        ),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(5)),
      ),
      progressIndicatorTheme: ProgressIndicatorThemeData(
        color: colors.primaryActionColor,
        linearTrackColor: colors.lineColor,
        circularTrackColor: colors.lineColor,
      ),
      switchTheme: SwitchThemeData(
        thumbColor: const WidgetStatePropertyAll(Colors.transparent),
        thumbIcon: WidgetStateProperty.resolveWith(
          (states) => Icon(
            Icons.circle,
            size: states.contains(WidgetState.selected)
                ? StyleConstants.switchActiveThumbSize
                : StyleConstants.switchInactiveThumbSize,
            color: states.contains(WidgetState.selected)
                ? colors.switchActiveThumbColor
                : colors.switchInactiveThumbColor,
          ),
        ),
        trackColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? colors.switchActiveTrackColor
              : switchInactiveTrackColor,
        ),
        trackOutlineColor: WidgetStatePropertyAll(
          colors.switchTrackOutlineColor,
        ),
        trackOutlineWidth: const WidgetStatePropertyAll(
          StyleConstants.switchTrackOutlineWidth,
        ),
      ),
      bottomNavigationBarTheme: BottomNavigationBarThemeData(
        elevation: 8,
        backgroundColor: colors.surfaceColor,
        selectedItemColor: colors.deepGreen,
        unselectedItemColor: colors.mutedInkColor,
      ),
    );
  }
}
