import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';

class StyleConstants {
  StyleConstants._();

  static bool _isDarkMode = true;

  static bool get isDarkMode => _isDarkMode;

  static void setDarkMode(bool value) {
    _isDarkMode = value;
  }

  // Adaptive semantic colors. Map fog colors remain independent from the app
  // theme because they describe map data rather than interface surfaces.
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
  static Color get shadowColor =>
      _isDarkMode ? const Color(0xFF000000) : const Color(0xFF182016);

  static const Color primaryGreen = Color(0xFFB8EA72);
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
  // Layout constants live here so this foundation does not depend on the
  // BottomNavBar widget implementation.
  static const double navBarHeight = 64;

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
  static const double mapPrimaryControlNavBarSpacing = 20;

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

  // Compatibility alias for components that have not yet migrated to semantic
  // color roles. Later UI PRs can replace these uses incrementally.
  static const Color defaultColor = primaryGreen;
  static Color get loadingMaskColor =>
      _isDarkMode ? const Color(0xAD000000) : const Color(0x59182016);
  static const double overlayFloatingRadius = 16.0;
}
