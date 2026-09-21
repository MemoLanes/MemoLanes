import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';

/// Shared semantic colors and surface effects. Map fog is a separate preference.
/// Component-only effects belong with their widgets and interpolate using
/// [darkProgress], keeping them in sync with Flutter's animated theme.
class AppColors extends ThemeExtension<AppColors> {
  const AppColors({
    required this.canvasColor,
    required this.surfaceColor,
    required this.elevatedSurfaceColor,
    required this.inkColor,
    required this.mutedInkColor,
    required this.subtleInkColor,
    required this.lineColor,
    required this.strongLineColor,
    required this.inverseInkColor,
    required this.shadowColor,
    required this.glassColor,
    required this.glassBorderColor,
    required this.glassHighlightColor,
    required this.deepGreen,
    required this.softGreen,
    required this.journeyYellow,
    required this.deepYellow,
    required this.softYellow,
    required this.onPrimaryActionColor,
    required this.dangerColor,
    required this.dangerInkColor,
    required this.dangerSurfaceColor,
    required this.recordingColor,
    required this.statusFairColor,
    required this.achievementGoldColor,
    required this.mapOverlayShadowAlpha,
    required this.pickerBarrierAlpha,
    required this.mapPopupBackgroundAlpha,
    required this.mapPopupBorderAlpha,
    required this.mapPopupShadowAlpha,
    required this.popupShadowAlpha,
    required this.strongBadgeBackground,
    required this.secondaryButtonForeground,
    required this.dialogSurface,
    required this.darkProgress,
  });

  // Reused palette values. Keep each literal in one place while semantic
  // aliases below continue to describe how the color is used.
  static const _lightCanvas = Color(0xFFFAFBF5);
  static const _lightInk = Color(0xFF182016);
  static const _lightDeepGreen = Color(0xFF3F9154);
  static const _darkElevatedSurface = Color(0xFF1C2620);
  static const _darkInk = Color(0xFFF1F5EF);
  static const _darkInverseInk = Color(0xFF10150F);
  static const _primaryGreen = Color(0xFFB8EA72);
  static const _onStrong = Color(0xFFF8FBF6);

  final Color canvasColor;
  final Color surfaceColor;
  final Color elevatedSurfaceColor;
  final Color inkColor;
  final Color mutedInkColor;
  final Color subtleInkColor;
  final Color lineColor;
  final Color strongLineColor;
  final Color inverseInkColor;
  final Color shadowColor;
  final Color glassColor;
  final Color glassBorderColor;
  final Color glassHighlightColor;
  final Color deepGreen;
  final Color softGreen;
  final Color journeyYellow;
  final Color deepYellow;
  final Color softYellow;
  final Color onPrimaryActionColor;
  final Color dangerColor;
  final Color dangerInkColor;
  final Color dangerSurfaceColor;
  final Color recordingColor;
  final Color statusFairColor;
  final Color achievementGoldColor;
  final double mapOverlayShadowAlpha;
  final double pickerBarrierAlpha;
  final double mapPopupBackgroundAlpha;
  final double mapPopupBorderAlpha;
  final double mapPopupShadowAlpha;
  final double popupShadowAlpha;
  final Color strongBadgeBackground;
  final Color secondaryButtonForeground;
  final Color dialogSurface;

  /// Animated light-to-dark position; also preserves interrupted transitions.
  final double darkProgress;

  Color get primaryGreen => _primaryGreen;
  Color get onStrongColor => _onStrong;
  Color get selectedCalendarInk => strongBadgeBackground;
  Color get badgeForeground => inverseInkColor;
  Color get onDangerColor => inverseInkColor;
  Color get primaryActionColor => primaryGreen;
  Color get selectedSurfaceColor => softGreen;
  Color get warningColor => journeyYellow;
  Color get warningInkColor => deepYellow;
  Color get warningSurfaceColor => softYellow;
  Color get statusExcellentColor => deepGreen;
  Color get statusGoodColor => deepYellow;
  Color get statusPoorColor => dangerColor;
  Color get profileAccentStartColor => deepGreen;
  Color get profileAccentEndColor => deepYellow;
  Color get switchActiveTrackColor => softGreen;
  Color get switchActiveThumbColor => deepGreen;
  Color get switchInactiveThumbColor => subtleInkColor;
  Color get switchTrackOutlineColor => strongLineColor;

