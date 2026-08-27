import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/constants/index.dart';

abstract final class AppTheme {
  static SystemUiOverlayStyle systemOverlayStyle(Brightness brightness) {
    final dark = brightness == Brightness.dark;
    return SystemUiOverlayStyle(
      statusBarColor: Colors.transparent,
      statusBarIconBrightness: dark ? Brightness.light : Brightness.dark,
      statusBarBrightness: brightness,
      systemNavigationBarColor: StyleConstants.canvasColor,
      systemNavigationBarIconBrightness:
          dark ? Brightness.light : Brightness.dark,
      systemNavigationBarDividerColor: StyleConstants.lineColor,
    );
  }

  static ThemeData data(
    Brightness brightness, {
    required SystemUiOverlayStyle systemOverlayStyle,
  }) {
    return ThemeData(
      useMaterial3: true,
      fontFamilyFallback:
          Platform.isIOS ? ['.AppleSystemUIFont', 'PingFang SC'] : null,
      brightness: brightness,
      scaffoldBackgroundColor: StyleConstants.canvasColor,
      canvasColor: StyleConstants.canvasColor,
      colorScheme: ColorScheme.fromSeed(
        seedColor: StyleConstants.primaryGreen,
        brightness: brightness,
        primary: StyleConstants.primaryActionColor,
        onPrimary: StyleConstants.onPrimaryActionColor,
        primaryContainer: StyleConstants.selectedSurfaceColor,
        onPrimaryContainer: StyleConstants.deepGreen,
        secondary: StyleConstants.warningColor,
        onSecondary: StyleConstants.isDarkMode
            ? StyleConstants.inverseInkColor
            : StyleConstants.inkColor,
        secondaryContainer: StyleConstants.warningSurfaceColor,
        onSecondaryContainer: StyleConstants.warningInkColor,
        error: StyleConstants.dangerColor,
        onError: StyleConstants.onDangerColor,
        errorContainer: StyleConstants.dangerSurfaceColor,
        onErrorContainer: StyleConstants.dangerInkColor,
        outline: StyleConstants.mutedInkColor,
        outlineVariant: StyleConstants.lineColor,
        surface: StyleConstants.surfaceColor,
        onSurface: StyleConstants.inkColor,
        onSurfaceVariant: StyleConstants.mutedInkColor,
        shadow: StyleConstants.shadowColor,
        scrim: StyleConstants.shadowColor,
        surfaceTint: Colors.transparent,
      ).copyWith(
        surfaceContainerLowest: StyleConstants.canvasColor,
        surfaceContainerLow: StyleConstants.surfaceColor,
        surfaceContainer: StyleConstants.surfaceColor,
        surfaceContainerHigh: StyleConstants.elevatedSurfaceColor,
        surfaceContainerHighest: StyleConstants.elevatedSurfaceColor,
      ),
      textTheme:
          (brightness == Brightness.dark ? ThemeData.dark() : ThemeData.light())
              .textTheme
              .merge(AppTypography.textTheme)
              .apply(
                bodyColor: StyleConstants.inkColor,
                displayColor: StyleConstants.inkColor,
              ),
      iconTheme: IconThemeData(color: StyleConstants.inkColor),
      dividerColor: StyleConstants.lineColor,
      cardTheme: CardThemeData(
        color: StyleConstants.surfaceColor,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        margin: EdgeInsets.zero,
      ),
      appBarTheme: AppBarTheme(
        elevation: 0,
        scrolledUnderElevation: 0,
        backgroundColor: StyleConstants.canvasColor,
        foregroundColor: StyleConstants.inkColor,
        surfaceTintColor: Colors.transparent,
        systemOverlayStyle: systemOverlayStyle,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          elevation: 0,
          backgroundColor: StyleConstants.primaryActionColor,
          foregroundColor: StyleConstants.onPrimaryActionColor,
          minimumSize: const Size(0, 44),
          textStyle: AppTypography.button,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(14),
          ),
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          elevation: 0,
          backgroundColor: StyleConstants.surfaceColor,
          foregroundColor: StyleConstants.isDarkMode
              ? StyleConstants.inkColor
              : StyleConstants.onPrimaryActionColor,
          minimumSize: const Size(0, 44),
          textStyle: AppTypography.button,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(14),
            side: BorderSide(color: StyleConstants.lineColor),
          ),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: StyleConstants.deepGreen,
          minimumSize: const Size(0, 44),
          side: BorderSide(color: StyleConstants.lineColor),
          textStyle: AppTypography.button,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(14),
          ),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: StyleConstants.deepGreen,
          textStyle: AppTypography.button,
        ),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: StyleConstants.isDarkMode
            ? StyleConstants.elevatedSurfaceColor
            : StyleConstants.canvasColor,
        surfaceTintColor: Colors.transparent,
        titleTextStyle: AppTypography.surfaceTitle.copyWith(
          color: StyleConstants.deepGreen,
        ),
        contentTextStyle: AppTypography.body.copyWith(
          color: StyleConstants.inkColor,
        ),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(24),
          side: BorderSide(color: StyleConstants.lineColor),
        ),
      ),
      datePickerTheme: DatePickerThemeData(
        backgroundColor: StyleConstants.isDarkMode
            ? StyleConstants.elevatedSurfaceColor
            : StyleConstants.canvasColor,
        surfaceTintColor: Colors.transparent,
        headerBackgroundColor: StyleConstants.softGreen,
        headerForegroundColor: StyleConstants.deepGreen,
        weekdayStyle: AppTypography.label.copyWith(
          color: StyleConstants.mutedInkColor,
        ),
        todayBorder: BorderSide(color: StyleConstants.deepGreen),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(24),
          side: BorderSide(color: StyleConstants.lineColor),
        ),
        cancelButtonStyle: _outlinedDialogButtonStyle,
        confirmButtonStyle: _filledDialogButtonStyle,
      ),
      timePickerTheme: TimePickerThemeData(
        backgroundColor: StyleConstants.canvasColor,
        dialBackgroundColor: StyleConstants.surfaceColor,
        dialHandColor: StyleConstants.deepGreen,
        hourMinuteColor: StyleConstants.softGreen,
        hourMinuteTextColor: StyleConstants.inkColor,
        dialTextColor: StyleConstants.inkColor,
        dayPeriodColor: StyleConstants.surfaceColor,
        dayPeriodTextColor: StyleConstants.inkColor,
        entryModeIconColor: StyleConstants.deepGreen,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(24),
          side: BorderSide(color: StyleConstants.lineColor),
        ),
        cancelButtonStyle: _outlinedDialogButtonStyle,
        confirmButtonStyle: _filledDialogButtonStyle,
      ),
      checkboxTheme: CheckboxThemeData(
        fillColor: WidgetStatePropertyAll(StyleConstants.surfaceColor),
        checkColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.disabled)
              ? StyleConstants.mutedInkColor
              : StyleConstants.deepGreen,
        ),
        side: WidgetStateBorderSide.resolveWith(
          (states) => BorderSide(
            color: states.contains(WidgetState.disabled)
                ? StyleConstants.lineColor
                : StyleConstants.deepGreen,
            width: 1.4,
          ),
        ),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(5),
        ),
      ),
      progressIndicatorTheme: ProgressIndicatorThemeData(
        color: StyleConstants.primaryActionColor,
        linearTrackColor: StyleConstants.lineColor,
        circularTrackColor: StyleConstants.lineColor,
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
                ? StyleConstants.switchActiveThumbColor
                : StyleConstants.switchInactiveThumbColor,
          ),
        ),
        trackColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? StyleConstants.switchActiveTrackColor
              : StyleConstants.switchInactiveTrackColor,
        ),
        trackOutlineColor: WidgetStatePropertyAll(
          StyleConstants.switchTrackOutlineColor,
        ),
        trackOutlineWidth: const WidgetStatePropertyAll(
          StyleConstants.switchTrackOutlineWidth,
        ),
      ),
      bottomNavigationBarTheme: BottomNavigationBarThemeData(
        elevation: 8,
        backgroundColor: StyleConstants.surfaceColor,
        selectedItemColor: StyleConstants.deepGreen,
        unselectedItemColor: StyleConstants.mutedInkColor,
      ),
    );
  }

  static ButtonStyle get _outlinedDialogButtonStyle =>
      OutlinedButton.styleFrom(
        foregroundColor: StyleConstants.deepGreen,
        side: BorderSide(color: StyleConstants.lineColor),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
        ),
      );

  static ButtonStyle get _filledDialogButtonStyle => FilledButton.styleFrom(
        backgroundColor: StyleConstants.primaryActionColor,
        foregroundColor: StyleConstants.onPrimaryActionColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
        ),
      );
}
