import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class StyleConstants {
  StyleConstants._();

  static const bool enableBackdropFilter = false;
  static bool _isDarkMode = true;

  static bool get isDarkMode => _isDarkMode;

  static void setDarkMode(bool value) {
    _isDarkMode = value;
  }

  // Adaptive foundations. The light values are the original MemoLanes UI;
  // the dark values are the night palette. Map fog/遮罩 colors are maintained
  // separately and are never derived from this setting.
  static Color get canvasColor =>
      _isDarkMode ? const Color(0xFF0B100D) : const Color(0xFFFAFBF5);
  static Color get surfaceColor =>
      _isDarkMode ? const Color(0xFF171918) : const Color(0xFFFFFFFF);
  static Color get elevatedSurfaceColor =>
      _isDarkMode ? const Color(0xFF1C2620) : const Color(0xFFFFFFFF);
  static Color get inkColor =>
      _isDarkMode ? const Color(0xFFF1F5EF) : const Color(0xFF182016);
  static Color get mutedInkColor =>
      _isDarkMode ? const Color(0xFFA2ADA6) : const Color(0xFF6C7567);
  static Color get subtleInkColor =>
      _isDarkMode ? const Color(0xFF77837B) : const Color(0xFF9CA59F);
  static Color get lineColor =>
      _isDarkMode ? const Color(0xFF2B3730) : const Color(0xFFE7EBD9);
  static Color get strongLineColor =>
      _isDarkMode ? const Color(0xFF44534A) : const Color(0xFFC8D1C1);
  static Color get inverseInkColor =>
      _isDarkMode ? const Color(0xFF10150F) : const Color(0xFFFFFFFF);
  static Color get onStrongColor => const Color(0xFFF8FBF6);
  static Color get shadowColor =>
      _isDarkMode ? const Color(0xFF000000) : const Color(0xFF182016);

  static Color get glassColor =>
      _isDarkMode ? const Color(0xFF111814) : const Color(0xFFFFFFFF);
  static Color get glassBorderColor =>
      _isDarkMode ? const Color(0xFF718078) : const Color(0xFFFFFFFF);
  static Color get glassHighlightColor =>
      _isDarkMode ? const Color(0xFFE5F5E8) : const Color(0xFFFFFFFF);

  // Brand and selection. The lime is deliberately reserved for compact
  // emphasis and primary actions instead of large decorative surfaces.
  static Color get primaryGreen => const Color(0xFFB8EA72);
  static Color get deepGreen =>
      _isDarkMode ? const Color(0xFF8ACB55) : const Color(0xFF3F9154);
  static Color get softGreen =>
      _isDarkMode ? const Color(0xFF21331F) : const Color(0xFFECF9D9);
  static Color get journeyYellow =>
      _isDarkMode ? const Color(0xFFFFD75A) : const Color(0xFFFFD72E);
  static Color get deepYellow =>
      _isDarkMode ? const Color(0xFFF0C74B) : const Color(0xFF8B6600);
  static Color get softYellow =>
      _isDarkMode ? const Color(0xFF352F19) : const Color(0xFFFFF5BD);

  static Color get primaryActionColor => primaryGreen;
  static Color get onPrimaryActionColor =>
      _isDarkMode ? const Color(0xFF10150F) : deepGreen;
  static Color get selectedSurfaceColor => softGreen;

  // Feedback colors. These are quieter than Flutter's stock red and orange,
  // and keep status meaning separate from brand accents.
  static Color get warningColor => journeyYellow;
  static Color get warningInkColor => deepYellow;
  static Color get warningSurfaceColor => softYellow;
  static Color get dangerColor =>
      _isDarkMode ? const Color(0xFFFF6F7D) : const Color(0xFFC7485D);
  static Color get dangerInkColor =>
      _isDarkMode ? const Color(0xFFFF9AA4) : const Color(0xFF8F2F42);
  static Color get dangerSurfaceColor =>
      _isDarkMode ? const Color(0xFF3A2026) : const Color(0xFFFBEAEC);
  static Color get onDangerColor =>
      _isDarkMode ? const Color(0xFF10150F) : surfaceColor;
  static Color get recordingColor =>
      _isDarkMode ? const Color(0xFFFF6268) : const Color(0xFFD95357);
  static Color get statusExcellentColor => deepGreen;
  static Color get statusGoodColor => deepYellow;
  static Color get statusFairColor =>
      _isDarkMode ? const Color(0xFFE89A55) : const Color(0xFFB56C32);
  static Color get statusPoorColor => dangerColor;

  // Achievement colors remain gold so the category keeps its own identity,
  // while using one shared hue instead of several unrelated yellows.
  static Color get achievementGoldColor =>
      _isDarkMode ? const Color(0xFFE1B84A) : const Color(0xFFB88722);
  static Color get achievementGoldSurfaceStart =>
      _isDarkMode ? const Color(0xFF3A311B) : const Color(0xFFFFF7D9);
  static Color get achievementGoldSurfaceEnd =>
      _isDarkMode ? const Color(0xFF272619) : const Color(0xFFF1E5B5);
  static Color get profileAccentStartColor => deepGreen;
  static Color get profileAccentEndColor => deepYellow;

  // Active switches use the soft green surface tone used across settings.
  // Keeping these colors role-specific lets settings controls be tuned without
  // changing buttons, GPS status indicators, or other selected states.
  static Color get switchActiveTrackColor => softGreen;
  static Color get switchActiveThumbColor => deepGreen;
  static Color get switchInactiveTrackColor =>
      _isDarkMode ? elevatedSurfaceColor : const Color(0xFFDDE2DC);
  static Color get switchInactiveThumbColor => subtleInkColor;
  static Color get switchTrackOutlineColor => strongLineColor;
  static const double switchActiveThumbSize = 18;
  static const double switchInactiveThumbSize = 16;
  static const double switchTrackOutlineWidth = 1;

  // navBar
  // Kept in the layout constants layer so shared safe-area calculations do
  // not need to depend on the BottomNavBar widget implementation.
  static const double navBarHeight = 58;

  // Visual bottom inset for the floating nav bar on gesture/home-indicator
  // devices. This intentionally differs from the raw safe-area value so iOS
  // and Android look closer while still clearing bottom rounded corners.
  static const double navBarGestureBottomInset = 32;

  // Gap above a non-gesture system navigation area, such as Android 3-button
  // navigation.
  static const double navBarSystemAreaGap = 5;

  // Fallback inset for screens without a reported bottom system area.
  static const double navBarMinimumBottomInset = 32;

  // Vertical space occupied by the nav bar and its fixed bottom inset.
  // Scrollable pages use this to keep content clear of the floating nav bar.
  static const double navBarSafeArea = navBarHeight + navBarMinimumBottomInset;

  // Gap between the nav bar and primary map controls such as recording buttons
  // and the time-machine ruler.
  static const double mapPrimaryControlNavBarSpacing = 14;

  // Bottom inset shared by primary map controls so they align across map modes.
  static const double mapPrimaryControlBottomInset =
      navBarSafeArea + mapPrimaryControlNavBarSpacing;

  static double navBarBottomInset(BuildContext context) {
    final bottomGestureInset = MediaQuery.systemGestureInsetsOf(context).bottom;
    final bottomSafeArea = MediaQuery.viewPaddingOf(context).bottom;

    return switch ((
      bottomGestureInset,
      bottomSafeArea,
      defaultTargetPlatform,
    )) {
      (> 0, _, _) => bottomGestureInset + navBarGestureBottomInset,
      (_, > 0, TargetPlatform.iOS) => navBarGestureBottomInset,
      (_, > 0, _) => bottomSafeArea + navBarSystemAreaGap,
      _ => navBarMinimumBottomInset,
    };
  }

  static double navBarSafeAreaForContext(BuildContext context) =>
      navBarHeight + navBarBottomInset(context);

  static double mapPrimaryControlBottomInsetForContext(BuildContext context) =>
      navBarSafeAreaForContext(context) + mapPrimaryControlNavBarSpacing;

  // Overlays
  static Color get loadingMaskColor =>
      _isDarkMode ? const Color(0xAD000000) : const Color(0x59182016);
  static const double overlayFloatingRadius = 16.0;

  // Shared elevation for glass buttons and cards displayed over the map.
  static double get mapOverlayShadowAlpha => _isDarkMode ? 0.42 : 0.18;
  static const double mapOverlayShadowBlurRadius = 26;
  static const double mapOverlayShadowSpreadRadius = -3;
  static const Offset mapOverlayShadowOffset = Offset(0, 8);

  // Calm, readable glass used by the time-machine ruler and other secondary
  // map controls that should remain visible without looking like solid cards.
  static double get timelineGlassBackgroundAlpha => _isDarkMode ? 0.84 : 0.60;
  static double get timelineGlassBorderAlpha => _isDarkMode ? 0.46 : 0.84;
  static const double timelineGlassBlurSigma = 24;
  static double get timelineGlassReflectionAlpha => 0.12;
}