  static const light = AppColors(
    canvasColor: _lightCanvas,
    surfaceColor: Colors.white,
    elevatedSurfaceColor: Colors.white,
    inkColor: _lightInk,
    mutedInkColor: Color(0xFF6C7567),
    subtleInkColor: Color(0xFF9CA59F),
    lineColor: Color(0xFFE7EBD9),
    strongLineColor: Color(0xFFC8D1C1),
    inverseInkColor: Colors.white,
    shadowColor: _lightInk,
    glassColor: Colors.white,
    glassBorderColor: Colors.white,
    glassHighlightColor: Colors.white,
    deepGreen: _lightDeepGreen,
    softGreen: Color(0xFFECF9D9),
    journeyYellow: Color(0xFFFFD72E),
    deepYellow: Color(0xFF8B6600),
    softYellow: Color(0xFFFFF5BD),
    onPrimaryActionColor: _lightDeepGreen,
    dangerColor: Color(0xFFC7485D),
    dangerInkColor: Color(0xFF8F2F42),
    dangerSurfaceColor: Color(0xFFFBEAEC),
    recordingColor: Color(0xFFD95357),
    statusFairColor: Color(0xFFB56C32),
    achievementGoldColor: Color(0xFFB88722),
    mapOverlayShadowAlpha: 0.18,
    pickerBarrierAlpha: 0.2,
    mapPopupBackgroundAlpha: 0.68,
    mapPopupBorderAlpha: 0.8,
    mapPopupShadowAlpha: 0.14,
    popupShadowAlpha: 0.12,
    strongBadgeBackground: _lightInk,
    secondaryButtonForeground: _lightDeepGreen,
    dialogSurface: _lightCanvas,
    darkProgress: 0,
  );

  static const dark = AppColors(
    canvasColor: Color(0xFF0B100D),
    surfaceColor: Color(0xFF171918),
    elevatedSurfaceColor: _darkElevatedSurface,
    inkColor: _darkInk,
    mutedInkColor: Color(0xFFA2ADA6),
    subtleInkColor: Color(0xFF77837B),
    lineColor: Color(0xFF2B3730),
    strongLineColor: Color(0xFF44534A),
    inverseInkColor: _darkInverseInk,
    shadowColor: Colors.black,
    glassColor: Color(0xFF111814),
    glassBorderColor: Color(0xFF718078),
    glassHighlightColor: Color(0xFFE5F5E8),
    deepGreen: Color(0xFF8ACB55),
    softGreen: Color(0xFF21331F),
    journeyYellow: Color(0xFFFFD75A),
    deepYellow: Color(0xFFF0C74B),
    softYellow: Color(0xFF352F19),
    onPrimaryActionColor: _darkInverseInk,
    dangerColor: Color(0xFFFF6F7D),
    dangerInkColor: Color(0xFFFF9AA4),
    dangerSurfaceColor: Color(0xFF3A2026),
    recordingColor: Color(0xFFFF6268),
    statusFairColor: Color(0xFFE89A55),
    achievementGoldColor: Color(0xFFE1B84A),
    mapOverlayShadowAlpha: 0.42,
    pickerBarrierAlpha: 0.58,
    mapPopupBackgroundAlpha: 0.94,
    mapPopupBorderAlpha: 0.48,
    mapPopupShadowAlpha: 0.48,
    popupShadowAlpha: 0.42,
    strongBadgeBackground: _darkInverseInk,
    secondaryButtonForeground: _darkInk,
    dialogSurface: _darkElevatedSurface,
    darkProgress: 1,
  );

