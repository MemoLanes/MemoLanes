import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:memolanes/common/component/bottom_nav_bar.dart';

class StyleConstants {
  StyleConstants._();

  // Part 1 deliberately keeps the app dark-only. A later UI v2 PR will make
  // these semantic roles adaptive and introduce the user-facing theme mode.
  static const bool isDarkMode = true;

  // Surface and content roles.
  static const Color canvasColor = Color(0xFF0B100D);
  static const Color surfaceColor = Color(0xFF171918);
  static const Color elevatedSurfaceColor = Color(0xFF1C2620);
  static const Color inkColor = Color(0xFFF1F5EF);
  static const Color mutedInkColor = Color(0xFFA2ADA6);
  static const Color subtleInkColor = Color(0xFF77837B);
  static const Color lineColor = Color(0xFF2B3730);
  static const Color strongLineColor = Color(0xFF44534A);
  static const Color inverseInkColor = Color(0xFF10150F);
  static const Color onStrongColor = Color(0xFFF8FBF6);
  static const Color shadowColor = Color(0xFF000000);

  // Translucent surfaces.
  static const Color glassColor = Color(0xFF111814);
  static const Color glassBorderColor = Color(0xFF718078);
  static const Color glassHighlightColor = Color(0xFFE5F5E8);

  // Brand, selection, and action roles.
  static const Color primaryGreen = Color(0xFFB8EA72);
  static const Color deepGreen = Color(0xFF8ACB55);
  static const Color softGreen = Color(0xFF21331F);
  static const Color journeyYellow = Color(0xFFFFD75A);
  static const Color deepYellow = Color(0xFFF0C74B);
  static const Color softYellow = Color(0xFF352F19);
  static const Color primaryActionColor = primaryGreen;
  static const Color onPrimaryActionColor = inverseInkColor;
  static const Color selectedSurfaceColor = softGreen;

  // Feedback roles.
  static const Color warningColor = journeyYellow;
  static const Color warningInkColor = deepYellow;
  static const Color warningSurfaceColor = softYellow;
  static const Color dangerColor = Color(0xFFFF6F7D);
  static const Color dangerInkColor = Color(0xFFFF9AA4);
  static const Color dangerSurfaceColor = Color(0xFF3A2026);
  static const Color onDangerColor = inverseInkColor;

  // navBar
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
  static const double navBarSafeArea =
      BottomNavBar.height + navBarMinimumBottomInset;

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
      BottomNavBar.height + navBarBottomInset(context);

  static double mapPrimaryControlBottomInsetForContext(BuildContext context) =>
      navBarSafeAreaForContext(context) + mapPrimaryControlNavBarSpacing;

  // colors
  // Compatibility alias for pages that have not migrated to semantic roles.
  static const Color defaultColor = primaryGreen;
  static const Color loadingMaskColor = Color.fromRGBO(0, 0, 0, 0.35);
  static const double overlayFloatingRadius = 16.0;

  // Shared elevation for glass controls displayed over the map.
  static const double mapOverlayShadowAlpha = 0.42;
  static const double mapOverlayShadowBlurRadius = 26;
  static const double mapOverlayShadowSpreadRadius = -3;
  static const Offset mapOverlayShadowOffset = Offset(0, 8);
}