  @override
  AppColors copyWith({
    Color? canvasColor,
    Color? surfaceColor,
    Color? elevatedSurfaceColor,
    Color? inkColor,
    Color? mutedInkColor,
    Color? subtleInkColor,
    Color? lineColor,
    Color? strongLineColor,
    Color? inverseInkColor,
    Color? shadowColor,
    Color? glassColor,
    Color? glassBorderColor,
    Color? glassHighlightColor,
    Color? deepGreen,
    Color? softGreen,
    Color? journeyYellow,
    Color? deepYellow,
    Color? softYellow,
    Color? onPrimaryActionColor,
    Color? dangerColor,
    Color? dangerInkColor,
    Color? dangerSurfaceColor,
    Color? recordingColor,
    Color? statusFairColor,
    Color? achievementGoldColor,
    double? mapOverlayShadowAlpha,
    double? pickerBarrierAlpha,
    double? mapPopupBackgroundAlpha,
    double? mapPopupBorderAlpha,
    double? mapPopupShadowAlpha,
    double? popupShadowAlpha,
    Color? strongBadgeBackground,
    Color? secondaryButtonForeground,
    Color? dialogSurface,
    double? darkProgress,
  }) => AppColors(
    canvasColor: canvasColor ?? this.canvasColor,
    surfaceColor: surfaceColor ?? this.surfaceColor,
    elevatedSurfaceColor: elevatedSurfaceColor ?? this.elevatedSurfaceColor,
    inkColor: inkColor ?? this.inkColor,
    mutedInkColor: mutedInkColor ?? this.mutedInkColor,
    subtleInkColor: subtleInkColor ?? this.subtleInkColor,
    lineColor: lineColor ?? this.lineColor,
    strongLineColor: strongLineColor ?? this.strongLineColor,
    inverseInkColor: inverseInkColor ?? this.inverseInkColor,
    shadowColor: shadowColor ?? this.shadowColor,
    glassColor: glassColor ?? this.glassColor,
    glassBorderColor: glassBorderColor ?? this.glassBorderColor,
    glassHighlightColor: glassHighlightColor ?? this.glassHighlightColor,
    deepGreen: deepGreen ?? this.deepGreen,
    softGreen: softGreen ?? this.softGreen,
    journeyYellow: journeyYellow ?? this.journeyYellow,
    deepYellow: deepYellow ?? this.deepYellow,
    softYellow: softYellow ?? this.softYellow,
    onPrimaryActionColor: onPrimaryActionColor ?? this.onPrimaryActionColor,
    dangerColor: dangerColor ?? this.dangerColor,
    dangerInkColor: dangerInkColor ?? this.dangerInkColor,
    dangerSurfaceColor: dangerSurfaceColor ?? this.dangerSurfaceColor,
    recordingColor: recordingColor ?? this.recordingColor,
    statusFairColor: statusFairColor ?? this.statusFairColor,
    achievementGoldColor: achievementGoldColor ?? this.achievementGoldColor,
    mapOverlayShadowAlpha: mapOverlayShadowAlpha ?? this.mapOverlayShadowAlpha,
    pickerBarrierAlpha: pickerBarrierAlpha ?? this.pickerBarrierAlpha,
    mapPopupBackgroundAlpha:
        mapPopupBackgroundAlpha ?? this.mapPopupBackgroundAlpha,
    mapPopupBorderAlpha: mapPopupBorderAlpha ?? this.mapPopupBorderAlpha,
    mapPopupShadowAlpha: mapPopupShadowAlpha ?? this.mapPopupShadowAlpha,
    popupShadowAlpha: popupShadowAlpha ?? this.popupShadowAlpha,
    strongBadgeBackground: strongBadgeBackground ?? this.strongBadgeBackground,
    secondaryButtonForeground:
        secondaryButtonForeground ?? this.secondaryButtonForeground,
    dialogSurface: dialogSurface ?? this.dialogSurface,
    darkProgress: darkProgress ?? this.darkProgress,
  );

  @override
  AppColors lerp(covariant AppColors? other, double t) {
    if (other == null) return this;
    return AppColors(
      canvasColor: Color.lerp(canvasColor, other.canvasColor, t)!,
      surfaceColor: Color.lerp(surfaceColor, other.surfaceColor, t)!,
      elevatedSurfaceColor: Color.lerp(
        elevatedSurfaceColor,
        other.elevatedSurfaceColor,
        t,
      )!,
      inkColor: Color.lerp(inkColor, other.inkColor, t)!,
      mutedInkColor: Color.lerp(mutedInkColor, other.mutedInkColor, t)!,
      subtleInkColor: Color.lerp(subtleInkColor, other.subtleInkColor, t)!,
      lineColor: Color.lerp(lineColor, other.lineColor, t)!,
      strongLineColor: Color.lerp(strongLineColor, other.strongLineColor, t)!,
      inverseInkColor: Color.lerp(inverseInkColor, other.inverseInkColor, t)!,
      shadowColor: Color.lerp(shadowColor, other.shadowColor, t)!,
      glassColor: Color.lerp(glassColor, other.glassColor, t)!,
      glassBorderColor: Color.lerp(
        glassBorderColor,
        other.glassBorderColor,
        t,
      )!,
      glassHighlightColor: Color.lerp(
        glassHighlightColor,
        other.glassHighlightColor,
        t,
      )!,
      deepGreen: Color.lerp(deepGreen, other.deepGreen, t)!,
      softGreen: Color.lerp(softGreen, other.softGreen, t)!,
      journeyYellow: Color.lerp(journeyYellow, other.journeyYellow, t)!,
      deepYellow: Color.lerp(deepYellow, other.deepYellow, t)!,
      softYellow: Color.lerp(softYellow, other.softYellow, t)!,
      onPrimaryActionColor: Color.lerp(
        onPrimaryActionColor,
        other.onPrimaryActionColor,
        t,
      )!,
      dangerColor: Color.lerp(dangerColor, other.dangerColor, t)!,
      dangerInkColor: Color.lerp(dangerInkColor, other.dangerInkColor, t)!,
      dangerSurfaceColor: Color.lerp(
        dangerSurfaceColor,
        other.dangerSurfaceColor,
        t,
      )!,
      recordingColor: Color.lerp(recordingColor, other.recordingColor, t)!,
      statusFairColor: Color.lerp(statusFairColor, other.statusFairColor, t)!,
      achievementGoldColor: Color.lerp(
        achievementGoldColor,
        other.achievementGoldColor,
        t,
      )!,
      mapOverlayShadowAlpha: lerpDouble(
        mapOverlayShadowAlpha,
        other.mapOverlayShadowAlpha,
        t,
      )!,
      pickerBarrierAlpha: lerpDouble(
        pickerBarrierAlpha,
        other.pickerBarrierAlpha,
        t,
      )!,
      mapPopupBackgroundAlpha: lerpDouble(
        mapPopupBackgroundAlpha,
        other.mapPopupBackgroundAlpha,
        t,
      )!,
      mapPopupBorderAlpha: lerpDouble(
        mapPopupBorderAlpha,
        other.mapPopupBorderAlpha,
        t,
      )!,
      mapPopupShadowAlpha: lerpDouble(
        mapPopupShadowAlpha,
        other.mapPopupShadowAlpha,
        t,
      )!,
      popupShadowAlpha: lerpDouble(
        popupShadowAlpha,
        other.popupShadowAlpha,
        t,
      )!,
      strongBadgeBackground: Color.lerp(
        strongBadgeBackground,
        other.strongBadgeBackground,
        t,
      )!,
      secondaryButtonForeground: Color.lerp(
        secondaryButtonForeground,
        other.secondaryButtonForeground,
        t,
      )!,
      dialogSurface: Color.lerp(dialogSurface, other.dialogSurface, t)!,
      darkProgress: lerpDouble(darkProgress, other.darkProgress, t)!,
    );
  }
}

extension AppThemeContext on BuildContext {
  AppColors get appColors => Theme.of(this).extension<AppColors>()!;
}
